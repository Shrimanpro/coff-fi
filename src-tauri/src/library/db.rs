use std::path::Path;

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = OFF;
CREATE TABLE IF NOT EXISTS settings(k TEXT PRIMARY KEY, v TEXT NOT NULL);
";

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

pub fn get_setting(conn: &Connection, k: &str) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT v FROM settings WHERE k = ?1", [k], |r| r.get(0)).optional()?)
}

pub fn set_setting(conn: &Connection, k: &str, v: &str) -> Result<()> {
    conn.execute("INSERT INTO settings(k, v) VALUES(?1, ?2) ON CONFLICT(k) DO UPDATE SET v = excluded.v", params![k, v])?;
    Ok(())
}
