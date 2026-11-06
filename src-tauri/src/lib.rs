mod audio;
mod cache;
mod integrations;
mod library;
mod lyrics;
#[cfg(target_os = "ios")]
mod native;
mod navidrome;
mod queue;
mod remote;
mod settings;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use parking_lot::{Mutex, RwLock};
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Emitter, Manager, State};

use audio::{Cmd, Event, PlaybackCfg, Player, Status};
use integrations::{Control, Integrations, Msg, NowPlaying};
use library::{covers, db, scan, Collection, FolderView, Home, SearchResults, Tile};
use queue::Queue;
use settings::{Navidrome, Repeat, Settings};

type Res<T> = Result<T, String>;

#[cfg(not(target_os = "ios"))]
type Watcher = library::watch::Watcher;
/// iOS has no file watcher; the library is rescanned when the app comes to the foreground.
#[cfg(target_os = "ios")]
type Watcher = ();

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub struct AppState {
    /// The active library: local files or the Navidrome mirror.
    db: Mutex<Connection>,
    db_path: RwLock<PathBuf>,
    local_path: PathBuf,
    remote_path: PathBuf,
    /// Settings always live in the local database, whichever library is active.
    cfg_db: Mutex<Connection>,
    settings: RwLock<Settings>,
    player: Player,
    integrations: Integrations,
    random: RwLock<Vec<i64>>,
    #[cfg_attr(target_os = "ios", allow(dead_code))]
    watcher: Mutex<Option<Watcher>>,
    scanning: AtomicBool,
    rescan_pending: AtomicBool,
    /// Sleep timer: what's armed, and a generation counter that cancels older timers.
    sleep: Mutex<SleepState>,
    sleep_gen: AtomicU64,
}

#[derive(Clone, Default, Serialize)]
struct SleepState {
    /// "off" | "minutes" | "end_of_track"
    mode: String,
    /// Unix ms when a "minutes" timer fires.
    until: Option<i64>,
}

const HISTORY_KEEP: i64 = 20;

impl AppState {
    fn remote(&self) -> bool {
        *self.db_path.read() == self.remote_path
    }
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
    // Switching sources goes through `set_source`, which checks the connection first.
    s.navidrome.active = old.navidrome.active;
    s.remote.id = old.remote.id.clone();
    save_settings(&st.cfg_db.lock(), &s)?;
    *st.settings.write() = s.clone();
    if s.volume != old.volume {
        st.player.send(Cmd::Volume(s.volume));
    }
    if s.eq != old.eq {
        st.player.send(Cmd::Eq(s.eq.clone()));
    }
    if s.output_device != old.output_device {
        st.player.send(Cmd::Device(s.output_device.clone()));
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
    let remote = st.remote();
    if !remote && (s.roots != old.roots || s.excluded != old.excluded || s.watch != old.watch) {
        start_watcher(&app, &st);
        rescan_now(&app);
    }
    if remote && s.navidrome != old.navidrome {
        navidrome::configure(Some(s.navidrome.clone()));
        rescan_now(&app);
    }
    let remote_changed = s.remote != old.remote;
    let mode = (s.volume, s.shuffle, s.repeat) != (old.volume, old.shuffle, old.repeat);
    st.integrations.send(Msg::Config(Box::new(s)));
    if remote_changed {
        remote::apply(&app);
    }
    if mode {
        mode_changed(&app);
    }
    Ok(())
}

/// Tells controlling devices about volume / shuffle / repeat changes.
fn mode_changed(app: &AppHandle) {
    let s = app.state::<AppState>().settings.read().clone();
    let _ = app.emit("player://mode", serde_json::json!({ "volume": s.volume, "shuffle": s.shuffle, "repeat": s.repeat }));
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

// ---------- other devices ----------

#[tauri::command]
fn remote_devices() -> Vec<remote::Device> {
    remote::devices()
}

#[derive(Serialize)]
struct RemoteState {
    /// The device we're controlling, if any.
    target: Option<remote::Device>,
    /// Devices controlling us.
    controllers: Vec<String>,
    /// Our own name on the network.
    name: String,
}

#[tauri::command]
fn remote_state(app: AppHandle) -> RemoteState {
    RemoteState { target: remote::client::target(), controllers: remote::server::controllers(), name: remote::my_name(&app) }
}

/// Starts controlling a discovered device (`id`) or one typed in by address.
#[tauri::command]
async fn remote_connect(app: AppHandle, id: Option<String>, address: Option<String>) -> Res<remote::Device> {
    let device = match (id, address) {
        (Some(id), _) => remote::find(&id).ok_or("That device isn't available any more")?,
        (None, Some(a)) => {
            let (host, port) = remote::parse_address(&a).ok_or("Enter an address like 192.168.1.20")?;
            remote::Device { id: format!("{host}:{port}"), name: host.clone(), platform: String::new(), host, port }
        }
        _ => return Err("Pick a device".into()),
    };
    let (name, kind) = (device.name.clone(), if device.platform == "ios" { "iPhone" } else { "device" });
    remote::client::connect(&app, device, remote::me(&app)).await.map_err(|e| {
        use remote::client::Unreachable;
        let hint = match e.downcast_ref::<Unreachable>() {
            Some(Unreachable::NoAnswer) => format!(
                " Make sure Coff-fi is open on {name}{}, both are on the same Wi-Fi, and Coff-fi is allowed under Local Network in Privacy & Security settings on both devices.",
                if kind == "iPhone" { " (unlocked, or playing music)" } else { "" }
            ),
            Some(Unreachable::Refused) => format!(" Check that “Let other devices control playback” is on in {name}'s Coff-fi settings (Devices)."),
            Some(Unreachable::NotFound) => " Open Coff-fi on it, then try again.".into(),
            None => String::new(),
        };
        format!("Couldn't connect to {name}: {e:#}.{hint}")
    })
}

#[tauri::command]
fn remote_disconnect(app: AppHandle) {
    remote::client::disconnect(&app, true);
}

/// Runs a library / queue / playback command on the device being controlled.
#[tauri::command]
async fn remote_call(cmd: String, args: Option<serde_json::Value>) -> Res<serde_json::Value> {
    remote::client::call(&cmd, args.unwrap_or(serde_json::Value::Null)).await
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

#[tauri::command]
fn list_devices() -> Vec<String> {
    audio::output::device_names()
}

#[tauri::command]
async fn lastfm_login(app: AppHandle, username: String, password: String) -> Res<()> {
    let st = app.state::<AppState>();
    let (key, secret) = {
        let s = st.settings.read();
        (s.lastfm.api_key.clone(), s.lastfm.api_secret.clone())
    };
    let user = username.clone();
    let sk = tauri::async_runtime::spawn_blocking(move || integrations::lastfm::login(&key, &secret, &user, &password))
        .await
        .map_err(err)?
        .map_err(err)?;
    let s = {
        let mut s = st.settings.write();
        s.lastfm.session_key = sk;
        s.lastfm.username = username;
        s.lastfm.enabled = true;
        s.clone()
    };
    save_settings(&st.cfg_db.lock(), &s)?;
    st.integrations.send(Msg::Config(Box::new(s)));
    Ok(())
}

// ---------- navidrome ----------

#[tauri::command]
async fn navidrome_test(server: Navidrome) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || navidrome::ping(&server)).await.map_err(err)?.map_err(|e| format!("{e:#}"))
}

/// Switches between local files and the Navidrome server. Turning Navidrome on requires a
/// working connection. Returns a description of the active source.
#[tauri::command]
async fn set_source(app: AppHandle, remote: bool) -> Res<String> {
    let st = app.state::<AppState>();
    let server = st.settings.read().navidrome.clone();
    let label = if remote {
        let srv = server.clone();
        let name = tauri::async_runtime::spawn_blocking(move || navidrome::ping(&srv)).await.map_err(err)?.map_err(|e| format!("{e:#}"))?;
        navidrome::configure(Some(server));
        name
    } else {
        "Local files".to_string()
    };
    switch_source(&app, remote)?;
    if remote {
        rescan_now(&app);
    }
    Ok(label)
}

fn switch_source(app: &AppHandle, remote: bool) -> Res<()> {
    let st = app.state::<AppState>();
    let path = if remote { st.remote_path.clone() } else { st.local_path.clone() };
    if *st.db_path.read() != path {
        // Each library keeps its own queues; park the current one before leaving.
        let q = st.player.queue.lock().clone();
        let _ = persist_current(&mut st.db.lock(), &q);
        st.player.send(Cmd::Database(path.clone()));
        let conn = db::open(&path).map_err(err)?;
        let q = load_current_queue(&conn, &st.settings.read());
        *st.random.write() = library::random_ids(&conn).unwrap_or_default();
        *st.db.lock() = conn;
        *st.db_path.write() = path.clone();
        *ACTIVE_DB.write() = Some(path);
        *st.player.queue.lock() = q;
        start_watcher(app, &st);
    }
    let s = {
        let mut s = st.settings.write();
        s.navidrome.active = remote;
        s.clone()
    };
    save_settings(&st.cfg_db.lock(), &s)?;
    if !remote {
        navidrome::configure(None);
    }
    let _ = app.emit("source://changed", remote);
    let _ = app.emit("library://changed", ());
    let _ = app.emit("queue://changed", ());
    Ok(())
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
            let remote = path == st.remote_path;
            match db::open(&path).and_then(|mut c| {
                let stats = if remote { navidrome::sync(&mut c)? } else { scan::scan(&mut c, &roots, &excluded)? };
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
                Err(e) if remote => {
                    let _ = app.emit("app://error", format!("Couldn't sync with Navidrome, switched back to local files. {e:#}"));
                    let _ = switch_source(&app, false);
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

#[cfg(target_os = "ios")]
fn start_watcher(_app: &AppHandle, _st: &AppState) {}

#[cfg(not(target_os = "ios"))]
fn start_watcher(app: &AppHandle, st: &AppState) {
    let s = st.settings.read().clone();
    let mut w = st.watcher.lock();
    *w = None;
    if s.watch && !st.remote() {
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
    let roots = if st.remote() { vec![navidrome::ROOT.to_string()] } else { st.settings.read().roots.clone() };
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

// ---------- song info ----------

/// Path of a local library song; Navidrome songs aren't files we can edit.
fn local_file(st: &AppState, id: i64) -> Res<String> {
    if st.remote() {
        return Err("Only local files can be edited".into());
    }
    Ok(db::track(&st.db.lock(), id).map_err(err)?.ok_or("Song not found")?.path)
}

#[tauri::command]
async fn get_tags(st: State<'_, AppState>, id: i64) -> Res<library::tags::FileInfo> {
    let path = local_file(&st, id)?;
    library::tags::read(Path::new(&path)).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn set_tags(app: AppHandle, id: i64, tags: library::tags::Tags) -> Res<db::Track> {
    let st = app.state::<AppState>();
    let path = local_file(&st, id)?;
    library::tags::write(Path::new(&path), &tags).map_err(|e| format!("{e:#}"))?;
    let track = {
        let conn = st.db.lock();
        scan::refresh_file(&conn, &path).map_err(err)?;
        db::track(&conn, id).map_err(err)?.ok_or("Song not found")?
    };
    let _ = app.emit("library://changed", ());
    Ok(track)
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
        if let Some(t) = db::track(&conn, id).ok().flatten() {
            navidrome::set_star(&t.path, fav);
        }
        fav
    };
    let _ = app.emit("stats://changed", ());
    Ok(fav)
}

#[tauri::command]
fn is_favorite(st: State<AppState>, id: i64) -> Res<bool> {
    db::is_favorite(&st.db.lock(), id).map_err(err)
}

#[tauri::command]
async fn get_lyrics(app: AppHandle, id: i64, refresh: bool) -> Res<Option<lyrics::Lyrics>> {
    let st = app.state::<AppState>();
    let path = st.db_path.read().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db::open(&path)?;
        let Some(t) = db::track(&conn, id)? else { return Ok(None) };
        lyrics::get(&conn, &t, refresh)
    })
    .await
    .map_err(err)?
    .map_err(|e: anyhow::Error| err(e))
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

#[derive(Serialize)]
struct SavedQueue {
    id: i64,
    name: String,
    kind: String,
    count: i64,
    updated_at: i64,
    covers: Vec<String>,
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
    update_cache(st);
    let _ = app.emit("queue://changed", ());
}

/// Keeps the 9-song queue cache in step with what's playing and coming up next.
fn update_cache(st: &AppState) {
    if !cache::enabled() {
        return;
    }
    if !st.remote() {
        cache::update(Vec::new());
        return;
    }
    let q = st.player.queue.lock().clone();
    let n = q.items.len();
    // The current song and the next 8 in play order; with repeat on the queue wraps around.
    let ahead = if q.repeat != Repeat::Off { n } else { n.saturating_sub(q.pos) };
    let span: Vec<i64> = (0..ahead.min(cache::SLOTS)).map(|k| q.items[(q.pos + k) % n]).collect();
    let tracks = db::tracks_by_ids(&st.db.lock(), &span).unwrap_or_default();
    let items: Vec<(i64, Option<String>)> = span
        .iter()
        .map(|id| (*id, tracks.iter().find(|t| t.id == *id).and_then(|t| navidrome::song_id(&t.path)).map(String::from)))
        .collect();
    let ids = cache::window(&items, 0, false);
    cache::update(ids);
}

/// Moves the current queue into history before it gets replaced.
fn archive_current(st: &AppState) {
    let q = st.player.queue.lock().clone();
    if q.is_empty() {
        return;
    }
    let conn = st.db.lock();
    let _ = write_queue(&conn, None, "history", &q);
    let _ = conn.execute(
        "DELETE FROM queues WHERE kind = 'history' AND id NOT IN (SELECT id FROM queues WHERE kind = 'history' ORDER BY updated_at DESC, id DESC LIMIT ?1)",
        [HISTORY_KEEP],
    );
    let _ = conn.execute("DELETE FROM queue_items WHERE queue_id NOT IN (SELECT id FROM queues)", []);
}

#[tauri::command]
fn play_tracks(app: AppHandle, st: State<AppState>, ids: Vec<i64>, start: usize, name: String) -> Res<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let same = {
        let q = st.player.queue.lock();
        q.items == ids && q.name == name
    };
    if !same {
        archive_current(&st);
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

#[tauri::command]
async fn list_queues(st: State<'_, AppState>) -> Res<Vec<SavedQueue>> {
    let conn = st.db.lock();
    let mut s = conn
        .prepare(
            "SELECT q.id, q.name, q.kind, q.updated_at, (SELECT COUNT(*) FROM queue_items i WHERE i.queue_id = q.id)
             FROM queues q WHERE q.kind != 'current' ORDER BY q.kind = 'history', q.updated_at DESC, q.id DESC",
        )
        .map_err(err)?;
    let rows: Vec<(i64, String, String, i64, i64)> =
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).map_err(err)?.filter_map(Result::ok).collect();
    let mut cov = conn
        .prepare("SELECT t.cover FROM queue_items i JOIN tracks t ON t.id = i.track_id WHERE i.queue_id = ?1 ORDER BY i.idx LIMIT 64")
        .map_err(err)?;
    Ok(rows
        .into_iter()
        .map(|(id, name, kind, updated_at, count)| {
            let c: Vec<Option<String>> = cov.query_map([id], |r| r.get(0)).map(|r| r.filter_map(Result::ok).collect()).unwrap_or_default();
            SavedQueue { id, name, kind, count, updated_at, covers: library::first_covers(c.iter(), 4) }
        })
        .collect())
}

#[tauri::command]
fn load_queue(app: AppHandle, st: State<AppState>, id: i64) -> Res<()> {
    let (name, items, pos) = read_queue(&st.db.lock(), id).map_err(err)?;
    archive_current(&st);
    let cur = {
        let mut q = st.player.queue.lock();
        q.restore(name, items, pos);
        q.current()
    };
    queue_changed(&app, &st);
    if let Some(id) = cur {
        st.player.send(Cmd::Load { id, start: 0.0, play: true });
    }
    Ok(())
}

#[tauri::command]
fn save_queue(st: State<AppState>, name: String) -> Res<()> {
    let mut q = st.player.queue.lock().clone();
    if q.is_empty() {
        return Err("The queue is empty".into());
    }
    q.name = name;
    write_queue(&st.db.lock(), None, "saved", &q).map_err(err)?;
    Ok(())
}

#[tauri::command]
fn delete_queue(st: State<AppState>, id: i64) -> Res<()> {
    let conn = st.db.lock();
    conn.execute("DELETE FROM queues WHERE id = ?1 AND kind != 'current'", [id]).map_err(err)?;
    conn.execute("DELETE FROM queue_items WHERE queue_id = ?1", [id]).map_err(err)?;
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
    // `/r/...`: a cover of the device we're controlling.
    let result = match path.strip_prefix("/r/") {
        Some(rest) => remote::client::cover(&format!("/{rest}")).ok_or_else(|| anyhow::anyhow!("no remote cover")),
        None => serve_cover(&path),
    };
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

/// True while the iOS app is in the background: the web UI isn't visible, so per-status
/// events to it are skipped (each one wakes the WebView's process).
static BACKGROUND: AtomicBool = AtomicBool::new(false);

pub(crate) fn in_background() -> bool {
    BACKGROUND.load(Ordering::Acquire)
}

/// App lifecycle (iOS): buffer further ahead in the background, and bring the UI up to date
/// when it becomes visible again.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
fn set_background(app: &AppHandle, on: bool) {
    BACKGROUND.store(on, Ordering::Release);
    let Some(st) = app.try_state::<AppState>() else { return };
    st.player.send(Cmd::Background(on));
    remote::set_background(app, on);
    // In the background with nothing playing iOS suspends the app: stop advertising until then.
    remote::set_present(app, !on || st.player.status.read().playing);
    if !on {
        let status = st.player.status.read().clone();
        let track = status.track_id.and_then(|id| db::track(&st.db.lock(), id).ok().flatten());
        let _ = app.emit("player://track", &track);
        let _ = app.emit("player://state", status);
    }
}

fn on_player_event(app: &AppHandle, e: Event) {
    let st = app.state::<AppState>();
    match e {
        Event::State(s) => {
            st.integrations.send(Msg::Playback { playing: s.playing, position: s.position });
            if let Ok(json) = serde_json::to_string(&s) {
                remote::server::broadcast("player://state", &json);
            }
            if in_background() {
                remote::set_present(app, s.playing);
            }
            if !in_background() {
                let _ = app.emit("player://state", s);
            }
        }
        Event::Track(id) => {
            let track = id.and_then(|id| db::track(&st.db.lock(), id).ok().flatten());
            let _ = app.emit("player://track", &track);
            if let Some(t) = &track {
                navidrome::scrobble(&t.path, false);
                // Warm the lyrics cache so the full-screen view has them immediately.
                let (t, path) = (t.clone(), st.db_path.read().clone());
                std::thread::spawn(move || {
                    if let Ok(conn) = db::open(&path) {
                        let _ = lyrics::get(&conn, &t, false);
                    }
                });
            }
            let _ = app.emit("queue://changed", ());
            let q = st.player.queue.lock().clone();
            let _ = persist_current(&mut st.db.lock(), &q);
            update_cache(&st);
            let integ = st.integrations.clone();
            match track {
                Some(t) => {
                    // Generating the cover thumbnail may take a moment; keep it off the player thread.
                    std::thread::spawn(move || {
                        let cover = t.cover.as_ref().and_then(|h| {
                            PROTO_CONN.with(|c| with_proto_conn(c, |conn, thumbs| covers::thumb(conn, thumbs, h, 640))).ok()
                        });
                        integ.send(Msg::Track(Some(NowPlaying { title: t.title, artist: t.artist, album: t.album, duration: t.duration, cover })));
                    });
                }
                None => integ.send(Msg::Track(None)),
            }
        }
        Event::Played(id) => {
            {
                let conn = st.db.lock();
                let _ = db::record_play(&conn, id);
                if let Some(t) = db::track(&conn, id).ok().flatten() {
                    navidrome::scrobble(&t.path, true);
                }
            }
            st.integrations.send(Msg::Played);
            let _ = app.emit("stats://changed", ());
        }
        Event::Error(msg) => {
            let _ = app.emit("app://error", msg);
        }
        Event::Slept => {
            *st.sleep.lock() = SleepState { mode: "off".into(), until: None };
            let _ = app.emit("sleep://changed", st.sleep.lock().clone());
        }
    }
}

#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
fn apply_control(app: &AppHandle, c: Control) {
    on_control(app, c)
}

fn on_control(app: &AppHandle, c: Control) {
    let st = app.state::<AppState>();
    let pos = st.player.status.read().position;
    st.player.send(match c {
        Control::Play => Cmd::SetPlaying(true),
        Control::Pause => Cmd::SetPlaying(false),
        Control::Toggle => Cmd::Toggle,
        Control::Next => Cmd::Next,
        Control::Prev => Cmd::Prev,
        Control::Stop => Cmd::Stop,
        Control::SeekTo(s) => Cmd::Seek(s),
        Control::SeekBy(d) => Cmd::Seek(pos + d),
    });
}

// ---------- platform & iOS extras ----------

#[tauri::command]
fn get_platform() -> &'static str {
    if cfg!(target_os = "ios") {
        "ios"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(windows) {
        "windows"
    } else {
        "linux"
    }
}

/// Whether a song plays from the on-device queue cache (for the "Cache · FLAC" quality line).
#[tauri::command]
fn is_cached(st: State<AppState>, id: i64) -> bool {
    cache::enabled()
        && db::track(&st.db.lock(), id).ok().flatten().is_some_and(|t| navidrome::song_id(&t.path).and_then(cache::cached).is_some())
}

/// Things only the native layer can show: "airplay" (route picker), "pick_folder" (Files).
#[tauri::command]
fn native_request(what: String) -> Res<()> {
    #[cfg(target_os = "ios")]
    {
        native::request(&what);
        Ok(())
    }
    #[cfg(not(target_os = "ios"))]
    Err(format!("{what} is only available on iOS"))
}

/// Toggles the favorite of the playing song (lock-screen "like").
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
fn toggle_current_favorite(app: &AppHandle) {
    let st = app.state::<AppState>();
    let Some(id) = st.player.status.read().track_id else { return };
    if let Ok(fav) = toggle_favorite(app.clone(), app.state::<AppState>(), id) {
        let _ = app.emit("player://favorite", (id, fav));
    }
}

/// Adds a music folder (from the iOS Files picker) and rescans. An `exclude:` prefix adds it to
/// the excluded folders instead.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
fn add_root(app: &AppHandle, path: String) {
    let st = app.state::<AppState>();
    let s = {
        let mut s = st.settings.write();
        let (list, path) = match path.strip_prefix("exclude:") {
            Some(p) => (&mut s.excluded, p.to_string()),
            None => (&mut s.roots, path),
        };
        if list.contains(&path) {
            return;
        }
        list.push(path);
        s.clone()
    };
    let _ = save_settings(&st.cfg_db.lock(), &s);
    #[cfg(target_os = "ios")]
    native::emit_settings_changed(app);
    if !st.remote() {
        rescan_now(app);
    }
}

#[tauri::command]
fn get_sleep(st: State<AppState>) -> SleepState {
    let s = st.sleep.lock().clone();
    if s.mode.is_empty() { SleepState { mode: "off".into(), until: None } } else { s }
}

/// Sleep timer. `mode`: "off" | "minutes" (pause after `minutes`) | "end_of_track".
#[tauri::command]
fn sleep_timer(app: AppHandle, st: State<AppState>, mode: String, minutes: Option<f64>) -> Res<SleepState> {
    let generation = st.sleep_gen.fetch_add(1, Ordering::AcqRel) + 1;
    st.player.send(Cmd::StopAfterCurrent(mode == "end_of_track"));
    let state = match mode.as_str() {
        "minutes" => {
            let mins = minutes.filter(|m| *m > 0.0).ok_or("Pick how many minutes")?;
            let until = db::now() * 1000 + (mins * 60_000.0) as i64;
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs_f64(mins * 60.0));
                let st = app.state::<AppState>();
                if st.sleep_gen.load(Ordering::Acquire) == generation {
                    st.player.send(Cmd::SetPlaying(false));
                    *st.sleep.lock() = SleepState { mode: "off".into(), until: None };
                    let _ = app.emit("sleep://changed", st.sleep.lock().clone());
                }
            });
            SleepState { mode, until: Some(until) }
        }
        "end_of_track" => SleepState { mode, until: None },
        "off" => SleepState { mode, until: None },
        _ => return Err(format!("unknown sleep mode {mode}")),
    };
    *st.sleep.lock() = state.clone();
    let _ = app.emit("sleep://changed", &state);
    Ok(state)
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
            let remote_path = data.join("navidrome.db");
            let _ = THUMBS.set(thumbs.clone());
            navidrome::init(app.path().app_cache_dir()?);
            #[cfg(target_os = "ios")]
            {
                cache::init(&app.path().app_cache_dir()?);
                native::set_app(app.handle().clone());
            }
            let cfg_conn = db::open(&local_path)?;
            let conn = &cfg_conn;

            let mut s: Settings = db::get_setting(conn, "settings")?.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default();
            if s.remote.id.is_empty() {
                let seed = format!("{:?}-{}-{}", std::time::SystemTime::now(), std::process::id(), data.display());
                s.remote.id = format!("{:016x}", xxhash_rust::xxh3::xxh3_64(seed.as_bytes()));
                save_settings(conn, &s).map_err(anyhow::Error::msg)?;
            }
            // iOS moves the app's container on every install/update, so absolute paths into our own
            // Documents folder go stale. Re-anchor them on the current Documents folder.
            #[cfg(target_os = "ios")]
            if let Ok(docs) = app.path().document_dir() {
                let mut changed = false;
                for list in [&mut s.roots, &mut s.excluded] {
                    for r in list.iter_mut() {
                        if Path::new(r.as_str()).exists() {
                            continue;
                        }
                        if let Some((_, rest)) = r.split_once("/Documents/") {
                            let fixed = docs.join(rest).to_string_lossy().into_owned();
                            if fixed != *r {
                                *r = fixed;
                                changed = true;
                            }
                        }
                    }
                }
                if changed {
                    save_settings(conn, &s).map_err(anyhow::Error::msg)?;
                }
            }
            // iOS: the app's own Documents folder (visible in the Files app) is the default library.
            #[cfg(target_os = "ios")]
            if s.roots.is_empty() {
                if let Ok(docs) = app.path().document_dir() {
                    let music = docs.join("Music");
                    let _ = std::fs::create_dir_all(&music);
                    s.roots.push(music.to_string_lossy().into_owned());
                    save_settings(conn, &s).map_err(anyhow::Error::msg)?;
                }
            }
            #[cfg(not(target_os = "ios"))]
            if s.roots.is_empty() {
                if let Ok(music) = app.path().audio_dir() {
                    if music.is_dir() {
                        s.roots.push(music.to_string_lossy().into_owned());
                        save_settings(conn, &s).map_err(anyhow::Error::msg)?;
                    }
                }
            }

            // Navidrome mode resumes on the last synced mirror; the startup sync verifies the
            // connection and falls back to local files if the server is unreachable.
            let remote = s.navidrome.active;
            if remote {
                navidrome::configure(Some(s.navidrome.clone()));
            }
            let db_path = if remote { remote_path.clone() } else { local_path.clone() };
            *ACTIVE_DB.write() = Some(db_path.clone());
            let conn = db::open(&db_path)?;
            let q = load_current_queue(&conn, &s);
            let random = library::random_ids(&conn).unwrap_or_default();

            let h = app.handle().clone();
            let player = Player::spawn(db_path.clone(), &s, q, move |e| on_player_event(&h, e));

            #[cfg(windows)]
            let hwnd = app.get_webview_window("main").and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize);
            #[cfg(not(windows))]
            let hwnd: Option<isize> = None;
            let h = app.handle().clone();
            let integ = integrations::spawn(s.clone(), hwnd, move |c| on_control(&h, c));

            app.manage(AppState {
                db: Mutex::new(conn),
                db_path: RwLock::new(db_path),
                local_path,
                remote_path,
                cfg_db: Mutex::new(cfg_conn),
                settings: RwLock::new(s),
                player,
                integrations: integ,
                random: RwLock::new(random),
                watcher: Mutex::new(None),
                scanning: AtomicBool::new(false),
                rescan_pending: AtomicBool::new(false),
                sleep: Mutex::new(SleepState { mode: "off".into(), until: None }),
                sleep_gen: AtomicU64::new(0),
            });
            let st = app.state::<AppState>();
            start_watcher(app.handle(), &st);
            rescan_now(app.handle());
            remote::init(app.handle());
            // Debug builds only: COFFI_EVAL=<js> runs a script in the webview after launch, to drive
            // the UI in the simulator (e.g. `SIMCTL_CHILD_COFFI_EVAL=... xcrun simctl launch`).
            #[cfg(debug_assertions)]
            if let Ok(js) = std::env::var("COFFI_EVAL") {
                let h = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    if let Some(w) = h.get_webview_window("main") {
                        let _ = w.eval(&js);
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_settings,
            set_volume,
            list_devices,
            lastfm_login,
            rescan,
            get_home,
            get_roots,
            get_folder,
            get_collection,
            get_palette,
            toggle_favorite,
            is_favorite,
            get_lyrics,
            play_tracks,
            enqueue,
            player_cmd,
            get_status,
            get_queue,
            queue_edit,
            list_queues,
            load_queue,
            save_queue,
            delete_queue,
            navidrome_test,
            search,
            set_source,
            get_tags,
            set_tags,
            get_platform,
            is_cached,
            native_request,
            get_sleep,
            sleep_timer,
            set_play_mode,
            remote_devices,
            remote_state,
            remote_connect,
            remote_disconnect,
            remote_call,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Coff-fi");
}
