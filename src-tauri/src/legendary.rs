//! Smart Legendary Selector: cross-references the user's account_items +
//! account_currencies with curated recipes from
//! `src-tauri/data/legendary_recipes.json` to compute per-legendary progress
//! (% owned + top missing leaves).
//!
//! The recipe model is intentionally shallow:
//!
//! - A **leaf** is an atomic requirement — either an item (with item_id and
//!   quantity) or a currency (with currency_id and quantity).
//! - A **component** is a named bundle of leaves (e.g. "Gift of Fortune" is a
//!   bundle of Mystic Clover ×77, Glob of Ectoplasm ×250, etc.). Components
//!   are curated once and re-used by every legendary that needs them.
//! - A **legendary recipe** references zero or more components plus its own
//!   direct leaves (typically just the precursor + a generation-specific
//!   gift's high-value ingredients).
//!
//! Deliberately *not modelled*:
//!
//! - Mystic Clover RNG. Clovers are treated as a leaf item — we count what
//!   the user has, not what it took to make them.
//! - Recursive gift trees (Gift of Quickness → Vision Crystal → ...).
//!   Recipes go one level deep. Anything beyond that is curated as a
//!   separate component if shared, or noted in `notes` if not.
//! - Account-bound precursors that don't have a single item_id on the TP
//!   (Gen2's Mechanism, Tigris, etc.). Flag those with `precursor_tradeable:
//!   false` and they're displayed but not aggregated into the missing list.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::Result;

const EMBEDDED_RECIPES: &str = include_str!("../data/legendary_recipes.json");

/// Fallback group name for direct leaves that carry no explicit `group` label.
const DIRECT_FALLBACK_GROUP: &str = "Specific";

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum LeafKind {
    Item,
    Currency,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RecipeLeaf {
    pub kind: LeafKind,
    pub id: u32,
    pub quantity: i64,
    /// Display name. Curated at edit-time; the live items_cache /
    /// currencies tables are authoritative for the localized text.
    pub name: String,
    #[serde(default)]
    pub notes: Option<String>,
    /// Optional display group for direct leaves. Component leaves are grouped
    /// by the component name and ignore this. Unlabelled direct leaves fall
    /// into the `"Specific"` bucket. See spec 2026-05-28-legendary-tier3.
    #[serde(default)]
    pub group: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Component {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub leaves: Vec<RecipeLeaf>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LegendaryRecipe {
    pub collection_key: String,
    #[serde(default)]
    pub components: Vec<String>,
    #[serde(default)]
    pub leaves: Vec<RecipeLeaf>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecipeCatalog {
    #[allow(dead_code)]
    #[serde(default, rename = "_meta")]
    pub meta: serde_json::Value,
    pub components: HashMap<String, Component>,
    pub legendaries: Vec<LegendaryRecipe>,
}

pub fn load() -> Result<RecipeCatalog> {
    let cat: RecipeCatalog = serde_json::from_str(EMBEDDED_RECIPES)?;
    Ok(cat)
}

/// Aggregate the total quantity needed across a recipe + its components.
/// Multiple components that share a leaf (e.g. Gift of Fortune and Mystic
/// Tribute both want Mystic Clover) are summed, never deduplicated.
pub fn aggregate_needs(
    catalog: &RecipeCatalog,
    recipe: &LegendaryRecipe,
) -> HashMap<(LeafKind, u32), AggregateLeaf> {
    let mut out: HashMap<(LeafKind, u32), AggregateLeaf> = HashMap::new();
    let mut add = |leaf: &RecipeLeaf| {
        let entry = out
            .entry((leaf.kind, leaf.id))
            .or_insert_with(|| AggregateLeaf {
                kind: leaf.kind,
                id: leaf.id,
                name: leaf.name.clone(),
                needed: 0,
            });
        entry.needed += leaf.quantity;
    };
    for component_key in &recipe.components {
        if let Some(component) = catalog.components.get(component_key) {
            for leaf in &component.leaves {
                add(leaf);
            }
        }
    }
    for leaf in &recipe.leaves {
        add(leaf);
    }
    out
}

#[derive(Debug, Clone)]
pub struct AggregateLeaf {
    pub kind: LeafKind,
    pub id: u32,
    pub name: String,
    pub needed: i64,
}

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

/// Per-legendary progress summary returned to the frontend.
///
/// `total_needed`, `total_owned`, `leaves_total`, and `leaves_complete` are
/// computed from the **deduplicated** aggregate: a leaf shared across multiple
/// components counts once, so these header totals may differ from
/// `sum(groups[].total_needed)` when an item appears in more than one group.
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
        let label = leaf.group.clone().unwrap_or_else(|| DIRECT_FALLBACK_GROUP.to_string());
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
        leaf_progress.sort_by_key(|b| std::cmp::Reverse(b.missing));
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
