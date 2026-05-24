# Roadmap

What's done, in progress, and planned. Items are grouped by status.

## Shipped

### Phase 1 (spec)
- ✅ Transparent always-on-top overlay
- ✅ DPAPI-encrypted API key storage
- ✅ Token-bucket rate limiter (300 req/min, exp. backoff on 429/5xx)
- ✅ Periodic sync (progress 5 min / WV 15 min / inventory 30 min)
- ✅ Boss timer engine (next_spawn / current_meta_phase / prev_spawn)
- ✅ Weighted urgency scorer
- ✅ Settings panel (opacity, accent / text / bg colors, scale)
- ✅ Window position + size persisted between launches
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
  collapse-to-strip
- ✅ Cross-window event sync (`pinned_changed`, `appearance_changed`)
- ✅ Windows toast notifications for pinned bosses + meta events
  (configurable lead time 1–15 min, test-notification button)
- ✅ Account-wide item search across bank, materials, shared
  inventory, every character's bags AND equipped slots; results in
  French
- ✅ Daily / weekly todos with automatic reset (00:00 UTC daily,
  Mondays 07:30 UTC weekly)
- ✅ Builds Manager infra — static JSON catalog + UI + copy-to-chat-
  code button. Real chat codes to be filled by the curator.

## Backlog — scoped, not yet implemented

### Smart Legendary Selector (largest)

Cross-reference the user's `account_items` + wallet against each
legendary's recipe, surface "X% complete" + the top 5 missing items.

Design fully captured (commit `57e38c8` agent transcript). Key
pieces:
- `data/legendary_recipes.json` — components dictionary +
  per-legendary ingredient trees, referencing item_ids, currency_ids,
  and collection_reward markers.
- A new wallet sync module (`/v2/account/wallet` → `account_currencies`
  table) since Spirit Shards, gold, map currencies live there.
- A recipe walker in commands.rs that aggregates need across the tree
  (avoiding double-counting Mystic Coins between Mystic Clover, Mystic
  Tribute, and Gift of Fortune), compares to owned, returns a sorted
  Vec<LegendaryProgress>.
- New UI tab or in-card "Recipe progress" expansion in Catalog.

Estimated effort: ~30 hours, dominated by recipe data curation across
the 32 legendaries.

Top gotchas documented in the agent transcript:
1. Recursive gifts — Mystic Coins appear in Tribute, Fortune,
   precursor; aggregate totals, don't decrement as you walk.
2. Currency vs item — Mystic Coin is an item, gold is a currency.
   Spirit Shards are *both* (currency id 23 and an ingredient item).
3. Collection rewards don't have item ids until crafted — track via
   `account_progress` against the collection step that grants them.
4. Aurora/Vision/Coalescence don't go through the Mystic Forge —
   they're forged by completing the collection achievement, which
   consumes the inventory items.
5. Mystic Clover RNG (~31 %) — treat clovers themselves as a leaf
   item; don't try to model their RNG cost in coins/ectos (would
   double-count with Tribute/Fortune).
6. Some Gen2 precursors (Mechanism, Tigris, etc.) are account-bound
   and not on the TP — flag with `precursor_tradeable: false`.

### Configurable hotkeys
`useHotkeys.ts` currently hard-codes Ctrl+Shift+G/H/B/P. Add UI to
let the user pick custom combos. Persist via the existing settings
table. Validate via `isRegistered` + swap via `unregister` / `register`
at runtime.

### Real builds for the Builds tab
The Builds Manager infra ships with three placeholder chat codes.
Replace by either:
- Hand-curated from snowcrows.com / hardstuck.gg (paste real chat
  codes into `data/builds.json`).
- Optional future: `chatr` Rust crate (mentioned by the audit agent)
  to parse / validate build template chat codes at compile time.

### Wallet currencies
Add `/v2/account/wallet` sync + a new `account_currencies` table.
Useful in itself (currencies show in Item Search) and a prereq for
Smart Legendary Selector. Estimated: ~1 hour.

## Aspirational backlog (large features, no commitment)

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
- Wallet currencies in the search results.

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

- `tauri build` MSI installer flow + CI.
- A `--reset` flag or settings-panel button to wipe the SQLite file
  (useful for re-running the bulk sync after spec changes).
- Logging level dropdown in Settings (currently set via `RUST_LOG`).
- Maybe a `feature_flag` table for staged rollouts of new behaviours.
