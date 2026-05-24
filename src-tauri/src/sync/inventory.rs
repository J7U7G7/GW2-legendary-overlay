use rusqlite::params;
use tracing::info;

use crate::api::client::ApiClient;
use crate::api::endpoints::{
    Character, get_account_bank, get_account_materials, get_characters_all,
    get_shared_inventory,
};
use crate::db::repository::Db;
use crate::error::Result;

/// Pull every item the user owns across bank, material storage, shared
/// inventory, and every character's bags. Wipes `account_items` and
/// re-inserts so deletes/transfers in-game are reflected. Returns the
/// total number of (item, location) rows written.
pub async fn sync_account_items(client: &ApiClient, db: &Db) -> Result<usize> {
    info!("starting account-items sync");
    let bank = get_account_bank(client).await?;
    let materials = get_account_materials(client).await?;
    let shared = get_shared_inventory(client).await?;
    let chars = get_characters_all(client).await?;

    let total = db.with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        tx.execute("DELETE FROM account_items", [])?;
        let mut inserted = 0usize;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO account_items (item_id, location, location_detail, count)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;

            for (idx, slot) in bank.iter().enumerate() {
                if let Some(s) = slot {
                    if s.count > 0 {
                        let detail = format!("slot {}", idx + 1);
                        stmt.execute(params![s.id, "bank", detail, s.count])?;
                        inserted += 1;
                    }
                }
            }

            for m in &materials {
                if m.count > 0 {
                    let detail = m
                        .category
                        .and_then(material_category_name)
                        .map(|s| s.to_string())
                        .or_else(|| m.category.map(|c| format!("category {c}")));
                    stmt.execute(params![m.id, "materials", detail, m.count])?;
                    inserted += 1;
                }
            }

            for (idx, slot) in shared.iter().enumerate() {
                if let Some(s) = slot {
                    if s.count > 0 {
                        let detail = format!("slot {}", idx + 1);
                        stmt.execute(params![s.id, "shared_inventory", detail, s.count])?;
                        inserted += 1;
                    }
                }
            }

            for ch in &chars {
                inserted += insert_character_items(&mut stmt, ch)?;
            }
        }
        tx.commit()?;
        Ok(inserted)
    })?;

    db.set_setting(
        "account_items_last_sync",
        &chrono::Utc::now().to_rfc3339(),
    )?;
    info!(total, "account-items sync complete");
    Ok(total)
}

fn insert_character_items(
    stmt: &mut rusqlite::Statement<'_>,
    ch: &Character,
) -> rusqlite::Result<usize> {
    let location = format!("character:{}", ch.name);
    let mut inserted = 0usize;
    // Bags
    for (bag_idx, bag_opt) in ch.bags.iter().enumerate() {
        let Some(bag) = bag_opt else { continue };
        for (slot_idx, slot_opt) in bag.inventory.iter().enumerate() {
            let Some(slot) = slot_opt else { continue };
            if slot.count == 0 {
                continue;
            }
            let detail = format!("bag {} slot {}", bag_idx + 1, slot_idx + 1);
            stmt.execute(params![slot.id, &location, detail, slot.count])?;
            inserted += 1;
        }
    }
    // Equipped items (a 'bouclier élevé' lives here, not in bags).
    for eq in &ch.equipment {
        let detail = format!("equipped: {}", eq.slot);
        stmt.execute(params![eq.id, &location, detail, 1u32])?;
        inserted += 1;
    }
    Ok(inserted)
}

/// Human-readable French names for the GW2 material storage categories.
/// Pulled from `/v2/materials?ids=all&lang=fr`; categories are stable enough
/// to hardcode rather than fetch every sync.
fn material_category_name(id: u32) -> Option<&'static str> {
    match id {
        5 => Some("Matériaux de cuisine"),
        6 => Some("Matériaux d'artisanat basiques"),
        29 => Some("Matériaux d'artisanat intermédiaires"),
        30 => Some("Pierres précieuses et joyaux"),
        37 => Some("Matériaux d'artisanat avancés"),
        38 => Some("Matériaux de festival"),
        46 => Some("Matériaux élevés"),
        49 => Some("Ingrédients culinaires"),
        50 => Some("Matériaux d'illustration"),
        _ => None,
    }
}

#[allow(dead_code)]
pub fn last_sync(db: &Db) -> Result<Option<String>> {
    db.get_setting("account_items_last_sync")
}

/// Collect every item id present in `account_items`. Used by the warm-cache
/// flow to make sure the items_cache table also has names for everything in
/// the player's bank/inventory, not just bits referenced by pinned
/// achievements.
pub fn distinct_item_ids(db: &Db) -> Result<Vec<u32>> {
    db.with_conn(|c| {
        let mut stmt = c.prepare("SELECT DISTINCT item_id FROM account_items")?;
        let rows = stmt.query_map([], |r| Ok(r.get::<_, i64>(0)? as u32))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
}
