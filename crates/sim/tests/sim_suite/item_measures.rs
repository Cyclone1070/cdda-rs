//! Every item type's weight and volume, replayed against the reference's own answers.
//!
//! `corpus/items/measures.json` is `--dump-item-measures`: for each of the 9,912 types, the reference constructs an item and asks it four questions through its own
//! functions - `item::weight`, `item::weight( integral )`, `item::volume`, `item::volume( integral )`. The prototype dump next door says what the **data** is; this says
//! what the **rules** make of it, which is the difference between a loader that copies numbers and a port that computes them.
//!
//! Two facts come from the prototypes (`corpus/prototypes/items.json`): the weight, the integral weight, the volume, and whether the type is counted in charges with what
//! stack size. The charges come from the measures dump, because the reference constructed the item with the type's own default.
//!
//! **What is compared and what is declared.** The port models: the base weight, the integral weight, the `NO_DROP` zero, the charge multiplication for weight, and the
//! charge/stack-size division for volume. It does not model corpses, crafted items, gun mods, per-item overrides or a container's own contents modifier, so the types that
//! need them are expected to differ - and the test says **which**, by name, so a new difference cannot hide among the declared ones.

#![allow(clippy::expect_used, clippy::panic, clippy::cast_possible_truncation)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    ItemTypeId, TypeVolume, TypeWeight, drop_item, item_volume, item_volume_integral, item_weight,
    item_weight_integral,
};
use std::collections::BTreeMap;

/// What the reference answered for one type, in milligrams and millilitres.
struct Measured {
    id: String,
    charges: i32,
    weight: i64,
    integral_weight: i64,
    volume: i64,
    integral_volume: i64,
}

/// What the prototypes say about one type.
#[derive(Clone, Copy)]
struct Facts {
    weight_mg: i64,
    integral_weight_mg: i64,
    volume: i64,
    integral_volume: i64,
    count_by_charges: bool,
    stack_size: i64,
    no_drop: bool,
}

fn dumps() -> (Vec<Measured>, BTreeMap<String, Facts>) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let measures = read_array(&root.join("corpus/items/measures.json"), "types");
    let prototypes = read_array(&root.join("corpus/prototypes/items.json"), "prototypes");
    let mut prototypes_by_id = BTreeMap::new();
    for entry in &prototypes {
        let id = text(entry, "id");
        prototypes_by_id.insert(
            id.clone(),
            Facts {
                weight_mg: whole(entry, "weight_mg"),
                integral_weight_mg: whole(entry, "integral_weight_mg"),
                volume: whole(entry, "volume"),
                integral_volume: whole(entry, "integral_volume"),
                count_by_charges: matches!(
                    entry.get("count_by_charges"),
                    Some(cdda_json::Value::Bool(true))
                ),
                stack_size: whole(entry, "stack_size"),
                no_drop: is_no_drop(entry),
            },
        );
    }
    let answers = measures
        .iter()
        .map(|entry| Measured {
            id: text(entry, "id"),
            charges: i32::try_from(whole(entry, "charges")).expect("charges"),
            weight: whole(entry, "weight"),
            integral_weight: whole(entry, "integral_weight"),
            volume: whole(entry, "volume"),
            integral_volume: whole(entry, "integral_volume"),
        })
        .collect();
    (answers, prototypes_by_id)
}

/// Asks the port the same four questions the reference was asked, and says which types disagree.
fn measure_all(measured: &[Measured], facts: &BTreeMap<String, Facts>) -> (usize, Vec<String>) {
    let mut world = World::new();
    let mut disagreements: Vec<String> = Vec::new();
    let mut compared = 0_usize;
    for (index, row) in measured.iter().enumerate() {
        let Some(facts_of_type) = facts.get(&row.id) else {
            continue;
        };
        let kind = ItemTypeId(u32::try_from(index).expect("an index"));
        let entity = drop_item(&mut world, (0, 0), kind, row.charges);
        let facts_table = *facts_of_type;
        // **Grams out, milligrams in.** The reference reports grams and counts milligrams - its `to_gram` truncates - and the port does the same, so the input has to be
        // milligrams (a 135 mg unit weight is invisible in grams) and the answer is compared as the reference prints it.
        let facts_of_type = |_: ItemTypeId| TypeWeight {
            milligrams: facts_table.weight_mg,
            integral_milligrams: facts_table.integral_weight_mg,
            count_by_charges: facts_table.count_by_charges,
            no_drop: facts_table.no_drop,
        };
        let weight = item_weight(&mut world, entity, facts_of_type);
        let integral = item_weight_integral(&mut world, entity, facts_of_type);
        let volume_facts = |_: ItemTypeId| TypeVolume {
            millilitres: facts_table.volume,
            integral_millilitres: facts_table.integral_volume,
            count_by_charges: facts_table.count_by_charges,
            stack_size: facts_table.stack_size,
        };
        let volume = item_volume(&mut world, entity, volume_facts);
        // **The fourth number.** `item::volume( integral: true )`, which 120 types answer differently - a folded frame takes less room than an unfolded one.
        let integral_volume = item_volume_integral(&mut world, entity, volume_facts);
        if weight != row.weight
            || integral != row.integral_weight
            || volume != row.volume
            || integral_volume != row.integral_volume
        {
            disagreements.push(format!(
                "{}: ours {weight}/{integral} g {volume}/{integral_volume} ml, theirs {}/{}/{}/{} (charges {})",
                row.id,
                row.weight,
                row.integral_weight,
                row.volume,
                row.integral_volume,
                row.charges
            ));
        }
        compared += 1;
    }
    (compared, disagreements)
}

fn read_array(path: &std::path::Path, key: &str) -> Vec<cdda_json::Object> {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let cdda_json::Value::Object(root) = cdda_json::parse(&bytes).expect("valid JSON") else {
        panic!("{} is an object", path.display())
    };
    let Some(cdda_json::Value::Array(list)) = root.get(key) else {
        panic!("{key}")
    };
    list.iter()
        .map(|entry| match entry {
            cdda_json::Value::Object(object) => object.clone(),
            other => panic!("an object: {other:?}"),
        })
        .collect()
}

fn text(entry: &cdda_json::Object, key: &str) -> String {
    match entry.get(key) {
        Some(cdda_json::Value::Str(value)) => value.clone(),
        other => panic!("{key} is text: {other:?}"),
    }
}

fn whole(entry: &cdda_json::Object, key: &str) -> i64 {
    match entry.get(key) {
        Some(cdda_json::Value::Int(value)) => *value,
        other => panic!("{key} is a whole number: {other:?}"),
    }
}

fn is_no_drop(entry: &cdda_json::Object) -> bool {
    match entry.get("flags") {
        Some(cdda_json::Value::Array(flags)) => flags
            .iter()
            .any(|flag| matches!(flag, cdda_json::Value::Str(name) if name == "NO_DROP")),
        _ => false,
    }
}

#[test]
fn every_type_weighs_and_measures_what_the_reference_s_rules_say() {
    let (measured, facts) = dumps();
    assert_eq!(
        measured.len(),
        9_912,
        "every type in the reference was measured"
    );
    let (compared, disagreements) = measure_all(&measured, &facts);
    assert_eq!(compared, 9_912, "every type was measured by the port too");
    // **The rest agree**, which is the point of the gate: the rules reproduce the reference over the whole dataset and not just over the types a hand-written test
    // would pick.
    //
    // The sixty-five that do not are the declared gaps, and the test names their categories so that a new disagreement cannot hide among them and a fixed one cannot
    // go unnoticed:
    //
    // * **eighteen corpses**, where the reference answers with the monster's size rather than the item's prototype (`src/item.cpp:7231-7234,7518-7520`);
    // * **forty-four guns and magazines with something in them** - built-in mods and loaded ammunition - where the reference adds the mods' and the ammo's weight and
    //   volume to the gun's (`src/item.cpp:7265-7271,7584-7590`);
    // * **`null`**, the item with no type, whose volume is zero before any rule runs (`src/item.cpp:7514-7516`);
    // * **two magazines that hold ammo** - `gasfilter_sm` and `ksg-25` - which is the same contents rule as the guns.
    let ours: Vec<&str> = disagreements
        .iter()
        .map(|line| line.split(':').next().unwrap_or_default())
        .collect();
    assert_eq!(
        ours.len(),
        66,
        "sixty-six types disagree: {disagreements:#?}"
    );
    let corpses: Vec<&&str> = ours.iter().filter(|id| id.starts_with("corpse")).collect();
    assert_eq!(corpses.len(), 18, "eighteen corpse types disagree");
    let not_a_corpse: Vec<&&str> = ours.iter().filter(|id| !id.starts_with("corpse")).collect();
    assert_eq!(
        not_a_corpse,
        vec![
            &"null",
            &"rm228",
            &"modular_m27_assault_rifle",
            &"m249",
            &"m249_semi",
            &"modular_m16a4",
            &"modular_m16_auto_rifle",
            &"steyr_aug",
            &"bren2_556",
            &"m2010",
            &"ruger_pr",
            &"m1918",
            &"m240",
            &"m60",
            &"m60_semi",
            &"scar_h_mk20",
            &"M24",
            &"m110a1",
            &"steyr_scout",
            &"tac338",
            &"savage112",
            &"mrad_smr",
            &"m107a1",
            &"as50",
            &"tac50",
            &"bfg50",
            &"ak74_semi",
            &"kord",
            &"sks",
            &"bren2_762",
            &"mosin44",
            &"psl",
            &"type99_sniper",
            &"rm11b_sniper_rifle",
            &"hk_mp5k",
            &"briefcase_smg",
            &"mp40",
            &"bt_apc9k",
            &"combination_gun",
            &"sapra",
            &"ree_33",
            &"robofac_gun",
            &"xedra_gun",
            &"robofac_gun_40mm",
            &"winchester_1897",
            &"belt40mm",
            &"gasfilter_sm",
            &"ksg-25",
        ],
        "the declared non-corpse gaps, and nothing else"
    );
}
