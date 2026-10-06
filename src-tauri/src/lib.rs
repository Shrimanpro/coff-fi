mod library;
mod settings;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::{Mutex, RwLock};
use rusqlite::Connection;
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Emitter, Manager, State};

use library::{covers, db, scan, Collection, FolderView, Home, Tile};
use settings::Settings;

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub struct AppState {
    db: Mutex<Connection>,
    db_path: RwLock<PathBuf>,
    /// Settings always live in the local database, whichever library is active.
    cfg_db: Mutex<Connection>,
    settings: RwLock<Settings>,
    scanning: AtomicBool,
    rescan_pending: AtomicBool,
}

// ---------- settings ----------

fn save_settings(conn: &Connection, s: &Settings) -> Res<()> {
    db::set_setting(conn, "settings", &serde_json::to_string(s).map_err(err)?).map_err(err)
}

#[tauri::command]
fn get_settings(st: State<AppState>) -> Settings {
    st.settings.read().clone()
}

#[tauri::command]
fn set_settings(app: AppHandle, st: State<AppState>, settings: Settings) -> Res<()> {
    let old = st.settings.read().clone();
    let mut s = settings;
    save_settings(&st.cfg_db.lock(), &s)?;
    *st.settings.write() = s.clone();
    if s.roots != old.roots || s.excluded != old.excluded {
        rescan_now(&app);
    }
    Ok(())
}

// ---------- library ----------

fn rescan_now(app: &AppHandle) {
    let st = app.state::<AppState>();
    if st.scanning.swap(true, Ordering::AcqRel) {
        st.rescan_pending.store(true, Ordering::Release);
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let st = app.state::<AppState>();
        let _ = app.emit("library://scanning", true);
        loop {
            let (roots, excluded) = {
                let s = st.settings.read();
                (s.roots.clone(), s.excluded.clone())
            };
            let path = st.db_path.read().clone();
            match db::open(&path).and_then(|mut c| {
                let stats = scan::scan(&mut c, &roots, &excluded)?;
                Ok(stats)
            }) {
                // The source may have changed while scanning; only publish results for the active one.
                Ok(stats) => {
                    let _ = app.emit("library://changed", &stats);
                }
                Err(e) => {
                    let _ = app.emit("app://error", format!("Library scan failed: {e:#}"));
                }
            }
            if !st.rescan_pending.swap(false, Ordering::AcqRel) {
                break;
            }
        }
        st.scanning.store(false, Ordering::Release);
        let _ = app.emit("library://scanning", false);
    });
}

#[tauri::command]
fn rescan(app: AppHandle) {
    rescan_now(&app);
}

#[tauri::command]
async fn get_home(st: State<'_, AppState>) -> Res<Home> {
    library::home(&st.db.lock()).map_err(err)
}

#[tauri::command]
async fn get_roots(st: State<'_, AppState>) -> Res<Vec<Tile>> {
    let roots = st.settings.read().roots.clone();
    library::root_tiles(&st.db.lock(), &roots).map_err(err)
}

#[tauri::command]
async fn get_folder(st: State<'_, AppState>, path: String) -> Res<FolderView> {
    library::folder_view(&st.db.lock(), &path).map_err(err)
}

#[tauri::command]
async fn get_collection(st: State<'_, AppState>, kind: String, id: String) -> Res<Collection> {
    library::collection(&st.db.lock(), &kind, &id).map_err(err)
}

// ---------- cover protocol ----------

thread_local! {
    static PROTO_CONN: RefCell<Option<(Connection, PathBuf)>> = const { RefCell::new(None) };
}

static THUMBS: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
/// The active library database; cover-serving threads reconnect when it changes.
static ACTIVE_DB: RwLock<Option<PathBuf>> = parking_lot::const_rwlock(None);

fn with_proto_conn<T>(cell: &RefCell<Option<(Connection, PathBuf)>>, f: impl FnOnce(&Connection, &Path) -> anyhow::Result<T>) -> anyhow::Result<T> {
    let mut slot = cell.borrow_mut();
    let active = ACTIVE_DB.read().clone().ok_or_else(|| anyhow::anyhow!("not initialised"))?;
    if slot.as_ref().is_none_or(|(_, p)| *p != active) {
        *slot = Some((db::open(&active)?, active));
    }
    let thumbs = THUMBS.get().ok_or_else(|| anyhow::anyhow!("not initialised"))?;
    let (c, _) = slot.as_ref().expect("initialised above");
    f(c, thumbs)
}

/// `coffi://localhost/cover/<hash>/<size>`, `/collage/<h1,h2,..>/<size>`, `/blur/<hash>`.
fn serve_cover(path: &str) -> anyhow::Result<Vec<u8>> {
    let decoded = urlencoding::decode(path)?.into_owned();
    let parts: Vec<&str> = decoded.trim_start_matches('/').split('/').collect();
    let file = PROTO_CONN.with(|c| {
        with_proto_conn(c, |conn, thumbs| match parts.as_slice() {
            ["cover", h, size] => covers::thumb(conn, thumbs, h, size.parse().unwrap_or(256)),
            ["collage", hs, size] => {
                let hs: Vec<String> = hs.split(',').filter(|s| !s.is_empty()).map(String::from).collect();
                covers::collage(conn, thumbs, &hs, size.parse().unwrap_or(256))
            }
            ["blur", h] => covers::blurred(conn, thumbs, h),
            _ => Err(anyhow::anyhow!("bad cover path")),
        })
    })?;
    Ok(std::fs::read(file)?)
}

fn cover_response(req: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let builder = Response::builder().header("Access-Control-Allow-Origin", "*");
    // convertFileSrc() percent-encodes the whole path, slashes included.
    let path = urlencoding::decode(req.uri().path()).map(|p| p.into_owned()).unwrap_or_default();
    let result = serve_cover(&path);
    match result {
        Ok(bytes) => builder
            .header("Content-Type", "image/jpeg")
            .header("Cache-Control", "public, max-age=31536000, immutable")
            .body(bytes)
            .unwrap_or_default(),
        Err(_) => builder.status(StatusCode::NOT_FOUND).body(Vec::new()).unwrap_or_default(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("coffi", |_ctx, req, responder| {
            rayon::spawn(move || responder.respond(cover_response(&req)));
        })
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            let thumbs = app.path().app_cache_dir()?.join("thumbs");
            std::fs::create_dir_all(&data)?;
            std::fs::create_dir_all(&thumbs)?;
            let local_path = data.join("library.db");
            let _ = THUMBS.set(thumbs.clone());
            let cfg_conn = db::open(&local_path)?;
            let conn = &cfg_conn;

            let mut s: Settings = db::get_setting(conn, "settings")?.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default();
            if s.roots.is_empty() {
                if let Ok(music) = app.path().audio_dir() {
                    if music.is_dir() {
                        s.roots.push(music.to_string_lossy().into_owned());
                        save_settings(conn, &s).map_err(anyhow::Error::msg)?;
                    }
                }
            }

            let db_path = local_path.clone();
            *ACTIVE_DB.write() = Some(db_path.clone());
            let conn = db::open(&db_path)?;



            app.manage(AppState {
                db: Mutex::new(conn),
                db_path: RwLock::new(db_path),
                cfg_db: Mutex::new(cfg_conn),
                settings: RwLock::new(s),
                scanning: AtomicBool::new(false),
                rescan_pending: AtomicBool::new(false),
            });
            rescan_now(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_settings,
            rescan,
            get_home,
            get_roots,
            get_folder,
            get_collection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Coff-fi");
}
