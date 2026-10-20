mod audio;
mod library;
mod queue;
mod settings;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::{Mutex, RwLock};
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Emitter, Manager, State};

use audio::{Cmd, Event, PlaybackCfg, Player, Status};
use library::{covers, db, scan, Collection, FolderView, Home, SearchResults, Tile};
use queue::Queue;
use settings::{Repeat, Settings};

type Res<T> = Result<T, String>;

type Watcher = library::watch::Watcher;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub struct AppState {
    db: Mutex<Connection>,
    db_path: RwLock<PathBuf>,
    /// Settings always live in the local database, whichever library is active.
    cfg_db: Mutex<Connection>,
    settings: RwLock<Settings>,
    player: Player,
    random: RwLock<Vec<i64>>,
    watcher: Mutex<Option<Watcher>>,
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
    if s.volume != old.volume {
        st.player.send(Cmd::Volume(s.volume));
    }
    if s.eq != old.eq {
        st.player.send(Cmd::Eq(s.eq.clone()));
    }
    let cfg = PlaybackCfg::from(&s);
    if cfg != PlaybackCfg::from(&old) {
        st.player.send(Cmd::Config(cfg));
    }
    if s.shuffle != old.shuffle || s.repeat != old.repeat {
        {
            let mut q = st.player.queue.lock();
            q.set_shuffle(s.shuffle);
            q.repeat = s.repeat;
        }
        queue_changed(&app, &st);
    }
    if s.roots != old.roots || s.excluded != old.excluded || s.watch != old.watch {
        start_watcher(&app, &st);
        rescan_now(&app);
    }
    Ok(())
}

/// Shuffle / repeat without a full settings round-trip (also used by controlling devices, which
/// must not overwrite this device's other settings).
#[tauri::command]
fn set_play_mode(app: AppHandle, st: State<AppState>, shuffle: Option<bool>, repeat: Option<Repeat>) -> Res<()> {
    let mut s = st.settings.read().clone();
    if let Some(v) = shuffle {
        s.shuffle = v;
    }
    if let Some(v) = repeat {
        s.repeat = v;
    }
    set_settings(app.clone(), app.state::<AppState>(), s)?;
    let _ = app.emit("settings://changed", ());
    Ok(())
}

/// Volume changes arrive at slider speed, so they skip the full settings round-trip.
#[tauri::command]
fn set_volume(st: State<AppState>, volume: f32) -> Res<()> {
    st.player.send(Cmd::Volume(volume));
    let s = {
        let mut s = st.settings.write();
        s.volume = volume.clamp(0.0, 1.0);
        s.clone()
    };
    save_settings(&st.cfg_db.lock(), &s)
}

fn load_current_queue(conn: &Connection, s: &Settings) -> Queue {
    let mut q = Queue::default();
    q.shuffle = s.shuffle;
    q.repeat = s.repeat;
    let current: Option<i64> = conn.query_row("SELECT id FROM queues WHERE kind = 'current'", [], |r| r.get(0)).ok();
    if let Some((name, items, pos)) = current.and_then(|id| read_queue(conn, id).ok()) {
        q.restore(name, items, pos);
    }
    q
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
                Ok((stats, library::random_ids(&c)?))
            }) {
                // The source may have changed while scanning; only publish results for the active one.
                Ok((stats, random)) if *st.db_path.read() == path => {
                    if stats.updated > 0 || stats.removed > 0 || st.random.read().is_empty() {
                        *st.random.write() = random;
                    }
                    let _ = app.emit("library://changed", &stats);
                }
                Ok(_) => {}
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

fn start_watcher(app: &AppHandle, st: &AppState) {
    let s = st.settings.read().clone();
    let mut w = st.watcher.lock();
    *w = None;
    if s.watch {
        let handle = app.clone();
        match library::watch::watch(&s.roots, move || rescan_now(&handle)) {
            Ok(x) => *w = Some(x),
            Err(e) => eprintln!("watcher: {e}"),
        }
    }
}

#[tauri::command]
fn rescan(app: AppHandle) {
    rescan_now(&app);
}

#[tauri::command]
async fn get_home(st: State<'_, AppState>) -> Res<Home> {
    let random = st.random.read().clone();
    library::home(&st.db.lock(), &random).map_err(err)
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
    let random = st.random.read().clone();
    library::collection(&st.db.lock(), &kind, &id, &random).map_err(err)
}

#[tauri::command]
async fn search(st: State<'_, AppState>, query: String) -> Res<SearchResults> {
    library::search(&st.db.lock(), &query).map_err(err)
}

#[tauri::command]
async fn get_palette(hash: String) -> Res<Vec<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        PROTO_CONN.with(|c| with_proto_conn(c, |conn, thumbs| covers::palette(conn, thumbs, &hash)))
    })
    .await
    .map_err(err)?
    .map_err(err)
}

#[tauri::command]
fn toggle_favorite(app: AppHandle, st: State<AppState>, id: i64) -> Res<bool> {
    let fav = {
        let conn = st.db.lock();
        let fav = db::toggle_favorite(&conn, id).map_err(err)?;
        fav
    };
    let _ = app.emit("stats://changed", ());
    Ok(fav)
}

#[tauri::command]
fn is_favorite(st: State<AppState>, id: i64) -> Res<bool> {
    db::is_favorite(&st.db.lock(), id).map_err(err)
}

// ---------- queue & playback ----------

#[derive(Serialize)]
struct QueueView {
    name: String,
    pos: usize,
    shuffle: bool,
    repeat: Repeat,
    tracks: Vec<db::Track>,
}

fn write_queue(conn: &Connection, id: Option<i64>, kind: &str, q: &Queue) -> rusqlite::Result<i64> {
    let id = match id {
        Some(id) => {
            conn.execute("UPDATE queues SET name = ?2, pos = ?3, updated_at = ?4 WHERE id = ?1", params![id, q.name, q.pos as i64, db::now()])?;
            conn.execute("DELETE FROM queue_items WHERE queue_id = ?1", [id])?;
            id
        }
        None => {
            conn.execute("INSERT INTO queues(name, kind, pos, updated_at) VALUES(?1, ?2, ?3, ?4)", params![q.name, kind, q.pos as i64, db::now()])?;
            conn.last_insert_rowid()
        }
    };
    let mut ins = conn.prepare_cached("INSERT INTO queue_items(queue_id, idx, track_id) VALUES(?1, ?2, ?3)")?;
    for (i, t) in q.items.iter().enumerate() {
        ins.execute(params![id, i as i64, t])?;
    }
    Ok(id)
}

fn persist_current(conn: &mut Connection, q: &Queue) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    let existing: Option<i64> = tx.query_row("SELECT id FROM queues WHERE kind = 'current'", [], |r| r.get(0)).ok();
    write_queue(&tx, existing, "current", q)?;
    tx.commit()
}

fn read_queue(conn: &Connection, id: i64) -> rusqlite::Result<(String, Vec<i64>, usize)> {
    let (name, pos): (String, i64) = conn.query_row("SELECT name, pos FROM queues WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let mut st = conn.prepare_cached("SELECT track_id FROM queue_items WHERE queue_id = ?1 ORDER BY idx")?;
    let items = st.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<Vec<i64>>>()?;
    Ok((name, items, pos.max(0) as usize))
}

fn queue_changed(app: &AppHandle, st: &AppState) {
    let q = st.player.queue.lock().clone();
    if let Err(e) = persist_current(&mut st.db.lock(), &q) {
        eprintln!("persist queue: {e}");
    }
    st.player.send(Cmd::QueueChanged);
    let _ = app.emit("queue://changed", ());
}

#[tauri::command]
fn play_tracks(app: AppHandle, st: State<AppState>, ids: Vec<i64>, start: usize, name: String) -> Res<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let id = {
        let mut q = st.player.queue.lock();
        q.set(name, ids.clone(), start);
        // With shuffle on, `set` moves the chosen track to the front.
        q.current()
    };
    queue_changed(&app, &st);
    if let Some(id) = id {
        st.player.send(Cmd::Load { id, start: 0.0, play: true });
    }
    Ok(())
}

#[tauri::command]
fn enqueue(app: AppHandle, st: State<AppState>, ids: Vec<i64>, next: bool) -> Res<()> {
    let was_empty = {
        let mut q = st.player.queue.lock();
        let empty = q.is_empty();
        if empty {
            q.name = "Queue".into();
        }
        q.add(&ids, next);
        empty
    };
    queue_changed(&app, &st);
    if was_empty && st.player.status.read().track_id.is_none() {
        if let Some(id) = st.player.queue.lock().current() {
            st.player.send(Cmd::Load { id, start: 0.0, play: true });
        }
    }
    Ok(())
}

#[tauri::command]
fn player_cmd(st: State<AppState>, cmd: String, value: Option<f64>) -> Res<()> {
    let c = match cmd.as_str() {
        "toggle" => Cmd::Toggle,
        "play" => Cmd::SetPlaying(true),
        "pause" => Cmd::SetPlaying(false),
        "next" => Cmd::Next,
        "prev" => Cmd::Prev,
        "stop" => Cmd::Stop,
        "seek" => Cmd::Seek(value.unwrap_or(0.0)),
        _ => return Err(format!("unknown command {cmd}")),
    };
    st.player.send(c);
    Ok(())
}

#[tauri::command]
fn get_status(st: State<AppState>) -> Status {
    st.player.status.read().clone()
}

#[tauri::command]
async fn get_queue(st: State<'_, AppState>) -> Res<QueueView> {
    let q = st.player.queue.lock().clone();
    let tracks = db::tracks_by_ids(&st.db.lock(), &q.items).map_err(err)?;
    Ok(QueueView { name: q.name, pos: q.pos, shuffle: q.shuffle, repeat: q.repeat, tracks })
}

#[tauri::command]
fn queue_edit(app: AppHandle, st: State<AppState>, op: String, a: usize, b: Option<usize>) -> Res<()> {
    let jump = {
        let mut q = st.player.queue.lock();
        let before = q.current();
        match op.as_str() {
            "jump" => {
                q.jump(a);
                q.current()
            }
            "remove" => {
                let removing_current = a == q.pos;
                q.remove(a);
                if removing_current { q.current().filter(|_| before.is_some()) } else { None }
            }
            "move" => {
                q.move_item(a, b.unwrap_or(a));
                None
            }
            _ => return Err(format!("unknown op {op}")),
        }
    };
    queue_changed(&app, &st);
    match (op.as_str(), jump) {
        (_, Some(id)) => st.player.send(Cmd::Load { id, start: 0.0, play: true }),
        ("remove", None) if st.player.queue.lock().is_empty() => st.player.send(Cmd::Stop),
        _ => {}
    }
    Ok(())
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

// ---------- events ----------

fn on_player_event(app: &AppHandle, e: Event) {
    let st = app.state::<AppState>();
    match e {
        Event::State(s) => {
                let _ = app.emit("player://state", s);
        }
        Event::Track(id) => {
            let track = id.and_then(|id| db::track(&st.db.lock(), id).ok().flatten());
            let _ = app.emit("player://track", &track);
            let _ = app.emit("queue://changed", ());
            let q = st.player.queue.lock().clone();
            let _ = persist_current(&mut st.db.lock(), &q);
        }
        Event::Played(id) => {
            {
                let conn = st.db.lock();
                let _ = db::record_play(&conn, id);
            }
            let _ = app.emit("stats://changed", ());
        }
        Event::Error(msg) => {
            let _ = app.emit("app://error", msg);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
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
            let q = load_current_queue(&conn, &s);
            let random = library::random_ids(&conn).unwrap_or_default();

            let h = app.handle().clone();
            let player = Player::spawn(db_path.clone(), &s, q, move |e| on_player_event(&h, e));


            app.manage(AppState {
                db: Mutex::new(conn),
                db_path: RwLock::new(db_path),
                cfg_db: Mutex::new(cfg_conn),
                settings: RwLock::new(s),
                player,
                random: RwLock::new(random),
                watcher: Mutex::new(None),
                scanning: AtomicBool::new(false),
                rescan_pending: AtomicBool::new(false),
            });
            let st = app.state::<AppState>();
            start_watcher(app.handle(), &st);
            rescan_now(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_settings,
            set_volume,
            rescan,
            get_home,
            get_roots,
            get_folder,
            get_collection,
            get_palette,
            toggle_favorite,
            is_favorite,
            play_tracks,
            enqueue,
            player_cmd,
            get_status,
            get_queue,
            queue_edit,
            search,
            set_play_mode,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Coff-fi");
}
