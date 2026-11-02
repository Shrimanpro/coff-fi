//! Control playback across devices on the local network (Spotify Connect-style).
//!
//! Every Coff-fi runs a small WebSocket server ([`server`]) and advertises it over Bonjour
//! ([`bonjour`]) unless sharing is switched off in Settings. Another Coff-fi can then connect
//! ([`client`]) and drive it: its UI forwards library, queue and playback commands
//! ([`dispatch`]) and shows the other device's events.

pub mod bonjour;
pub mod client;
mod dispatch;
pub mod server;

use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Listener, Manager};

use crate::AppState;

/// Another Coff-fi on the network.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub host: String,
    pub port: u16,
}

struct Host {
    /// Tells a dead server's `on_dead` apart from the current one's.
    generation: u64,
    server: server::Server,
    reg: Option<bonjour::Registration>,
    advertised: String,
}

static HOST: Mutex<Option<Host>> = Mutex::new(None);
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// A server is being started (so two quick `apply` calls don't start two).
static STARTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static BROWSER: Mutex<Option<bonjour::Browser>> = Mutex::new(None);
static DEVICES: Mutex<Vec<Device>> = Mutex::new(Vec::new());
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

/// Who we are, for the hello sent when connecting to another device.
pub fn me(app: &AppHandle) -> Value {
    let id = app.state::<AppState>().settings.read().remote.id.clone();
    json!({ "id": id, "name": my_name(app), "platform": crate::get_platform() })
}

pub fn devices() -> Vec<Device> {
    DEVICES.lock().clone()
}

pub fn find(id: &str) -> Option<Device> {
    DEVICES.lock().iter().find(|d| d.id == id).cloned()
}

/// Starts sharing (per settings) and discovery, and forwards shared events to controllers.
pub fn init(app: &AppHandle) {
    for &name in SHARED_EVENTS {
        app.listen_any(name, move |e| server::broadcast(name, e.payload()));
    }
    client::init(app);
    apply(app);
    start_browse(app);
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
    if STARTING.swap(true, std::sync::atomic::Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let generation = GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        match server::start(app.clone()).await {
            Ok(server) => {
                let reg = advertise(&app, &name, server.port);
                let mut host = HOST.lock();
                // Sharing may have been switched off while the server was starting.
                if app.state::<AppState>().settings.read().remote.allow && host.is_none() {
                    *host = Some(Host { generation, server, reg, advertised: name });
                }
            }
            Err(e) => eprintln!("remote: can't start server: {e}"),
        }
        STARTING.store(false, std::sync::atomic::Ordering::Release);
        // Settings may have changed while starting (e.g. sharing switched off, then on again).
        if HOST.lock().is_none() && app.state::<AppState>().settings.read().remote.allow {
            apply(&app);
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

fn start_browse(app: &AppHandle) {
    let mut b = BROWSER.lock();
    if b.is_some() {
        return;
    }
    let me = app.state::<AppState>().settings.read().remote.id.clone();
    let h = app.clone();
    match bonjour::browse(move |mut found| {
        found.sort_by_key(|f| std::cmp::Reverse(f.seq));
        let mut seen = std::collections::HashSet::new();
        let mut list: Vec<Device> = found
            .into_iter()
            .filter(|f| f.txt.get("id").is_none_or(|id| *id != me))
            .filter(|f| seen.insert(f.txt.get("id").cloned().unwrap_or_else(|| f.instance.clone())))
            .map(|f| Device {
                id: f.txt.get("id").cloned().unwrap_or_else(|| f.instance.clone()),
                name: f.txt.get("name").cloned().unwrap_or_else(|| f.instance.clone()),
                platform: f.txt.get("platform").cloned().unwrap_or_default(),
                host: f.host,
                port: f.port,
            })
            .collect();
        list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        *DEVICES.lock() = list.clone();
        let _ = h.emit("remote://devices", list);
    }) {
        Ok(br) => *b = Some(br),
        Err(e) => eprintln!("remote: {e:#}"),
    }
}

/// Parses "host", "host:port" or "[v6]:port" typed by the user.
pub fn parse_address(s: &str) -> Option<(String, u16)> {
    let s = s.trim().trim_start_matches("ws://").trim_end_matches('/');
    if s.is_empty() {
        return None;
    }
    if let Some(rest) = s.strip_prefix('[') {
        let (h, tail) = rest.split_once(']')?;
        let port = tail.strip_prefix(':').map_or(Some(server::DEFAULT_PORT), |p| p.parse().ok())?;
        return Some((h.to_string(), port));
    }
    match s.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') => Some((h.to_string(), p.parse().ok()?)),
        _ => Some((s.to_string(), server::DEFAULT_PORT)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        assert_eq!(parse_address("192.168.1.4"), Some(("192.168.1.4".into(), server::DEFAULT_PORT)));
        assert_eq!(parse_address(" mac.local:5000 "), Some(("mac.local".into(), 5000)));
        assert_eq!(parse_address("[fe80::1]:9"), Some(("fe80::1".into(), 9)));
        assert_eq!(parse_address("fe80::1"), Some(("fe80::1".into(), server::DEFAULT_PORT)));
        assert_eq!(parse_address("mac.local:abc"), None);
        assert_eq!(parse_address(""), None);
    }
}
