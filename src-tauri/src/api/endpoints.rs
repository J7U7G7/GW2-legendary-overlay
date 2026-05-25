use serde::{Deserialize, Serialize};

use crate::api::client::ApiClient;
use crate::error::{AppError, Result};

const REQUIRED_PERMISSIONS: &[&str] =
    &["account", "progression", "unlocks", "inventories", "characters", "wallet"];

#[derive(Debug, Deserialize)]
pub struct TokenInfo {
    pub id: String,
    pub name: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct AccountAchievement {
    pub id: u32,
    pub current: Option<u32>,
    pub max: Option<u32>,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub bits: Vec<u32>,
    pub repeated: Option<u32>,
    #[serde(default = "default_unlocked")]
    pub unlocked: bool,
}

fn default_unlocked() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct DailyCategories {
    pub pve: Vec<DailyEntry>,
    pub pvp: Vec<DailyEntry>,
    pub wvw: Vec<DailyEntry>,
    pub fractals: Vec<DailyEntry>,
    #[serde(default)]
    pub special: Vec<DailyEntry>,
}

#[derive(Debug, Deserialize)]
pub struct DailyEntry {
    pub id: u32,
    pub level: DailyLevel,
    #[serde(default)]
    pub required_access: Option<RequiredAccess>,
}

#[derive(Debug, Deserialize)]
pub struct DailyLevel {
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Deserialize)]
pub struct RequiredAccess {
    pub product: String,
    pub condition: String,
}

#[derive(Debug, Deserialize)]
pub struct AchievementDetail {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub requirement: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default)]
    pub tiers: Vec<AchievementTier>,
    pub rewards: Option<serde_json::Value>,
    #[serde(default)]
    pub bits: Vec<serde_json::Value>,
    #[serde(default)]
    pub point_cap: Option<i32>,
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AchievementTier {
    pub count: u32,
    pub points: i32,
}

#[derive(Debug, Deserialize)]
pub struct WizardsVaultObjective {
    pub id: u32,
    pub title: String,
    pub track: String,
    pub acclaim: u32,
    pub progress_current: u32,
    pub progress_complete: u32,
    #[serde(default)]
    pub claimed: bool,
}

#[derive(Debug, Deserialize)]
pub struct WizardsVaultPeriod {
    #[serde(default)]
    pub meta_progress_current: u32,
    #[serde(default)]
    pub meta_progress_complete: u32,
    #[serde(default)]
    pub meta_reward_claimed: bool,
    #[serde(default)]
    pub objectives: Vec<WizardsVaultObjective>,
}

impl TokenInfo {
    pub fn check_required_permissions(&self) -> Result<()> {
        let missing: Vec<&str> = REQUIRED_PERMISSIONS
            .iter()
            .copied()
            .filter(|p| !self.permissions.iter().any(|owned| owned == p))
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(AppError::MissingPermissions(missing.join(", ")))
        }
    }
}

#[allow(dead_code)] // public API consumed by sync/* in upcoming step
pub async fn get_tokeninfo(c: &ApiClient) -> Result<TokenInfo> {
    c.get_json("/v2/tokeninfo").await
}

#[allow(dead_code)]
pub async fn get_account_achievements(c: &ApiClient) -> Result<Vec<AccountAchievement>> {
    c.get_json("/v2/account/achievements").await
}

/// DEPRECATED by ArenaNet: `/v2/achievements/daily` returns 503 since the
/// Wizard's Vault rollout (March 2024). Kept here for completeness but the
/// sync engine must rely on `/v2/account/wizardsvault/*` for daily tracking.
#[allow(dead_code)]
pub async fn get_daily(c: &ApiClient) -> Result<DailyCategories> {
    c.get_json("/v2/achievements/daily").await
}

#[allow(dead_code)]
pub async fn get_daily_tomorrow(c: &ApiClient) -> Result<DailyCategories> {
    c.get_json("/v2/achievements/daily/tomorrow").await
}

#[allow(dead_code)]
pub async fn get_achievements_batch(c: &ApiClient, ids: &[u32]) -> Result<Vec<AchievementDetail>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let ids_csv = ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    c.get_json(&format!("/v2/achievements?ids={ids_csv}")).await
}

#[derive(Debug, Deserialize)]
pub struct ItemDetail {
    pub id: u32,
    pub name: String,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub rarity: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[allow(dead_code)]
pub async fn get_items_batch(c: &ApiClient, ids: &[u32]) -> Result<Vec<ItemDetail>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let ids_csv = ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    // lang=fr because the user plays the French client and expects to search
    // 'bouclier' / 'élevé' / etc. To make this configurable, plumb a setting
    // through and parameterise here.
    c.get_json(&format!("/v2/items?ids={ids_csv}&lang=fr")).await
}

#[derive(Debug, Deserialize)]
pub struct SkinDetail {
    pub id: u32,
    pub name: String,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub rarity: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[allow(dead_code)]
pub async fn get_skins_batch(c: &ApiClient, ids: &[u32]) -> Result<Vec<SkinDetail>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let ids_csv = ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    // FR for consistency with /v2/items.
    c.get_json(&format!("/v2/skins?ids={ids_csv}&lang=fr")).await
}

#[derive(Debug, Deserialize)]
pub struct InventorySlot {
    pub id: u32,
    pub count: u32,
}

#[derive(Debug, Deserialize)]
pub struct MaterialStack {
    pub id: u32,
    pub count: u32,
    #[serde(default)]
    pub category: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CharacterBag {
    #[serde(default)]
    pub inventory: Vec<Option<InventorySlot>>,
}

#[derive(Debug, Deserialize)]
pub struct EquipmentSlot {
    pub id: u32,
    pub slot: String,
}

#[derive(Debug, Deserialize)]
pub struct Character {
    pub name: String,
    #[serde(default)]
    pub bags: Vec<Option<CharacterBag>>,
    #[serde(default)]
    pub equipment: Vec<EquipmentSlot>,
}

#[allow(dead_code)]
pub async fn get_account_bank(c: &ApiClient) -> Result<Vec<Option<InventorySlot>>> {
    c.get_json("/v2/account/bank").await
}

#[allow(dead_code)]
pub async fn get_account_materials(c: &ApiClient) -> Result<Vec<MaterialStack>> {
    c.get_json("/v2/account/materials").await
}

#[allow(dead_code)]
pub async fn get_shared_inventory(c: &ApiClient) -> Result<Vec<Option<InventorySlot>>> {
    c.get_json("/v2/account/inventory").await
}

#[allow(dead_code)]
pub async fn get_characters_all(c: &ApiClient) -> Result<Vec<Character>> {
    c.get_json("/v2/characters?ids=all").await
}

#[derive(Debug, Deserialize)]
pub struct WalletEntry {
    pub id: u32,
    pub value: i64,
}

#[derive(Debug, Deserialize)]
pub struct CurrencyDetail {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub order: i32,
}

#[allow(dead_code)]
pub async fn get_account_wallet(c: &ApiClient) -> Result<Vec<WalletEntry>> {
    c.get_json("/v2/account/wallet").await
}

#[allow(dead_code)]
pub async fn get_currencies_batch(c: &ApiClient, ids: &[u32]) -> Result<Vec<CurrencyDetail>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let ids_csv = ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    c.get_json(&format!("/v2/currencies?ids={ids_csv}&lang=fr")).await
}

#[allow(dead_code)]
pub async fn get_wizardsvault_daily(c: &ApiClient) -> Result<WizardsVaultPeriod> {
    c.get_json("/v2/account/wizardsvault/daily").await
}

#[allow(dead_code)]
pub async fn get_wizardsvault_weekly(c: &ApiClient) -> Result<WizardsVaultPeriod> {
    c.get_json("/v2/account/wizardsvault/weekly").await
}

#[allow(dead_code)]
pub async fn get_wizardsvault_special(c: &ApiClient) -> Result<WizardsVaultPeriod> {
    c.get_json("/v2/account/wizardsvault/special").await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_check_passes_when_all_present() {
        let info = TokenInfo {
            id: "x".into(),
            name: "x".into(),
            permissions: REQUIRED_PERMISSIONS.iter().map(|s| (*s).to_string()).collect(),
        };
        info.check_required_permissions().unwrap();
    }

    #[test]
    fn permission_check_reports_all_missing() {
        let info = TokenInfo {
            id: "x".into(),
            name: "x".into(),
            permissions: vec!["account".into(), "wallet".into()],
        };
        let err = info.check_required_permissions().unwrap_err();
        let msg = err.to_string();
        for p in ["progression", "unlocks", "inventories", "characters"] {
            assert!(msg.contains(p), "expected {p} in {msg}");
        }
    }
}
