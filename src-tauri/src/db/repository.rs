use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};

use crate::db::schema;
use crate::error::Result;

pub struct Db {
    conn: Mutex<Connection>,
}

#[allow(dead_code)] // public infrastructure used in upcoming sync/api steps
impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;
        schema::migrate(&conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let guard = self.conn.lock().expect("db mutex poisoned");
        f(&guard)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
            Ok(())
        })
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        self.with_conn(|c| {
            let value = c
                .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| {
                    r.get::<_, Option<String>>(0)
                })
                .optional()?
                .flatten();
            Ok(value)
        })
    }

    pub fn count_achievements(&self) -> Result<i64> {
        self.with_conn(|c| Ok(c.query_row("SELECT COUNT(*) FROM achievements", [], |r| r.get(0))?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.get_setting("missing").unwrap(), None);
        db.set_setting("api_key_present", "true").unwrap();
        assert_eq!(db.get_setting("api_key_present").unwrap(), Some("true".into()));
        db.set_setting("api_key_present", "false").unwrap();
        assert_eq!(db.get_setting("api_key_present").unwrap(), Some("false".into()));
    }

    #[test]
    fn count_achievements_starts_at_zero() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.count_achievements().unwrap(), 0);
    }
}
