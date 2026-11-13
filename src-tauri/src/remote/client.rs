//! The controlling side: one connection to the device being controlled. Calls are sent as
//! requests and matched to replies by id; the other device's events are re-emitted to our own UI
//! with a `remote:` prefix (e.g. `remote:player://state`).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio::task::AbortHandle;
use tokio_tungstenite::tungstenite::Message;

use super::Device;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const CALL_TIMEOUT: Duration = Duration::from_secs(15);

enum Reply {
    Json(Result<Value, String>),
    Bytes(Vec<u8>),
}

struct Conn {
    device: Device,
    tx: mpsc::UnboundedSender<Message>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Reply>>>,
    next: AtomicU64,
    tasks: Vec<AbortHandle>,
}

impl Drop for Conn {
    fn drop(&mut self) {
        self.tasks.iter().for_each(AbortHandle::abort);
    }
}

static CONN: Mutex<Option<Arc<Conn>>> = Mutex::new(None);
/// Reconnect requests from a dropped connection's reader. Handled by one task started in
/// [`init`] (the reader can't call `connect` itself: that would make the future type recursive).
static RETRY: std::sync::OnceLock<mpsc::UnboundedSender<Device>> = std::sync::OnceLock::new();

pub fn init(app: &AppHandle) {
    let (tx, mut rx) = mpsc::unbounded_channel::<Device>();
    if RETRY.set(tx).is_err() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(device) = rx.recv().await {
            if CONN.lock().is_some() {
                continue;
            }
            if let Err(e) = connect(&app, device.clone(), super::me(&app)).await {
                emit_target(&app, None, Some(format!("Lost connection to {}: {e:#}", device.name)));
            }
        }
    });
}
/// The device to reconnect to when the iOS app returns from the background.
static RESUME: Mutex<Option<Device>> = Mutex::new(None);

pub fn target() -> Option<Device> {
    CONN.lock().as_ref().map(|c| c.device.clone())
}

fn emit_target(app: &AppHandle, device: Option<&Device>, reason: Option<String>) {
    let _ = app.emit("remote://target", json!({ "device": device, "reason": reason }));
}

/// Why a connection failed, in words (instead of "No route to host (os error 65)").
#[derive(Debug)]
pub enum Unreachable {
    /// The name didn't resolve.
    NotFound,
    /// Nothing answered: the device is asleep, Coff-fi isn't running there (or is suspended),
    /// or the network keeps devices apart.
    NoAnswer,
    /// Coff-fi isn't listening there (sharing is off, or it quit).
    Refused,
}

impl std::fmt::Display for Unreachable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Unreachable::NotFound => "it isn't on this network any more",
            Unreachable::NoAnswer => "it didn't answer",
            Unreachable::Refused => "Coff-fi isn't accepting connections there",
        })
    }
}

impl std::error::Error for Unreachable {}

fn classify(e: &std::io::Error) -> Unreachable {
    if e.kind() == std::io::ErrorKind::ConnectionRefused { Unreachable::Refused } else { Unreachable::NoAnswer }
}

/// Resolves `host` and connects to all its addresses at once, keeping the first that answers:
/// `.local` names usually have an IPv4 and a link-local IPv6 address, and either may be dead.
async fn tcp(host: &str, port: u16) -> Result<TcpStream> {
    let host = host.trim_end_matches('.');
    let addrs: Vec<SocketAddr> = match tokio::net::lookup_host((host, port)).await {
        Ok(a) => a.collect(),
        Err(_) => return Err(Unreachable::NotFound.into()),
    };
    if addrs.is_empty() {
        return Err(Unreachable::NotFound.into());
    }
    let mut set = tokio::task::JoinSet::new();
    for a in addrs {
        set.spawn(async move { tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(a)).await });
    }
    let mut why = Unreachable::NoAnswer;
    while let Some(r) = set.join_next().await {
        match r {
            Ok(Ok(Ok(s))) => return Ok(s),
            // A refusal on any address beats "no answer": the device is there.
            Ok(Ok(Err(e))) if matches!(classify(&e), Unreachable::Refused) => why = Unreachable::Refused,
            _ => {}
        }
    }
    Err(why.into())
}

pub async fn connect(app: &AppHandle, mut device: Device, me: Value) -> Result<Device> {
    disconnect(app, false);
    let stream = tcp(&device.host, device.port).await?;
    let _ = stream.set_nodelay(true);
    let url = format!("ws://{}:{}/", device.host.trim_end_matches('.'), device.port);
    let (ws, _) = tokio::time::timeout(CONNECT_TIMEOUT, tokio_tungstenite::client_async(url, stream)).await.map_err(|_| Unreachable::NoAnswer)??;
    let (mut sink, mut source) = ws.split();
    sink.send(Message::text(json!({ "t": "hello", "name": me["name"], "id": me["id"], "platform": me["platform"] }).to_string())).await?;
    let welcome = match tokio::time::timeout(CONNECT_TIMEOUT, source.next()).await.map_err(|_| Unreachable::NoAnswer)? {
        Some(Ok(Message::Text(t))) => serde_json::from_str::<Value>(t.as_str())?,
        _ => bail!("not a Coff-fi device"),
    };
    if welcome["t"] != "welcome" {
        bail!("not a Coff-fi device");
    }
    if let Some(n) = welcome["info"]["name"].as_str().filter(|n| !n.is_empty()) {
        device.name = n.to_string();
    }
    if let Some(p) = welcome["info"]["platform"].as_str() {
        device.platform = p.to_string();
    }

    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let writer = tauri::async_runtime::spawn(async move {
        while let Some(m) = rx.recv().await {
            if sink.send(m).await.is_err() {
                break;
            }
        }
    });
    let conn = Arc::new_cyclic(|weak: &std::sync::Weak<Conn>| {
        let (weak, app) = (weak.clone(), app.clone());
        let reader = tauri::async_runtime::spawn(async move {
            while let Some(Ok(msg)) = source.next().await {
                // Not upgradable only for the instant before the connection is installed.
                let Some(conn) = weak.upgrade() else { continue };
                match msg {
                    Message::Text(t) => {
                        let Ok(v) = serde_json::from_str::<Value>(t.as_str()) else { continue };
                        match v["t"].as_str() {
                            Some("res") => {
                                let id = v["id"].as_u64().unwrap_or(0);
                                let r = match v.get("err").and_then(Value::as_str) {
                                    Some(e) => Err(e.to_string()),
                                    None => Ok(v.get("ok").cloned().unwrap_or(Value::Null)),
                                };
                                if let Some(p) = conn.pending.lock().remove(&id) {
                                    let _ = p.send(Reply::Json(r));
                                }
                            }
                            Some("event") => {
                                if let Some(name) = v["name"].as_str() {
                                    let _ = app.emit(&format!("remote:{name}"), v.get("payload").cloned().unwrap_or(Value::Null));
                                }
                            }
                            _ => {}
                        }
                    }
                    Message::Binary(b) if b.len() >= 8 => {
                        let id = u64::from_be_bytes(b[..8].try_into().unwrap_or_default());
                        if let Some(p) = conn.pending.lock().remove(&id) {
                            let _ = p.send(Reply::Bytes(b[8..].to_vec()));
                        }
                    }
                    _ => {}
                }
            }
            // The connection dropped: the other device quit, slept or turned sharing off, or iOS
            // cut our socket while this app was suspended.
            let Some(conn) = weak.upgrade() else { return };
            let device = conn.device.clone();
            let mut cur = CONN.lock();
            if !cur.as_ref().is_some_and(|c| Arc::ptr_eq(c, &conn)) {
                return;
            }
            *cur = None;
            drop(cur);
            drop(conn);
            if crate::in_background() {
                *RESUME.lock() = Some(device);
                emit_target(&app, None, None);
                return;
            }
            // One quiet retry before telling the user (a socket cut during suspension is
            // usually only noticed just after coming back).
            if let Some(tx) = RETRY.get() {
                let _ = tx.send(device);
            }
        });
        Conn {
            device: device.clone(),
            tx,
            pending: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            tasks: vec![writer.inner().abort_handle(), reader.inner().abort_handle()],
        }
    });
    *CONN.lock() = Some(conn);
    *RESUME.lock() = None;
    emit_target(app, Some(&device), None);
    Ok(device)
}

/// Stops controlling the other device. `notify` tells our UI (false when replacing the target).
pub fn disconnect(app: &AppHandle, notify: bool) {
    *RESUME.lock() = None;
    let old = CONN.lock().take();
    if let Some(c) = old {
        let _ = c.tx.send(Message::Close(None));
        drop(c);
        if notify {
            emit_target(app, None, None);
        }
    }
}

/// Reconnects to a device that dropped while the app was in the background.
pub async fn resume(app: &AppHandle, me: Value) {
    let Some(d) = RESUME.lock().take() else { return };
    if CONN.lock().is_some() {
        return;
    }
    if let Err(e) = connect(app, d.clone(), me).await {
        emit_target(app, None, Some(format!("Lost connection to {}: {e:#}", d.name)));
    }
}

async fn request(cmd: &str, args: Value) -> Result<Reply, String> {
    let conn = CONN.lock().clone().ok_or_else(|| "Not connected to another device".to_string())?;
    let id = conn.next.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = oneshot::channel();
    conn.pending.lock().insert(id, tx);
    let msg = json!({ "t": "call", "id": id, "cmd": cmd, "args": args }).to_string();
    if conn.tx.send(Message::text(msg)).is_err() {
        conn.pending.lock().remove(&id);
        return Err(format!("Lost connection to {}", conn.device.name));
    }
    match tokio::time::timeout(CALL_TIMEOUT, rx).await {
        Ok(Ok(r)) => Ok(r),
        Ok(Err(_)) => Err(format!("Lost connection to {}", conn.device.name)),
        Err(_) => {
            conn.pending.lock().remove(&id);
            Err(format!("{} didn't answer", conn.device.name))
        }
    }
}

/// Runs a command on the controlled device.
pub async fn call(cmd: &str, args: Value) -> Result<Value, String> {
    match request(cmd, args).await? {
        Reply::Json(r) => r,
        Reply::Bytes(_) => Err("unexpected reply".into()),
    }
}

/// A cover image from the controlled device (`path` as in the `coffi://` protocol). Blocking;
/// called from the cover protocol's worker threads.
pub fn cover(path: &str) -> Option<Vec<u8>> {
    let path = path.to_string();
    match tauri::async_runtime::block_on(async move { request("cover", json!({ "path": path })).await }) {
        Ok(Reply::Bytes(b)) if !b.is_empty() => Some(b),
        _ => None,
    }
}
