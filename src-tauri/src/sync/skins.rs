//! Skin resolution. Mirror of `sync::items` but for Skin-typed bits.
//!
//! Many legendary armor collections (Obsidian Armor, Astral Armor, Envoy)
//! reference armor *skins* rather than concrete items in their achievement
//! bits. The `/v2/items` endpoint does not resolve skin ids — they live in
//! a separate registry behind `/v2/skins`. Without this cache the UI shows
//! 'Skin #12078' instead of the localized armor piece name.

use rusqlite::params;
use tracing::{debug, info};

use crate::api::client::ApiClient;
use crate::api::endpoints::{SkinDetail, get_skins_batch, get_skins_batch_en};
use crate::db::repository::Db;
use crate::error::Result;

const BATCH_SIZE: usize = 200;

/// Collect every Skin-typed bit id referenced by a pinned achievement that
/// isn't already cached. Symmetric to `items::find_missing_item_ids`.
pub fn find_missing_skin_ids(db: &Db) -> Result<Vec<u32>> {
    db.with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT a.bits FROM pinned_achievements pin
             JOIN achievements a ON a.id = pin.achievement_id
             WHERE a.bits IS NOT NULL",
        )?;
        let mut rows = stmt.query([])?;
        let mut all_ids: std::collections::HashSet<u32> = std::collections::HashSet::new();
        while let Some(row) = rows.next()? {
            let bits_json: String = row.get(0)?;
            if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&bits_json) {
                for bit in arr {
                    let kind = bit.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if kind == "Skin" {
                        if let Some(id) = bit.get("id").and_then(|v| v.as_u64()) {
                            all_ids.insert(id as u32);
                        }
                    }
                }
            }
        }
        if all_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders =
            std::iter::repeat_n("?", all_ids.len()).collect::<Vec<_>>().join(",");
        let sql = format!("SELECT id FROM skins_cache WHERE id IN ({placeholders})");
        let mut stmt = c.prepare(&sql)?;
        let params: Vec<rusqlite::types::Value> = all_ids
            .iter()
            .map(|id| rusqlite::types::Value::Integer(*id as i64))
            .collect();
        let cached: std::collections::HashSet<u32> = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |r| {
                Ok(r.get::<_, i64>(0)? as u32)
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(all_ids.difference(&cached).copied().collect())
    })
}

pub async fn fetch_and_cache_skins(client: &ApiClient, db: &Db, ids: &[u32]) -> Result<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    info!(count = ids.len(), "fetching skins into cache (fr + en)");
    let mut total = 0usize;
    for chunk in ids.chunks(BATCH_SIZE) {
        let (skins_fr, skins_en) = tokio::join!(
            get_skins_batch(client, chunk),
            get_skins_batch_en(client, chunk),
        );
        let skins_fr = skins_fr?;
        let skins_en = skins_en?;
        debug!(returned_fr = skins_fr.len(), returned_en = skins_en.len(), "skin batch fetched");
        let en_by_id: std::collections::HashMap<u32, String> = skins_en
            .into_iter()
            .map(|s| (s.id, s.name))
            .collect();
        upsert_skins(db, &skins_fr, &en_by_id)?;
        total += skins_fr.len();
    }
    Ok(total)
}

fn upsert_skins(
    db: &Db,
    skins: &[SkinDetail],
    en_by_id: &std::collections::HashMap<u32, String>,
) -> Result<()> {
    db.with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO skins_cache (id, name, name_en, type, rarity, icon, description, last_synced)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    name_en = excluded.name_en,
                    type = excluded.type,
                    rarity = excluded.rarity,
                    icon = excluded.icon,
                    description = excluded.description,
                    last_synced = CURRENT_TIMESTAMP",
            )?;
            for skin in skins {
                let name_en = en_by_id.get(&skin.id).cloned();
                stmt.execute(params![
                    skin.id,
                    skin.name,
                    name_en,
                    skin.kind,
                    skin.rarity,
                    skin.icon,
                    skin.description,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    })
}

#[derive(Debug, Clone)]
pub struct CachedSkin {
    pub name: String,
    pub name_en: Option<String>,
    pub rarity: Option<String>,
    pub description: Option<String>,
}

pub fn lookup_skins(
    db: &Db,
    ids: &[u32],
) -> Result<std::collections::HashMap<u32, CachedSkin>> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    db.with_conn(|c| {
        let placeholders = std::iter::repeat_n("?", ids.len()).collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, name, name_en, rarity, description FROM skins_cache WHERE id IN ({placeholders})"
        );
        let mut stmt = c.prepare(&sql)?;
        let params: Vec<rusqlite::types::Value> = ids
            .iter()
            .map(|id| rusqlite::types::Value::Integer(*id as i64))
            .collect();
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |r| {
                Ok((
                    r.get::<_, i64>(0)? as u32,
                    CachedSkin {
                        name: r.get(1)?,
                        name_en: r.get(2)?,
                        rarity: r.get(3)?,
                        description: r.get(4)?,
                    },
                ))
            })?
            .filter_map(|r| r.ok());
        Ok(rows.collect())
    })
}
