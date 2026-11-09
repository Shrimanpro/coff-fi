//! Bridge between the Rust core and the native Swift layer on iOS
//! (`gen/apple/Sources/coff-fi/Native/*.swift`).
//!
//! Swift → Rust: the `coffi_*` functions below are exported from the static library and declared
//! in `Native/CoffiBridge.h`.
//! Rust → Swift: Swift hands us C function pointers through [`coffi_native_register`] before the
//! app starts (see `main.mm`), so the static library has no undefined symbols.

use std::ffi::{c_char, CStr, CString};
use std::sync::{Arc, OnceLock};

use parking_lot::RwLock;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::integrations::{Control, NowPlaying};
use crate::AppState;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Callbacks {
    /// JSON `{title, artist, album, duration, cover}` or null when nothing is loaded.
    pub now_playing: extern "C" fn(json: *const c_char),
    /// Playback state for the lock screen; `rate` is the playback speed.
    pub playback: extern "C" fn(has_track: bool, playing: bool, position: f64, rate: f64),
    /// UI requests that need UIKit: "airplay", "pick_folder".
    pub request: extern "C" fn(what: *const c_char),
}

static CALLBACKS: OnceLock<Callbacks> = OnceLock::new();
static APP: OnceLock<AppHandle> = OnceLock::new();
type ControlFn = Arc<dyn Fn(Control) + Send + Sync>;
static CONTROL: RwLock<Option<ControlFn>> = parking_lot::const_rwlock(None);

/// Called once from `main.mm` before the Tauri app starts.
#[no_mangle]
pub extern "C" fn coffi_native_register(cb: Callbacks) {
    let _ = CALLBACKS.set(cb);
}

/// The phone's name (UIDevice), the default name shown to other devices.
#[no_mangle]
pub extern "C" fn coffi_set_device_name(name: *const c_char) {
    crate::remote::set_system_name(&unsafe { str_arg(name) });
}

pub fn set_app(app: AppHandle) {
    let _ = APP.set(app);
}

fn cstring(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

/// Asks Swift to do something with UIKit (show the AirPlay picker, the Files folder picker...).
pub fn request(what: &str) {
    if let Some(cb) = CALLBACKS.get() {
        let s = cstring(what);
        (cb.request)(s.as_ptr());
    }
}

// ---------- media controls (lock screen, Control Center, Dynamic Island) ----------

/// The iOS stand-in for souvlaki's `MediaControls`; the integrations thread drives it.
pub struct MediaControls;

impl MediaControls {
    pub fn new(on_control: Arc<impl Fn(Control) + Send + Sync + 'static>) -> Self {
        *CONTROL.write() = Some(on_control);
        Self
    }

    pub fn set_track(&mut self, t: Option<&NowPlaying>, _speed: f32) {
        let Some(cb) = CALLBACKS.get() else { return };
        #[derive(Serialize)]
        struct Meta<'a> {
            title: &'a str,
            artist: &'a str,
            album: &'a str,
            duration: f64,
            cover: Option<String>,
        }
        match t {
            Some(t) => {
                let m = Meta {
                    title: &t.title,
                    artist: &t.artist,
                    album: &t.album,
                    duration: t.duration,
                    cover: t.cover.as_ref().map(|p| p.to_string_lossy().into_owned()),
                };
                let s = cstring(&serde_json::to_string(&m).unwrap_or_default());
                (cb.now_playing)(s.as_ptr());
            }
            None => (cb.now_playing)(std::ptr::null()),
        }
    }

    pub fn set_playback(&mut self, has_track: bool, playing: bool, position: f64, speed: f32) {
        if let Some(cb) = CALLBACKS.get() {
            (cb.playback)(has_track, playing, position, speed as f64);
        }
    }
}

impl Drop for MediaControls {
    fn drop(&mut self) {
        *CONTROL.write() = None;
        if let Some(cb) = CALLBACKS.get() {
            (cb.now_playing)(std::ptr::null());
        }
    }
}

/// Remote commands from the lock screen, headphones, CarPlay or audio-session events.
/// 0 play, 1 pause, 2 toggle, 3 next, 4 previous, 5 stop, 6 seek to `value`, 7 seek by `value`,
/// 8 toggle favorite of the playing song.
#[no_mangle]
pub extern "C" fn coffi_control(cmd: i32, value: f64) {
    if cmd == 8 {
        if let Some(app) = APP.get() {
            crate::toggle_current_favorite(app);
        }
        return;
    }
    let c = match cmd {
        0 => Control::Play,
        1 => Control::Pause,
        2 => Control::Toggle,
        3 => Control::Next,
        4 => Control::Prev,
        5 => Control::Stop,
        6 => Control::SeekTo(value),
        7 => Control::SeekBy(value),
        _ => return,
    };
    // Audio-session interruptions must work even when lock-screen controls are switched off.
    let f = CONTROL.read().clone();
    match (f, APP.get()) {
        (Some(f), _) => f(c),
        (None, Some(app)) => crate::apply_control(app, c),
        _ => {}
    }
}

/// App lifecycle notifications: "background" | "foreground".
#[no_mangle]
pub extern "C" fn coffi_lifecycle(event: *const c_char) {
    let Some(app) = APP.get() else { return };
    let ev = unsafe { str_arg(event) };
    if ev == "background" {
        crate::set_background(app, true);
        return;
    }
    if ev == "foreground" {
        crate::set_background(app, false);
        // There's no file watcher on iOS; pick up files added through the Files app instead.
        let st = app.state::<AppState>();
        if !st.remote() {
            crate::rescan_now(app);
        }
    }
}

/// The user picked a music folder in the Files app (path of the security-scoped folder, or
/// null when cancelled).
#[no_mangle]
pub extern "C" fn coffi_folder_picked(path: *const c_char) {
    let Some(app) = APP.get() else { return };
    if path.is_null() {
        return;
    }
    let p = unsafe { str_arg(path) };
    crate::add_root(app, p);
}

unsafe fn str_arg(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    CStr::from_ptr(p).to_string_lossy().into_owned()
}

fn out(s: String) -> *mut c_char {
    cstring(&s).into_raw()
}

/// Frees a string returned by `coffi_find`.
#[no_mangle]
pub extern "C" fn coffi_string_free(p: *mut c_char) {
    if !p.is_null() {
        drop(unsafe { CString::from_raw(p) });
    }
}

// ---------- Siri / App Intents ----------

/// True once the library is open and the player is running.
#[no_mangle]
pub extern "C" fn coffi_ready() -> bool {
    APP.get().is_some_and(|a| a.try_state::<AppState>().is_some())
}

#[derive(Serialize)]
pub struct Entity {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub subtitle: String,
}

/// Searches the library for Siri. `kind`: "playlist" | "album" | "artist" | "song".
/// An empty `query` returns suggestions. Returns a JSON array of [`Entity`] (free with
/// `coffi_string_free`).
#[no_mangle]
pub extern "C" fn coffi_find(kind: *const c_char, query: *const c_char) -> *mut c_char {
    let (kind, query) = unsafe { (str_arg(kind), str_arg(query)) };
    let list = APP.get().map(|app| crate::siri_find(app, &kind, &query)).unwrap_or_default();
    out(serde_json::to_string(&list).unwrap_or_else(|_| "[]".into()))
}

/// Plays an entity found with `coffi_find`. Returns false if it doesn't exist (anymore).
#[no_mangle]
pub extern "C" fn coffi_play(kind: *const c_char, id: *const c_char, shuffle: bool) -> bool {
    let (kind, id) = unsafe { (str_arg(kind), str_arg(id)) };
    APP.get().is_some_and(|app| crate::siri_play(app, &kind, &id, shuffle))
}

pub fn emit_settings_changed(app: &AppHandle) {
    let _ = app.emit("settings://changed", ());
}
