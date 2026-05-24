use chrono::Utc;
use rusqlite::params;
use tracing::{info, warn};

use crate::api::client::ApiClient;
use crate::api::endpoints::AchievementDetail;
use crate::db::repository::Db;
use crate::error::Result;

const PAGE_SIZE: u32 = 200;
const LAST_FULL_SYNC_KEY: &str = "achievements_last_full_sync";

/// Bulk-sync all GW2 achievement definitions into the local cache.
/// Returns the number of rows upserted.
pub async fn sync_all_definitions(client: &ApiClient, db: &Db) -> Result<usize> {
    info!("starting full achievement definition sync");
    let mut page = 0u32;
    let mut total: usize = 0;
    loop {
        let path = format!("/v2/achievements?page={page}&page_size={PAGE_SIZE}");
        let batch: Vec<AchievementDetail> = client.get_json(&path).await?;
        let n = batch.len();
        if n == 0 {
            break;
        }
        upsert_batch(db, &batch)?;
        total += n;
        info!(page, returned = n, "page upserted");
        if (n as u32) < PAGE_SIZE {
            break;
        }
        page += 1;
    }

    let now = Utc::now().to_rfc3339();
    db.set_setting(LAST_FULL_SYNC_KEY, &now)?;
    info!(total, "full achievement definition sync complete");
    Ok(total)
}

fn upsert_batch(db: &Db, batch: &[AchievementDetail]) -> Result<()> {
    db.with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO achievements
                    (id, name, description, requirement, type, flags, tiers, rewards, bits, points, icon, last_synced)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, CURRENT_TIMESTAMP)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    description = excluded.description,
                    requirement = excluded.requirement,
                    type = excluded.type,
                    flags = excluded.flags,
                    tiers = excluded.tiers,
                    rewards = excluded.rewards,
                    bits = excluded.bits,
                    points = excluded.points,
                    icon = excluded.icon,
                    last_synced = CURRENT_TIMESTAMP",
            )?;
            for a in batch {
                let total_points: i32 = a.tiers.iter().map(|t| t.points).sum();
                let flags_json = serde_json::to_string(&a.flags)?;
                let tiers_json = serde_json::to_string(&a.tiers)?;
                let rewards_json =
                    a.rewards.as_ref().map(serde_json::to_string).transpose()?;
                let bits_json = serde_json::to_string(&a.bits)?;
                stmt.execute(params![
                    a.id,
                    a.name,
                    a.description,
                    a.requirement,
                    a.kind,
                    flags_json,
                    tiers_json,
                    rewards_json,
                    bits_json,
                    total_points,
                    a.icon,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    })
}

/// Whether a full sync has ever completed.
#[allow(dead_code)]
pub fn has_full_sync(db: &Db) -> Result<bool> {
    Ok(db.get_setting(LAST_FULL_SYNC_KEY)?.is_some())
}

/// Last full-sync timestamp (RFC 3339), if any.
#[allow(dead_code)]
pub fn last_full_sync(db: &Db) -> Result<Option<String>> {
    db.get_setting(LAST_FULL_SYNC_KEY)
}

#[allow(dead_code)] // used by scheduler in step 4d
pub fn is_stale(db: &Db, max_age_days: i64) -> Result<bool> {
    let Some(ts) = db.get_setting(LAST_FULL_SYNC_KEY)? else {
        return Ok(true);
    };
    let parsed = chrono::DateTime::parse_from_rfc3339(&ts);
    match parsed {
        Ok(dt) => Ok(Utc::now().signed_duration_since(dt.with_timezone(&Utc)).num_days() >= max_age_days),
        Err(e) => {
            warn!(error = %e, "invalid last_full_sync timestamp, treating as stale");
            Ok(true)
        }
    }
}
