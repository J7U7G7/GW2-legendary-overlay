use chrono::{DateTime, Datelike, Duration, NaiveDate, Timelike, Utc, Weekday};
use rusqlite::params;
use tracing::{debug, info};

use crate::api::client::ApiClient;
use crate::api::endpoints::WizardsVaultPeriod;
use crate::db::repository::Db;
use crate::error::Result;

pub const PERIOD_DAILY: &str = "daily";
pub const PERIOD_WEEKLY: &str = "weekly";
pub const PERIOD_SPECIAL: &str = "special";

/// Daily reset is 00:00 UTC. The period_start is today's UTC date until midnight.
pub fn daily_period_start(now: DateTime<Utc>) -> NaiveDate {
    now.date_naive()
}

/// Weekly reset is Mondays at 07:30 UTC. Before that on a Monday, we're still
/// in the previous week's period.
pub fn weekly_period_start(now: DateTime<Utc>) -> NaiveDate {
    let date = now.date_naive();
    let days_since_monday = date.weekday().num_days_from_monday() as i64;
    let mut monday = date - Duration::days(days_since_monday);
    if now.weekday() == Weekday::Mon && now.hour() < 7
        || now.weekday() == Weekday::Mon && now.hour() == 7 && now.minute() < 30
    {
        monday -= Duration::days(7);
    }
    monday
}

pub async fn sync_daily(client: &ApiClient, db: &Db) -> Result<usize> {
    let period_start = daily_period_start(Utc::now());
    sync_period(client, db, PERIOD_DAILY, period_start, "/v2/account/wizardsvault/daily").await
}

pub async fn sync_weekly(client: &ApiClient, db: &Db) -> Result<usize> {
    let period_start = weekly_period_start(Utc::now());
    sync_period(client, db, PERIOD_WEEKLY, period_start, "/v2/account/wizardsvault/weekly").await
}

/// "Special" objectives currently follow the weekly cadence per ArenaNet's
/// implementation; revisit if that ever changes. When no special event is
/// active the endpoint returns a non-WizardsVaultPeriod shape (likely an
/// empty array or null) that fails to decode. We treat that as "no special
/// period" rather than propagating the error — it's expected most of the
/// year and showed up as a recurring ERROR line in every user's log.
pub async fn sync_special(client: &ApiClient, db: &Db) -> Result<usize> {
    let period_start = weekly_period_start(Utc::now());
    match sync_period(
        client,
        db,
        PERIOD_SPECIAL,
        period_start,
        "/v2/account/wizardsvault/special",
    )
    .await
    {
        Ok(n) => Ok(n),
        Err(e) => {
            debug!(error = %e, "no active special WV period — treating as 0 objectives");
            Ok(0)
        }
    }
}

async fn sync_period(
    client: &ApiClient,
    db: &Db,
    period_type: &str,
    period_start: NaiveDate,
    path: &str,
) -> Result<usize> {
    let period: WizardsVaultPeriod = client.get_json(path).await?;
    debug!(period_type, objectives = period.objectives.len(), "fetched WV period");

    let n = period.objectives.len();
    db.with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO wizardsvault
                    (period_type, period_start, objective_id, title, track, acclaim,
                     progress_current, progress_complete, claimed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(period_type, period_start, objective_id) DO UPDATE SET
                    title = excluded.title,
                    track = excluded.track,
                    acclaim = excluded.acclaim,
                    progress_current = excluded.progress_current,
                    progress_complete = excluded.progress_complete,
                    claimed = excluded.claimed",
            )?;
            let period_start_str = period_start.format("%Y-%m-%d").to_string();
            for o in &period.objectives {
                stmt.execute(params![
                    period_type,
                    period_start_str,
                    o.id,
                    o.title,
                    o.track,
                    o.acclaim,
                    o.progress_current,
                    o.progress_complete,
                    o.claimed as i64,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    })?;

    info!(period_type, %period_start, n, "WV period persisted");
    Ok(n)
}

/// Delete rows for periods strictly older than `period_start`. Called after a
/// fresh sync of the same period type to garbage-collect stale rows.
#[allow(dead_code)]
pub fn purge_older(db: &Db, period_type: &str, period_start: NaiveDate) -> Result<usize> {
    db.with_conn(|c| {
        let n = c.execute(
            "DELETE FROM wizardsvault WHERE period_type = ?1 AND period_start < ?2",
            params![period_type, period_start.format("%Y-%m-%d").to_string()],
        )?;
        Ok(n)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    #[test]
    fn daily_period_is_today_utc() {
        let now = utc(2026, 5, 24, 12, 0);
        assert_eq!(daily_period_start(now), NaiveDate::from_ymd_opt(2026, 5, 24).unwrap());
    }

    #[test]
    fn weekly_period_on_wednesday_is_previous_monday() {
        // 2026-05-27 is a Wednesday
        let now = utc(2026, 5, 27, 12, 0);
        assert_eq!(weekly_period_start(now), NaiveDate::from_ymd_opt(2026, 5, 25).unwrap());
    }

    #[test]
    fn weekly_period_on_monday_after_reset_is_today() {
        // 2026-05-25 Monday 08:00 UTC — after 07:30 reset
        let now = utc(2026, 5, 25, 8, 0);
        assert_eq!(weekly_period_start(now), NaiveDate::from_ymd_opt(2026, 5, 25).unwrap());
    }

    #[test]
    fn weekly_period_on_monday_before_reset_is_previous_monday() {
        // 2026-05-25 Monday 07:00 UTC — before 07:30 reset
        let now = utc(2026, 5, 25, 7, 0);
        assert_eq!(weekly_period_start(now), NaiveDate::from_ymd_opt(2026, 5, 18).unwrap());
    }

    #[test]
    fn weekly_period_on_monday_exactly_at_reset_is_today() {
        // 2026-05-25 Monday 07:30:00 UTC — at reset boundary
        let now = utc(2026, 5, 25, 7, 30);
        assert_eq!(weekly_period_start(now), NaiveDate::from_ymd_opt(2026, 5, 25).unwrap());
    }

    #[test]
    fn weekly_period_on_sunday_is_previous_monday() {
        // 2026-05-24 Sunday 23:00 UTC
        let now = utc(2026, 5, 24, 23, 0);
        assert_eq!(weekly_period_start(now), NaiveDate::from_ymd_opt(2026, 5, 18).unwrap());
    }

    #[test]
    fn purge_removes_strictly_older_rows() {
        let db = Db::open_in_memory().unwrap();
        let mk = |period: &str, date: &str, obj: u32| {
            db.with_conn(|c| {
                c.execute(
                    "INSERT INTO wizardsvault (period_type, period_start, objective_id,
                        title, track, acclaim, progress_current, progress_complete, claimed)
                     VALUES (?1, ?2, ?3, '', '', 0, 0, 0, 0)",
                    params![period, date, obj],
                )?;
                Ok(())
            })
            .unwrap();
        };
        mk("daily", "2026-05-22", 1);
        mk("daily", "2026-05-23", 2);
        mk("daily", "2026-05-24", 3);
        mk("weekly", "2026-05-18", 9); // different period type, must survive

        let removed = purge_older(&db, "daily", NaiveDate::from_ymd_opt(2026, 5, 24).unwrap()).unwrap();
        assert_eq!(removed, 2);

        let count: i64 = db
            .with_conn(|c| Ok(c.query_row("SELECT COUNT(*) FROM wizardsvault", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(count, 2);
    }
}
