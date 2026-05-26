# Full English Display Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Switch every GW2 data display from French to English by flipping three endpoints from `lang=fr` to `lang=en`, wiping the cached FR data via schema migration v10, and removing the v0.1.10 bilingual machinery (dual fetches, `name_en` columns, `resolved_name_en` field, fallback URL cascade).

**Architecture:** Single-language storage. The user accepts losing FR searchability in exchange for consistency with the EN-centric GW2 ecosystem (wiki, snowcrows, ArcDPS). Schema v10 wipes the four FR-cached tables and drops the now-redundant `name_en` columns. The bulk achievement re-sync is auto-triggered on the next boot via the existing staleness check, extended to treat `count(*) = 0` as stale.

**Tech Stack:** Tauri 2 (Rust backend + WebView2 frontend), rusqlite (SQLite 3.43+ via the `bundled` feature, supports `ALTER TABLE DROP COLUMN`), React 19 + TypeScript 5.8, Zustand store, GitHub Actions for tagged release builds.

**Spec:** [`docs/superpowers/specs/2026-05-26-full-english-display-design.md`](../specs/2026-05-26-full-english-display-design.md)

**Target tag:** `v0.1.11`

---

## File Structure

Files **modified** (no new files created):

| File | Responsibility | What changes |
|---|---|---|
| `src-tauri/src/db/schema.rs` | SQL migration ladder | Append migration v10 (wipe 4 tables + drop 2 `name_en` columns). Update `fresh_migration_creates_all_tables` test for the new column shape. |
| `src-tauri/src/sync/achievements.rs` | Bulk achievement sync + freshness | Extend `is_stale` so empty table also returns true. Add unit test for the count=0 path. |
| `src-tauri/src/api/endpoints.rs` | HTTP wrappers | `get_items_batch` / `get_skins_batch` / `get_currencies_batch` switch `lang=fr` → `lang=en`. Delete `get_items_batch_en` + `get_skins_batch_en` (v0.1.10 duals, now unused). |
| `src-tauri/src/sync/items.rs` | Items cache populate + lookup | Revert `tokio::join!` dual-fetch to single call. Drop `name_en` from `CachedItem`. SQL upsert returns to single-name shape. `lookup_items` SELECT drops the `name_en` column. |
| `src-tauri/src/sync/skins.rs` | Skins cache populate + lookup | Symmetric: revert dual-fetch, drop `name_en` from `CachedSkin`, simplify upsert + lookup. |
| `src-tauri/src/commands.rs` | IPC surface | `PinnedBitView` drops `resolved_name_en` field. `parse_bits` returns 3-tuple `(name, description, rarity)` per kind. |
| `src/types/gw2.ts` | TypeScript mirror types | Drop `resolved_name_en` from `PinnedBit`. |
| `src/components/PinnedPanel.tsx` | Renders pinned achievements + bits | Replace `wikiUrlForBit`'s 3-stage cascade with a direct page URL when `resolved_name` is set (now EN), search fallback otherwise. |
| `package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` | Version pinning | `0.1.10` → `0.1.11`. |

Files **deliberately NOT modified** despite appearing in the spec:

- `src-tauri/src/api/endpoints.rs::get_achievements_batch` — already lang-less (defaults to EN at GW2 API level).
- `src-tauri/src/api/endpoints.rs::get_wizardsvault_*` — already lang-less.
- `src-tauri/src/sync/wizardsvault.rs` — endpoint switch not needed; the table wipe in migration v10 alone is enough since the source is already EN.
- UI chrome / placeholder strings — already in English.

---

## Task 1: Migration v10 (wipe FR caches + drop name_en columns + extend is_stale)

**Files:**
- Modify: `src-tauri/src/db/schema.rs`
- Modify: `src-tauri/src/sync/achievements.rs`

### Steps

- [ ] **Step 1: Write the failing test for the count=0 path**

Append this test to the test module at the bottom of `src-tauri/src/sync/achievements.rs` (the file already has a `#[cfg(test)] mod tests { … }` block — add inside):

```rust
#[test]
fn is_stale_returns_true_when_table_empty_even_if_timestamp_fresh() {
    let db = Db::open_in_memory().unwrap();
    // Pretend a sync just happened (timestamp fresh).
    db.set_setting(LAST_FULL_SYNC_KEY, &Utc::now().to_rfc3339()).unwrap();
    // No rows in achievements → must be stale anyway.
    assert!(is_stale(&db, 7).unwrap());
}
```

You may need `use crate::db::repository::Db;` and `use chrono::Utc;` in the test module if not already present — copy from the existing test imports.

- [ ] **Step 2: Run the test to verify it FAILS**

```powershell
$env:Path += ";$env:USERPROFILE\.cargo\bin"
cargo test --manifest-path src-tauri/Cargo.toml --lib sync::achievements::tests::is_stale_returns_true_when_table_empty_even_if_timestamp_fresh
```

Expected: FAIL. The current `is_stale` only checks the timestamp, which we just set to fresh → it returns `false` despite the empty table.

- [ ] **Step 3: Extend `is_stale` to treat count=0 as stale**

Open `src-tauri/src/sync/achievements.rs` and replace the existing `is_stale` body with one that first checks the row count, then falls back to the timestamp check.

```rust
pub fn is_stale(db: &Db, max_age_days: i64) -> Result<bool> {
    // Empty cache → always stale. Without this the v0.1.11 migration that
    // wipes `achievements` would never trigger a re-fetch because the
    // `last_full_sync` timestamp in `settings` survives the wipe.
    let count: i64 = db.with_conn(|c| {
        Ok(c.query_row("SELECT COUNT(*) FROM achievements", [], |r| r.get(0))?)
    })?;
    if count == 0 {
        return Ok(true);
    }
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
```

- [ ] **Step 4: Run the test to verify it now PASSES**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib sync::achievements::tests::is_stale_returns_true_when_table_empty_even_if_timestamp_fresh
```

Expected: PASS.

- [ ] **Step 5: Append migration v10 to the MIGRATIONS array**

Open `src-tauri/src/db/schema.rs`. At the end of the `MIGRATIONS` const slice, add a new entry:

```rust
const MIGRATIONS: &[&str] = &[
    // ... existing migrations v1..v9 untouched ...
    // v9: EN names for items + skins (v0.1.10) — added alongside FR
    BILINGUAL_NAMES_SCHEMA,
    // v10: switch the whole UI to English. Wipe the four tables that held
    // FR-localized data so the next sync re-fetches in EN, and drop the
    // now-redundant `name_en` columns from v9.
    FULL_ENGLISH_SCHEMA,
];
```

(The exact prior entries are already present — only add the new `FULL_ENGLISH_SCHEMA,` line at the end of the slice.)

- [ ] **Step 6: Define the FULL_ENGLISH_SCHEMA constant**

Just after the `BILINGUAL_NAMES_SCHEMA` const definition in the same file, add:

```rust
const FULL_ENGLISH_SCHEMA: &str = r#"
    DELETE FROM items_cache;
    DELETE FROM skins_cache;
    DELETE FROM achievements;
    DELETE FROM wizardsvault;
    ALTER TABLE items_cache DROP COLUMN name_en;
    ALTER TABLE skins_cache DROP COLUMN name_en;
"#;
```

- [ ] **Step 7: Run the existing migration tests to verify v10 runs cleanly**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib db::schema::tests
```

Expected: PASS — both `fresh_migration_creates_all_tables` and `migration_is_idempotent` succeed. The fresh-migration test doesn't reference column names directly so the drop should not break it.

- [ ] **Step 8: Commit**

```powershell
cd "C:\Users\perso\Downloads\gw2 overlay\gw2-overlay"
git add src-tauri/src/db/schema.rs src-tauri/src/sync/achievements.rs
git commit -m "feat(db): schema v10 — wipe FR caches + drop name_en columns

Prepares for the v0.1.11 full-English switch:
- Wipes items_cache, skins_cache, achievements, wizardsvault (the
  four tables that held lang=fr data).
- Drops the items_cache.name_en + skins_cache.name_en columns added
  in v9 (v0.1.10) — redundant once the primary name is itself EN.

Also extends achievements::is_stale to treat an empty table as
stale even when the last_full_sync timestamp in settings is fresh.
Without this, the bulk re-sync wouldn't trigger after the wipe
because settings survives the migration.

New unit test covers the count=0 → stale path."
```

---

## Task 2: Endpoint lang switches + remove `_en` variants

**Files:**
- Modify: `src-tauri/src/api/endpoints.rs`

### Steps

- [ ] **Step 1: Flip `get_items_batch` from `lang=fr` to `lang=en`**

In `src-tauri/src/api/endpoints.rs`, find `get_items_batch` (around line 174-184). Update the URL and the surrounding comment. Replace this block:

```rust
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
```

with:

```rust
#[allow(dead_code)]
pub async fn get_items_batch(c: &ApiClient, ids: &[u32]) -> Result<Vec<ItemDetail>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let ids_csv = ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    // English everywhere — the overlay matches the wider GW2 ecosystem (wiki,
    // snowcrows, ArcDPS) rather than the user's in-game client language.
    c.get_json(&format!("/v2/items?ids={ids_csv}&lang=en")).await
}
```

- [ ] **Step 2: Delete `get_items_batch_en`**

In the same file, immediately following `get_items_batch`, delete the entire `get_items_batch_en` function block including its doc comment and `#[allow(dead_code)]` attribute. The block to remove looks like:

```rust
/// Same batch as `get_items_batch` but with `lang=en`. Used purely to
/// populate `name_en` so wiki links can deep-link to the canonical English
/// page (the FR name doesn't resolve on wiki.guildwars2.com).
#[allow(dead_code)]
pub async fn get_items_batch_en(c: &ApiClient, ids: &[u32]) -> Result<Vec<ItemDetail>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let ids_csv = ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    c.get_json(&format!("/v2/items?ids={ids_csv}&lang=en")).await
}
```

- [ ] **Step 3: Flip `get_skins_batch` from `lang=fr` to `lang=en`**

In the same file, find `get_skins_batch`. Replace the URL string `lang=fr` with `lang=en` and update the comment from `// FR for consistency with /v2/items.` to `// EN to match the rest of the overlay.`.

- [ ] **Step 4: Delete `get_skins_batch_en`**

Delete the entire `get_skins_batch_en` function block immediately below `get_skins_batch` (mirror of step 2).

- [ ] **Step 5: Flip `get_currencies_batch` from `lang=fr` to `lang=en`**

Find `get_currencies_batch` (around line 312-316). Replace `&lang=fr` with `&lang=en` in the URL.

- [ ] **Step 6: Verify the file still compiles in isolation**

```powershell
cargo check --manifest-path src-tauri/Cargo.toml --lib
```

Expected: errors only about `get_items_batch_en` / `get_skins_batch_en` being unresolved imports from `sync/items.rs` and `sync/skins.rs`. Those are the call sites we'll fix in Tasks 3-4. Do not commit yet.

---

## Task 3: Revert `sync/items.rs` to single-fetch + drop `name_en`

**Files:**
- Modify: `src-tauri/src/sync/items.rs`

### Steps

- [ ] **Step 1: Drop `get_items_batch_en` from the use statement**

At the top of `src-tauri/src/sync/items.rs`, change:

```rust
use crate::api::endpoints::{ItemDetail, get_items_batch, get_items_batch_en};
```

back to:

```rust
use crate::api::endpoints::{ItemDetail, get_items_batch};
```

- [ ] **Step 2: Replace `fetch_and_cache_items` with the single-fetch form**

Find the current `fetch_and_cache_items` function (it uses `tokio::join!` and builds an `en_by_id` map). Replace the whole function with the simpler version:

```rust
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
```

- [ ] **Step 3: Replace `upsert_items` with the single-name form**

Find `upsert_items` (it currently takes an `en_by_id: &HashMap<u32, String>` parameter). Replace with:

```rust
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
```

- [ ] **Step 4: Drop `name_en` from `CachedItem`**

Find the `CachedItem` struct definition. Remove the `pub name_en: Option<String>,` field. Result:

```rust
#[derive(Debug, Clone)]
pub struct CachedItem {
    pub name: String,
    pub rarity: Option<String>,
    pub description: Option<String>,
}
```

- [ ] **Step 5: Simplify `lookup_items` SQL + row mapper**

Find `lookup_items`. Change the SQL from `SELECT id, name, name_en, rarity, description …` to `SELECT id, name, rarity, description …`. Adjust the row mapper to read 4 columns instead of 5:

```rust
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
```

- [ ] **Step 6: Verify compile**

```powershell
cargo check --manifest-path src-tauri/Cargo.toml --lib
```

Expected: errors only remain in `sync/skins.rs` (same pattern, Task 4) and in `commands.rs` (it still references `name_en`, Task 5). Do not commit yet.

---

## Task 4: Revert `sync/skins.rs` to single-fetch + drop `name_en`

**Files:**
- Modify: `src-tauri/src/sync/skins.rs`

### Steps

- [ ] **Step 1: Drop `get_skins_batch_en` from the use statement**

Change:

```rust
use crate::api::endpoints::{SkinDetail, get_skins_batch, get_skins_batch_en};
```

to:

```rust
use crate::api::endpoints::{SkinDetail, get_skins_batch};
```

- [ ] **Step 2: Replace `fetch_and_cache_skins` with the single-fetch form**

Replace the function entirely:

```rust
pub async fn fetch_and_cache_skins(client: &ApiClient, db: &Db, ids: &[u32]) -> Result<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    info!(count = ids.len(), "fetching skins into cache");
    let mut total = 0usize;
    for chunk in ids.chunks(BATCH_SIZE) {
        let skins = get_skins_batch(client, chunk).await?;
        debug!(returned = skins.len(), "skin batch fetched");
        upsert_skins(db, &skins)?;
        total += skins.len();
    }
    Ok(total)
}
```

- [ ] **Step 3: Replace `upsert_skins` with the single-name form**

```rust
fn upsert_skins(db: &Db, skins: &[SkinDetail]) -> Result<()> {
    db.with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO skins_cache (id, name, type, rarity, icon, description, last_synced)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, CURRENT_TIMESTAMP)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    type = excluded.type,
                    rarity = excluded.rarity,
                    icon = excluded.icon,
                    description = excluded.description,
                    last_synced = CURRENT_TIMESTAMP",
            )?;
            for skin in skins {
                stmt.execute(params![
                    skin.id,
                    skin.name,
                    skin.kind,
                    skin.rarity,
                    skin.icon,
                    skin.description,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    })
}
```

- [ ] **Step 4: Drop `name_en` from `CachedSkin`**

```rust
#[derive(Debug, Clone)]
pub struct CachedSkin {
    pub name: String,
    pub rarity: Option<String>,
    pub description: Option<String>,
}
```

- [ ] **Step 5: Simplify `lookup_skins` SQL + row mapper**

Change SQL to `SELECT id, name, rarity, description FROM skins_cache WHERE id IN ({placeholders})`. Adjust mapper:

```rust
let rows = stmt
    .query_map(rusqlite::params_from_iter(params.iter()), |r| {
        Ok((
            r.get::<_, i64>(0)? as u32,
            CachedSkin {
                name: r.get(1)?,
                rarity: r.get(2)?,
                description: r.get(3)?,
            },
        ))
    })?
    .filter_map(|r| r.ok());
```

- [ ] **Step 6: Verify compile**

```powershell
cargo check --manifest-path src-tauri/Cargo.toml --lib
```

Expected: errors only remain in `commands.rs` (Task 5). Do not commit yet.

---

## Task 5: Simplify `PinnedBitView` and `parse_bits`

**Files:**
- Modify: `src-tauri/src/commands.rs`

### Steps

- [ ] **Step 1: Drop `resolved_name_en` from the `PinnedBitView` struct**

Find the struct (search for `pub struct PinnedBitView`). Remove the field + its doc comment:

```rust
/// English name of the resolved Item/Skin — used by the FE to build
/// a wiki link that actually deep-links to the canonical page. None
/// when the bit is unresolved or its cached row predates the bilingual
/// migration (v9).
pub resolved_name_en: Option<String>,
```

Resulting struct:

```rust
#[derive(Serialize, Clone)]
pub struct PinnedBitView {
    pub index: u32,
    pub kind: String,
    pub ref_id: Option<i64>,
    pub text: Option<String>,
    pub done: bool,
    pub resolved_name: Option<String>,
    pub resolved_description: Option<String>,
    pub resolved_rarity: Option<String>,
}
```

- [ ] **Step 2: Change `parse_bits` resolution to a 3-tuple**

Find the match block inside `parse_bits` that builds `(resolved_name, resolved_name_en, resolved_description, resolved_rarity)`. Replace with the 3-tuple version:

```rust
let (resolved_name, resolved_description, resolved_rarity) = match kind.as_str() {
    "Item" => ref_id
        .and_then(|id| item_cache.get(&(id as u32)))
        .map(|it| (Some(it.name.clone()), it.description.clone(), it.rarity.clone()))
        .unwrap_or((None, None, None)),
    "Skin" => ref_id
        .and_then(|id| skin_cache.get(&(id as u32)))
        .map(|sk| (Some(sk.name.clone()), sk.description.clone(), sk.rarity.clone()))
        .unwrap_or((None, None, None)),
    _ => (None, None, None),
};
```

- [ ] **Step 3: Remove `resolved_name_en` from the `PinnedBitView` constructor**

A few lines below the match, in the `PinnedBitView { … }` literal, delete the line `resolved_name_en,`. Resulting block:

```rust
PinnedBitView {
    index: idx as u32,
    kind,
    ref_id,
    text,
    done: done_set.contains(&(idx as u32)),
    resolved_name,
    resolved_description,
    resolved_rarity,
}
```

- [ ] **Step 4: Full build to verify the backend compiles end-to-end**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml --lib
```

Expected: succeeds, zero errors.

- [ ] **Step 5: Run the whole test suite**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

Expected: 67 passed (66 pre-existing + the new `is_stale_returns_true_when_table_empty_even_if_timestamp_fresh`), 0 failed.

- [ ] **Step 6: Run clippy**

```powershell
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: zero warnings, zero errors.

- [ ] **Step 7: Commit the backend half**

```powershell
git add src-tauri/src/api/endpoints.rs src-tauri/src/sync/items.rs src-tauri/src/sync/skins.rs src-tauri/src/commands.rs
git commit -m "refactor(api): all GW2 endpoints fetch lang=en, drop v0.1.10 bilingual code

- get_items_batch / get_skins_batch / get_currencies_batch flip
  lang=fr → lang=en. get_items_batch_en + get_skins_batch_en deleted.
- sync/items.rs + sync/skins.rs revert from tokio::join! dual-fetch
  to single-call. CachedItem + CachedSkin lose name_en. upsert_items
  + upsert_skins + lookup_* SQL all simplified to single-name shape.
- commands::PinnedBitView drops resolved_name_en field. parse_bits
  returns a 3-tuple (name, description, rarity) per kind.

Behaviour matches what schema v10 already prepared for: empty
items_cache + skins_cache + achievements get repopulated in EN on
next sync."
```

---

## Task 6: Frontend types + simplify wiki URL builder

**Files:**
- Modify: `src/types/gw2.ts`
- Modify: `src/components/PinnedPanel.tsx`

### Steps

- [ ] **Step 1: Drop `resolved_name_en` from `PinnedBit`**

In `src/types/gw2.ts`, find the `PinnedBit` type. Remove the `resolved_name_en` field and its doc comment. Resulting type:

```ts
export type PinnedBit = {
  index: number;
  kind: string;
  ref_id: number | null;
  text: string | null;
  done: boolean;
  resolved_name: string | null;
  resolved_description: string | null;
  resolved_rarity: string | null;
};
```

- [ ] **Step 2: Simplify `wikiUrlForBit` in PinnedPanel**

Open `src/components/PinnedPanel.tsx`. Find the top-of-file helpers `directWikiUrl`, `searchWikiUrl`, `wikiUrlForBit`. Replace `wikiUrlForBit` with the simpler 2-tier cascade (direct EN page when name resolved, search fallback for unresolved bits):

```ts
/** Pick the best wiki URL for a bit. `resolved_name` is now always EN since
 * v0.1.11 — the v0.1.10 `resolved_name_en` parallel field is gone. */
function wikiUrlForBit(bit: PinnedBit, fallbackText: string): string | null {
  if (bit.resolved_name) return directWikiUrl(bit.resolved_name);
  if (fallbackText.length > 0) return searchWikiUrl(fallbackText);
  return null;
}
```

`directWikiUrl` and `searchWikiUrl` above it are unchanged.

- [ ] **Step 3: Run the frontend build**

```powershell
cd "C:\Users\perso\Downloads\gw2 overlay\gw2-overlay"
npm run build
```

Expected: `tsc` passes with zero errors, Vite produces a bundle.

- [ ] **Step 4: Commit the frontend half**

```powershell
git add src/types/gw2.ts src/components/PinnedPanel.tsx
git commit -m "refactor(ui): drop resolved_name_en + simplify wiki URL cascade

PinnedBit no longer carries resolved_name_en — the backend's
resolved_name is itself the EN name since v0.1.11. wikiUrlForBit's
3-tier cascade collapses to 2 tiers: direct wiki page if a resolved
name exists, search fallback otherwise. The old 'EN search by FR
name' middle tier is gone."
```

---

## Task 7: Version bump + final validation + tagged release

**Files:**
- Modify: `package.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`

### Steps

- [ ] **Step 1: Bump `package.json` version**

In `package.json`, change `"version": "0.1.10"` to `"version": "0.1.11"`.

- [ ] **Step 2: Bump `src-tauri/Cargo.toml` version**

Find the `[package]` block. Change `version = "0.1.10"` to `version = "0.1.11"`.

- [ ] **Step 3: Bump `src-tauri/tauri.conf.json` version**

Change `"version": "0.1.10"` to `"version": "0.1.11"`. (Top-level field, near `productName`.)

- [ ] **Step 4: Final pre-commit ritual (backend tests + clippy + frontend build)**

```powershell
$env:Path += ";$env:USERPROFILE\.cargo\bin"
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run build
```

Expected: 67 tests pass, clippy zero warnings, Vite build succeeds.

- [ ] **Step 5: Commit the version bump + push to main**

```powershell
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "chore(release): bump to 0.1.11

Full English display ships in this version. Cumulative changes:
- All GW2 data (items, skins, achievements, WV objectives,
  currencies) now displayed in English. Search uses EN names too.
- Schema v10 wipes the four FR-cached tables on first launch and
  drops the now-redundant name_en columns from v9. Bulk
  achievements re-sync runs once (~50s) on the next launch.
- v0.1.10's dual-fetch bilingual machinery removed.
- Bonus collateral fix: the 'Open achievement on wiki ↗' link now
  resolves to a useful EN search result (item.name is EN).
- Bumped package.json / Cargo.toml / tauri.conf.json from 0.1.10."
git push origin main
```

Expected output: `main -> main` push succeeds.

- [ ] **Step 6: Tag v0.1.11 and push the tag to trigger the release workflow**

```powershell
git tag -a v0.1.11 -m "v0.1.11 — full English display

Every GW2 data string (items, skins, achievements, Wizard's Vault
objectives, wallet currencies) now renders in English to match the
broader GW2 ecosystem (wiki, snowcrows, ArcDPS). Migration v10
wipes the FR-cached tables on first launch; bulk re-sync (~50s)
repopulates them in EN.

Side effect: 'Open achievement on wiki ↗' now lands on useful
results instead of a broken FR-name search. The v0.1.10 dual-
fetch + name_en machinery is removed.

Search by FR item names no longer works — use EN
('ascended shield', not 'bouclier élevé')."
git push origin v0.1.11
```

Expected output: `new tag v0.1.11 -> v0.1.11`. The GitHub Actions release workflow fires within seconds.

- [ ] **Step 7: Monitor the release workflow**

Open https://github.com/J7U7G7/GW2-legendary-overlay/actions in a browser. Wait ~15 minutes for the build to complete. Verify:
- The CI workflow on the version-bump commit passes.
- The Release workflow on the v0.1.11 tag passes and attaches MSI + NSIS installers + `latest.json` to the release page at https://github.com/J7U7G7/GW2-legendary-overlay/releases/tag/v0.1.11.

If anything fails, capture the failing step's output (last 30 lines) for triage. Manual smoke test after install (see "Manual smoke test" in spec):

1. App launches → settings preserved, API key still works.
2. ~50s in: achievement bulk re-sync completes; Catalog tab shows EN names.
3. Pin an achievement → bits resolve to EN names ("Mithril Ingot" not "Lingot de mithril").
4. Click 🔗 on a bit → lands on canonical wiki page directly.
5. Click "Open achievement on wiki ↗" → useful EN result.
6. Search "ectoplasm" in Items → returns user's ecto stack. "ectoplasme" returns nothing (expected).

---

## Validation summary

After Task 7, the project should be in the following state:

- `main` is at the version-bump commit, ahead of `v0.1.10` by ~4 commits.
- Tag `v0.1.11` exists locally and on origin.
- GitHub Actions Release workflow has produced a signed MSI + NSIS + `latest.json` attached to the GH Release.
- `latest.json` advertises version `0.1.11` so users on v0.1.10 see the in-app updater banner on their next launch.
- 67/67 lib tests pass; clippy zero warnings; npm build clean.

## Rollback

If after install the EN switch causes more friction than expected:

- Users on v0.1.10 stay on v0.1.10 (they can ignore the update banner).
- A hypothetical v0.1.12 could re-introduce FR via a Settings toggle (Option C from brainstorming) — but the schema would need careful handling to repopulate FR names without re-wiping EN names on every install.
