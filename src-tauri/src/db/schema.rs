use rusqlite::{Connection, params};
use tracing::info;

use crate::error::Result;

const MIGRATIONS: &[&str] = &[
    // v1: initial schema (spec §5.2)
    INITIAL_SCHEMA,
    // v2: pinning + legendary collections
    PIN_SCHEMA,
    // v3: world boss pinning (separate from achievement pinning)
    PIN_BOSS_SCHEMA,
    // v4: items cache (resolves Item-typed bits to human-readable names)
    ITEMS_CACHE_SCHEMA,
    // v5: account-wide item inventory (bank, materials, characters, shared)
    ACCOUNT_ITEMS_SCHEMA,
    // v6: custom daily/weekly todos
    TODOS_SCHEMA,
    // v7: wallet currencies (account values + cached definitions)
    WALLET_SCHEMA,
    // v8: skins cache (resolves Skin-typed bits — e.g. Obsidian Armor steps)
    SKINS_CACHE_SCHEMA,
    // v9: EN names for items + skins so wiki links can deep-link to the
    //     canonical English page instead of broken FR-name searches
    BILINGUAL_NAMES_SCHEMA,
    // v10: switch the whole UI to English. Wipe the four tables that held
    // FR-localized data so the next sync re-fetches in EN, and drop the
    // now-redundant `name_en` columns from v9.
    FULL_ENGLISH_SCHEMA,
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

const PIN_BOSS_SCHEMA: &str = r#"
    CREATE TABLE pinned_bosses (
        boss_id TEXT PRIMARY KEY,
        pinned_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
"#;

const ITEMS_CACHE_SCHEMA: &str = r#"
    CREATE TABLE items_cache (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        type TEXT,
        rarity TEXT,
        icon TEXT,
        description TEXT,
        last_synced TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
"#;

const ACCOUNT_ITEMS_SCHEMA: &str = r#"
    CREATE TABLE account_items (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        item_id INTEGER NOT NULL,
        location TEXT NOT NULL,
        location_detail TEXT,
        count INTEGER NOT NULL,
        last_synced TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
    CREATE INDEX idx_account_items_item ON account_items(item_id);
    CREATE INDEX idx_account_items_location ON account_items(location);
"#;

const TODOS_SCHEMA: &str = r#"
    CREATE TABLE todos (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        text TEXT NOT NULL,
        period TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        completed_at TEXT,
        period_start TEXT NOT NULL
    );
    CREATE INDEX idx_todos_period ON todos(period);
"#;

const SKINS_CACHE_SCHEMA: &str = r#"
    CREATE TABLE skins_cache (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        type TEXT,
        rarity TEXT,
        description TEXT,
        icon TEXT,
        last_synced TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
"#;

const BILINGUAL_NAMES_SCHEMA: &str = r#"
    ALTER TABLE items_cache ADD COLUMN name_en TEXT;
    ALTER TABLE skins_cache ADD COLUMN name_en TEXT;
"#;

const FULL_ENGLISH_SCHEMA: &str = r#"
    DELETE FROM items_cache;
    DELETE FROM skins_cache;
    DELETE FROM achievements;
    DELETE FROM wizardsvault;
    ALTER TABLE items_cache DROP COLUMN name_en;
    ALTER TABLE skins_cache DROP COLUMN name_en;
"#;

const WALLET_SCHEMA: &str = r#"
    CREATE TABLE currencies (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        icon TEXT,
        sort_order INTEGER NOT NULL DEFAULT 0,
        last_synced TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE account_currencies (
        currency_id INTEGER PRIMARY KEY,
        value INTEGER NOT NULL DEFAULT 0,
        last_synced TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
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
            "account_currencies",
            "account_items",
            "account_progress",
            "achievement_metadata",
            "achievements",
            "currencies",
            "daily_assignments",
            "items_cache",
            "legendary_collection_members",
            "legendary_collections",
            "pinned_achievements",
            "pinned_bosses",
            "settings",
            "skins_cache",
            "todos",
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
