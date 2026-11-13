//! Bonjour (DNS-SD) advertising and discovery for platforms without Apple's `dns_sd` (Windows,
//! Linux, Android), done in Rust by `mdns-sd`. Same API as `bonjour.rs`, and the same
//! `_coffi._tcp` service on the wire, so these devices and Apple ones find each other.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::thread::JoinHandle;

use anyhow::{anyhow, Result};
use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent, ServiceInfo};
use parking_lot::Mutex;

pub const SERVICE: &str = "_coffi._tcp";

fn ty_domain() -> String {
    format!("{SERVICE}.local.")
}

/// One responder for the whole app (it owns the multicast sockets and a thread), started on
/// first use.
static DAEMON: Mutex<Option<ServiceDaemon>> = Mutex::new(None);

fn daemon() -> Result<ServiceDaemon> {
    let mut d = DAEMON.lock();
    if let Some(d) = d.as_ref() {
        return Ok(d.clone());
    }
    let new = ServiceDaemon::new().map_err(|e| anyhow!("mDNS unavailable: {e}"))?;
    *d = Some(new.clone());
    Ok(new)
}

// ---------- advertising ----------

/// This device's advertisement; removed from the network when dropped.
pub struct Registration {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Drop for Registration {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
    }
}

/// A DNS label made of the safe characters of `s`.
fn label(s: &str) -> String {
    let l: String = s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(40).collect();
    if l.is_empty() { "device".into() } else { l }
}

pub fn register(name: &str, port: u16, txt: &[(&str, &str)]) -> Result<Registration> {
    let daemon = daemon()?;
    // Our own host name rather than the computer's, so we never clash with the OS's responder
    // (Windows answers for `<COMPUTERNAME>.local` itself).
    let id = txt.iter().find(|(k, _)| *k == "id").map_or(name, |(_, v)| v);
    let host = format!("coffi-{}.local.", label(id));
    let info = ServiceInfo::new(&ty_domain(), name, &host, "", port, txt)
        .map_err(|e| anyhow!("mDNS registration failed: {e}"))?
        .enable_addr_auto();
    let fullname = info.get_fullname().to_string();
    daemon.register(info).map_err(|e| anyhow!("mDNS registration failed: {e}"))?;
    Ok(Registration { daemon, fullname })
}

// ---------- discovery ----------

#[derive(Clone, Debug)]
pub struct Found {
    /// The advertised instance name (unique on the network).
    pub instance: String,
    pub host: String,
    pub port: u16,
    pub txt: HashMap<String, String>,
    /// Increases with every resolution, so the newest of two adverts of one device wins (an
    /// old name can linger in the network's cache after a rename).
    pub seq: u64,
}

static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A running browse; stops when dropped.
pub struct Browser {
    daemon: ServiceDaemon,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Browser {
    fn drop(&mut self) {
        // Ends the event stream with `SearchStopped`, which ends the thread.
        let _ = self.daemon.stop_browse(&ty_domain());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Where to connect: an IPv4 address if there is one, then a routable IPv6 one. Link-local IPv6
/// is useless without its interface, so fall back to the `.local` name then.
fn pick_host(r: &ResolvedService) -> String {
    let rank = |ip: &IpAddr| match ip {
        IpAddr::V4(_) => 0,
        IpAddr::V6(v6) if (v6.segments()[0] & 0xffc0) != 0xfe80 => 1,
        IpAddr::V6(_) => 2,
    };
    r.addresses
        .iter()
        .filter(|a| !a.is_loopback())
        .map(|a| a.to_ip_addr())
        .filter(|ip| rank(ip) < 2)
        .min_by_key(rank)
        .map_or_else(|| r.host.clone(), |ip| ip.to_string())
}

fn instance_name(fullname: &str) -> String {
    fullname.strip_suffix(&format!(".{}", ty_domain())).unwrap_or(fullname).replace("\\.", ".")
}

/// Watches the network for other Coff-fi devices; `on_change` gets the full current list.
pub fn browse(on_change: impl Fn(Vec<Found>) + Send + Sync + 'static) -> Result<Browser> {
    let daemon = daemon()?;
    let rx = daemon.browse(&ty_domain()).map_err(|e| anyhow!("mDNS browse failed: {e}"))?;
    let on_change = Arc::new(on_change);
    let thread = std::thread::Builder::new().name("mdns".into()).spawn(move || {
        // Full name -> resolved details.
        let mut seen: HashMap<String, Found> = HashMap::new();
        while let Ok(ev) = rx.recv() {
            match ev {
                ServiceEvent::ServiceResolved(r) => {
                    let txt = r.txt_properties.iter().map(|p| (p.key().to_string(), p.val_str().to_string())).collect();
                    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let f = Found { instance: instance_name(&r.fullname), host: pick_host(&r), port: r.port, txt, seq };
                    seen.insert(r.fullname.clone(), f);
                }
                ServiceEvent::ServiceRemoved(_, fullname) => {
                    if seen.remove(&fullname).is_none() {
                        continue;
                    }
                }
                ServiceEvent::SearchStopped(_) => break,
                _ => continue,
            }
            on_change(seen.values().cloned().collect());
        }
    })?;
    Ok(Browser { daemon, thread: Some(thread) })
}
