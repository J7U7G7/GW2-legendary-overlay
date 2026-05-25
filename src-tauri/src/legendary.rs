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
pub struct MissingLeaf {
    pub kind: LeafKind,
    pub id: u32,
    pub name: String,
    pub needed: i64,
    pub owned: i64,
    pub missing: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct LegendaryProgress {
    pub collection_key: String,
    pub total_needed: i64,
    pub total_owned: i64,
    pub ratio: f64,
    pub leaves_total: usize,
    pub leaves_complete: usize,
    pub top_missing: Vec<MissingLeaf>,
}

/// Compute progress for one legendary against the user's owned items +
/// currencies. `top_n` caps the size of the returned `top_missing` list.
pub fn compute_progress(
    catalog: &RecipeCatalog,
    recipe: &LegendaryRecipe,
    owned_items: &HashMap<u32, i64>,
    owned_currencies: &HashMap<u32, i64>,
    top_n: usize,
) -> LegendaryProgress {
    let needs = aggregate_needs(catalog, recipe);

    let mut total_needed: i64 = 0;
    let mut total_owned_capped: i64 = 0;
    let mut leaves_complete = 0usize;
    let mut missing_list: Vec<MissingLeaf> = Vec::new();

    for leaf in needs.values() {
        let owned = match leaf.kind {
            LeafKind::Item => owned_items.get(&leaf.id).copied().unwrap_or(0),
            LeafKind::Currency => owned_currencies.get(&leaf.id).copied().unwrap_or(0),
        };
        let owned_capped = owned.min(leaf.needed);
        total_needed += leaf.needed;
        total_owned_capped += owned_capped;
        let missing = (leaf.needed - owned).max(0);
        if missing == 0 {
            leaves_complete += 1;
        } else {
            missing_list.push(MissingLeaf {
                kind: leaf.kind,
                id: leaf.id,
                name: leaf.name.clone(),
                needed: leaf.needed,
                owned,
                missing,
            });
        }
    }

    // Largest missing first — "where am I most blocked".
    missing_list.sort_by_key(|l| std::cmp::Reverse(l.missing));
    missing_list.truncate(top_n);

    let ratio = if total_needed > 0 {
        total_owned_capped as f64 / total_needed as f64
    } else {
        0.0
    };

    LegendaryProgress {
        collection_key: recipe.collection_key.clone(),
        total_needed,
        total_owned: total_owned_capped,
        ratio,
        leaves_total: needs.len(),
        leaves_complete,
        top_missing: missing_list,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat() -> RecipeCatalog {
        // Minimal in-memory catalog for unit tests, independent of the
        // shipped JSON. The embedded JSON is exercised separately by
        // `embedded_recipes_parse`.
        let mut components = HashMap::new();
        components.insert(
            "test_gift".to_string(),
            Component {
                name: "Test Gift".into(),
                description: None,
                leaves: vec![
                    RecipeLeaf {
                        kind: LeafKind::Item,
                        id: 100,
                        quantity: 250,
                        name: "Ecto".into(),
                        notes: None,
                    },
                    RecipeLeaf {
                        kind: LeafKind::Currency,
                        id: 1,
                        quantity: 50000,
                        name: "Coin".into(),
                        notes: None,
                    },
                ],
            },
        );
        RecipeCatalog {
            meta: serde_json::Value::Null,
            components,
            legendaries: vec![LegendaryRecipe {
                collection_key: "bolt".into(),
                components: vec!["test_gift".into()],
                leaves: vec![RecipeLeaf {
                    kind: LeafKind::Item,
                    id: 200,
                    quantity: 1,
                    name: "Zap".into(),
                    notes: None,
                }],
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
    fn progress_caps_owned_at_needed_and_lists_missing() {
        let c = cat();
        let r = &c.legendaries[0];
        let owned_items: HashMap<u32, i64> = [(100, 999), (200, 0)].into_iter().collect();
        let owned_currencies: HashMap<u32, i64> = [(1, 25000)].into_iter().collect();

        let p = compute_progress(&c, r, &owned_items, &owned_currencies, 5);
        // ecto: 999 owned, 250 needed → capped at 250
        // coin: 25000 owned, 50000 needed → 25000 capped (still missing 25000)
        // zap: 0 owned, 1 needed → missing 1
        assert_eq!(p.total_needed, 250 + 50000 + 1);
        assert_eq!(p.total_owned, 250 + 25000);
        assert_eq!(p.leaves_complete, 1, "only ecto is complete");
        assert_eq!(p.top_missing.len(), 2);
        // largest missing first → coin (25000) before zap (1)
        assert_eq!(p.top_missing[0].id, 1);
        assert_eq!(p.top_missing[0].missing, 25000);
        assert_eq!(p.top_missing[1].id, 200);
        assert_eq!(p.top_missing[1].missing, 1);
    }

    #[test]
    fn progress_truncates_missing_to_top_n() {
        let mut components = HashMap::new();
        components.insert(
            "big".into(),
            Component {
                name: "Big".into(),
                description: None,
                leaves: (1..=10)
                    .map(|i| RecipeLeaf {
                        kind: LeafKind::Item,
                        id: 1000 + i,
                        quantity: 100,
                        name: format!("Item {i}"),
                        notes: None,
                    })
                    .collect(),
            },
        );
        let cat = RecipeCatalog {
            meta: serde_json::Value::Null,
            components,
            legendaries: vec![LegendaryRecipe {
                collection_key: "x".into(),
                components: vec!["big".into()],
                leaves: vec![],
                notes: None,
            }],
        };
        let p = compute_progress(
            &cat,
            &cat.legendaries[0],
            &HashMap::new(),
            &HashMap::new(),
            5,
        );
        assert_eq!(p.top_missing.len(), 5);
        assert_eq!(p.leaves_total, 10);
        assert_eq!(p.leaves_complete, 0);
    }

    #[test]
    fn embedded_recipes_parse() {
        let cat = load().expect("legendary_recipes.json should parse");
        assert!(!cat.components.is_empty(), "expected at least one component");
        assert!(!cat.legendaries.is_empty(), "expected at least one legendary");
        // Sanity: every legendary's referenced components exist.
        for rec in &cat.legendaries {
            for k in &rec.components {
                assert!(
                    cat.components.contains_key(k),
                    "legendary {} references unknown component {k}",
                    rec.collection_key
                );
            }
        }
    }
}
