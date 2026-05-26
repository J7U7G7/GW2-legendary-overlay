use rusqlite::params;
use tracing::{debug, info};

use crate::api::client::ApiClient;
use crate::api::endpoints::{ItemDetail, get_items_batch};
use crate::db::repository::Db;
use crate::error::Result;

const BATCH_SIZE: usize = 200;

/// Look at every `Item`-typed bit referenced by any pinned achievement and
/// return the ones we don't yet have in `items_cache`. Used by the warm-cache
/// command to know what to fetch from `/v2/items`.
pub fn find_missing_item_ids(db: &Db) -> Result<Vec<u32>> {
    db.with_conn(|c| {
        // Bits are stored as a JSON array per achievement. We pull every
        // pinned achievement's bits column, walk the JSON, and collect Item
        // IDs that don't have a row in items_cache yet.
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
                    if kind == "Item" {
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
        // Subtract ids already cached.
        let placeholders =
            std::iter::repeat_n("?", all_ids.len()).collect::<Vec<_>>().join(",");
        let sql = format!("SELECT id FROM items_cache WHERE id IN ({placeholders})");
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

/// Fetch `/v2/items` for the given ids (in 200-id batches) and upsert into
/// `items_cache`. Returns the number of rows written.
pub async fn fetch_and_cache_items(client: &ApiClient, db: &Db, ids: &[u32]) -> Result<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    info!(count = ids.len(), "fetching items into cache");
    let mut total = 0usize;
    for chunk in ids.chunks(BATCH_SIZE) {
        let items = get_items_batch(client, chunk).await?;
        debug!(returned = items.len(), "batch fetched");
        upsert_items(db, &items)?;
        total += items.len();
    }
    Ok(total)
}

fn upsert_items(db: &Db, items: &[ItemDetail]) -> Result<()> {
    db.with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO items_cache (id, name, type, rarity, icon, description, last_synced)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, CURRENT_TIMESTAMP)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    type = excluded.type,
                    rarity = excluded.rarity,
                    icon = excluded.icon,
                    description = excluded.description,
                    last_synced = CURRENT_TIMESTAMP",
            )?;
            for item in items {
                stmt.execute(params![
                    item.id,
                    item.name,
                    item.kind,
                    item.rarity,
                    item.icon,
                    item.description,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    })
}

#[derive(Debug, Clone)]
pub struct CachedItem {
    pub name: String,
    pub rarity: Option<String>,
    pub description: Option<String>,
}

/// Bulk-look-up cached items for a set of ids. Returns a map from id to row.
pub fn lookup_items(
    db: &Db,
    ids: &[u32],
) -> Result<std::collections::HashMap<u32, CachedItem>> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    db.with_conn(|c| {
        let placeholders = std::iter::repeat_n("?", ids.len()).collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, name, rarity, description FROM items_cache WHERE id IN ({placeholders})"
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
                    CachedItem {
                        name: r.get(1)?,
                        rarity: r.get(2)?,
                        description: r.get(3)?,
                    },
                ))
            })?
            .filter_map(|r| r.ok());
        Ok(rows.collect())
    })
}
