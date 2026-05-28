# Legendary Tier 3 (grouped recipe progress) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the flat `top_missing` list in legendary recipe progress with per-gift **groups**, each carrying its own ratio and leaves, and tag Living World direct leaves with group labels.

**Architecture:** Add an optional `group` label to `RecipeLeaf`. `compute_progress` builds an ordered list of `ProgressGroup`s (one per shared component, then direct-leaf groups by label, `"Specific"` fallback last) and allocates the user's owned pool greedily across groups in order so per-group numbers reconcile with the deduplicated header totals. The Catalog card renders collapsible group sections.

**Tech Stack:** Rust (rusqlite, serde), React + TypeScript (Zustand, Tailwind), Tauri 2.

**Spec:** `docs/superpowers/specs/2026-05-28-legendary-tier3-design.md`

---

## File Structure

- `src-tauri/src/legendary.rs` — add `group` field; new `LeafProgress` / `ProgressGroup` structs; rewrite `LegendaryProgress` and `compute_progress`; rewrite unit tests.
- `src-tauri/src/commands.rs` — drop the now-removed `top_n` argument at the `compute_progress` call site.
- `src/types/gw2.ts` — replace `MissingLeaf` with `LeafProgress`; add `ProgressGroup`; swap `top_missing` for `groups`.
- `src/components/CatalogView.tsx` — retarget `formatLeafQty`; add `GroupSection`; rewrite `RecipeProgressBlock`.
- `src-tauri/data/legendary_recipes.json` — bump `format_version` to `1.4`; tag `aurora`/`vision` Living World direct leaves with `group`.

---

## Task 1: Rust engine — grouped progress

**Files:**
- Modify: `src-tauri/src/legendary.rs` (struct `RecipeLeaf` ~43-53; structs `MissingLeaf`/`LegendaryProgress` ~128-147; fn `compute_progress` ~151-207; test module ~209-349)
- Modify: `src-tauri/src/commands.rs:371` (call-site arity — must change in the same task or the crate won't compile)

- [ ] **Step 1: Rewrite the test module (failing tests)**

Replace the entire `#[cfg(test)] mod tests { ... }` block (lines ~209-349) with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(kind: LeafKind, id: u32, qty: i64, name: &str, group: Option<&str>) -> RecipeLeaf {
        RecipeLeaf {
            kind,
            id,
            quantity: qty,
            name: name.into(),
            notes: None,
            group: group.map(|s| s.to_string()),
        }
    }

    fn cat() -> RecipeCatalog {
        let mut components = HashMap::new();
        components.insert(
            "test_gift".to_string(),
            Component {
                name: "Test Gift".into(),
                description: None,
                leaves: vec![
                    leaf(LeafKind::Item, 100, 250, "Ecto", None),
                    leaf(LeafKind::Currency, 1, 50000, "Coin", None),
                ],
            },
        );
        RecipeCatalog {
            meta: serde_json::Value::Null,
            components,
            legendaries: vec![LegendaryRecipe {
                collection_key: "bolt".into(),
                components: vec!["test_gift".into()],
                leaves: vec![leaf(LeafKind::Item, 200, 1, "Zap", None)],
                notes: None,
            }],
        }
    }

    #[test]
    fn aggregate_sums_components_and_direct_leaves() {
        let c = cat();
        let r = &c.legendaries[0];
        let needs = aggregate_needs(&c, r);
        assert_eq!(needs.len(), 3);
        assert_eq!(needs[&(LeafKind::Item, 100)].needed, 250);
        assert_eq!(needs[&(LeafKind::Currency, 1)].needed, 50000);
        assert_eq!(needs[&(LeafKind::Item, 200)].needed, 1);
    }

    #[test]
    fn groups_follow_component_then_direct_order() {
        let c = cat();
        let p = compute_progress(&c, &c.legendaries[0], &HashMap::new(), &HashMap::new());
        assert_eq!(p.groups.len(), 2);
        assert_eq!(p.groups[0].name, "Test Gift");
        assert_eq!(p.groups[1].name, "Specific");
        assert_eq!(p.groups[1].leaves.len(), 1);
        assert_eq!(p.groups[1].leaves[0].id, 200);
    }

    #[test]
    fn header_totals_match_dedup_aggregate() {
        let c = cat();
        let owned_items: HashMap<u32, i64> = [(100, 999), (200, 0)].into_iter().collect();
        let owned_currencies: HashMap<u32, i64> = [(1, 25000)].into_iter().collect();
        let p = compute_progress(&c, &c.legendaries[0], &owned_items, &owned_currencies);
        // ecto 250 (complete), coin 25000/50000, zap 0/1
        assert_eq!(p.total_needed, 250 + 50000 + 1);
        assert_eq!(p.total_owned, 250 + 25000);
        assert_eq!(p.leaves_total, 3);
        assert_eq!(p.leaves_complete, 1);
        let group_owned: i64 = p.groups.iter().map(|g| g.total_owned).sum();
        assert_eq!(group_owned, p.total_owned, "per-group owned must reconcile with header");
    }

    #[test]
    fn direct_leaves_group_by_label_first_seen_order() {
        let cat = RecipeCatalog {
            meta: serde_json::Value::Null,
            components: HashMap::new(),
            legendaries: vec![LegendaryRecipe {
                collection_key: "x".into(),
                components: vec![],
                leaves: vec![
                    leaf(LeafKind::Item, 1, 10, "A", Some("Living World S3")),
                    leaf(LeafKind::Item, 2, 10, "B", None),
                    leaf(LeafKind::Item, 3, 10, "C", Some("Living World S3")),
                ],
                notes: None,
            }],
        };
        let p = compute_progress(&cat, &cat.legendaries[0], &HashMap::new(), &HashMap::new());
        assert_eq!(p.groups.len(), 2);
        assert_eq!(p.groups[0].name, "Living World S3");
        assert_eq!(p.groups[0].leaves.len(), 2);
        assert_eq!(p.groups[1].name, "Specific");
        assert_eq!(p.groups[1].leaves.len(), 1);
    }

    #[test]
    fn owned_allocated_greedily_across_groups_in_order() {
        let mut components = HashMap::new();
        components.insert(
            "g".into(),
            Component {
                name: "G".into(),
                description: None,
                leaves: vec![leaf(LeafKind::Item, 100, 60, "Shared", None)],
            },
        );
        let cat = RecipeCatalog {
            meta: serde_json::Value::Null,
            components,
            legendaries: vec![LegendaryRecipe {
                collection_key: "x".into(),
                components: vec!["g".into()],
                leaves: vec![leaf(LeafKind::Item, 100, 60, "Shared", Some("Extra"))],
                notes: None,
            }],
        };
        let owned: HashMap<u32, i64> = [(100, 80)].into_iter().collect();
        let p = compute_progress(&cat, &cat.legendaries[0], &owned, &HashMap::new());
        let g = p.groups.iter().find(|x| x.name == "G").unwrap();
        let e = p.groups.iter().find(|x| x.name == "Extra").unwrap();
        assert_eq!(g.leaves[0].owned, 60, "first group filled first");
        assert_eq!(g.leaves[0].missing, 0);
        assert_eq!(e.leaves[0].owned, 20, "second group gets the remainder");
        assert_eq!(e.leaves[0].missing, 40);
        assert_eq!(p.total_needed, 120);
        assert_eq!(p.total_owned, 80);
        assert_eq!(g.total_owned + e.total_owned, 80);
    }

    #[test]
    fn group_leaves_sorted_missing_desc_complete_last() {
        let cat = RecipeCatalog {
            meta: serde_json::Value::Null,
            components: HashMap::new(),
            legendaries: vec![LegendaryRecipe {
                collection_key: "x".into(),
                components: vec![],
                leaves: vec![
                    leaf(LeafKind::Item, 1, 10, "small", Some("G")),
                    leaf(LeafKind::Item, 2, 1000, "big", Some("G")),
                    leaf(LeafKind::Item, 3, 5, "done", Some("G")),
                ],
                notes: None,
            }],
        };
        let owned: HashMap<u32, i64> = [(3, 5)].into_iter().collect();
        let p = compute_progress(&cat, &cat.legendaries[0], &owned, &HashMap::new());
        let g = &p.groups[0];
        assert_eq!(g.leaves[0].id, 2);
        assert_eq!(g.leaves[1].id, 1);
        assert_eq!(g.leaves[2].id, 3);
        assert!(g.leaves[2].complete);
        assert_eq!(g.leaves_complete, 1);
        assert_eq!(g.leaves_total, 3);
    }

    #[test]
    fn embedded_recipes_parse() {
        let cat = load().expect("legendary_recipes.json should parse");
        assert!(!cat.components.is_empty(), "expected at least one component");
        assert!(!cat.legendaries.is_empty(), "expected at least one legendary");
        let component_names: std::collections::HashSet<&str> =
            cat.components.values().map(|c| c.name.as_str()).collect();
        for rec in &cat.legendaries {
            for k in &rec.components {
                assert!(
                    cat.components.contains_key(k),
                    "legendary {} references unknown component {k}",
                    rec.collection_key
                );
            }
            for leaf in &rec.leaves {
                if let Some(g) = &leaf.group {
                    assert!(!g.is_empty(), "empty group label in {}", rec.collection_key);
                    assert!(
                        !component_names.contains(g.as_str()),
                        "direct-leaf group {g} in {} collides with a component name",
                        rec.collection_key
                    );
                }
            }
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail (compile error)**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib legendary`
Expected: FAIL — `RecipeLeaf` has no field `group`; `compute_progress` takes 5 args / `groups`/`LeafProgress`/`ProgressGroup` don't exist.

- [ ] **Step 3: Add the `group` field to `RecipeLeaf`**

In the `RecipeLeaf` struct (~43-53), add after the `notes` field:

```rust
    #[serde(default)]
    pub notes: Option<String>,
    /// Optional display group for direct leaves. Component leaves are grouped
    /// by the component name and ignore this. Unlabelled direct leaves fall
    /// into the `"Specific"` bucket. See spec 2026-05-28-legendary-tier3.
    #[serde(default)]
    pub group: Option<String>,
}
```

- [ ] **Step 4: Replace the output structs**

Replace `MissingLeaf` and `LegendaryProgress` (the two structs at ~128-147) with:

```rust
#[derive(Debug, Serialize, Clone)]
pub struct LeafProgress {
    pub kind: LeafKind,
    pub id: u32,
    pub name: String,
    pub needed: i64,
    /// Owned quantity attributed to *this* group (greedy allocation).
    pub owned: i64,
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
    /// Sorted largest-missing first; complete leaves last.
    pub leaves: Vec<LeafProgress>,
}

#[derive(Debug, Serialize, Clone)]
pub struct LegendaryProgress {
    pub collection_key: String,
    pub total_needed: i64,
    pub total_owned: i64,
    pub ratio: f64,
    pub leaves_total: usize,
    pub leaves_complete: usize,
    pub groups: Vec<ProgressGroup>,
}
```

- [ ] **Step 5: Rewrite `compute_progress`**

Replace the whole `compute_progress` fn (~149-207, including its doc comment) with:

```rust
/// Compute grouped progress for one legendary against the user's owned items +
/// currencies. Returns one `ProgressGroup` per shared component (in
/// `recipe.components` order), then one per direct-leaf `group` label (first-
/// seen order), with unlabelled direct leaves in a trailing `"Specific"` group.
/// Owned quantity is allocated greedily across groups in order, so per-group
/// totals reconcile exactly with the deduplicated top-level header totals.
pub fn compute_progress(
    catalog: &RecipeCatalog,
    recipe: &LegendaryRecipe,
    owned_items: &HashMap<u32, i64>,
    owned_currencies: &HashMap<u32, i64>,
) -> LegendaryProgress {
    let owned_of = |kind: LeafKind, id: u32| -> i64 {
        match kind {
            LeafKind::Item => owned_items.get(&id).copied().unwrap_or(0),
            LeafKind::Currency => owned_currencies.get(&id).copied().unwrap_or(0),
        }
    };

    // 1. Ordered (group_name, leaves): components first, then direct leaves
    //    bucketed by `group` label in first-seen order ("Specific" fallback).
    let mut grouped: Vec<(String, Vec<RecipeLeaf>)> = Vec::new();
    for component_key in &recipe.components {
        if let Some(component) = catalog.components.get(component_key) {
            grouped.push((component.name.clone(), component.leaves.clone()));
        }
    }
    let direct_start = grouped.len();
    for leaf in &recipe.leaves {
        let label = leaf.group.clone().unwrap_or_else(|| "Specific".to_string());
        match grouped[direct_start..].iter_mut().find(|(n, _)| *n == label) {
            Some((_, v)) => v.push(leaf.clone()),
            None => grouped.push((label, vec![leaf.clone()])),
        }
    }

    // 2. Greedy owned allocation: one shared pool per (kind, id).
    let mut pool: HashMap<(LeafKind, u32), i64> = HashMap::new();
    let mut groups: Vec<ProgressGroup> = Vec::new();
    for (name, leaves) in &grouped {
        let mut leaf_progress: Vec<LeafProgress> = Vec::new();
        let mut g_needed = 0i64;
        let mut g_owned = 0i64;
        let mut g_complete = 0usize;
        for leaf in leaves {
            let key = (leaf.kind, leaf.id);
            let remaining = pool.entry(key).or_insert_with(|| owned_of(leaf.kind, leaf.id));
            let owned_here = (*remaining).min(leaf.quantity);
            *remaining -= owned_here;
            let missing = (leaf.quantity - owned_here).max(0);
            let complete = missing == 0;
            if complete {
                g_complete += 1;
            }
            g_needed += leaf.quantity;
            g_owned += owned_here;
            leaf_progress.push(LeafProgress {
                kind: leaf.kind,
                id: leaf.id,
                name: leaf.name.clone(),
                needed: leaf.quantity,
                owned: owned_here,
                missing,
                complete,
            });
        }
        leaf_progress.sort_by(|a, b| b.missing.cmp(&a.missing));
        let ratio = if g_needed > 0 {
            g_owned as f64 / g_needed as f64
        } else {
            0.0
        };
        groups.push(ProgressGroup {
            name: name.clone(),
            total_needed: g_needed,
            total_owned: g_owned,
            ratio,
            leaves_total: leaf_progress.len(),
            leaves_complete: g_complete,
            leaves: leaf_progress,
        });
    }

    // 3. Top-level header from the deduplicated aggregate (semantics unchanged).
    let needs = aggregate_needs(catalog, recipe);
    let mut total_needed = 0i64;
    let mut total_owned = 0i64;
    let mut leaves_complete = 0usize;
    for leaf in needs.values() {
        let owned = owned_of(leaf.kind, leaf.id);
        total_needed += leaf.needed;
        total_owned += owned.min(leaf.needed);
        if owned >= leaf.needed {
            leaves_complete += 1;
        }
    }
    let ratio = if total_needed > 0 {
        total_owned as f64 / total_needed as f64
    } else {
        0.0
    };

    LegendaryProgress {
        collection_key: recipe.collection_key.clone(),
        total_needed,
        total_owned,
        ratio,
        leaves_total: needs.len(),
        leaves_complete,
        groups,
    }
}
```

- [ ] **Step 6: Fix the command call site (same task — required to compile)**

`compute_progress` now takes 4 args. In `src-tauri/src/commands.rs` at line ~371, change:

```rust
            crate::legendary::compute_progress(&catalog, rec, &owned_items, &owned_currencies, 5)
```

to:

```rust
            crate::legendary::compute_progress(&catalog, rec, &owned_items, &owned_currencies)
```

- [ ] **Step 7: Run the full lib test suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: PASS (66 tests). The legendary module now has 7 tests; the old `progress_truncates_missing_to_top_n` is removed, so the crate total goes from 67 to 66 — expected.

- [ ] **Step 8: Clippy**

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/legendary.rs src-tauri/src/commands.rs
git -c user.name="Ulysse" -c user.email="tripleseptconsulting@gmail.com" commit -m "feat(legendary): grouped recipe progress engine"
```

---

## Task 2: TypeScript types

**Files:**
- Modify: `src/types/gw2.ts:147-164`

- [ ] **Step 1: Replace `MissingLeaf` + `LegendaryProgress`**

Replace the `MissingLeaf` type (~147-154) and `LegendaryProgress` type (~156-164) with:

```typescript
export type LeafProgress = {
  kind: LeafKind;
  id: number;
  name: string;
  needed: number;
  owned: number;
  missing: number;
  complete: boolean;
};

export type ProgressGroup = {
  name: string;
  total_needed: number;
  total_owned: number;
  ratio: number;
  leaves_total: number;
  leaves_complete: number;
  leaves: LeafProgress[];
};

export type LegendaryProgress = {
  collection_key: string;
  total_needed: number;
  total_owned: number;
  ratio: number;
  leaves_total: number;
  leaves_complete: number;
  groups: ProgressGroup[];
};
```

(Leave the `LeafKind` export above it untouched.)

- [ ] **Step 2: Commit**

The build is verified in Task 3 (CatalogView is the only consumer and currently references the removed `MissingLeaf`, so `tsc` won't be green until Task 3). Commit the types now:

```bash
git add src/types/gw2.ts
git -c user.name="Ulysse" -c user.email="tripleseptconsulting@gmail.com" commit -m "feat(legendary): grouped progress TS types"
```

---

## Task 3: Catalog rendering

**Files:**
- Modify: `src/components/CatalogView.tsx:6` (import), `:26-38` (`formatLeafQty`), `:52-93` (`RecipeProgressBlock`)

- [ ] **Step 1: Update the type import**

Line 6, change:

```typescript
import type { LegendaryCollection, LegendaryProgress, MissingLeaf } from "../types/gw2";
```

to:

```typescript
import type { LegendaryCollection, LeafProgress, LegendaryProgress, ProgressGroup } from "../types/gw2";
```

- [ ] **Step 2: Retarget `formatLeafQty`**

Change the signature at line ~26 from `leaf: MissingLeaf` to `leaf: LeafProgress`:

```typescript
function formatLeafQty(leaf: LeafProgress): string {
```

(The body is unchanged — it reads `leaf.kind`, `leaf.id`, `leaf.missing`, all still present.)

- [ ] **Step 3: Replace `RecipeProgressBlock` with grouped rendering**

Replace the whole `RecipeProgressBlock` function (~52-93) with a `GroupSection` helper followed by the rewritten block:

```tsx
function GroupSection({ group }: { group: ProgressGroup }) {
  const complete = group.leaves_complete === group.leaves_total;
  const [open, setOpen] = useState(!complete);
  const pct = Math.round(group.ratio * 100);
  const missing = group.leaves.filter((l) => !l.complete);
  return (
    <div className="mt-1.5">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="flex items-center justify-between w-full text-left text-[10px] opacity-80 hover:opacity-100"
      >
        <span className="font-semibold truncate">
          {complete ? "✓ " : open ? "▾ " : "▸ "}
          {group.name}
        </span>
        <span className="font-mono opacity-60 shrink-0 ml-2">
          {pct}% · {group.leaves_complete}/{group.leaves_total}
        </span>
      </button>
      {open && !complete && (
        <ul className="mt-1 ml-2 space-y-0.5">
          {missing.map((leaf, i) => (
            <li
              key={`${leaf.kind}:${leaf.id}:${i}`}
              className="text-[10px] flex items-center justify-between gap-2"
              title={leaf.name}
            >
              <span className="truncate opacity-80">
                {leaf.kind === "currency" ? "💰 " : ""}
                {leaf.name}
              </span>
              <span className="font-mono shrink-0">
                <span className="opacity-50">{leaf.owned.toLocaleString()}</span>
                <span className="opacity-50">/</span>
                <span>{leaf.needed.toLocaleString()}</span>
                <span className="text-red-300/80 ml-1">−{formatLeafQty(leaf)}</span>
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function RecipeProgressBlock({ progress }: { progress: LegendaryProgress }) {
  const pct = Math.round(progress.ratio * 100);
  const allComplete = progress.leaves_complete === progress.leaves_total;
  return (
    <div className="border-t border-white/5 px-3 py-2 bg-white/[0.03]">
      <div className="flex items-center justify-between text-[10px] mb-1.5">
        <span className="opacity-70 font-semibold">📦 Recipe: {pct}% complete</span>
        <span className="opacity-60 font-mono">
          {progress.leaves_complete}/{progress.leaves_total} leaves
        </span>
      </div>
      <ProgressBar ratio={progress.ratio} />
      {allComplete ? (
        <p className="text-[10px] text-[var(--accent-color)] italic mt-1.5">
          All tracked leaves complete. Remaining work is in achievement steps.
        </p>
      ) : (
        progress.groups.map((g) => <GroupSection key={g.name} group={g} />)
      )}
    </div>
  );
}
```

- [ ] **Step 4: Build (tsc strict + Vite)**

Run: `npm run build`
Expected: PASS — no type errors. (`MissingLeaf` no longer referenced anywhere.)

- [ ] **Step 5: Commit**

```bash
git add src/components/CatalogView.tsx
git -c user.name="Ulysse" -c user.email="tripleseptconsulting@gmail.com" commit -m "feat(legendary): render recipe progress as collapsible gift groups"
```

---

## Task 4: Tag Living World direct leaves (data)

**Files:**
- Modify: `src-tauri/data/legendary_recipes.json` (`_meta`; `aurora` and `vision` `leaves` arrays)

No item-id changes here — we only add `group` labels to leaves whose ids were already API-verified, so no `/v2/items` re-verification is needed.

- [ ] **Step 1: Bump the format version + note**

In `_meta`, change `"format_version": "1.3",` to `"format_version": "1.4",`, change `"last_updated": "2026-05-26"` to `"last_updated": "2026-05-28"`, and add a new key right after the `format_version` line:

```jsonc
    "format_version": "1.4",
    "group_field_note": "1.4 adds an optional `group` field on direct leaves. Component leaves are grouped by the component name; unlabelled direct leaves fall into the 'Specific' bucket. A direct-leaf group label must not equal a component name.",
```

- [ ] **Step 2: Tag `aurora` direct leaves**

In the `aurora` legendary's `leaves` array, add `"group": "Living World S3"` to each of these leaves (match by `name`): **Unbound Magic, Blood Ruby, Petrified Wood, Fresh Winterberry, Jade Shard, Orrian Pearl, Fire Orchid Blossom**. Leave **Bloodstone Capacitor** and **Obsidian Shard** untagged (they fall into "Specific").

Example — the Blood Ruby leaf changes from:

```jsonc
        { "kind": "item", "id": 79899, "quantity": 250, "name": "Blood Ruby" }
```

to:

```jsonc
        { "kind": "item", "id": 79899, "quantity": 250, "name": "Blood Ruby", "group": "Living World S3" }
```

(Read the file first to copy each leaf's exact current text, then Edit. `id`/`quantity` values stay as-is.)

- [ ] **Step 3: Tag `vision` direct leaves**

In the `vision` legendary's `leaves` array, add `"group": "Living World S4"` to each of these (match by `name`): **Volatile Magic, Elegy Mosaic, Kralkatite Ore, Difluorite Crystal, Inscribed Shard, Lump of Mistonium, Branded Mass, Mistborn Mote**. Leave **Obsidian Shard** untagged.

- [ ] **Step 4: Verify the data parses + grouping invariants hold**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib legendary::tests::embedded_recipes_parse`
Expected: PASS — confirms `format 1.4` parses and no group label collides with a component name.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/data/legendary_recipes.json
git -c user.name="Ulysse" -c user.email="tripleseptconsulting@gmail.com" commit -m "data(legendary): tag aurora/vision Living World leaves with groups"
```

---

## Task 5: Final verification ritual

**Files:** none (gate only)

- [ ] **Step 1: Full pre-commit ritual**

Run, from the repo root:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run build
```

Expected: all three green (66 lib tests pass, zero clippy warnings, tsc + Vite build clean).

- [ ] **Step 2: Smoke-check in the running app (optional but recommended)**

Run: `npm run tauri dev`, open the **Catalog** tab, expand a Gen1 weapon (e.g. *The Bifrost*) and a Living World trinket (*Aurora*). Confirm the recipe block now shows collapsible gift sections, that Aurora shows a "Living World S3" group, and that complete groups render collapsed with a ✓.

- [ ] **Step 3: No commit needed** (verification only).

---

## Notes for the implementer

- `compute_progress` is reachable only from `commands.rs` (not from `src-tauri/tests/`), so the single call site to fix is the one in Task 1, Step 6.
- Group default expansion is per-`GroupSection` local state (`useState`), so it resets when the card is collapsed/reopened — that matches the existing `CollectionCard` behaviour and needs no store wiring.
- Do not pass `lang=fr` anywhere; leaf names are curated EN strings (Pivot 5).
