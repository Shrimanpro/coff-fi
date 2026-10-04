mod library;
mod settings;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::{Mutex, RwLock};
use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager, State};

use library::{db, scan};
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data)?;
            let local_path = data.join("library.db");
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Coff-fi");
}
