# Full English display

Status: approved, ready to implement
Target version: v0.1.11

## Problem

After v0.1.10 the overlay is bilingual in an uncomfortable way: item +
skin + achievement + Wizard's Vault names are fetched and displayed in
French (`?lang=fr`), but every wiki link target is English. The user
reads "Lingot de mithril" then clicks → wiki page titled "Mithril
Ingot". Cognitive translation cost on every wiki click. The achievement-
level link (`Open achievement on wiki ↗`) still searches with the FR
name and returns useless results.

The user evaluated three resolutions (all-EN / all-FR / settings
toggle) and chose **all-EN** for these reasons:

- Matches the rest of the GW2 ecosystem (wiki, snowcrows, hardstuck,
  ArcDPS, BlishHUD, discord raids — all primarily EN).
- Removes the mental-translation tax at click time.
- Simplifies the code: drops the dual-fetch + name_en machinery added
  in v0.1.10.
- Forward-compatible: if the app gets distributed to a wider audience,
  EN is the safer default than FR.

Trade-offs accepted by the user: searching by FR item name (e.g.
"bouclier élevé") no longer works — has to be "ascended shield". The
user plays the GW2 client in FR but agrees this is a one-time
adaptation, not an ongoing friction.

## Scope

### Data sources switching to English

The five `/v2/*` endpoints that currently carry a `lang=fr` query:

- `/v2/items` — used by Item Search, bit resolution
- `/v2/skins` — used for Skin-typed bits (armor collections)
- `/v2/achievements` — bulk-cached achievement definitions (~8200
  entries) + the `bits[].text` field
- `/v2/account/wizardsvault/{daily,weekly,special}` — objective titles
  and tracks
- `/v2/currencies` — currency display names (wallet integration)

### Already English (no change)

- Boss + meta event names in `src-tauri/data/boss_schedule.json`
  (curated EN at file-edit time).
- Legendary collection names in
  `src-tauri/data/legendary_collections.json` (curated EN).
- Legendary recipe leaf names in
  `src-tauri/data/legendary_recipes.json` (curated EN).
- Build names in `src-tauri/data/builds.json` (Snowcrows + MetaBattle
  source language).
- UI chrome (buttons, panel headers, tooltips) — already written in
  English.
- Toast notification text — composed from boss names which are already
  English.

### Code removals (the v0.1.10 bilingual machinery)

- `items_cache.name_en` column (added in schema v9)
- `skins_cache.name_en` column (schema v9)
- `api::endpoints::get_items_batch_en` and `get_skins_batch_en`
- `tokio::join!` dual-fetch in `sync::items::fetch_and_cache_items`
  and `sync::skins::fetch_and_cache_skins`
- `resolved_name_en` field on `PinnedBitView` / `PinnedBit`
- `wikiUrlForBit` fallback cascade in `PinnedPanel.tsx` — simplifies
  to a direct `wiki/<name>` URL since `resolved_name` is now EN.

## Migration (schema v10)

Single migration runs on the first launch of v0.1.11. SQL:

```sql
DELETE FROM items_cache;
DELETE FROM skins_cache;
DELETE FROM achievements;
DELETE FROM wizardsvault;
ALTER TABLE items_cache DROP COLUMN name_en;
ALTER TABLE skins_cache DROP COLUMN name_en;
```

### What survives

- `settings` — encrypted API key, appearance, hotkeys, notification
  lead, all UI preferences.
- `pinned_achievements` — IDs only, no names to migrate.
- `pinned_bosses` — string boss IDs, stable across versions.
- `legendary_collections` + `legendary_collection_members` — re-seeded
  on every boot by `catalog::load_all`, no migration needed.
- `account_progress` — counts + bit indices, no names.
- `account_items` — item IDs + locations + counts, no names.
- `currencies` + `account_currencies` — wiped + repopulated within
  5 min by the wallet sync loop on next launch.
- `todos`, `daily_assignments` (zombie since pivot 1).

### What gets re-fetched after migration

- Bulk achievements re-sync (`spawn_achievements_bootstrap`): empty
  table must be treated as stale. Implementation reads
  `achievements::is_stale` first; if the check is purely
  timestamp-based, extend it so `count(*) = 0` also returns true.
- WV objectives repopulate at the next 15-minute tick OR via the
  manual ↻ Sync button in the header.
- Items + skins caches repopulate lazily via `warm_item_cache` when
  the user pins / refreshes Catalog.

### Transient state during re-sync

For roughly 50 seconds after the first v0.1.11 launch, the Catalog +
Search + Pinned tabs show "Loading…" or unresolved `Item #N` / `Skin
#M` strings because the achievements + items + skins tables are
empty. This is identical to the fresh-install experience and is
considered acceptable.

## Code changes

### Backend (Rust)

| File | Change |
|---|---|
| `src-tauri/src/db/schema.rs` | Append migration v10. Update `fresh_migration_creates_all_tables` test list (no new tables — schema still 16). |
| `src-tauri/src/api/endpoints.rs` | `get_items_batch`, `get_skins_batch`, `get_currencies_batch` change `&lang=fr` → `&lang=en`. Remove `get_items_batch_en` and `get_skins_batch_en` (no callers after sync refactor). |
| `src-tauri/src/sync/items.rs` | Revert dual-fetch to single call. `upsert_items` drops the `en_by_id` parameter and the `name_en` column from its INSERT. `CachedItem` loses its `name_en: Option<String>` field. |
| `src-tauri/src/sync/skins.rs` | Symmetric changes for skins. |
| `src-tauri/src/sync/achievements.rs` | Verify the bulk fetch uses `lang=en`. The endpoint helper may not currently pass a lang param — append `&lang=en` to the URL builder. |
| `src-tauri/src/sync/wizardsvault.rs` | Add `&lang=en` to each of `/v2/account/wizardsvault/daily,weekly,special`. |
| `src-tauri/src/commands.rs` | `PinnedBitView` drops `resolved_name_en`. `parse_bits` returns a 3-tuple `(name, description, rarity)` instead of a 4-tuple. Verify `spawn_achievements_bootstrap` triggers a re-sync when the table is empty (not just when stale by timestamp). |

### Frontend (TS)

| File | Change |
|---|---|
| `src/types/gw2.ts` | `PinnedBit` drops `resolved_name_en`. |
| `src/components/PinnedPanel.tsx` | `wikiUrlForBit` simplifies: direct `wiki/<name>` URL when `resolved_name` is set, search fallback otherwise. The pre-v0.1.10 `kind:id` fallback stays gone. |
| `src/lib/format.ts` | `wikiUrl` unchanged — already produces EN page URLs from EN names. |
| `src/components/CatalogView.tsx` / `MyItemsView.tsx` / `SearchView.tsx` | No code changes — placeholders and labels are already in English. |

### Version bump

`package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` —
all three move from 0.1.10 to 0.1.11.

## Bonus fix (collateral)

The achievement-level "Open achievement on wiki ↗" link in
`PinnedPanel::AchievementDetails` currently builds a search URL with
the FR achievement name → no useful results. After this design lands,
`item.name` is the EN achievement name → the existing
`searchWikiUrl(item.name)` returns the correct result. No code change
needed there, but the bug is fixed as a side-effect. Worth flagging
in the release notes.

## Validation

Pre-commit ritual:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib              # 66 tests
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run build                                                       # tsc strict + Vite
```

Manual smoke test after install of v0.1.11:

1. App launches → boss & wallet sync run normally.
2. ~50s in, achievement bulk re-sync completes → Catalog/Pinned/Search
   show EN names.
3. Pin an achievement (e.g. an Obsidian Armor step) → bits resolve to
   EN names ("Mithril Ingot" not "Lingot de mithril").
4. Click 🔗 on a bit → lands on the canonical wiki page directly.
5. Click "Open achievement on wiki ↗" → lands on a useful EN search
   page (or direct page).
6. Search for "ectoplasm" in Items → returns the user's ecto stack.
   (Search for "ectoplasme" returns nothing — expected.)

## Risks + open considerations

- **`is_stale` semantics**: must trigger re-sync on count=0, not just
  on age. If the current check only looks at last_synced timestamps,
  empty table after wipe might not be considered stale. Implementation
  must verify and adjust.
- **WV special endpoint quirk**: the soft-fail in
  `sync_special` (introduced in v0.1.6) is still appropriate — the
  endpoint occasionally returns a non-period shape that fails to
  decode, regardless of language. Keep the soft-fail.
- **Achievement bits text field**: the `text` field of an achievement
  bit is the localized free-text from the API. Wiping `achievements`
  + re-fetching with lang=en repopulates this correctly.
- **User search habits**: existing user pinned achievements stay pinned
  but their cached search queries (if any persisted) might no longer
  find anything. We don't persist search queries, so this is moot.
- **Rollback path**: if the user wants to return to FR, they'd need a
  v0.1.x with `lang=fr` again. We're not building a Settings toggle in
  this iteration (deferred Option C from the brainstorming).

## Non-goals

- Settings toggle for language (deferred).
- Translating UI chrome to FR (the chrome stays EN — that was already
  the case pre-v0.1.10).
- Backfilling existing FR rows to EN in-place: we wipe + re-fetch.
