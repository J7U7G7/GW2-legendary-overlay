use std::sync::Arc;
use std::time::Duration;

use tauri::async_runtime::{JoinHandle, spawn};
use tokio::time::{MissedTickBehavior, interval};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::api::client::ApiClient;
use crate::db::repository::Db;
use crate::sync::{achievements, progress, wizardsvault};

/// How often `/v2/account/achievements` is polled. Spec §5.2: every 5 min.
const PROGRESS_INTERVAL: Duration = Duration::from_secs(300);
/// How often each WV period (daily / weekly / special) is re-pulled.
const WIZARDSVAULT_INTERVAL: Duration = Duration::from_secs(900);
/// Re-run the bulk achievement definitions sync if older than this many days.
const ACHIEVEMENTS_STALE_DAYS: i64 = 7;

#[derive(Clone)]
pub struct SyncEngine {
    client: Arc<ApiClient>,
    db: Arc<Db>,
    snapshot: Arc<progress::ProgressSnapshot>,
    token: CancellationToken,
}

impl SyncEngine {
    pub fn new(client: Arc<ApiClient>, db: Arc<Db>) -> Self {
        Self {
            client,
            db,
            snapshot: Arc::new(progress::ProgressSnapshot::new()),
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
