//! The stomach emptying itself, against a recorded trace of every comestible in both compartments.
//!
//! `corpus/items/digest.json` is `--dump-digest`: for all 1,686 comestibles, a bite put into a 1,500 mL stomach and a 1,500 mL set of guts, and then **six**
//! half-hour digests of each, at a fixed metabolic pressure of 2,500 kcal a day and a hunger of one. Each step records what left (water, solids, calories), what the
//! compartment still holds (total volume, water, kilocalories), and the vitamins that left in the first step and over all six.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::similar_names,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]

use cdda_content::vitamins::Vitamin;
use cdda_json::Value;
use cdda_sim::digest::{DigestRates, MetabolicRates, StomachContents, digest_rates};
use cdda_sim::food_summary::FoodSummary;
use std::collections::BTreeMap;

/// One recorded compartment trace.
struct Trace {
    capacity_ml: i64,
    /// Per step: water out, solids out, calories out, volume left, water left, kilocalories left.
    steps: Vec<[i64; 6]>,
    vitamins_first: BTreeMap<String, i32>,
    vitamins_total: BTreeMap<String, i32>,
}

/// One recorded food.
struct Recorded {
    id: String,
    ingested: FoodSummary,
    stomach: Trace,
    guts: Trace,
}

fn integer(object: &cdda_json::Object, key: &str) -> i64 {
    match object.get(key) {
        Some(Value::Int(number)) => *number,
        Some(Value::Float(number)) => *number as i64,
        other => panic!("{key}: {other:?}"),
    }
}

fn vitamin_map(value: Option<&Value>) -> BTreeMap<String, i32> {
    match value {
        Some(Value::Object(map)) => map
            .members()
            .iter()
            .map(|(id, amount)| match amount {
                Value::Int(amount) => (id.clone(), i32::try_from(*amount).expect("units")),
                other => panic!("{id}: {other:?}"),
            })
            .collect(),
        other => panic!("vitamins: {other:?}"),
    }
}

fn trace(value: &Value) -> Trace {
    let Value::Object(object) = value else { panic!("a trace") };
    let Some(Value::Array(steps)) = object.get("steps") else {
        panic!("steps")
    };
    Trace {
        capacity_ml: integer(object, "capacity_ml"),
        steps: steps
            .iter()
            .map(|step| {
                let Value::Array(numbers) = step else { panic!("a step") };
                let mut row = [0_i64; 6];
                for (slot, number) in row.iter_mut().zip(numbers) {
                    let Value::Int(number) = number else { panic!("a number") };
                    *slot = *number;
                }
                row
            })
            .collect(),
        vitamins_first: vitamin_map(object.get("vitamins_first")),
        vitamins_total: vitamin_map(object.get("vitamins_total")),
    }
}

fn recorded() -> (MetabolicRates, Vec<Recorded>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/digest.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let Some(Value::Object(rates)) = root.get("rates") else {
        panic!("rates")
    };
    let float = |object: &cdda_json::Object, key: &str| match object.get(key) {
        Some(Value::Float(number)) => *number as f32,
        Some(Value::Int(number)) => *number as f32,
        other => panic!("{key}: {other:?}"),
    };
    let metabolic = MetabolicRates {
        kcal: float(rates, "kcal"),
        hunger: float(rates, "hunger"),
        thirst: float(rates, "thirst"),
        sleepiness: float(rates, "sleepiness"),
    };
    let Some(Value::Array(foods)) = root.get("foods") else {
        panic!("foods")
    };
    let foods = foods
        .iter()
        .map(|food| {
            let Value::Object(food) = food else { panic!("a food") };
            let id = match food.get("id") {
                Some(Value::Str(id)) => id.clone(),
                other => panic!("{other:?}"),
            };
            let Some(Value::Object(ingested)) = food.get("ingested") else {
                panic!("ingested")
            };
            Recorded {
                id,
                ingested: FoodSummary {
                    water_ml: integer(ingested, "water_ml"),
                    solids_ml: integer(ingested, "solids_ml"),
                    calories: integer(ingested, "calories"),
                    vitamins: vitamin_map(ingested.get("vitamins")),
                },
                stomach: trace(food.get("stomach").expect("a stomach")),
                guts: trace(food.get("guts").expect("guts")),
            }
        })
        .collect();
    (metabolic, foods)
}

/// Runs the port's six steps for one compartment and compares every number.
fn compare(
    row: &Recorded,
    trace: &Trace,
    rates: &DigestRates,
    is_stomach: bool,
    vitamins: &BTreeMap<String, Vitamin>,
    label: &str,
    differing: &mut Vec<String>,
) {
    let mut contents = StomachContents::new(1_500, is_stomach);
    contents.ingest(&row.ingested);
    if contents.capacity_ml() != trace.capacity_ml {
        differing.push(format!(
            "{}.{label}.capacity: the port says {}, the reference says {}",
            row.id,
            contents.capacity_ml(),
            trace.capacity_ml
        ));
    }
    let mut total: BTreeMap<String, i32> = BTreeMap::new();
    let mut first: BTreeMap<String, i32> = BTreeMap::new();
    for (step, expected) in trace.steps.iter().enumerate() {
        let digested = contents.digest(rates, 1, 1, vitamins);
        if step == 0 {
            first.clone_from(&digested.vitamins);
        }
        for (id, units) in &digested.vitamins {
            *total.entry(id.clone()).or_insert(0) += units;
        }
        let ours = [
            digested.water_ml,
            digested.solids_ml,
            digested.calories,
            contents.contains_ml(),
            contents.water_ml,
            contents.kcal(),
        ];
        if ours != *expected {
            differing.push(format!(
                "{}.{label}.step{step}: the port says {ours:?}, the reference says {expected:?}",
                row.id
            ));
        }
    }
    let nonzero = |map: &BTreeMap<String, i32>| -> BTreeMap<String, i32> {
        map.iter()
            .filter(|(_, units)| **units != 0)
            .map(|(id, units)| (id.clone(), *units))
            .collect()
    };
    if nonzero(&first) != trace.vitamins_first {
        differing.push(format!(
            "{}.{label}.vitamins_first: the port says {:?}, the reference says {:?}",
            row.id,
            nonzero(&first),
            trace.vitamins_first
        ));
    }
    if nonzero(&total) != trace.vitamins_total {
        differing.push(format!(
            "{}.{label}.vitamins_total: the port says {:?}, the reference says {:?}",
            row.id,
            nonzero(&total),
            trace.vitamins_total
        ));
    }
}

#[test]
fn every_comestible_empties_itself_the_way_the_reference_says() {
    let (metabolic, rows) = recorded();
    assert_eq!(rows.len(), 1686, "every comestible");

    // The vitamins, so a drug one can skip the trickle. The recording's own foods are enough: none of them holds a vitamin the registry lacks.
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [data.join("core"), data.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let vitamins: BTreeMap<String, Vitamin> = cdda_content::vitamins::load_paths(&paths).expect("vitamins");

    let stomach_rates = digest_rates(&metabolic, true, 1_500);
    let guts_rates = digest_rates(&metabolic, false, 1_500);
    // The two rules are not the same rule: a stomach's solid rate is a sixth of its capacity and its minimum calorie release is a flat 5,000 thousandths, while the
    // guts' minimum is the daily intake spread over the day. If these two ever agree, the gate has stopped proving which branch ran.
    assert_eq!(stomach_rates.solids_ml, 250, "a sixth of 1,500 mL");
    assert_eq!(guts_rates.solids_ml, 250, "a flat 250 mL");
    assert_eq!(stomach_rates.min_calories, 5_000);
    assert_eq!(guts_rates.min_calories, 104_166, "2,500 kcal / 24 * 1 * 1000, floored");

    let mut differing: Vec<String> = Vec::new();
    let mut calories_left_the_guts = 0;
    let mut with_vitamins = 0;
    let mut drug_vitamins = 0;
    let mut drying_foods = 0;
    let mut at_the_minimum = 0;
    let drug = ["vit_betablocker", "vit_bloodthinner", "vit_ibuprofen", "vit_mme"];
    for row in &rows {
        compare(row, &row.stomach, &stomach_rates, true, &vitamins, "stomach", &mut differing);
        compare(row, &row.guts, &guts_rates, false, &vitamins, "guts", &mut differing);
        if row.guts.steps.iter().any(|step| step[2] > 0) {
            calories_left_the_guts += 1;
        }
        if !row.stomach.vitamins_total.is_empty() {
            with_vitamins += 1;
        }
        if row.ingested.vitamins.keys().any(|id| drug.contains(&id.as_str())) {
            drug_vitamins += 1;
        }
        if row.ingested.water_ml < 0 {
            drying_foods += 1;
        }
        if row.stomach.steps.iter().any(|step| step[2] == stomach_rates.min_calories) {
            at_the_minimum += 1;
        }
    }
    // **Non-vacuity**: a compartment that released nothing would pass a comparison against itself.
    assert!(
        calories_left_the_guts > 1_000,
        "foods that gave the guts calories: {calories_left_the_guts}"
    );
    assert!(with_vitamins > 1_300, "foods that released vitamins: {with_vitamins}");
    // The three branches a happy-path comparison would never reach.
    assert_eq!(drug_vitamins, 12, "foods carrying a drug vitamin, which skips the trickle");
    assert!(drying_foods > 300, "foods that take water back out: {drying_foods}");
    assert!(at_the_minimum > 400, "steps that released the flat minimum: {at_the_minimum}");
    assert!(
        differing.is_empty(),
        "{} disagreements over {} comestibles; the first twelve are {:#?}",
        differing.len(),
        rows.len(),
        differing.get(..12).unwrap_or(&differing)
    );
}

/// The two paths the recording cannot reach, because every one of its steps is a single five-minute and a single half-hour interval.
///
/// Reference: `stomach_contents::digest` (`src/stomach.cpp:400-437`). Water leaves in five-minute steps, so the count multiplies its rate; and a call with **no**
/// half hours must move water only and return before it touches the solids or the calories — that is the path a character on a short tick takes.
#[test]
fn water_moves_in_five_minute_steps_and_a_water_only_tick_touches_nothing_else() {
    let vitamins = BTreeMap::new();
    let rates = DigestRates {
        solids_ml: 250,
        water_ml: 250,
        min_vitamin: 1,
        percent_vitamin: 1.0 / 6.0,
        min_calories: 5_000,
        percent_kcal: 1.0 / 6.0,
    };
    let full = FoodSummary {
        water_ml: 1_000,
        solids_ml: 400,
        calories: 300_000,
        vitamins: BTreeMap::new(),
    };

    // Three five-minute intervals move three times the water rate, and the half hour moves a sixth of the solids.
    let mut contents = StomachContents::new(1_500, true);
    contents.ingest(&full);
    let digested = contents.digest(&rates, 3, 1, &vitamins);
    assert_eq!(digested.water_ml, 750, "three five-minute steps of 250 mL");
    assert_eq!(digested.solids_ml, 250);
    // 400 mL of solids left after a sixth of them moved, and 250 mL of water after three five-minute steps.
    assert_eq!(contents.contains_ml(), 150 + 250);

    // A water-only tick returns before it settles the solids or the calories.
    let mut contents = StomachContents::new(1_500, true);
    contents.ingest(&full);
    let digested = contents.digest(&rates, 1, 0, &vitamins);
    assert_eq!(digested.water_ml, 250);
    assert_eq!(digested.solids_ml, 0, "no half hour has gone by");
    assert_eq!(digested.calories, 0, "no half hour has gone by");
    assert_eq!(contents.calories, 300_000, "the calories have not been touched");
    assert_eq!(contents.contents_ml, 400, "the solids have not been touched");
}
