# Roadmap

What's done, in progress, and planned. Items are grouped by status.

## Shipped

### Phase 1 (spec)
- ✅ Transparent always-on-top overlay
- ✅ DPAPI-encrypted API key storage
- ✅ Token-bucket rate limiter (300 req/min, exp. backoff on 429/5xx)
- ✅ Periodic sync — five loops: achievements bootstrap (weekly) /
  progress 5 min / WV 15 min / inventory 30 min / wallet 5 min / boss
  watcher 30 s
- ✅ Boss timer engine (next_spawn / current_meta_phase / prev_spawn)
- ✅ Filler-phase filtering — Idle/Reset/Prep/Preparations no longer
  surface as "active" meta phases
- ✅ Weighted urgency scorer
- ✅ Settings panel (opacity, accent / text / bg colors, scale,
  notification lead time)
- ✅ Window position + size persisted between launches (per-window
  via `tauri-plugin-window-state`)
- ✅ Click-through toggle + global hotkeys (Ctrl+Shift+G/H/B/P)

### Phase 2 (initial scope)
- ✅ Pinning architecture: `pinned_achievements` + `pinned_bosses`
- ✅ Boss-first pinning with collapsible linked achievements
- ✅ Curated legendary catalog — 32 collections
- ✅ Catalog grouped by generation + kind filter
- ✅ Recipe link per collection (opens wiki page)
- ✅ Achievement search across the cached definition set
- ✅ Per-bit detail expansion (description, requirement, items with
  resolved French names)
- ✅ Urgency inheritance — bits inside an achievement linked to a boss
  spawning in the next 10 min get an orange band
- ✅ Three-window split (main / bosses / achievements) with per-window
  collapse-to-strip + min-size constraints
- ✅ Cross-window event sync (`pinned_changed`, `appearance_changed`)
- ✅ Windows toast notifications for pinned bosses + meta events
  (configurable lead time 1–15 min, test-notification button)
- ✅ Account-wide item search across bank, materials, shared
  inventory, every character's bags AND equipped slots; results in
  French
- ✅ Daily / weekly todos with automatic reset (00:00 UTC daily,
  Mondays 07:30 UTC weekly)
- ✅ Builds Manager — static JSON catalog + filter chips for
  Mode / Source / Class + copy-to-chat-code button. 10 entries from
  Snowcrows + MetaBattle (chat codes are placeholders pending in-game
  curation by the user)

### Phase 3 (post-pivot expansion)
- ✅ **Wallet currencies sync** — `/v2/account/wallet` →
  `account_currencies` table + `currencies` cache (FR names).
  Surfaced in Item Search alongside items. Gold formatted as gold /
  silver / copper.
- ✅ **Smart Legendary Selector** — recipe walker + per-card "📦 N%"
  recipe progress + top 5 missing leaves. 32/32 legendaries curated
  (6 shared components: Gift of Fortune, Gift of Mastery, Mystic
  Tribute, Gen 1 Signature, Vision Crystal, plus per-weapon inlined
  signature work). All item_ids verified against the live
  `/v2/items` API.

## Backlog — scoped, not yet implemented

### Smart Legendary Selector — tier 2 (extension)
The walker + UI shipped covers 80 % of the picture for any
legendary. Tier 2 expands depth in three directions:
1. **Recursive gift subtrees** — Vision Crystal currently bottoms
   out as a 1-quantity leaf. Expand its upstream mats (Augur's Stone
   + Bloodstone Brick + Dragonite Ingot + Empyreal Star) into its
   `leaves` array. Same for high-reuse Gen 1 specific gifts (Gift
   of Quickness, Gift of the Reaper, etc.) once one is built and
   tracked by ≥ 3 legendaries.
2. **LWS3 / LWS4 / PoF currency leaves** — Aurora/Vision/Coalescence
   currently use rough placeholders. Curate per-step real spend in
   Unbound Magic, Volatile Magic, Trade Contracts, Elegy Mosaics.
3. **Craftable precursor subtrees** — for the cheap craftable
   precursors (Howl, etc.), expand the precursor leaf into its
   ascended-material upstream so the walker sees partial progress.

### Configurable hotkeys
`useHotkeys.ts` currently hard-codes Ctrl+Shift+G/H/B/P. Add UI to
let the user pick custom combos. Persist via the existing settings
table. Validate via `isRegistered` + swap via `unregister` / `register`
at runtime.

### Real builds for the Builds tab
The Builds catalog ships with 10 entries (Snowcrows + MetaBattle)
but the chat codes are placeholders. Replace by:
- Hand-curated from in-game (Hero panel → Build template →
  right-click → Copy Chat Code).
- Optional future: `chatr` Rust crate to parse/validate build
  template chat codes at compile time.

### Reset DB button
A settings-panel button to wipe `gw2-overlay.sqlite` (useful for
re-running the bulk sync after a spec change or to recover from a
botched migration during dev). Estimated: ~1 hour.

## Aspirational backlog (large features, no commitment)

### MSI installer + CI
- `tauri build` produces an MSI in
  `src-tauri/target/release/bundle/msi/` — wire up a GitHub Actions
  workflow that builds it on tag push and attaches it to the GH
  release.
- Push-CI workflow that runs `cargo test --lib` + `cargo clippy
  --all-targets -- -D warnings` + `npm run build` on every push.

### Pathing (TaCo-style markers)
In-world POI markers rendered by reading the GW2 Mumble API position
stream and drawing on the overlay. Requires:
- Mumble shared memory reader for player coords.
- `.taco` marker pack loader.
- A transparent click-through canvas separate from the React UI
  (likely a WGPU surface) for 3D projection.

Substantial scope. Consider whether linking to existing BlishHUD /
ArcDPS is saner than reimplementing.

### Mounts radial menu
Circular hotkey palette popping up under the cursor when triggered
(e.g. a mouse side-button). Click a mount icon to invoke its keybind.

### Item Search v2
- Filter by rarity / location.
- Click an item to open its wiki page.
- (Currencies are already in the search results since wallet sync
  shipped.)

### Builds Manager v2
- Per-build trait + skill rendering (palette ids → icons).
- Build template parse + validate via `chatr`.
- Auto-update from snowcrows / hardstuck (if they ever publish JSON).

### Account dashboard
- Total AP per area / per expansion.
- Wallet balances at a glance.
- Outstanding mastery / spirit-shard / fractal-relic totals.

### Multi-account
Currently exactly one API key at a time. Multi-account would need a
"profile" abstraction over the DB + a profile-picker in the header.

### Linux / macOS port
DPAPI is Windows-only. Replace key storage with `tauri-plugin-stronghold`
(IronFish) or `keyring-rs` (Credential Manager / Keychain wrapper).
WebView2 dependency also limits us to Windows; Linux needs `webkit2gtk`.

## Tooling / housekeeping

- Logging level dropdown in Settings (currently set via `RUST_LOG`).
- Maybe a `feature_flag` table for staged rollouts of new behaviours.
