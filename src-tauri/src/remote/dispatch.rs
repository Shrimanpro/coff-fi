//! Commands another device may run on this one: the library, the queue and playback. Settings,
//! tag editing, scanning and the library source stay local.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;

/// This device's name and play mode, for controllers.
pub fn info(app: &AppHandle) -> Value {
    let st = app.state::<AppState>();
    let s = st.settings.read();
    json!({
        "id": s.remote.id,
        "name": super::my_name(app),
        "platform": crate::get_platform(),
        "volume": s.volume,
        "shuffle": s.shuffle,
        "repeat": s.repeat,
        "speed": s.speed,
    })
}

fn arg<T: DeserializeOwned>(args: &Value, key: &str) -> Result<T, String> {
    serde_json::from_value(args.get(key).cloned().unwrap_or(Value::Null)).map_err(|e| format!("bad argument {key}: {e}"))
}

fn ok<T: Serialize>(r: Result<T, String>) -> Result<Value, String> {
    r.and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
}

pub async fn call(app: &AppHandle, cmd: &str, a: Value) -> Result<Value, String> {
    let st = || app.state::<AppState>();
    match cmd {
        "get_home" => ok(crate::get_home(st()).await),
        "get_roots" => ok(crate::get_roots(st()).await),
        "get_folder" => ok(crate::get_folder(st(), arg(&a, "path")?).await),
        "get_collection" => ok(crate::get_collection(st(), arg(&a, "kind")?, arg(&a, "id")?).await),
        "search" => ok(crate::search(st(), arg(&a, "query")?).await),
        "get_palette" => ok(crate::get_palette(arg(&a, "hash")?).await),
        "get_lyrics" => ok(crate::get_lyrics(app.clone(), arg(&a, "id")?, arg::<Option<bool>>(&a, "refresh")?.unwrap_or(false)).await),
        "toggle_favorite" => {
            let id: i64 = arg(&a, "id")?;
            let fav = crate::toggle_favorite(app.clone(), st(), id)?;
            // Our own UI may be showing this song as well.
            let _ = app.emit("player://favorite", (id, fav));
            ok(Ok(fav))
        }
        "is_favorite" => ok(crate::is_favorite(st(), arg(&a, "id")?)),
        "play_tracks" => ok(crate::play_tracks(app.clone(), st(), arg(&a, "ids")?, arg(&a, "start")?, arg(&a, "name")?)),
        "enqueue" => ok(crate::enqueue(app.clone(), st(), arg(&a, "ids")?, arg(&a, "next")?)),
        "player_cmd" => ok(crate::player_cmd(st(), arg(&a, "cmd")?, arg(&a, "value")?)),
        "get_status" => ok(Ok(crate::get_status(st()))),
        "set_volume" => {
            crate::set_volume(st(), arg(&a, "volume")?)?;
            // Keep our own volume slider honest.
            let _ = app.emit("settings://changed", ());
            crate::mode_changed(app);
            ok(Ok(()))
        }
        "set_play_mode" => ok(crate::set_play_mode(app.clone(), st(), arg(&a, "shuffle")?, arg(&a, "repeat")?)),
        "get_sleep" => ok(Ok(crate::get_sleep(st()))),
        "sleep_timer" => ok(crate::sleep_timer(app.clone(), st(), arg(&a, "mode")?, arg(&a, "minutes")?)),
        "get_queue" => ok(crate::get_queue(st()).await),
        "queue_edit" => ok(crate::queue_edit(app.clone(), st(), arg(&a, "op")?, arg(&a, "a")?, arg(&a, "b")?)),
        "list_queues" => ok(crate::list_queues(st()).await),
        "load_queue" => ok(crate::load_queue(app.clone(), st(), arg(&a, "id")?)),
        "save_queue" => ok(crate::save_queue(st(), arg(&a, "name")?)),
        "delete_queue" => ok(crate::delete_queue(st(), arg(&a, "id")?)),
        "remote_info" => Ok(info(app)),
        _ => Err(format!("{cmd} can't be run from another device")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments() {
        let a = json!({ "ids": [1, 2], "next": true, "b": null });
        assert_eq!(arg::<Vec<i64>>(&a, "ids").unwrap(), vec![1, 2]);
        assert!(arg::<bool>(&a, "next").unwrap());
        assert_eq!(arg::<Option<usize>>(&a, "b").unwrap(), None);
        assert_eq!(arg::<Option<f64>>(&a, "missing").unwrap(), None);
        assert!(arg::<i64>(&a, "missing").is_err());
    }
}
