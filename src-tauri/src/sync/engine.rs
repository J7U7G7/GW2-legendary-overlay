use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use tauri::AppHandle;
use tauri::async_runtime::{JoinHandle, spawn};
use tauri_plugin_notification::NotificationExt;
use tokio::time::{MissedTickBehavior, interval};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::api::client::ApiClient;
use crate::db::repository::Db;
use crate::sync::{achievements, progress, wizardsvault};
use crate::timers::engine::next_spawn;
use crate::timers::schedule::Schedule;

type NotifiedSet = HashSet<(String, chrono::DateTime<Utc>)>;

/// How often `/v2/account/achievements` is polled. Spec §5.2: every 5 min.
const PROGRESS_INTERVAL: Duration = Duration::from_secs(300);
/// How often each WV period (daily / weekly / special) is re-pulled.
const WIZARDSVAULT_INTERVAL: Duration = Duration::from_secs(900);
/// Re-run the bulk achievement definitions sync if older than this many days.
const ACHIEVEMENTS_STALE_DAYS: i64 = 7;
/// Cadence for the boss-watcher tick (notifies when a pinned boss is about
/// to spawn). 30s is short enough to catch the ≤ 2 min trigger window without
/// hammering the DB.
const BOSS_WATCHER_INTERVAL: Duration = Duration::from_secs(30);
/// Fallback lead time when the user hasn't set one. Overridden by the
/// `notification_lead_minutes` setting if present.
const DEFAULT_BOSS_NOTIFY_LEAD_MINUTES: i64 = 2;

#[derive(Clone)]
pub struct SyncEngine {
    client: Arc<ApiClient>,
    db: Arc<Db>,
    schedule: Arc<Schedule>,
    app: AppHandle,
    snapshot: Arc<progress::ProgressSnapshot>,
    /// Set of (boss_id, spawn_time) pairs we've already notified for. Avoids
    /// re-firing every 30s tick during the 2-minute window.
    notified: Arc<Mutex<NotifiedSet>>,
    token: CancellationToken,
}

impl SyncEngine {
    pub fn new(
        client: Arc<ApiClient>,
        db: Arc<Db>,
        schedule: Arc<Schedule>,
        app: AppHandle,
    ) -> Self {
        Self {
            client,
            db,
            schedule,
            app,
            snapshot: Arc::new(progress::ProgressSnapshot::new()),
            notified: Arc::new(Mutex::new(HashSet::new())),
            token: CancellationToken::new(),
        }
    }

    pub fn snapshot(&self) -> Arc<progress::ProgressSnapshot> {
        Arc::clone(&self.snapshot)
    }

    /// Spawn all periodic sync tasks. Returns their handles so the caller can
    /// await graceful shutdown if it wants to; calling `shutdown()` will cause
    /// each task to exit on its next tick.
    pub fn start(&self) -> Vec<JoinHandle<()>> {
        // Hydrate snapshot from any persisted progress so the first remote
        // sync's diff is meaningful (not just "everything is new").
        if let Err(e) = self.snapshot.load_from_db(&self.db) {
            error!(error = %e, "failed to hydrate progress snapshot from db");
        } else {
            info!(rows = self.snapshot.len(), "progress snapshot hydrated");
        }

        vec![
            self.spawn_achievements_bootstrap(),
            self.spawn_progress_loop(),
            self.spawn_wizardsvault_loop(),
            self.spawn_boss_watcher_loop(),
        ]
    }

    pub fn shutdown(&self) {
        info!("sync engine shutdown requested");
        self.token.cancel();
    }

    fn spawn_achievements_bootstrap(&self) -> JoinHandle<()> {
        let client = Arc::clone(&self.client);
        let db = Arc::clone(&self.db);
        let token = self.token.clone();
        spawn(async move {
            let stale = achievements::is_stale(&db, ACHIEVEMENTS_STALE_DAYS).unwrap_or(true);
            if !stale {
                info!("achievement definitions are fresh, skipping bulk sync");
                return;
            }
            tokio::select! {
                _ = token.cancelled() => {
                    info!("achievements bootstrap cancelled before start");
                }
                res = achievements::sync_all_definitions(&client, &db) => {
                    match res {
                        Ok(n) => info!(synced = n, "achievement definitions bootstrap done"),
                        Err(e) => error!(error = %e, "achievement definitions bootstrap failed"),
                    }
                }
            }
        })
    }

    fn spawn_progress_loop(&self) -> JoinHandle<()> {
        let client = Arc::clone(&self.client);
        let db = Arc::clone(&self.db);
        let snap = Arc::clone(&self.snapshot);
        let token = self.token.clone();
        spawn(async move {
            let mut tick = interval(PROGRESS_INTERVAL);
            tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        info!("progress loop stopped");
                        return;
                    }
                    _ = tick.tick() => {
                        match progress::sync_progress(&client, &db, &snap).await {
                            Ok(changes) if !changes.is_empty() => {
                                info!(changes = changes.len(), "progress changes detected");
                            }
                            Ok(_) => {}
                            Err(e) => error!(error = %e, "progress sync failed"),
                        }
                    }
                }
            }
        })
    }

    fn spawn_boss_watcher_loop(&self) -> JoinHandle<()> {
        let db = Arc::clone(&self.db);
        let schedule = Arc::clone(&self.schedule);
        let app = self.app.clone();
        let notified = Arc::clone(&self.notified);
        let token = self.token.clone();
        spawn(async move {
            let mut tick = interval(BOSS_WATCHER_INTERVAL);
            tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        info!("boss watcher loop stopped");
                        return;
                    }
                    _ = tick.tick() => {
                        if let Err(e) = check_pinned_boss_notifications(&app, &db, &schedule, &notified) {
                            error!(error = %e, "boss watcher tick failed");
                        }
                    }
                }
            }
        })
    }

    fn spawn_wizardsvault_loop(&self) -> JoinHandle<()> {
        let client = Arc::clone(&self.client);
        let db = Arc::clone(&self.db);
        let token = self.token.clone();
        spawn(async move {
            let mut tick = interval(WIZARDSVAULT_INTERVAL);
            tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        info!("wizardsvault loop stopped");
                        return;
                    }
                    _ = tick.tick() => {
                        for (name, result) in [
                            ("daily",   wizardsvault::sync_daily(&client, &db).await),
                            ("weekly",  wizardsvault::sync_weekly(&client, &db).await),
                            ("special", wizardsvault::sync_special(&client, &db).await),
                        ] {
                            if let Err(e) = result {
                                error!(period = name, error = %e, "wizardsvault sync failed");
                            }
                        }
                    }
                }
            }
        })
    }
}

/// Check whether any pinned boss is within `BOSS_NOTIFY_LEAD_MINUTES` of its
/// next spawn and fire a Windows toast notification. De-duped per (boss_id,
/// spawn_time) pair so the 30-second tick doesn't re-fire during the window.
fn check_pinned_boss_notifications(
    app: &AppHandle,
    db: &Db,
    schedule: &Schedule,
    notified: &Mutex<NotifiedSet>,
) -> crate::error::Result<()> {
    let now = Utc::now();
    let pinned = db.list_pinned_boss_ids()?;
    if pinned.is_empty() {
        return Ok(());
    }
    let lead = db
        .get_setting("notification_lead_minutes")?
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(DEFAULT_BOSS_NOTIFY_LEAD_MINUTES);

    for boss_id in pinned {
        let Some(boss) = schedule.world_bosses.iter().find(|b| b.id == boss_id) else {
            continue;
        };
        let next = next_spawn(boss, now);
        let mins_until = (next - now).num_minutes();
        if !(0..=lead).contains(&mins_until) {
            continue;
        }
        let key = (boss_id.clone(), next);
        let mut guard = notified.lock().expect("notified mutex poisoned");
        if !guard.insert(key) {
            // Already notified for this spawn — skip.
            continue;
        }
        // Garbage-collect entries whose spawn is in the past.
        guard.retain(|(_, t)| *t > now - chrono::Duration::hours(1));
        drop(guard);

        let title = format!("GW2: {} spawning soon", boss.name);
        let body = if mins_until <= 0 {
            format!("now at {}", boss.map)
        } else {
            format!("in {}m at {}", mins_until, boss.map)
        };
        if let Err(e) = app.notification().builder().title(&title).body(&body).show() {
            warn!(error = %e, boss = %boss.id, "notification failed");
        } else {
            info!(boss = %boss.id, mins_until, "sent boss notification");
        }
    }
    Ok(())
}
