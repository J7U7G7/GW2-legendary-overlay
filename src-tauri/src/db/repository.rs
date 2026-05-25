use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};

use crate::db::schema;
use crate::error::Result;

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

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

    /// Wipe every *data* table while preserving schema, `_migrations`, and
    /// `settings` (which holds the encrypted API key + user preferences).
    /// Curated catalog tables (`legendary_collections` /
    /// `legendary_collection_members`) are repopulated by the catalog loader
    /// at startup, so wiping them is safe — they refill on next launch.
    pub fn wipe_data(&self) -> Result<()> {
        const DATA_TABLES: &[&str] = &[
            "achievements",
            "account_progress",
            "daily_assignments",
            "wizardsvault",
            "achievement_metadata",
            "pinned_achievements",
            "legendary_collection_members",
            "legendary_collections",
            "pinned_bosses",
            "items_cache",
            "skins_cache",
            "account_items",
            "todos",
            "currencies",
            "account_currencies",
        ];
        self.with_conn(|c| {
            let tx = c.unchecked_transaction()?;
            for table in DATA_TABLES {
                tx.execute(&format!("DELETE FROM {table}"), [])?;
            }
            tx.commit()?;
            Ok(())
        })
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

    pub fn pin_achievement(&self, achievement_id: u32, collection_key: Option<&str>) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO pinned_achievements (achievement_id, collection_key)
                 VALUES (?1, ?2)
                 ON CONFLICT(achievement_id) DO UPDATE SET collection_key = excluded.collection_key",
                params![achievement_id, collection_key],
            )?;
            Ok(())
        })
    }

    pub fn unpin_achievement(&self, achievement_id: u32) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "DELETE FROM pinned_achievements WHERE achievement_id = ?1",
                params![achievement_id],
            )?;
            Ok(())
        })
    }

    pub fn list_pinned_ids(&self) -> Result<Vec<u32>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT achievement_id FROM pinned_achievements ORDER BY pinned_at",
            )?;
            let mapped = stmt.query_map([], |r| Ok(r.get::<_, i64>(0)? as u32))?;
            let mut out = Vec::new();
            for row in mapped {
                out.push(row?);
            }
            Ok(out)
        })
    }

    pub fn pin_boss(&self, boss_id: &str) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT OR IGNORE INTO pinned_bosses (boss_id) VALUES (?1)",
                params![boss_id],
            )?;
            Ok(())
        })
    }

    pub fn unpin_boss(&self, boss_id: &str) -> Result<()> {
        self.with_conn(|c| {
            c.execute("DELETE FROM pinned_bosses WHERE boss_id = ?1", params![boss_id])?;
            Ok(())
        })
    }

    pub fn list_pinned_boss_ids(&self) -> Result<Vec<String>> {
        self.with_conn(|c| {
            let mut stmt =
                c.prepare("SELECT boss_id FROM pinned_bosses ORDER BY pinned_at")?;
            let mapped = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut out = Vec::new();
            for row in mapped {
                out.push(row?);
            }
            Ok(out)
        })
    }

    /// Remove both the explicit boss pin and every pinned achievement that is
    /// associated with that boss via `achievement_metadata`. Used when the user
    /// dismisses a boss group from the Pinned tab.
    pub fn remove_boss_group(&self, boss_id: &str) -> Result<()> {
        self.with_conn(|c| {
            c.execute("DELETE FROM pinned_bosses WHERE boss_id = ?1", params![boss_id])?;
            c.execute(
                "DELETE FROM pinned_achievements
                 WHERE achievement_id IN (
                    SELECT achievement_id FROM achievement_metadata
                    WHERE associated_boss = ?1
                 )",
                params![boss_id],
            )?;
            Ok(())
        })
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

    #[test]
    fn pin_unpin_round_trip() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.list_pinned_ids().unwrap().is_empty());
        db.pin_achievement(1234, Some("aurora")).unwrap();
        db.pin_achievement(5678, None).unwrap();
        let pinned = db.list_pinned_ids().unwrap();
        assert_eq!(pinned, vec![1234, 5678]);
        db.unpin_achievement(1234).unwrap();
        assert_eq!(db.list_pinned_ids().unwrap(), vec![5678]);
    }

    #[test]
    fn remove_boss_group_cascades() {
        let db = Db::open_in_memory().unwrap();
        db.with_conn(|c| {
            c.execute(
                "INSERT INTO achievement_metadata (achievement_id, associated_boss)
                 VALUES (900, 'tequatl'), (901, 'tequatl'), (1, 'shadow_behemoth')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        db.pin_boss("tequatl").unwrap();
        db.pin_achievement(900, None).unwrap();
        db.pin_achievement(901, None).unwrap();
        db.pin_achievement(1, None).unwrap();

        db.remove_boss_group("tequatl").unwrap();

        assert!(db.list_pinned_boss_ids().unwrap().is_empty());
        // Only the unrelated shadow_behemoth achievement should survive.
        assert_eq!(db.list_pinned_ids().unwrap(), vec![1]);
    }

    #[test]
    fn boss_pin_unpin_round_trip() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.list_pinned_boss_ids().unwrap().is_empty());
        db.pin_boss("tequatl").unwrap();
        db.pin_boss("shadow_behemoth").unwrap();
        db.pin_boss("tequatl").unwrap(); // idempotent
        assert_eq!(db.list_pinned_boss_ids().unwrap(), vec!["tequatl", "shadow_behemoth"]);
        db.unpin_boss("tequatl").unwrap();
        assert_eq!(db.list_pinned_boss_ids().unwrap(), vec!["shadow_behemoth"]);
    }

    #[test]
    fn pinning_same_id_updates_collection() {
        let db = Db::open_in_memory().unwrap();
        db.pin_achievement(1234, None).unwrap();
        db.pin_achievement(1234, Some("vision")).unwrap();
        // Still one row, collection updated
        let count: i64 = db
            .with_conn(|c| {
                Ok(c.query_row("SELECT COUNT(*) FROM pinned_achievements", [], |r| r.get(0))?)
            })
            .unwrap();
        assert_eq!(count, 1);
        let collection: Option<String> = db
            .with_conn(|c| {
                Ok(c.query_row(
                    "SELECT collection_key FROM pinned_achievements WHERE achievement_id = 1234",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(collection.as_deref(), Some("vision"));
    }
}
