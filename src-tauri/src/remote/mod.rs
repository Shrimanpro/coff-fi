//! Control playback across devices on the local network (Spotify Connect-style).
//!
//! Every Coff-fi runs a small WebSocket server ([`server`]) and advertises it over Bonjour
//! ([`bonjour`]) unless sharing is switched off in Settings. Another Coff-fi can then connect
//! ([`client`]) and drive it: its UI forwards library, queue and playback commands
//! ([`dispatch`]) and shows the other device's events.

pub mod bonjour;
mod dispatch;
pub mod server;

use parking_lot::{Mutex, RwLock};
use tauri::{AppHandle, Emitter, Listener, Manager};

use crate::AppState;

struct Host {
    server: server::Server,
    reg: Option<bonjour::Registration>,
    advertised: String,
}

static HOST: Mutex<Option<Host>> = Mutex::new(None);
/// The OS's name for this device (iOS hands it over at launch).
static SYSTEM_NAME: RwLock<Option<String>> = parking_lot::const_rwlock(None);

/// Events every controller receives (`player://state` is sent straight from the player event
/// handler, since it isn't emitted to our own UI while in the background).
const SHARED_EVENTS: &[&str] =
    &["player://track", "queue://changed", "player://favorite", "player://mode", "library://changed", "stats://changed", "sleep://changed"];

fn system_name() -> String {
    if let Some(n) = SYSTEM_NAME.read().clone() {
        return n;
    }
    #[cfg(target_os = "macos")]
    if let Ok(o) = std::process::Command::new("/usr/sbin/scutil").args(["--get", "ComputerName"]).output() {
        let n = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if o.status.success() && !n.is_empty() {
            *SYSTEM_NAME.write() = Some(n.clone());
            return n;
        }
    }
    let mut buf = [0u8; 256];
    let n = if unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) } == 0 {
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        String::from_utf8_lossy(&buf[..end]).trim_end_matches(".local").to_string()
    } else {
        String::new()
    };
    if n.is_empty() { crate::get_platform().to_string() } else { n }
}

/// What this device calls itself on the network.
pub fn my_name(app: &AppHandle) -> String {
    let custom = app.state::<AppState>().settings.read().remote.name.trim().to_string();
    if custom.is_empty() { system_name() } else { custom }
}

/// Starts sharing (per settings) and discovery, and forwards shared events to controllers.
pub fn init(app: &AppHandle) {
    for &name in SHARED_EVENTS {
        app.listen_any(name, move |e| server::broadcast(name, e.payload()));
    }
    apply(app);
}

/// Brings the server and the Bonjour advertisement in line with the settings.
pub fn apply(app: &AppHandle) {
    let allow = app.state::<AppState>().settings.read().remote.allow;
    if !allow {
        *HOST.lock() = None;
        return;
    }
    let name = my_name(app);
    {
        let mut host = HOST.lock();
        if let Some(h) = host.as_mut() {
            if h.advertised != name {
                h.reg = None;
                h.reg = advertise(app, &name, h.server.port);
                h.advertised = name;
            }
            return;
        }
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match server::start(app.clone()).await {
            Ok(server) => {
                let reg = advertise(&app, &name, server.port);
                let mut host = HOST.lock();
                // Sharing may have been switched off while the server was starting.
                if app.state::<AppState>().settings.read().remote.allow && host.is_none() {
                    *host = Some(Host { server, reg, advertised: name });
                }
            }
            Err(e) => eprintln!("remote: can't start server: {e}"),
        }
    });
}

fn advertise(app: &AppHandle, name: &str, port: u16) -> Option<bonjour::Registration> {
    let id = app.state::<AppState>().settings.read().remote.id.clone();
    match bonjour::register(name, port, &[("id", &id), ("name", name), ("platform", crate::get_platform())]) {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("remote: {e:#}");
            None
        }
    }
}

