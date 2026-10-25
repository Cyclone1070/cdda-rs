//! `fun_for` against the reference's own numbers, for every comestible and every branch.
//!
//! `corpus/items/fun-for.json` is `--dump-fun-for`: for each of the 1,686 comestibles, the pair
//! `Character::fun_for` returns in twenty-two eater states (`src/consumption.cpp:483`). The transcribed upstream cases next door are inequalities because the
//! reference had no recording; this is the recording, and it settles what an inequality cannot — that a cold cola keeps the ceiling it had warm, that a rotten
//! garlic still costs only its own stated fun to hide, that a saprophage's rotten food tastes exactly as good as it did fresh.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::similar_names,
    clippy::redundant_closure_for_method_calls
)]

use cdda_content::items::ItemType;
use cdda_sim::fun_for::{Eater, Food, comestible_fun, fun_for};
use cdda_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// One row of the sweep, mirrored from the harness's `fun_configurations`.
struct Configuration {
    name: &'static str,
    trait_id: Option<&'static str>,
    effect_id: Option<&'static str>,
    item_flag: Option<&'static str>,
    relative_rot: f32,
    bionic_id: Option<&'static str>,
    bionic_active: bool,
    power_kj: i64,
}

const CONFIGURATIONS: [Configuration; 22] = [
    Configuration { name: "plain", trait_id: None, effect_id: None, item_flag: None, relative_rot: 0.0, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "sick_cold", trait_id: None, effect_id: Some("common_cold"), item_flag: None, relative_rot: 0.0, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "sick_flu", trait_id: None, effect_id: Some("flu"), item_flag: None, relative_rot: 0.0, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "item_cold", trait_id: None, effect_id: None, item_flag: Some("COLD"), relative_rot: 0.0, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "item_frozen", trait_id: None, effect_id: None, item_flag: Some("FROZEN"), relative_rot: 0.0, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "rotten", trait_id: None, effect_id: None, item_flag: None, relative_rot: 1.5, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "rotten_saprophage", trait_id: Some("SAPROPHAGE"), effect_id: None, item_flag: None, relative_rot: 1.5, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "rotten_saprovore", trait_id: Some("SAPROVORE"), effect_id: None, item_flag: None, relative_rot: 1.5, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "gourmand", trait_id: Some("GOURMAND"), effect_id: None, item_flag: None, relative_rot: 0.0, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "thresh_feline", trait_id: Some("THRESH_FELINE"), effect_id: None, item_flag: None, relative_rot: 0.0, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "thresh_lupine", trait_id: Some("THRESH_LUPINE"), effect_id: None, item_flag: None, relative_rot: 0.0, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "thresh_cattle", trait_id: Some("THRESH_CATTLE"), effect_id: None, item_flag: None, relative_rot: 0.0, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "thresh_rabbit", trait_id: Some("THRESH_RABBIT"), effect_id: None, item_flag: None, relative_rot: 0.0, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "thresh_mouse", trait_id: Some("THRESH_MOUSE"), effect_id: None, item_flag: None, relative_rot: 0.0, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "thresh_rat", trait_id: Some("THRESH_RAT"), effect_id: None, item_flag: None, relative_rot: 0.0, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "thresh_bird", trait_id: Some("THRESH_BIRD"), effect_id: None, item_flag: None, relative_rot: 0.0, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "sick_gourmand", trait_id: Some("GOURMAND"), effect_id: Some("common_cold"), item_flag: None, relative_rot: 0.0, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "cold_item_cold_eater", trait_id: None, effect_id: Some("common_cold"), item_flag: Some("COLD"), relative_rot: 0.0, bionic_id: None, bionic_active: false, power_kj: 0 },
    Configuration { name: "grossfood", trait_id: None, effect_id: None, item_flag: None, relative_rot: 0.0, bionic_id: Some("bio_faulty_grossfood"), bionic_active: false, power_kj: 0 },
    Configuration { name: "taste_blocker_10kj", trait_id: None, effect_id: None, item_flag: None, relative_rot: 0.0, bionic_id: Some("bio_taste_blocker"), bionic_active: true, power_kj: 10 },
    Configuration { name: "taste_blocker_20kj", trait_id: None, effect_id: None, item_flag: None, relative_rot: 0.0, bionic_id: Some("bio_taste_blocker"), bionic_active: true, power_kj: 20 },
    Configuration { name: "taste_blocker_rotten_20kj", trait_id: None, effect_id: None, item_flag: None, relative_rot: 1.5, bionic_id: Some("bio_taste_blocker"), bionic_active: true, power_kj: 20 },
];

/// One recorded food: its id, the fun its data states, the fun the *item* is worth, where its rot lands, whether it is food, and the pairs.
struct Recorded {
    id: String,
    stated_fun: i32,
    item_fun: i32,
    rot_set: f32,
    is_food: bool,
    pairs: Vec<(i32, i32)>,
}

fn recorded() -> (Vec<String>, Vec<Recorded>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/fun-for.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(names)) = root.get("configurations") else {
        panic!("the dump names its configurations")
    };
    let configurations = names
        .iter()
        .map(|name| match name {
            Value::Str(name) => name.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    let Some(Value::Array(foods)) = root.get("foods") else {
        panic!("the dump holds foods")
    };
    let rows = foods
        .iter()
        .map(|food| {
            let Value::Object(food) = food else { panic!("a food") };
            let id = match food.get("id") {
                Some(Value::Str(id)) => id.clone(),
                other => panic!("{other:?}"),
            };
            let number = |key: &str| match food.get(key) {
                Some(Value::Int(fun)) => i32::try_from(*fun).expect("a number"),
                Some(Value::Float(number)) => *number as i32,
                other => panic!("{key}: {other:?}"),
            };
            let rot_set = match food.get("rot_set") {
                Some(Value::Float(rot)) => *rot as f32,
                Some(Value::Int(rot)) => *rot as f32,
                other => panic!("rot_set: {other:?}"),
            };
            let is_food = match food.get("is_food") {
                Some(Value::Bool(is_food)) => *is_food,
                other => panic!("{other:?}"),
            };
            let Some(Value::Array(pairs)) = food.get("pairs") else {
                panic!("pairs")
            };
            let pairs = pairs
                .iter()
                .map(|pair| {
                    let Value::Array(pair) = pair else { panic!("a pair") };
                    let [Value::Int(fun), Value::Int(fun_max)] = pair.as_slice() else {
                        panic!("two numbers")
                    };
                    (
                        i32::try_from(*fun).expect("a fun value"),
                        i32::try_from(*fun_max).expect("a ceiling"),
                    )
                })
                .collect();
            Recorded {
                id,
                stated_fun: number("fun"),
                item_fun: number("item_fun"),
                rot_set,
                is_food,
                pairs,
            }
        })
        .collect();
    (configurations, rows)
}

#[test]
fn every_comestible_is_worth_what_the_reference_says_to_every_eater() {
    let (names, rows) = recorded();
    assert_eq!(names.len(), 22, "twenty-two eater states");
    assert_eq!(rows.len(), 1686, "and every comestible");

    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [data.join("core"), data.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let items: BTreeMap<String, ItemType> = cdda_content::items::load_paths(&paths)
        .expect("the items load")
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();

    let mut differing: Vec<String> = Vec::new();
    let mut per_configuration: BTreeMap<&str, usize> = BTreeMap::new();
    let mut moved_by_rot = 0;
    let mut moved_by_gourmand = 0;
    let flag_data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let flag_dirs = [flag_data.clone()];
    let flag_paths: Vec<&std::path::Path> = flag_dirs.iter().map(std::path::PathBuf::as_path).collect();
    let taste_mods = cdda_content::flags::load_paths(&flag_paths).expect("the flags load");
    let taste_mod = |flag: &str| taste_mods.get(flag).map_or(0, |flag| flag.taste_mod);

    for row in &rows {
        let Recorded { id, stated_fun, item_fun, rot_set, is_food, pairs } = row;
        let item = items.get(id).unwrap_or_else(|| panic!("{id} is in the corpus"));
        let slot = item
            .comestible
            .as_ref()
            .unwrap_or_else(|| panic!("{id} has a comestible slot"));
        assert_eq!(slot.fun, *stated_fun, "{id}: the recording and the content agree on its stated fun");
        // The item's own taste, which is the input `fun_for` actually takes: the stated fun plus every flag's `taste_mod`, floored at -5 once mushy.
        let ours_item_fun = comestible_fun(*stated_fun, &item.flags, taste_mod);
        if ours_item_fun != *item_fun {
            differing.push(format!(
                "{id}: the port says its item fun is {ours_item_fun}, the reference says {item_fun}"
            ));
            *per_configuration.entry("item_fun").or_default() += 1;
        }
        for (index, configuration) in CONFIGURATIONS.iter().enumerate() {
            let mut food_flags: BTreeSet<String> = item.flags.clone();
            if let Some(flag) = configuration.item_flag {
                food_flags.insert(flag.to_owned());
            }
            let food = Food {
                type_id: id,
                fun: *item_fun,
                monotony_penalty: slot.monotony_penalty,
                flags: &food_flags,
                // A food that does not spoil has **no** rot, whatever `set_relative_rot` was asked for, so the recording says where it actually landed.
                relative_rot: if configuration.relative_rot > 0.0 { *rot_set } else { 0.0 },
                component_hash: 0,
                is_food: *is_food,
            };
            let traits: BTreeSet<String> = configuration.trait_id.map(|id| id.to_owned()).into_iter().collect();
            let effects: BTreeSet<String> = configuration.effect_id.map(|id| id.to_owned()).into_iter().collect();
            let bionics: BTreeSet<String> =
                configuration.bionic_id.map(|id| id.to_owned()).into_iter().collect();
            let active: BTreeSet<String> = if configuration.bionic_active {
                bionics.clone()
            } else {
                BTreeSet::new()
            };
            let no_flags = BTreeSet::new();
            let eater = Eater {
                effects: &effects,
                traits: &traits,
                flags: &no_flags,
                active_bionics: &active,
                bionics: &bionics,
                power_kj: configuration.power_kj,
                history: &[],
                now: 0,
            };
            let ours = fun_for(&food, &eater);
            let theirs = *pairs.get(index).unwrap_or_else(|| panic!("{id}: every configuration has a pair"));
            if ours != theirs {
                *per_configuration.entry(configuration.name).or_default() += 1;
                differing.push(format!("{id}/{name}: the port says {ours:?}, the reference says {theirs:?}", name = configuration.name));
            }
            if index == 5 && Some(&theirs) != pairs.first() {
                moved_by_rot += 1;
            }
            if index == 8 && Some(&theirs) != pairs.first() {
                moved_by_gourmand += 1;
            }
        }
    }
    if std::env::var("FUN_FOR_DEBUG").is_ok() {
        let plain: Vec<&String> = differing.iter().filter(|line| line.contains("/plain:")).collect();
        eprintln!("plain: {plain:#?}");
    }
    // **Non-vacuity**: a port that returned the stated fun untouched would still match on most rows, so the counts that matter are the ones where a branch moved the
    // answer. Rot moves 224 foods and the gourmand 1,256; if either fell to zero the comparison would be proving nothing about that branch.
    assert!(moved_by_rot > 200, "rot changes the answer for {moved_by_rot} foods");
    assert!(moved_by_gourmand > 1200, "and the gourmand for {moved_by_gourmand}");
    assert!(
        differing.is_empty(),
        "{} of {} pairs disagree, by configuration {per_configuration:?}; the first twelve are {:#?}",
        differing.len(),
        rows.len() * CONFIGURATIONS.len(),
        differing.get(..12).unwrap_or(&differing)
    );
}
