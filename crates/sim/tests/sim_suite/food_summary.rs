//! What a bite puts in the stomach, against the reference's own numbers.
//!
//! `corpus/items/food-summary.json` is `--dump-food-summary`: for all 1,686 comestibles, the four numbers `Character::consume_effects` computes before it hands the
//! food to the stomach (`src/consumption.cpp:1689-1714`) — the calories the body actually gets, the vitamins converted from RDA percentages, the water the food
//! carries, and the room its dry bulk takes up once its density and its calories per gram are accounted for. `stomach::ingest` (`src/stomach.cpp:392-398`) adds all
//! four unchanged, so this recording *is* the stomach's state after one bite of an empty stomach.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::similar_names,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]

use cdda_content::items::ItemType;
use cdda_content::vitamins::{self, Vitamin};
use cdda_json::Value;
use cdda_sim::food_summary::{
    Diet, ItemFacts, bite_volume, effective_kcal, food_summary, food_volume_ratio,
};
use std::collections::{BTreeMap, BTreeSet};

/// One recorded food: the inputs the port has to reach and the summary it has to produce.
struct Recorded {
    id: String,
    weight_g: i64,
    volume_ml: i64,
    count: i64,
    quench: i32,
    kcal: i32,
    water_ml: i64,
    solid_ml: i64,
    ratio: f64,
    solids_ml: i64,
    vitamins: BTreeMap<String, i32>,
}

fn recorded() -> Vec<Recorded> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/items/food-summary.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump holds types")
    };
    let number = |object: &cdda_json::Object, key: &str| match object.get(key) {
        Some(Value::Int(number)) => *number,
        Some(Value::Float(number)) => *number as i64,
        other => panic!("{key}: {other:?}"),
    };
    types
        .iter()
        .map(|entry| {
            let Value::Object(entry) = entry else {
                panic!("a type")
            };
            let id = match entry.get("id") {
                Some(Value::Str(id)) => id.clone(),
                other => panic!("{other:?}"),
            };
            let ratio = match entry.get("ratio") {
                Some(Value::Float(ratio)) => *ratio,
                Some(Value::Int(ratio)) => *ratio as f64,
                other => panic!("ratio: {other:?}"),
            };
            let vitamins = match entry.get("vitamins") {
                Some(Value::Object(map)) => map
                    .members()
                    .iter()
                    .map(|(id, amount)| match amount {
                        Value::Int(amount) => (id.clone(), i32::try_from(*amount).expect("units")),
                        other => panic!("{id}: {other:?}"),
                    })
                    .collect(),
                other => panic!("vitamins: {other:?}"),
            };
            Recorded {
                id,
                weight_g: number(entry, "weight_g"),
                volume_ml: number(entry, "volume_ml"),
                count: number(entry, "count"),
                quench: i32::try_from(number(entry, "quench")).expect("a quench"),
                kcal: i32::try_from(number(entry, "kcal")).expect("calories"),
                water_ml: number(entry, "water_ml"),
                solid_ml: number(entry, "solid_ml"),
                ratio,
                solids_ml: number(entry, "solids_ml"),
                vitamins,
            }
        })
        .collect()
}

#[test]
fn every_comestible_puts_in_the_stomach_what_the_reference_says() {
    let rows = recorded();
    assert_eq!(rows.len(), 1686, "every comestible");

    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [data.join("core"), data.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let items: BTreeMap<String, ItemType> = cdda_content::items::load_paths(&paths)
        .expect("the items load")
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();
    let vitamins_registry: BTreeMap<String, Vitamin> =
        vitamins::load_paths(&paths).expect("the vitamins load");

    let mut differing: Vec<String> = Vec::new();
    let mut with_water = 0;
    let mut with_vitamins = 0;
    let mut scaled_by_ratio = 0;
    for row in &rows {
        let item = items
            .get(&row.id)
            .unwrap_or_else(|| panic!("{} is in the corpus", row.id));
        let slot = item
            .comestible
            .as_ref()
            .unwrap_or_else(|| panic!("{} has a comestible slot", row.id));
        // The weight and volume the reference saw, from the port's own item model: a bite is one item, so the count divides nothing. The item model keeps milligrams,
        // and the reference's `units::to_gram` truncates, so this truncates too.
        let facts = ItemFacts {
            weight_g: item.item_weight(1) / 1_000,
            // A bite is one item, and a charge-counted item's volume is its type's divided between its stack size.
            volume_ml: bite_volume(
                item.volume,
                1,
                item.effective_stack_size(),
                item.counts_by_charge(),
            ),
            count: 1,
            quench: slot.quench,
            stated_kcal: slot.kcal,
            stated_vitamins: &slot.vitamins,
        };
        let flags: BTreeSet<String> = item.flags.clone();
        let summary = food_summary(&facts, &flags, 0.0, Diet::default(), &vitamins_registry);

        if facts.count != row.count || facts.quench != row.quench {
            differing.push(format!(
                "{}.inputs: the port says count {} quench {}, the reference says count {} quench {}",
                row.id, facts.count, facts.quench, row.count, row.quench
            ));
        }
        let ours_ratio = food_volume_ratio(
            effective_kcal(facts.stated_kcal, &flags, 0.0, Diet::default()),
            facts.weight_g / facts.count.max(1),
        );
        if (ours_ratio - row.ratio).abs() > 1e-9 {
            differing.push(format!(
                "{}.ratio: the port says {ours_ratio:.6}, the reference says {:.6}",
                row.id, row.ratio
            ));
        }
        if facts.weight_g != row.weight_g || facts.volume_ml != row.volume_ml {
            differing.push(format!(
                "{}.input: the port says {} g / {} mL, the reference says {} g / {} mL",
                row.id, facts.weight_g, facts.volume_ml, row.weight_g, row.volume_ml
            ));
        }
        if summary.water_ml != row.water_ml {
            differing.push(format!(
                "{}.water_ml: the port says {}, the reference says {}",
                row.id, summary.water_ml, row.water_ml
            ));
        }
        if summary.solids_ml != row.solids_ml {
            differing.push(format!(
                "{}.solids_ml: the port says {}, the reference says {}",
                row.id, summary.solids_ml, row.solids_ml
            ));
        }
        if summary.kcal() != i64::from(row.kcal) {
            differing.push(format!(
                "{}.kcal: the port says {}, the reference says {}",
                row.id,
                summary.kcal(),
                row.kcal
            ));
        }
        // The recording leaves zero entries out, and a derived all-zero map is the normal shape for a food that states none, so only the nonzero units are compared.
        let ours_vitamins: BTreeMap<String, i32> = summary
            .vitamins
            .iter()
            .filter(|(_, units)| **units != 0)
            .map(|(id, units)| (id.clone(), *units))
            .collect();
        if ours_vitamins != row.vitamins {
            differing.push(format!(
                "{}.vitamins: the port says {:?}, the reference says {:?}",
                row.id, ours_vitamins, row.vitamins
            ));
        }
        if row.water_ml != 0 {
            with_water += 1;
        }
        if !row.vitamins.is_empty() {
            with_vitamins += 1;
        }
        if row.solids_ml != row.solid_ml {
            scaled_by_ratio += 1;
        }
    }
    // **Non-vacuity**: the four calculations each have to move a real share of the corpus, or the comparison proves nothing about that step.
    assert!(with_water > 300, "water-carrying foods: {with_water}");
    assert!(with_vitamins > 400, "foods with vitamins: {with_vitamins}");
    assert!(
        scaled_by_ratio > 100,
        "foods whose bulk the ratio moved: {scaled_by_ratio}"
    );
    assert!(
        differing.is_empty(),
        "{} of {} comestibles disagree; the first twelve are {:#?}",
        differing.len(),
        rows.len(),
        differing.get(..12).unwrap_or(&differing)
    );
}
