//! The side that gets controlled: a WebSocket server other Coff-fi devices connect to.
//!
//! Protocol (JSON text frames):
//! - client → `{"t":"hello","id","name","platform"}` first, answered by `{"t":"welcome", ...info}`
//! - client → `{"t":"call","id":n,"cmd":"...","args":{...}}`, answered by `{"t":"res","id":n,"ok":v}`
//!   or `{"t":"res","id":n,"err":"..."}`; `cmd: "cover"` is answered by a binary frame
//!   `[u64 big-endian id][jpeg bytes]` (empty image = not found)
//! - server → `{"t":"event","name":"player://track","payload":...}` for every shared event

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio::task::AbortHandle;
use tokio_tungstenite::tungstenite::Message;

use super::dispatch;

/// Tried first so "Connect by address" works without knowing a port.
pub const DEFAULT_PORT: u16 = 47321;

struct Controller {
    key: u64,
    name: String,
    tx: UnboundedSender<Message>,
    tasks: Vec<AbortHandle>,
}

static CONTROLLERS: Mutex<Vec<Controller>> = Mutex::new(Vec::new());
static NEXT: AtomicU64 = AtomicU64::new(1);

/// Names of the devices currently connected to (controlling) this one.
pub fn controllers() -> Vec<String> {
    CONTROLLERS.lock().iter().map(|c| c.name.clone()).collect()
}

fn controllers_changed(app: &AppHandle, joined: Option<&str>, left: Option<&str>) {
    let _ = app.emit("remote://controllers", json!({ "names": controllers(), "joined": joined, "left": left }));
}

/// Sends an event to every connected controller. `payload` is already JSON.
pub fn broadcast(name: &str, payload: &str) {
    let list = CONTROLLERS.lock();
    if list.is_empty() {
        return;
    }
    let payload = if payload.is_empty() { "null" } else { payload };
    let text = format!(r#"{{"t":"event","name":{},"payload":{payload}}}"#, Value::from(name));
    for c in list.iter() {
        let _ = c.tx.send(Message::text(text.clone()));
    }
}

/// A running server: stops listening (and drops every controller) when dropped.
pub struct Server {
    pub port: u16,
    accept: AbortHandle,
    app: AppHandle,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.accept.abort();
        let dropped: Vec<Controller> = std::mem::take(&mut *CONTROLLERS.lock());
        for c in &dropped {
            c.tasks.iter().for_each(AbortHandle::abort);
        }
        if !dropped.is_empty() {
            controllers_changed(&self.app, None, None);
        }
    }
}

async fn bind() -> std::io::Result<TcpListener> {
    // `[::]` is dual-stack on Apple platforms, so IPv4 clients get in too.
    for addr in [format!("[::]:{DEFAULT_PORT}"), "[::]:0".into(), format!("0.0.0.0:{DEFAULT_PORT}"), "0.0.0.0:0".into()] {
        if let Ok(l) = TcpListener::bind(&addr).await {
            return Ok(l);
        }
    }
    TcpListener::bind("0.0.0.0:0").await
}

/// Errors that leave the listening socket usable.
fn transient(e: &std::io::Error) -> bool {
    use std::io::ErrorKind::*;
    matches!(e.kind(), ConnectionAborted | ConnectionReset | Interrupted | WouldBlock | TimedOut)
        || matches!(e.raw_os_error(), Some(libc::EMFILE | libc::ENFILE | libc::ENOBUFS | libc::ENOMEM))
}

/// Starts listening. `on_dead` runs if the listening socket stops working, which iOS does to
/// every listening socket of an app it suspends ("defuncting"); the caller then starts over.
pub async fn start(app: AppHandle, on_dead: impl FnOnce() + Send + 'static) -> std::io::Result<Server> {
    let listener = bind().await?;
    let port = listener.local_addr()?.port();
    let a = app.clone();
    let accept = tauri::async_runtime::spawn(async move {
        let mut errors = 0;
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    errors = 0;
                    let app = a.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = serve(app, stream, peer).await {
                            eprintln!("remote: {peer}: {e}");
                        }
                    });
                }
                Err(e) if transient(&e) && errors < 20 => {
                    errors += 1;
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                Err(e) => {
                    eprintln!("remote: listener died: {e}");
                    break;
                }
            }
        }
        on_dead();
    });
    Ok(Server { port, accept: accept.inner().abort_handle(), app })
}

#[derive(Deserialize)]
struct Hello {
    t: String,
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct Call {
    id: u64,
    cmd: String,
    #[serde(default)]
    args: Value,
}

async fn serve(app: AppHandle, stream: TcpStream, peer: SocketAddr) -> anyhow::Result<()> {
    let _ = stream.set_nodelay(true);
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let (mut sink, mut source) = ws.split();

    // The first frame has to be a hello, quickly.
    let first = tokio::time::timeout(Duration::from_secs(5), source.next()).await?;
    let hello: Hello = match first {
        Some(Ok(Message::Text(t))) => serde_json::from_str(t.as_str())?,
        _ => anyhow::bail!("no hello"),
    };
    anyhow::ensure!(hello.t == "hello", "expected hello");
    let name = if hello.name.trim().is_empty() { peer.ip().to_string() } else { hello.name.trim().to_string() };
    sink.send(Message::text(json!({ "t": "welcome", "info": dispatch::info(&app) }).to_string())).await?;

    let (tx, mut rx) = unbounded_channel::<Message>();
    let writer = tauri::async_runtime::spawn(async move {
        while let Some(m) = rx.recv().await {
            if sink.send(m).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });
    let key = NEXT.fetch_add(1, Ordering::Relaxed);
    let (app2, tx2) = (app.clone(), tx.clone());
    let reader = tauri::async_runtime::spawn(async move {
        while let Some(Ok(msg)) = source.next().await {
            let Message::Text(t) = msg else { continue };
            let Ok(call) = serde_json::from_str::<Call>(t.as_str()) else { continue };
            let (app, tx) = (app2.clone(), tx2.clone());
            // Each call runs on its own, so a slow library query doesn't hold up "pause".
            tauri::async_runtime::spawn(async move {
                if call.cmd == "cover" {
                    let path = call.args.get("path").and_then(Value::as_str).unwrap_or_default().to_string();
                    let bytes = tauri::async_runtime::spawn_blocking(move || crate::serve_cover(&path).unwrap_or_default()).await.unwrap_or_default();
                    let mut frame = call.id.to_be_bytes().to_vec();
                    frame.extend_from_slice(&bytes);
                    let _ = tx.send(Message::binary(frame));
                    return;
                }
                let reply = match dispatch::call(&app, &call.cmd, call.args).await {
                    Ok(v) => json!({ "t": "res", "id": call.id, "ok": v }),
                    Err(e) => json!({ "t": "res", "id": call.id, "err": e }),
                };
                let _ = tx.send(Message::text(reply.to_string()));
            });
        }
    });
    CONTROLLERS.lock().push(Controller {
        key,
        name: name.clone(),
        tx,
        tasks: vec![writer.inner().abort_handle(), reader.inner().abort_handle()],
    });
    controllers_changed(&app, Some(&name), None);

    let _ = reader.await;
    writer.abort();
    let was_listed = {
        let mut list = CONTROLLERS.lock();
        let before = list.len();
        list.retain(|c| c.key != key);
        before != list.len()
    };
    if was_listed {
        controllers_changed(&app, None, Some(&name));
    }
    Ok(())
}
