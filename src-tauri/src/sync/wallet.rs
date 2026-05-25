//! Wallet sync: pulls /v2/account/wallet and persists the values, then
//! resolves any currency_ids we don't yet have a definition for via
//! /v2/currencies?ids=... and caches them in the `currencies` table.
//!
//! The wallet is small (~70 currencies) so we replace it wholesale on every
//! sync. Definitions never change so we only fetch the ids that aren't
//! already cached.

use std::collections::HashSet;

use rusqlite::params;
use tracing::info;

use crate::api::client::ApiClient;
use crate::api::endpoints;
use crate::db::repository::Db;
use crate::error::Result;

pub async fn sync_wallet(client: &ApiClient, db: &Db) -> Result<usize> {
    let wallet = endpoints::get_account_wallet(client).await?;
    let total = wallet.len();

    let known_ids: HashSet<u32> = db.with_conn(|c| {
        let mut stmt = c.prepare("SELECT id FROM currencies")?;
        let ids: rusqlite::Result<Vec<u32>> = stmt
            .query_map([], |r| Ok(r.get::<_, i64>(0)? as u32))?
            .collect();
        Ok(ids?.into_iter().collect())
    })?;

    let unknown: Vec<u32> = wallet
        .iter()
        .map(|w| w.id)
        .filter(|id| !known_ids.contains(id))
        .collect();

    if !unknown.is_empty() {
        // /v2/currencies accepts batches; the full list is tiny so a single
        // call always suffices, but split into 200-id chunks to stay polite.
        for chunk in unknown.chunks(200) {
            let defs = endpoints::get_currencies_batch(client, chunk).await?;
            db.with_conn(|c| {
                let tx = c.unchecked_transaction()?;
                for d in &defs {
                    tx.execute(
                        "INSERT OR REPLACE INTO currencies
                            (id, name, description, icon, sort_order, last_synced)
                         VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)",
                        params![
                            d.id as i64,
                            d.name,
                            d.description,
                            d.icon,
                            d.order as i64,
                        ],
                    )?;
                }
                tx.commit()?;
                Ok(())
            })?;
            info!(synced = defs.len(), "currency definitions cached");
        }
    }

    db.with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        // Replace wholesale: drop the entire current snapshot then insert all
        // wallet entries returned by the API. Anything the user no longer
        // holds (shouldn't happen for currencies, but consistency matters)
        // gets removed.
        tx.execute("DELETE FROM account_currencies", [])?;
        for w in &wallet {
            tx.execute(
                "INSERT INTO account_currencies (currency_id, value, last_synced)
                 VALUES (?1, ?2, CURRENT_TIMESTAMP)",
                params![w.id as i64, w.value],
            )?;
        }
        tx.commit()?;
        Ok(())
    })?;

    info!(currencies = total, "wallet sync complete");
    Ok(total)
}

#[cfg(test)]
mod tests {
    use crate::db::repository::Db;

    #[test]
    fn empty_db_starts_with_no_known_currencies() {
        let db = Db::open_in_memory().expect("memory db");
        let count: i64 = db
            .with_conn(|c| {
                Ok(c.query_row("SELECT COUNT(*) FROM currencies", [], |r| r.get(0))?)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn currency_round_trip() {
        let db = Db::open_in_memory().expect("memory db");
        db.with_conn(|c| {
            c.execute(
                "INSERT INTO currencies (id, name, description, icon, sort_order) \
                 VALUES (1, 'Coin', 'gold', 'icon.png', 0)",
                [],
            )?;
            c.execute(
                "INSERT INTO account_currencies (currency_id, value) VALUES (1, 1234)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let (name, value): (String, i64) = db
            .with_conn(|c| {
                Ok(c.query_row(
                    "SELECT c.name, ac.value FROM currencies c \
                     JOIN account_currencies ac ON ac.currency_id = c.id WHERE c.id = 1",
                    [],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
                )?)
            })
            .unwrap();
        assert_eq!(name, "Coin");
        assert_eq!(value, 1234);
    }
}
