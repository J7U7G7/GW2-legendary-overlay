use rusqlite::{Connection, params};
use tracing::info;

use crate::error::Result;

const MIGRATIONS: &[&str] = &[
    // v1: initial schema (spec §5.2)
    INITIAL_SCHEMA,
    // v2: pinning + legendary collections
    PIN_SCHEMA,
];

const INITIAL_SCHEMA: &str = r#"
    CREATE TABLE achievements (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        requirement TEXT,
        type TEXT,
        flags TEXT,
        tiers TEXT,
        rewards TEXT,
        bits TEXT,
        points INTEGER,
        icon TEXT,
        last_synced TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE account_progress (
        achievement_id INTEGER PRIMARY KEY,
        current INTEGER,
        max INTEGER,
        done INTEGER NOT NULL DEFAULT 0,
        bits TEXT,
        repeated INTEGER,
        unlocked INTEGER NOT NULL DEFAULT 1,
        last_updated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE daily_assignments (
        date TEXT NOT NULL,
        category TEXT NOT NULL,
        achievement_id INTEGER NOT NULL,
        level_min INTEGER,
        level_max INTEGER,
        required_access TEXT,
        PRIMARY KEY (date, category, achievement_id)
    );

    CREATE TABLE wizardsvault (
        period_type TEXT NOT NULL,
        period_start TEXT NOT NULL,
        objective_id INTEGER NOT NULL,
        title TEXT,
        track TEXT,
        acclaim INTEGER,
        progress_current INTEGER,
        progress_complete INTEGER,
        claimed INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (period_type, period_start, objective_id)
    );

    CREATE TABLE settings (
        key TEXT PRIMARY KEY,
        value TEXT
    );

    CREATE TABLE achievement_metadata (
        achievement_id INTEGER PRIMARY KEY,
        associated_map TEXT,
        associated_boss TEXT,
        associated_meta TEXT,
        estimated_time_minutes INTEGER,
        tags TEXT
    );

    CREATE INDEX idx_daily_assignments_date ON daily_assignments(date);
    CREATE INDEX idx_account_progress_done ON account_progress(done);
    CREATE INDEX idx_wizardsvault_period ON wizardsvault(period_type, period_start);
"#;

const PIN_SCHEMA: &str = r#"
    CREATE TABLE pinned_achievements (
        achievement_id INTEGER PRIMARY KEY,
        collection_key TEXT,
        pinned_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE legendary_collections (
        key TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        generation TEXT NOT NULL,
        kind TEXT NOT NULL,
        sort_order INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE legendary_collection_members (
        collection_key TEXT NOT NULL,
        achievement_id INTEGER NOT NULL,
        step INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (collection_key, achievement_id),
        FOREIGN KEY (collection_key) REFERENCES legendary_collections(key) ON DELETE CASCADE
    );

    CREATE INDEX idx_legendary_members_collection ON legendary_collection_members(collection_key);
    CREATE INDEX idx_pinned_collection ON pinned_achievements(collection_key);
"#;

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    let current: i64 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM _migrations", [], |r| r.get(0))?;

    for (idx, sql) in MIGRATIONS.iter().enumerate() {
        let version = (idx + 1) as i64;
        if version <= current {
            continue;
        }
        info!(version, "applying migration");
        conn.execute_batch(sql)?;
        conn.execute("INSERT INTO _migrations (version) VALUES (?1)", params![version])?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    #[test]
    fn fresh_migration_creates_all_tables() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let t = tables(&conn);
        for expected in [
            "_migrations",
            "account_progress",
            "achievement_metadata",
            "achievements",
            "daily_assignments",
            "legendary_collection_members",
            "legendary_collections",
            "pinned_achievements",
            "settings",
            "wizardsvault",
        ] {
            assert!(t.contains(&expected.to_string()), "missing table {expected}; got {t:?}");
        }
    }

    #[test]
    fn migration_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, MIGRATIONS.len() as i64);
    }
}
