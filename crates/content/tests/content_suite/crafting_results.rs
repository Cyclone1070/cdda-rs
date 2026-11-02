//! What a recipe makes, checked against the reference's own products.
//!
//! Reference: `recipe::create_results` and `recipe::create_byproducts` (`recipe.cpp:737-900`). The recipe dump carries the **folded** number one batch makes and the
//! byproducts it leaves; the crafting dump asks the reference for the products of a batch of one and two. The two must agree with the port's model of both rules -
//! `made_per_batch` and `byproducts` - for every recipe in the data, which is the only way to know that reading them was right.

#![allow(clippy::panic, clippy::expect_used)]

use std::collections::BTreeMap;

fn corpus(name: &str) -> std::sync::Arc<cdda_json::Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus")
        .join(name);
    cdda_content::read_json(&path).unwrap_or_else(|| panic!("corpus/{name} is in the repository"))
}

/// Everything the port thinks a recipe makes for one batch and for two, from the loaded recipe itself.
fn port_products(recipe: &cdda_content::recipes::Recipe, batch: i32) -> (i32, Vec<(String, i32)>) {
    let batch = batch.max(1);
    let made = recipe.made_per_batch.max(1).saturating_mul(batch);
    let byproducts = recipe
        .byproducts
        .iter()
        .map(|(item, count)| (item.clone(), count.saturating_mul(batch)))
        .collect();
    (made, byproducts)
}

#[test]
fn the_loaded_recipes_make_what_the_reference_makes() {
    let recipes_dump = corpus("recipes.json");
    let (recipes, count) = cdda_content::recipes::from_dump(&recipes_dump).expect("the recipe dump reads");
    assert_eq!(recipes.len(), count, "the dump agrees with its own count");
    let by_id: BTreeMap<&str, &cdda_content::recipes::Recipe> =
        recipes.iter().map(|recipe| (recipe.id.as_str(), recipe)).collect();
    let products = corpus("crafting-results.json");
    let cdda_json::Value::Object(root) = products.as_ref() else {
        panic!("the crafting dump is an object");
    };
    let Some(cdda_json::Value::Array(entries)) = root.get("recipes") else {
        panic!("the crafting dump holds a recipe list");
    };
    let mut checked = 0;
    let mut wrapped_results = 0;
    let mut varied_byproducts = 0;
    let mut grouped_byproducts = 0;
    let mut failures = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(record) = entry else {
            continue;
        };
        let Some(cdda_json::Value::Str(id)) = record.get("id") else {
            continue;
        };
        let Some(recipe) = by_id.get(id.as_str()) else {
            continue;
        };
        let Some(cdda_json::Value::Array(batches)) = record.get("results") else {
            continue;
        };
        for one in batches {
            let cdda_json::Value::Object(batch) = one else {
                continue;
            };
            let Some(cdda_json::Value::Int(size)) = batch.get("batch") else {
                continue;
            };
            let Ok(size) = i32::try_from(*size) else {
                continue;
            };
            let (made, byproducts) = port_products(recipe, size);
            let wanted = folded(batch.get("made"));
            let wanted_spare = folded(batch.get("byproducts"));
            checked += 1;
            let mut mine = BTreeMap::new();
            if made > 0 {
                mine.insert(recipe.result.clone(), made);
            }
            if mine != wanted {
                // The reference wraps a `contained` result in its container - `can_beans` comes out inside a `can_medium` (`create_result`, `recipe.cpp:768-771`) -
                // and the port does not model containers yet. That is a **known hole, counted rather than ignored**: anything else disagreeing is a real failure.
                let wrapped = wanted.len() == 1
                    && wanted.values().next().copied() == Some(made)
                    && !wanted.contains_key(&recipe.result);
                if wrapped {
                    wrapped_results += 1;
                } else {
                    failures.push(format!("{} batch {size}: makes {mine:?}, the reference {wanted:?}", recipe.id));
                }
            }
            let spare: BTreeMap<String, i32> = byproducts.iter().cloned().collect();
            if spare != wanted_spare {
                // **`byproduct_group`** is an inline item group whose counts are ranges - `{"item": "arrow_wood_heavy", "count": [0, 4]}` - so the reference's answer
                // is one draw per batch and the loader reads no group at all. That is why its own answer is a listed count or nothing. **Known hole, counted**:
                // reading the inline group needs the group model at load time and a draw at craft time.
                let same_items = spare.keys().eq(wanted_spare.keys());
                if spare.is_empty() && !wanted_spare.is_empty() {
                    grouped_byproducts += 1;
                } else if same_items {
                    varied_byproducts += 1;
                } else {
                    failures.push(format!(
                        "{} batch {size}: leaves {spare:?}, the reference {wanted_spare:?}",
                        recipe.id
                    ));
                }
            }
        }
    }
    assert!(checked > 10_000, "the comparison saw {checked} batches, not a handful");
    // The container hole has a number, so closing it is a change to one line rather than a discovery.
    assert_eq!(
        grouped_byproducts, 5,
        "the batches whose byproducts are drawn from an inline `byproduct_group` the loader does not read"
    );
    assert_eq!(
        varied_byproducts, 9,
        "the batches whose byproducts the reference draws from a group"
    );
    assert_eq!(
        wrapped_results, 302,
        "the batches whose result the reference puts in a container"
    );
    assert!(
        failures.is_empty(),
        "{} of {checked} batches disagree, first few: {:?}",
        failures.len(),
        failures.iter().take(6).collect::<Vec<_>>()
    );
}

/// A dump list of `[item, count]` pairs, folded into a map.
fn folded(value: Option<&cdda_json::Value>) -> BTreeMap<String, i32> {
    let mut out = BTreeMap::new();
    let Some(cdda_json::Value::Array(pairs)) = value else {
        return out;
    };
    for pair in pairs {
        let cdda_json::Value::Array(parts) = pair else {
            continue;
        };
        let (Some(cdda_json::Value::Str(item)), Some(cdda_json::Value::Int(count))) =
            (parts.first(), parts.get(1))
        else {
            continue;
        };
        if let Ok(count) = i32::try_from(*count) {
            *out.entry(item.clone()).or_insert(0) += count;
        }
    }
    out
}
