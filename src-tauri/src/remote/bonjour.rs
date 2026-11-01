//! Bonjour (DNS-SD) advertising and discovery through the system's `dns_sd.h` API, which lives in
//! libSystem on macOS and iOS. Going through mDNSResponder (instead of a multicast socket of our
//! own) means iOS needs no multicast entitlement, only `NSBonjourServices` in Info.plist.

use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::Arc;
use std::thread::JoinHandle;

use anyhow::{bail, Result};
use parking_lot::Mutex;

pub const SERVICE: &str = "_coffi._tcp";

type DnsRef = *mut c_void;
type BrowseReply = extern "C" fn(DnsRef, u32, u32, i32, *const c_char, *const c_char, *const c_char, *mut c_void);
type ResolveReply = extern "C" fn(DnsRef, u32, u32, i32, *const c_char, *const c_char, u16, u16, *const u8, *mut c_void);
type RegisterReply = extern "C" fn(DnsRef, u32, i32, *const c_char, *const c_char, *const c_char, *mut c_void);

const FLAG_ADD: u32 = 0x2;

extern "C" {
    fn DNSServiceRegister(
        sd: *mut DnsRef,
        flags: u32,
        interface: u32,
        name: *const c_char,
        regtype: *const c_char,
        domain: *const c_char,
        host: *const c_char,
        port: u16,
        txt_len: u16,
        txt: *const c_void,
        cb: Option<RegisterReply>,
        ctx: *mut c_void,
    ) -> i32;
    fn DNSServiceBrowse(sd: *mut DnsRef, flags: u32, interface: u32, regtype: *const c_char, domain: *const c_char, cb: BrowseReply, ctx: *mut c_void) -> i32;
    fn DNSServiceResolve(
        sd: *mut DnsRef,
        flags: u32,
        interface: u32,
        name: *const c_char,
        regtype: *const c_char,
        domain: *const c_char,
        cb: ResolveReply,
        ctx: *mut c_void,
    ) -> i32;
    fn DNSServiceRefSockFD(sd: DnsRef) -> c_int;
    fn DNSServiceProcessResult(sd: DnsRef) -> i32;
    fn DNSServiceRefDeallocate(sd: DnsRef);
}

fn cstr(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

unsafe fn string(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

/// TXT record wire format: a sequence of length-prefixed `key=value` strings.
pub fn encode_txt(pairs: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (k, v) in pairs {
        let mut e = format!("{k}={v}").into_bytes();
        e.truncate(255);
        out.push(e.len() as u8);
        out.extend_from_slice(&e);
    }
    out
}

pub fn decode_txt(mut b: &[u8]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    while let Some((&len, rest)) = b.split_first() {
        let len = (len as usize).min(rest.len());
        let entry = String::from_utf8_lossy(&rest[..len]);
        if let Some((k, v)) = entry.split_once('=') {
            out.insert(k.to_string(), v.to_string());
        }
        b = &rest[len..];
    }
    out
}

// ---------- advertising ----------

/// This device's advertisement; removed from the network when dropped.
pub struct Registration(DnsRef);

// The ref is only touched again by `DNSServiceRefDeallocate`, from whichever thread drops it.
unsafe impl Send for Registration {}
unsafe impl Sync for Registration {}

impl Drop for Registration {
    fn drop(&mut self) {
        unsafe { DNSServiceRefDeallocate(self.0) };
    }
}

pub fn register(name: &str, port: u16, txt: &[(&str, &str)]) -> Result<Registration> {
    let (name, ty, txt) = (cstr(name), cstr(SERVICE), encode_txt(txt));
    let mut sd: DnsRef = std::ptr::null_mut();
    // With no callback, mDNSResponder picks a unique name by itself on conflicts.
    let err = unsafe {
        DNSServiceRegister(
            &mut sd,
            0,
            0,
            name.as_ptr(),
            ty.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            port.to_be(),
            txt.len() as u16,
            txt.as_ptr() as *const c_void,
            None,
            std::ptr::null_mut(),
        )
    };
    if err != 0 {
        bail!("Bonjour registration failed ({err})");
    }
    Ok(Registration(sd))
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

type OnChange = Arc<dyn Fn(Vec<Found>) + Send + Sync>;

struct BrowseCtx {
    /// Instance name -> (interfaces it was seen on, resolved details).
    seen: Mutex<HashMap<String, (u32, Option<Found>)>>,
    on_change: OnChange,
}

impl BrowseCtx {
    fn changed(&self) {
        let list: Vec<Found> = self.seen.lock().values().filter_map(|(_, f)| f.clone()).collect();
        (self.on_change)(list);
    }
}

/// A running browse; stops when dropped.
pub struct Browser {
    stop: c_int,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Browser {
    fn drop(&mut self) {
        unsafe {
            libc::write(self.stop, [1u8].as_ptr() as *const c_void, 1);
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        unsafe { libc::close(self.stop) };
    }
}

extern "C" fn on_browse(_: DnsRef, flags: u32, interface: u32, err: i32, name: *const c_char, ty: *const c_char, domain: *const c_char, ctx: *mut c_void) {
    if err != 0 {
        return;
    }
    // SAFETY: `ctx` is the `Arc<BrowseCtx>` owned by the browse thread for the ref's lifetime.
    let ctx = unsafe { Arc::<BrowseCtx>::increment_strong_count(ctx as *const BrowseCtx); Arc::from_raw(ctx as *const BrowseCtx) };
    let (name, ty, domain) = unsafe { (string(name), string(ty), string(domain)) };
    if flags & FLAG_ADD != 0 {
        let first = {
            let mut seen = ctx.seen.lock();
            let e = seen.entry(name.clone()).or_insert((0, None));
            e.0 += 1;
            e.1.is_none()
        };
        if first {
            // Resolving takes a network round-trip; keep the browse loop responsive.
            std::thread::spawn(move || {
                if let Some(f) = resolve(interface, &name, &ty, &domain) {
                    if let Some(e) = ctx.seen.lock().get_mut(&name) {
                        e.1 = Some(f);
                    }
                    ctx.changed();
                }
            });
        }
    } else {
        let gone = {
            let mut seen = ctx.seen.lock();
            match seen.get_mut(&name) {
                Some(e) if e.0 > 1 => {
                    e.0 -= 1;
                    false
                }
                Some(_) => seen.remove(&name).is_some(),
                None => false,
            }
        };
        if gone {
            ctx.changed();
        }
    }
}

/// Watches the network for other Coff-fi devices; `on_change` gets the full current list.
pub fn browse(on_change: impl Fn(Vec<Found>) + Send + Sync + 'static) -> Result<Browser> {
    let mut fds = [0 as c_int; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        bail!("pipe failed");
    }
    let [stop_r, stop_w] = fds;
    let ctx = Arc::new(BrowseCtx { seen: Mutex::new(HashMap::new()), on_change: Arc::new(on_change) });
    let ty = cstr(SERVICE);
    let mut sd: DnsRef = std::ptr::null_mut();
    let err = unsafe { DNSServiceBrowse(&mut sd, 0, 0, ty.as_ptr(), std::ptr::null(), on_browse, Arc::as_ptr(&ctx) as *mut c_void) };
    if err != 0 {
        unsafe {
            libc::close(stop_r);
            libc::close(stop_w);
        }
        bail!("Bonjour browse failed ({err})");
    }
    struct SendRef(DnsRef);
    unsafe impl Send for SendRef {}
    let sd = SendRef(sd);
    let thread = std::thread::Builder::new().name("bonjour".into()).spawn(move || {
        let sd = sd;
        let fd = unsafe { DNSServiceRefSockFD(sd.0) };
        loop {
            let mut p = [libc::pollfd { fd, events: libc::POLLIN, revents: 0 }, libc::pollfd { fd: stop_r, events: libc::POLLIN, revents: 0 }];
            // Blocks until mDNSResponder has news or we're told to stop: no polling interval.
            let n = unsafe { libc::poll(p.as_mut_ptr(), 2, -1) };
            if n < 0 {
                if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                break;
            }
            if p[1].revents != 0 {
                break;
            }
            if p[0].revents != 0 && unsafe { DNSServiceProcessResult(sd.0) } != 0 {
                break;
            }
        }
        unsafe {
            DNSServiceRefDeallocate(sd.0);
            libc::close(stop_r);
        }
        drop(ctx);
    })?;
    Ok(Browser { stop: stop_w, thread: Some(thread) })
}

extern "C" fn on_resolve(_: DnsRef, _: u32, _: u32, err: i32, fullname: *const c_char, host: *const c_char, port: u16, txt_len: u16, txt: *const u8, ctx: *mut c_void) {
    if err != 0 {
        return;
    }
    // SAFETY: `ctx` points at the `Option<Found>` on `resolve`'s stack, alive during the call.
    let out = unsafe { &mut *(ctx as *mut Option<Found>) };
    let txt = if txt.is_null() { &[][..] } else { unsafe { std::slice::from_raw_parts(txt, txt_len as usize) } };
    let full = unsafe { string(fullname) };
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    *out = Some(Found { instance: full, host: unsafe { string(host) }, port: u16::from_be(port), txt: decode_txt(txt), seq });
}

fn resolve(interface: u32, name: &str, ty: &str, domain: &str) -> Option<Found> {
    let (cname, cty, cdom) = (cstr(name), cstr(ty), cstr(domain));
    let mut out: Option<Found> = None;
    let mut sd: DnsRef = std::ptr::null_mut();
    let err = unsafe {
        DNSServiceResolve(&mut sd, 0, interface, cname.as_ptr(), cty.as_ptr(), cdom.as_ptr(), on_resolve, &mut out as *mut _ as *mut c_void)
    };
    if err != 0 {
        return None;
    }
    let mut p = libc::pollfd { fd: unsafe { DNSServiceRefSockFD(sd) }, events: libc::POLLIN, revents: 0 };
    if unsafe { libc::poll(&mut p, 1, 5000) } > 0 {
        unsafe { DNSServiceProcessResult(sd) };
    }
    unsafe { DNSServiceRefDeallocate(sd) };
    out.map(|mut f| {
        f.instance = name.to_string();
        f
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn txt_round_trip() {
        let b = encode_txt(&[("id", "abc"), ("name", "Mac = mine"), ("platform", "macos")]);
        let m = decode_txt(&b);
        assert_eq!(m["id"], "abc");
        assert_eq!(m["name"], "Mac = mine");
        assert_eq!(m["platform"], "macos");
        assert!(decode_txt(&[9, b'x']).is_empty());
    }
}
