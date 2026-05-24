use std::collections::HashMap;
use std::sync::Mutex;

use rusqlite::params;
use serde::Serialize;
use tracing::{debug, info};

use crate::api::client::ApiClient;
use crate::api::endpoints::{self, AccountAchievement};
use crate::db::repository::Db;
use crate::error::Result;

/// In-memory snapshot of the last-seen `/v2/account/achievements` payload.
/// Rebuilt at startup from the `account_progress` table; updated on every sync.
#[derive(Default)]
pub struct ProgressSnapshot {
    inner: Mutex<HashMap<u32, AccountAchievement>>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProgressChange {
    NewlyDone { id: u32 },
    Progressed { id: u32, prev: Option<u32>, current: Option<u32> },
    NewlyUnlocked { id: u32 },
}

impl ProgressSnapshot {
    pub fn new() -> Self {
        Self::default()
    }

    /// Hydrate the snapshot from rows already persisted to `account_progress`.
    pub fn load_from_db(&self, db: &Db) -> Result<()> {
        let rows: Vec<AccountAchievement> = db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT achievement_id, current, max, done, bits, repeated, unlocked
                 FROM account_progress",
            )?;
            let mapped = stmt.query_map([], |r| {
                let bits_json: Option<String> = r.get(4)?;
                let bits: Vec<u32> = bits_json
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();
                Ok(AccountAchievement {
                    id: r.get::<_, i64>(0)? as u32,
                    current: r.get::<_, Option<i64>>(1)?.map(|v| v as u32),
                    max: r.get::<_, Option<i64>>(2)?.map(|v| v as u32),
                    done: r.get::<_, i64>(3)? != 0,
                    bits,
                    repeated: r.get::<_, Option<i64>>(5)?.map(|v| v as u32),
                    unlocked: r.get::<_, i64>(6)? != 0,
                })
            })?;
            let mut out = Vec::new();
            for row in mapped {
                out.push(row?);
            }
            Ok(out)
        })?;

        let mut guard = self.inner.lock().expect("snapshot mutex poisoned");
        guard.clear();
        for a in rows {
            guard.insert(a.id, a);
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("snapshot mutex poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Pull `/v2/account/achievements`, persist to DB, diff against the snapshot,
/// and return the per-achievement changes since the previous sync.
pub async fn sync_progress(
    client: &ApiClient,
    db: &Db,
    snapshot: &ProgressSnapshot,
) -> Result<Vec<ProgressChange>> {
    let fresh = endpoints::get_account_achievements(client).await?;
    debug!(rows = fresh.len(), "fetched account progress");

    let changes = {
        let prev = snapshot.inner.lock().expect("snapshot mutex poisoned");
        diff(&prev, &fresh)
    };

    persist(db, &fresh)?;

    {
        let mut guard = snapshot.inner.lock().expect("snapshot mutex poisoned");
        guard.clear();
        for a in fresh {
            guard.insert(a.id, a);
        }
    }

    info!(changes = changes.len(), "progress sync complete");
    Ok(changes)
}

fn diff(prev: &HashMap<u32, AccountAchievement>, fresh: &[AccountAchievement]) -> Vec<ProgressChange> {
    let mut out = Vec::new();
    for a in fresh {
        match prev.get(&a.id) {
            None => {
                if a.done {
                    out.push(ProgressChange::NewlyDone { id: a.id });
                } else if a.unlocked {
                    out.push(ProgressChange::NewlyUnlocked { id: a.id });
                }
            }
            Some(p) => {
                if !p.done && a.done {
                    out.push(ProgressChange::NewlyDone { id: a.id });
                } else if p.current != a.current {
                    out.push(ProgressChange::Progressed {
                        id: a.id,
                        prev: p.current,
                        current: a.current,
                    });
                } else if !p.unlocked && a.unlocked {
                    out.push(ProgressChange::NewlyUnlocked { id: a.id });
                }
            }
        }
    }
    out
}

fn persist(db: &Db, rows: &[AccountAchievement]) -> Result<()> {
    db.with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO account_progress
                    (achievement_id, current, max, done, bits, repeated, unlocked, last_updated)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP)
                 ON CONFLICT(achievement_id) DO UPDATE SET
                    current = excluded.current,
                    max = excluded.max,
                    done = excluded.done,
                    bits = excluded.bits,
                    repeated = excluded.repeated,
                    unlocked = excluded.unlocked,
                    last_updated = CURRENT_TIMESTAMP",
            )?;
            for a in rows {
                let bits_json = serde_json::to_string(&a.bits)?;
                stmt.execute(params![
                    a.id,
                    a.current,
                    a.max,
                    a.done as i64,
                    bits_json,
                    a.repeated,
                    a.unlocked as i64,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aa(id: u32, current: Option<u32>, done: bool, unlocked: bool) -> AccountAchievement {
        AccountAchievement {
            id,
            current,
            max: Some(10),
            done,
            bits: vec![],
            repeated: None,
            unlocked,
        }
    }

    #[test]
    fn diff_detects_newly_done() {
        let prev: HashMap<u32, AccountAchievement> =
            [(1, aa(1, Some(8), false, true))].into_iter().collect();
        let fresh = vec![aa(1, Some(10), true, true)];
        assert_eq!(diff(&prev, &fresh), vec![ProgressChange::NewlyDone { id: 1 }]);
    }

    #[test]
    fn diff_detects_progress() {
        let prev: HashMap<u32, AccountAchievement> =
            [(1, aa(1, Some(2), false, true))].into_iter().collect();
        let fresh = vec![aa(1, Some(5), false, true)];
        assert_eq!(
            diff(&prev, &fresh),
            vec![ProgressChange::Progressed { id: 1, prev: Some(2), current: Some(5) }]
        );
    }

    #[test]
    fn diff_detects_first_seen_done() {
        let prev: HashMap<u32, AccountAchievement> = HashMap::new();
        let fresh = vec![aa(42, Some(10), true, true)];
        assert_eq!(diff(&prev, &fresh), vec![ProgressChange::NewlyDone { id: 42 }]);
    }

    #[test]
    fn diff_detects_first_seen_unlock_when_not_done() {
        let prev: HashMap<u32, AccountAchievement> = HashMap::new();
        let fresh = vec![aa(42, Some(1), false, true)];
        assert_eq!(diff(&prev, &fresh), vec![ProgressChange::NewlyUnlocked { id: 42 }]);
    }

    #[test]
    fn diff_ignores_unchanged() {
        let prev: HashMap<u32, AccountAchievement> =
            [(1, aa(1, Some(5), false, true))].into_iter().collect();
        let fresh = vec![aa(1, Some(5), false, true)];
        assert!(diff(&prev, &fresh).is_empty());
    }

    #[test]
    fn diff_done_takes_priority_over_progress() {
        let prev: HashMap<u32, AccountAchievement> =
            [(1, aa(1, Some(5), false, true))].into_iter().collect();
        let fresh = vec![aa(1, Some(10), true, true)];
        assert_eq!(diff(&prev, &fresh), vec![ProgressChange::NewlyDone { id: 1 }]);
    }

    #[test]
    fn persist_and_reload_round_trip() {
        let db = Db::open_in_memory().unwrap();
        let rows = vec![
            aa(1, Some(3), false, true),
            aa(2, Some(10), true, true),
            aa(99, None, false, true),
        ];
        persist(&db, &rows).unwrap();

        let snap = ProgressSnapshot::new();
        snap.load_from_db(&db).unwrap();
        assert_eq!(snap.len(), 3);
        let inner = snap.inner.lock().unwrap();
        assert_eq!(inner[&1].current, Some(3));
        assert!(inner[&2].done);
        assert_eq!(inner[&99].current, None);
    }
}
