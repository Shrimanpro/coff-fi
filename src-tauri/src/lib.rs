mod library;
mod settings;


use parking_lot::{Mutex, RwLock};
use rusqlite::Connection;
use tauri::{AppHandle, Manager, State};

use library::db;
use settings::Settings;

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub struct AppState {
    /// Settings always live in the local database, whichever library is active.
    cfg_db: Mutex<Connection>,
    settings: RwLock<Settings>,
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
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data)?;
            let local_path = data.join("library.db");
            let cfg_conn = db::open(&local_path)?;
            let conn = &cfg_conn;

            let mut s: Settings = db::get_setting(conn, "settings")?.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default();




            app.manage(AppState {
                cfg_db: Mutex::new(cfg_conn),
                settings: RwLock::new(s),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Coff-fi");
}
