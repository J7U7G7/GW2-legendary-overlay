//! Static catalogs loaded from JSON at boot:
//! - `legendary_collections.json` → `legendary_collections` + `legendary_collection_members`
//! - `achievement_boss_links.json` → `achievement_metadata.associated_boss`
//!
//! Both files are embedded via `include_str!` and upserted idempotently on
//! every startup so editing the JSON and rebuilding is enough to refresh.

use rusqlite::params;
use serde::Deserialize;
use tracing::info;

use crate::db::repository::Db;
use crate::error::Result;

const LEGENDARY_JSON: &str = include_str!("../../data/legendary_collections.json");
const BOSS_LINKS_JSON: &str = include_str!("../../data/achievement_boss_links.json");

#[derive(Debug, Deserialize)]
struct LegendaryCatalog {
    collections: Vec<LegendaryCollection>,
}

#[derive(Debug, Deserialize)]
struct LegendaryCollection {
    key: String,
    name: String,
    generation: String,
    kind: String,
    sort_order: i64,
    members: Vec<LegendaryMember>,
}

#[derive(Debug, Deserialize)]
struct LegendaryMember {
    achievement_id: u32,
    #[serde(default)]
    step: i64,
}

#[derive(Debug, Deserialize)]
struct BossLinkCatalog {
    links: Vec<BossLink>,
}

#[derive(Debug, Deserialize)]
struct BossLink {
    boss_id: String,
    achievement_ids: Vec<u32>,
}

pub fn load_all(db: &Db) -> Result<()> {
    load_legendaries(db)?;
    load_boss_links(db)?;
    Ok(())
}

fn load_legendaries(db: &Db) -> Result<()> {
    let catalog: LegendaryCatalog = serde_json::from_str(LEGENDARY_JSON)?;
    let mut total_members = 0usize;

    db.with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        {
            let mut up_col = tx.prepare(
                "INSERT INTO legendary_collections (key, name, generation, kind, sort_order)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(key) DO UPDATE SET
                    name = excluded.name,
                    generation = excluded.generation,
                    kind = excluded.kind,
                    sort_order = excluded.sort_order",
            )?;
            let mut up_mem = tx.prepare(
                "INSERT INTO legendary_collection_members (collection_key, achievement_id, step)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(collection_key, achievement_id) DO UPDATE SET step = excluded.step",
            )?;
            for col in &catalog.collections {
                up_col.execute(params![
                    col.key,
                    col.name,
                    col.generation,
                    col.kind,
                    col.sort_order
                ])?;
                for m in &col.members {
                    up_mem.execute(params![col.key, m.achievement_id, m.step])?;
                    total_members += 1;
                }
            }
        }
        tx.commit()?;
        Ok(())
    })?;

    info!(
        collections = catalog.collections.len(),
        members = total_members,
        "legendary catalog loaded"
    );
    Ok(())
}

fn load_boss_links(db: &Db) -> Result<()> {
    let catalog: BossLinkCatalog = serde_json::from_str(BOSS_LINKS_JSON)?;
    let mut total = 0usize;

    db.with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        {
            let mut up = tx.prepare(
                "INSERT INTO achievement_metadata (achievement_id, associated_boss)
                 VALUES (?1, ?2)
                 ON CONFLICT(achievement_id) DO UPDATE SET
                    associated_boss = excluded.associated_boss",
            )?;
            for link in &catalog.links {
                for aid in &link.achievement_ids {
                    up.execute(params![aid, link.boss_id])?;
                    total += 1;
                }
            }
        }
        tx.commit()?;
        Ok(())
    })?;

    info!(links = catalog.links.len(), achievements = total, "boss-link catalog loaded");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legendary_catalog_loads_into_db() {
        let db = Db::open_in_memory().unwrap();
        load_legendaries(&db).unwrap();
        let count: i64 = db
            .with_conn(|c| {
                Ok(c.query_row("SELECT COUNT(*) FROM legendary_collections", [], |r| r.get(0))?)
            })
            .unwrap();
        assert!(count >= 3, "expected at least 3 curated collections, got {count}");
    }

    #[test]
    fn boss_links_catalog_loads_into_db() {
        let db = Db::open_in_memory().unwrap();
        load_boss_links(&db).unwrap();
        let teq_count: i64 = db
            .with_conn(|c| {
                Ok(c.query_row(
                    "SELECT COUNT(*) FROM achievement_metadata WHERE associated_boss = 'tequatl'",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        assert!(teq_count >= 11, "expected ≥ 11 Tequatl-linked achievements, got {teq_count}");
    }

    #[test]
    fn catalogs_are_idempotent() {
        let db = Db::open_in_memory().unwrap();
        load_all(&db).unwrap();
        load_all(&db).unwrap();
        let col_count: i64 = db
            .with_conn(|c| {
                Ok(c.query_row("SELECT COUNT(*) FROM legendary_collections", [], |r| r.get(0))?)
            })
            .unwrap();
        assert_eq!(col_count, 3);
    }
}
