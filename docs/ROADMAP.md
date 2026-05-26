# Roadmap

What's done and what's planned. Items are grouped by status. Currently
at v0.1.11 (full English display).

## Shipped

### Phase 1 (initial spec)
- ✅ Transparent always-on-top overlay
- ✅ DPAPI-encrypted API key storage
- ✅ Token-bucket rate limiter (300 req/min, exp. backoff on 429/5xx)
- ✅ Boss timer engine (next_spawn / current_meta_phase / prev_spawn)
- ✅ Filler-phase filtering — Idle/Reset/Prep/Preparations no longer
  surface as "active" meta phases
- ✅ Weighted urgency scorer
- ✅ Settings panel (opacity, accent / text / bg colors, scale,
  notification lead time)
- ✅ Window position + size persisted between launches (per-window
  via `tauri-plugin-window-state`)
- ✅ Click-through toggle + global hotkeys (configurable since v0.1.2)

### Phase 2 (initial scope)
- ✅ Pinning architecture: `pinned_achievements` + `pinned_bosses`
- ✅ Boss-first pinning with collapsible linked achievements
- ✅ Curated legendary catalog — 32 collections
- ✅ Catalog grouped by generation + kind filter, recipe link per
  collection (opens English wiki page)
- ✅ Achievement search across the cached definition set
- ✅ Per-bit detail expansion (description, requirement, items + skins
  with resolved English names)
- ✅ Urgency inheritance — bits inside an achievement linked to a boss
  spawning in the next 10 min get an orange band
- ✅ Three-window split (main / bosses / achievements) with per-window
  collapse-to-strip + min-size constraints
- ✅ Cross-window event sync (`pinned_changed`, `appearance_changed`,
  `hotkeys_changed`)
- ✅ Windows toast notifications for pinned bosses + meta events
  (configurable lead time 1–15 min, test-notification button)
- ✅ Account-wide item search across bank, materials, shared
  inventory, every character's bags AND equipped slots
- ✅ Daily / weekly todos with automatic reset (00:00 UTC daily,
  Mondays 07:30 UTC weekly)
- ✅ Builds Manager — static JSON catalog + filter chips for
  Mode / Source / Class + copy-to-chat-code button. 10 entries from
  Snowcrows + MetaBattle (chat codes still need real in-game curation)

### Phase 3 (post-pivot expansion)
- ✅ **Wallet currencies sync** — `/v2/account/wallet` →
  `account_currencies` table + `currencies` cache. Surfaced in Item
  Search alongside items. Gold formatted as gold / silver / copper.
- ✅ **Skin bit resolution** — `/v2/skins` cached in `skins_cache`.
  Bits of type Skin (Obsidian/Astral/Envoy armor steps) now resolve
  to real names instead of `Skin #12078`.
- ✅ **Smart Legendary Selector** — recipe walker + per-card "📦 N%"
  recipe progress + top 5 missing leaves. 32/32 legendaries curated
  with verified item IDs against the live `/v2/items` API.
- ✅ **Smart Legendary tier 2** — Vision Crystal expanded to raw
  upstream mats; LWS3/4/PoF currencies for Aurora/Vision/Coalescence;
  Howl + Frostfang craftable precursor sub-recipes; Gen 2 weapons
  multiply Vision Crystal ×4 with extra direct leaves.

### Phase 4 (distribution + diagnostics, v0.1.1–v0.1.6)
- ✅ **Auto-updater** — `tauri-plugin-updater` against GitHub Releases
  endpoint. Signed bundles via minisign. Non-modal banner shows
  current + available version, install + relaunch in-app.
- ✅ **GitHub Actions CI** — push to main runs `cargo test --lib` +
  `cargo clippy --all-targets -D warnings` + `npm run build`.
- ✅ **GitHub Actions Release workflow** — tag push (`v*`) signs MSI
  + NSIS bundles, generates `latest.json`, attaches everything to a
  GH Release. `bundle.createUpdaterArtifacts: true` required.
- ✅ **File-based logging** — daily-rotated logs at
  `%APPDATA%/com.tripleseptconsulting.gw2overlay/logs/`. Panic hook
  routes Rust panics through tracing. FE→backend log bridge via
  `cmd_log_event` for production-state diagnostics that `console.log`
  can't reach.
- ✅ **GitHub Issue templates** — bug_report.yml + feature_request.yml
  + config.yml. Settings panel buttons pre-fill the new-issue URL
  with version + UA.
- ✅ **Configurable hotkeys** — Settings panel section captures key
  combos; persist via settings table; broadcast `hotkeys_changed` for
  live re-bind. Robust `tryBind` with per-shortcut fallback to
  defaults so a single bad combo can't kill the others.
- ✅ **Reset DB button** in Settings (double-confirm) wipes data
  tables but preserves API key + preferences.
- ✅ **Reset window layout** button in Settings — deletes the
  plugin's `.window-state` file + `relaunch()`.
- ✅ Renamed `GW2 Overlay` → `GW2 Legendary Overlay` (productName).

### Phase 5 (UX coherence, v0.1.11)
- ✅ **Full English display** — items / skins / achievements / WV /
  currencies all rendered in English. Schema v10 wipes the
  FR-localized caches on first launch; bulk re-sync (~50s)
  repopulates them in EN. Wiki links deep-link to canonical English
  pages instead of broken FR-name searches. The v0.1.10 dual-fetch
  + `name_en` machinery is removed.

## Backlog — scoped, not yet implemented

### Smart Legendary Selector — tier 3
Tier 2 covers depth-of-one for shared components. Tier 3 would:
- Expand "Gift of [Weapon]" specific gifts (Gift of Quickness, Gift
  of the Reaper, etc.) into raw leaves for the ≥3-reuse cases.
- Mark precursors as `tradeable: false` for the account-bound Gen 2
  ones (Mechanism, Tigris, etc.) so the walker can flag them
  appropriately rather than counting them as missing.
- Per-step LWS3/4 currency breakdown rather than aggregate totals.

### Real builds for the Builds tab
The Builds catalog ships with 10 entries (Snowcrows + MetaBattle)
but the chat codes are placeholders. Replace by hand-curated from
in-game (Hero panel → Build template → right-click → Copy Chat
Code). Optional future: `chatr` Rust crate to parse/validate build
template chat codes at compile time.

### Achievement-level wiki link is now EN-correct
Since v0.1.11 the `Open achievement on wiki ↗` link uses the now-EN
achievement name → search results on the EN wiki actually work.
Direct-page lookup (instead of search) would need an
`achievements_cache.name_en` column or some sort of pre-known wiki
title — minor polish.

### Logging level dropdown
Currently set via `RUST_LOG`. A Settings dropdown (debug / info /
warn / error) would let non-dev users help diagnose without env-var
gymnastics.

## Aspirational backlog (large features, no commitment)

### Pathing (TaCo-style markers)
In-world POI markers rendered by reading the GW2 Mumble API position
stream and drawing on the overlay. Requires Mumble shared memory
reader, `.taco` marker pack loader, and a transparent click-through
canvas (likely a WGPU surface) for 3D projection. Substantial scope.
Consider whether linking to existing BlishHUD / ArcDPS is saner than
reimplementing.

### Mounts radial menu
Circular hotkey palette popping up under the cursor (e.g. on a mouse
side-button). Click a mount icon to invoke its keybind.

### Item Search v2
- Filter by rarity / location.
- Click an item to open its wiki page (already deep-linked since
  v0.1.11 but Items tab uses different rendering).

### Builds Manager v2
- Per-build trait + skill rendering (palette ids → icons).
- Build template parse + validate via `chatr`.
- Auto-update from snowcrows / hardstuck (if they ever publish JSON).

### Account dashboard
- Total AP per area / per expansion.
- Wallet balances at a glance.
- Outstanding mastery / spirit-shard / fractal-relic totals.

### Multi-account / multi-user
Currently exactly one API key at a time. Multi-account would need a
"profile" abstraction over the DB + a profile-picker in the header.
If the app ever distributes beyond the curator, also add a "Language"
toggle (FR/EN) — for v0.1.11 we accepted the EN-only simplification
because the project still ships to one user.

### Linux / macOS port
DPAPI is Windows-only. Replace key storage with `keyring-rs`
(Credential Manager / Keychain wrapper) or `tauri-plugin-stronghold`
(IronFish). WebView2 dependency also limits us to Windows; Linux
needs `webkit2gtk`.

### Code-signing certificate
Today's MSI/NSIS bundles are unsigned with Authenticode → Windows
SmartScreen warns on install. Acceptable for personal use; consider
~€200/yr cert if the audience widens.

## Tooling / housekeeping

- A `feature_flag` table for staged rollouts of new behaviours
  (currently every change ships to every user immediately).
- `--reset` CLI flag as an alternative to the Settings Reset DB
  button for headless recovery.
