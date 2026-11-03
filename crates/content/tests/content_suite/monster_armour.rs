//! A monster is armoured against three kinds of blow, and the loader keeps them apart.
//!
//! Reference: `mtype::armor` holds a `resistances` object; `armor_bash`, `armor_cut` and `armor_stab` are three of its types, and a monster can be armoured against a blade's edge
//! and open to its point. The port's melee rules resolve all three damage types (round 1092), so a loader that dropped the third would make every monster softer than its data.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_json::{Value, parse};

/// What a prototype's dump says about each kind of blow.
fn dumped() -> Vec<(String, i64, i64, i64)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
    let bytes = std::fs::read(path).expect("the monster dump exists");
    let root = parse(&bytes).expect("the dump is valid JSON");
    let Value::Object(root) = root else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(prototypes)) = root.get("prototypes") else {
        panic!("the dump has no prototypes");
    };
    let number = |value: Option<&Value>| -> i64 {
        match value {
            Some(Value::Int(value)) => *value,
            _ => panic!("the dump states armour as whole numbers"),
        }
    };
    prototypes
        .iter()
        .map(|one| {
            let Value::Object(fields) = one else {
                panic!("a prototype is an object")
            };
            let Some(Value::Str(id)) = fields.get("id") else {
                panic!("every prototype has an id")
            };
            (
                id.clone(),
                number(fields.get("armor_bash")),
                number(fields.get("armor_cut")),
                number(fields.get("armor_stab")),
            )
        })
        .collect()
}

/// Every one of the 1,154 prototypes keeps the three armour numbers its data gives it.
#[test]
fn a_monsters_three_armours_arrive_as_its_data_states_them() {
    let rows = dumped();
    assert_eq!(rows.len(), 1_154, "every monster prototype in the dump");

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
    let bytes = std::fs::read(path).expect("the monster dump exists");
    let root = parse(&bytes).expect("the dump is valid JSON");
    let Value::Object(root) = root else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(prototypes)) = root.get("prototypes") else {
        panic!("the dump has no prototypes");
    };
    let loaded = cdda_content::monsters::load_objects(prototypes).expect("the dump loads");
    let by_id: std::collections::BTreeMap<&str, &cdda_content::monsters::MonsterType> =
        loaded.iter().map(|monster| (monster.id.as_str(), monster)).collect();

    let mut wrong = Vec::new();
    for (id, bash, cut, stab) in &rows {
        let Some(monster) = by_id.get(id.as_str()) else {
            wrong.push(format!("{id}: not loaded"));
            continue;
        };
        let got = (
            i64::from(monster.armor_bash),
            i64::from(monster.armor_cut),
            i64::from(monster.armor_stab),
        );
        if got != (*bash, *cut, *stab) {
            wrong.push(format!("{id}: ({bash}, {cut}, {stab}) became {got:?}"));
        }
    }
    let first_few: Vec<&String> = wrong.iter().take(5).collect();
    assert!(wrong.is_empty(), "{} of 1,154 wrong: {first_few:?}", wrong.len());

    // Stabbing armour is not a copy of cutting armour: some types have one and not the other, which is the whole reason the two fields exist.
    let apart = rows
        .iter()
        .filter(|(_, _, cut, stab)| cut != stab)
        .count();
    assert!(
        apart > 0,
        "some monsters resist cutting and stabbing differently: {apart}"
    );
}
