//! Integration tests hitting the real GW2 API.
//!
//! Run with:
//!   $env:GW2_API_KEY = "..."; cargo test --test api_integration -- --ignored --nocapture
//!
//! Marked #[ignore] so they don't block the default `cargo test` run.

use gw2_overlay_lib::api::auth::ApiKey;
use gw2_overlay_lib::api::client::ApiClient;
use gw2_overlay_lib::api::endpoints;
use gw2_overlay_lib::error::AppError;

fn client_from_env() -> ApiClient {
    let raw = std::env::var("GW2_API_KEY")
        .expect("set GW2_API_KEY env var to run integration tests");
    let key = ApiKey::parse(&raw).expect("invalid GW2_API_KEY format");
    ApiClient::new(Some(key)).expect("client builder")
}

#[tokio::test]
#[ignore = "hits real GW2 API; needs GW2_API_KEY"]
async fn tokeninfo_returns_required_permissions() {
    let c = client_from_env();
    let info = endpoints::get_tokeninfo(&c).await.expect("tokeninfo");
    println!("permissions: {:?}", info.permissions);
    info.check_required_permissions().expect("missing permissions");
}

#[tokio::test]
#[ignore = "hits real GW2 API; needs GW2_API_KEY"]
async fn account_achievements_parse() {
    let c = client_from_env();
    let list = endpoints::get_account_achievements(&c).await.expect("account/achievements");
    assert!(!list.is_empty(), "account should have at least some achievement progress");
    println!("got {} account achievement rows", list.len());
}

#[tokio::test]
#[ignore = "hits real GW2 API; needs GW2_API_KEY"]
async fn batch_achievement_details() {
    let c = client_from_env();
    // Static known IDs: Centaur Slayer / Teamwork Gets It Done / Skritt Slayer.
    let ids = [1u32, 2, 3];
    let details = endpoints::get_achievements_batch(&c, &ids).await.expect("batch");
    assert_eq!(details.len(), ids.len());
    for d in &details {
        println!("#{} {}", d.id, d.name);
        assert!(!d.name.is_empty());
    }
}

#[tokio::test]
#[ignore = "hits real GW2 API; needs GW2_API_KEY"]
async fn wizardsvault_daily_parses() {
    let c = client_from_env();
    let wv = endpoints::get_wizardsvault_daily(&c).await.expect("wizardsvault/daily");
    println!(
        "WV daily: {}/{} meta, {} objectives",
        wv.meta_progress_current,
        wv.meta_progress_complete,
        wv.objectives.len()
    );
}

#[tokio::test]
#[ignore = "hits real GW2 API; needs GW2_API_KEY"]
async fn wizardsvault_weekly_parses() {
    let c = client_from_env();
    let wv = endpoints::get_wizardsvault_weekly(&c).await.expect("wizardsvault/weekly");
    println!(
        "WV weekly: {}/{} meta, {} objectives",
        wv.meta_progress_current,
        wv.meta_progress_complete,
        wv.objectives.len()
    );
}

/// Documents the deprecation: `/v2/achievements/daily` returns 503 since the
/// Wizard's Vault rollout. We treat that as `AppError::Unavailable(503)` after
/// retries. If ArenaNet ever restores the endpoint, this test will start
/// failing — that's a signal to revisit the deprecation comment in
/// `api/endpoints.rs::get_daily`.
#[tokio::test]
#[ignore = "hits real GW2 API; needs GW2_API_KEY"]
async fn legacy_daily_endpoint_is_unavailable() {
    let c = client_from_env();
    let res = endpoints::get_daily(&c).await;
    match res {
        Err(AppError::Unavailable(503)) => {} // expected
        other => panic!("expected Unavailable(503), got {other:?}"),
    }
}
