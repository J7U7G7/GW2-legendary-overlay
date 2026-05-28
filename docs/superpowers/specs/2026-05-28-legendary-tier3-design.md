# Smart Legendary Selector — Tier 3 (grouped recipe progress)

- **Date:** 2026-05-28
- **Status:** Design approved, pending implementation plan
- **Scope:** ROADMAP "Smart Legendary Selector — tier 3" sub-items
  (a) expand reused gifts and (c) per-step Living World currency
  breakdown. Sub-item (b) (account-bound precursor flagging) is
  explicitly **out of scope** for this round.

## Problem

`legendary.rs` computes per-legendary progress by flattening every
component + direct leaf into a single deduplicated `(kind, id)` map
(`aggregate_needs`), then returns a flat `top_missing` list capped at
5. The Catalog card renders that flat list.

Two consequences this design fixes:

1. **No structure in the output.** The user can't see *which gift* a
   missing material belongs to. Living World currencies (Blood Ruby,
   Petrified Wood, Kralkatite Ore…) sit anonymously next to Mystic
   Clovers and ectos.
2. **Specific "Gift of [Weapon]" sub-gifts are barely modelled.**
   Gen1 weapons carry only 1-3 direct leaves; the weapon-specific
   gift's ingredients are mostly absent, so the `%` understates the
   real work and over-credits near-complete recipes.

## Approach (chosen: A — `group` label + grouped output)

Add an optional `group` label to leaves and restructure the progress
output to be a list of **groups** (one per component, plus
direct-leaf groups), each carrying its own ratio and leaves. No
nested/recursive components (rejected approach B — cycle and
double-count handling not worth it for 32 hand-curated recipes). No
flat-aggregate-with-provenance (rejected approach C — can't cleanly
name sub-groups of non-shared direct leaves).

This is a **breaking change** to the `LegendaryProgress` shape (Rust +
TS): `top_missing` is removed in favour of `groups`.

## 1. Data model — `legendary_recipes.json` format 1.4

Add an optional `group: Option<String>` field to `RecipeLeaf`:

```jsonc
{ "kind": "item", "id": 79899, "quantity": 1, "name": "Blood Ruby",
  "group": "Living World S3" }
```

Grouping rules:

- **Shared component leaves** (`gift_of_fortune`, `gift_of_mastery`,
  `mystic_tribute`, `gen1_signature`, `vision_crystal`, plus any new
  shared components): the component's `name` **is** the group name.
  Leaves inside a component do **not** need a `group` field; if one is
  present it is ignored (the component name wins).
- **Direct leaves** (`recipe.leaves`): grouped by their `group` label.
  Leaves with no `group` fall into a single fallback bucket named
  `"Specific"` (precursor, one-off signature items).

`_meta.format_version` → `"1.4"`. Add a `_meta` note documenting the
`group` field and the fallback bucket name.

### Curation criterion for (a) — the ≥3-reuse rule

A sub-ingredient of a "Gift of [Weapon]" is promoted to a **shared
component** (its own reusable group) **only if it appears in ≥3
legendaries**. Otherwise it stays as direct leaves tagged with a
named `group` (e.g. `"Gift of the Bifrost"`). This bounds the
curation effort to materials with real cross-recipe reuse.

The data curation is **incremental**: this round tags the Living
World currencies already present as direct leaves on
aurora/vision/coalescence (and the gen1/gen2 specific direct leaves)
with `group` labels, and promotes any ≥3-reuse sub-gift found during
curation. The engine supports the full structure on delivery; the
remaining per-legendary expansion can land in later data-only commits.

## 2. Engine — `legendary.rs`

New output structures; `MissingLeaf` and `top_missing` are removed.

```rust
#[derive(Debug, Serialize, Clone)]
pub struct LeafProgress {
    pub kind: LeafKind,
    pub id: u32,
    pub name: String,
    pub needed: i64,
    pub owned: i64,    // attributed to THIS group (see allocation rule)
    pub missing: i64,
    pub complete: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct ProgressGroup {
    pub name: String,
    pub total_needed: i64,
    pub total_owned: i64,
    pub ratio: f64,
    pub leaves_total: usize,
    pub leaves_complete: usize,
    pub leaves: Vec<LeafProgress>, // sorted missing-desc; complete last
}

#[derive(Debug, Serialize, Clone)]
pub struct LegendaryProgress {
    pub collection_key: String,
    pub total_needed: i64,
    pub total_owned: i64,
    pub ratio: f64,
    pub leaves_total: usize,
    pub leaves_complete: usize,
    pub groups: Vec<ProgressGroup>, // replaces top_missing
}
```

### Group ordering

1. Components in `recipe.components` order, each as one group (group
   name = component `name`).
2. Direct-leaf groups in first-seen order of their `group` label.
3. The `"Specific"` fallback bucket last (if any unlabelled direct
   leaves exist).

### Owned-allocation rule (handles an id spanning groups)

The same `(kind, id)` can appear in more than one group. (Rare within
a single legendary because Gen1 uses `gift_of_fortune` and Gen2 uses
`mystic_tribute`, which don't co-occur — but the rule is defined so
the numbers are always self-consistent.)

For each `(kind, id)`:

- `pool` = total owned across all locations for that id.
- Walk the leaf occurrences in **group order**. Each occurrence is
  credited `owned_here = min(pool_remaining, needed_here)`, then
  `pool_remaining -= owned_here`.

This guarantees `sum(owned per group) == min(total_owned, total_needed)`,
so the per-group numbers reconcile exactly with the top-level header.

### Top-level fields

- `total_needed` / `total_owned` / `ratio`: computed on the
  **deduplicated aggregate** (same semantics as today's
  `aggregate_needs`), so the collapsed card header `%` is unchanged.
- `leaves_total` / `leaves_complete`: counted over the deduplicated
  aggregate (a leaf shared across groups counts once toward the
  header totals), so the header reads the same as before.

Per-group `ratio` is `total_owned / total_needed` for that group
(0.0 when the group needs nothing). Per-group `leaves_total` /
`leaves_complete` count the leaf occurrences within that group.

## 3. Types TS + command

- `src/types/gw2.ts`:
  - Remove `MissingLeaf`, add `LeafProgress` and `ProgressGroup`.
  - `LegendaryProgress`: replace `top_missing: MissingLeaf[]` with
    `groups: ProgressGroup[]`.
- `cmd_legendary_progress` (`commands.rs`): signature unchanged
  (`Vec<LegendaryProgress>`). The internal `top_n` argument to
  `compute_progress` is removed — all leaves are returned, grouped.
- `src/lib/tauri.ts`: `api.legendaryProgress()` name unchanged; only
  the return type changes via the shared `gw2.ts` types.

## 4. Rendering — `CatalogView.tsx`

- **Collapsed card header:** unchanged — `📦 N%` + the `≥95%` badge,
  driven by `progress.ratio`.
- **`RecipeProgressBlock`:** replace the flat `top_missing` list with
  a list of collapsible group sections. Each section header shows the
  group `name`, its `ratio%`, and `leaves_complete/leaves_total`.
  - Expanding a group lists its **missing** leaves (reusing
    `formatLeafQty` for gold/silver coin formatting).
  - A fully-complete group (`leaves_complete == leaves_total`) renders
    collapsed with a ✓ and no leaf detail.
  - Default expansion state: groups with missing leaves start
    expanded; complete groups start collapsed.
- **`formatLeafQty`:** retarget from `MissingLeaf` to `LeafProgress`
  (the `missing` and `kind`/`id` fields it reads are preserved).

## 5. Tests

Unit tests in `legendary.rs`:

- Component leaves grouped under the component name; group order
  matches `recipe.components` order.
- Direct leaves grouped by `group` label; unlabelled direct leaves
  land in the `"Specific"` bucket which sorts last.
- Greedy owned-allocation across an id that appears in two groups:
  first group filled before the second; `sum(owned) == capped total`.
- Per-group ratio / counts correct; top-level header totals match the
  deduplicated aggregate (regression: existing
  `progress_caps_owned_at_needed_and_lists_missing` expectations
  re-expressed against `groups`).
- `embedded_recipes_parse` extended to assert every non-empty `group`
  label round-trips and no direct leaf silently collides with a
  component name.

Build gate: `npm run build` (tsc strict) for the TS type change;
`cargo test --lib` + `cargo clippy --all-targets -- -D warnings`.

## Out of scope

- Sub-item (b): marking Gen2 account-bound precursors
  `tradeable: false`. Tracked separately in ROADMAP.
- Nested/recursive components.
- Exhaustive per-legendary gift expansion for all 32 collections
  (engine supports it; data lands incrementally).
