#![allow(clippy::expect_used, clippy::panic)]

/// Every round states the ammunition type it belongs to, and that type is **not** its own id for most of them.
///
/// Reference: `islot_ammo::type` (`itype.h:1063`). The distinction is the one that broke the CLI's `shoot` step the moment a shotgun was tried: a gun names its ammunition type (`"shot"`), the
/// rounds table is keyed by round id (`shot_bird`), and the two agree only where the data happens to make them equal - `223` is one such accident.
#[test]
fn every_round_states_the_ammunition_type_it_belongs_to() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/ammo.json");
    let bytes = std::fs::read(path).expect("the ammo dump exists");
    let value = cdda_json::parse(&bytes).expect("valid JSON");
    let rounds = cdda_content::ammo::load_dump(&value).expect("the dump loads");

    let of = |id: &str| {
        rounds
            .iter()
            .find(|round| round.id == id)
            .expect("the round is in the dump")
            .ammo_type
            .clone()
    };
    assert_eq!(of("shot_bird"), "shot", "birdshot is a `shot` round");
    assert_eq!(of("shot_00"), "shot", "and so is buckshot");
    assert_eq!(
        of("223"),
        "223",
        "while a .223's type happens to share its id"
    );

    // Counts prove the gate is not looking at a handful: every round states a type, and most of those types carry more than one round.
    let mut by_type: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for round in &rounds {
        assert!(
            !round.ammo_type.is_empty(),
            "round `{}` states no ammunition type",
            round.id
        );
        *by_type.entry(round.ammo_type.clone()).or_default() += 1;
    }
    assert_eq!(
        by_type.values().sum::<usize>(),
        rounds.len(),
        "every round counted once"
    );
    let shared = by_type.values().filter(|count| **count > 1).count();
    assert!(
        shared > 50,
        "most ammunition types have several rounds: {shared} types of {}",
        by_type.len()
    );
    assert_eq!(
        by_type.get("shot").copied(),
        Some(
            rounds
                .iter()
                .filter(|round| round.ammo_type == "shot")
                .count()
        )
        .filter(|count| *count > 1),
        "and `shot` is one of the crowded ones"
    );
}

/// Every round states the ammunition effects it carries, and black powder is one of them.
///
/// Reference: `islot_ammo::ammo_effects` (`itype.h:1100`). **This is the data round 1173's black-powder branch was waiting for**, and the gate counts rather than sampling: the set is not small,
/// most rounds carry none, and the named ones the rules read are all present in it.
#[test]
fn every_round_states_the_ammunition_effects_it_carries() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/ammo.json");
    let bytes = std::fs::read(path).expect("the ammo dump exists");
    let value = cdda_json::parse(&bytes).expect("valid JSON");
    let rounds = cdda_content::ammo::load_dump(&value).expect("the dump loads");

    let carrying = |name: &str| rounds.iter().filter(|round| round.has_effect(name)).count();
    let powder = carrying("BLACKPOWDER");
    assert!(
        powder > 50,
        "black powder rounds exist and are not rare: {powder} of {}",
        rounds.len()
    );
    assert!(
        carrying("NON_FOULING") > 0,
        "as do non-fouling rounds: {}",
        carrying("NON_FOULING")
    );
    assert!(
        carrying("INCENDIARY") > 0,
        "and incendiaries: {}",
        carrying("INCENDIARY")
    );

    // The shape of the set: most rounds carry nothing at all, and the ones that do carry several.
    let empty = rounds
        .iter()
        .filter(|round| round.effects.is_empty())
        .count();
    assert!(
        empty > 100,
        "most rounds carry no effects: {empty} of {}",
        rounds.len()
    );
    let most = rounds.iter().map(|round| round.effects.len()).max();
    assert!(
        most.is_some_and(|count| count >= 3),
        "and some carry several: {most:?}"
    );
    assert!(
        rounds
            .iter()
            .find(|round| round.id == "223")
            .is_some_and(|round| !round.has_effect("BLACKPOWDER")),
        "a .223 is not black powder"
    );
}

/// Every prototype states how well it resists being chipped, and the values are the item's own.
///
/// Reference: `item::chip_resistance()` (`item.cpp:9375-9409`). **The column is what `handle_melee_wear` divides by**, so a dump without it cannot price a weapon's wear at all - which is why
/// the prototypes dump went to version 2 for it. The gate counts rather than samples: the set spans zero to twenty and most items sit in the low single digits.
#[test]
fn every_prototype_states_its_chip_resistance() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/items.json");
    let bytes = std::fs::read(path).expect("the prototypes dump exists");
    let value = cdda_json::parse(&bytes).expect("valid JSON");
    let resistance = cdda_content::items::chip_resistance(&value);

    assert!(
        resistance.len() > 9_000,
        "the dump covers the catalogue: {}",
        resistance.len()
    );
    assert_eq!(
        resistance.get("rock").copied(),
        Some(20),
        "a rock resists chipping well"
    );
    assert_eq!(
        resistance.get("ruger_mini").copied(),
        Some(10),
        "and a rifle moderately"
    );
    assert_eq!(
        resistance.get("knife_combat").copied(),
        Some(9),
        "a combat knife a little less"
    );
    // The spread is real: something in the catalogue is as soft as it gets, and something is twice the rock.
    assert!(
        resistance.values().any(|value| *value == 0),
        "the softest items are zero"
    );
    assert!(
        resistance.values().any(|value| *value >= 20),
        "and the hardest are twenty or more"
    );
}

/// Every prototype that can catch a fault says which ones, in the data's own order.
///
/// Reference: `itype::faults`. **The order is part of the answer** - the weighted pick walks the list from the front, so sorting it would change which fault an identical die chooses.
#[test]
fn the_dump_carries_each_kinds_faults_with_weights() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/items.json");
    let bytes = std::fs::read(path).expect("the prototypes dump exists");
    let value = cdda_json::parse(&bytes).expect("valid JSON");
    let weights = cdda_content::items::fault_weights(&value);

    assert!(
        weights.len() > 500,
        "hundreds of prototypes catch faults: {}",
        weights.len()
    );
    // The salvaged bionic fault every bionic carries, at full weight.
    assert_eq!(
        weights
            .get("bio_adrenaline")
            .and_then(|list| list.first())
            .map(|(fault, weight)| (fault.as_str(), *weight)),
        Some(("fault_bionic_salvaged", 100)),
        "a bionic catches the salvaged fault, weight one hundred"
    );
    // Some prototypes list more than one, which is what makes the pick a pick.
    assert!(
        weights.values().any(|list| list.len() > 1),
        "some types catch several faults"
    );
    assert!(
        weights
            .values()
            .all(|list| list.iter().all(|(_, weight)| *weight > 0)),
        "and every weight is positive"
    );
}

/// Kinds are numbered from the sorted ids, not from the order the files were read in.
///
/// The rule round 1213 wrote down: the prototypes dump keys its columns by **id string** and the store keys everything by a number, so **the number must not depend on file order** - otherwise a mod
/// that adds one item renumbers every kind after it and the chip table disagrees with the fault table.
#[test]
fn kinds_are_numbered_from_the_sorted_ids() {
    use std::collections::BTreeMap;

    let ids = vec![
        "rock".to_owned(),
        "arrow".to_owned(),
        "knife".to_owned(),
        // A duplicate, because two data files may define the same id and the reference's own loader keeps one.
        "rock".to_owned(),
    ];
    let numbers = cdda_content::items::kind_numbers(&ids);
    let expected: BTreeMap<String, u32> = [
        ("arrow".to_owned(), 0),
        ("knife".to_owned(), 1),
        ("rock".to_owned(), 2),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        numbers, expected,
        "sorted, deduplicated and numbered from zero"
    );

    // The same list in a different order numbers exactly the same, which is the whole point.
    let reordered = vec!["knife".to_owned(), "rock".to_owned(), "arrow".to_owned()];
    assert_eq!(
        cdda_content::items::kind_numbers(&reordered),
        expected,
        "file order cannot change a kind's number"
    );
}

/// A fault carries the line the game prints when an item catches it.
///
/// Reference: `Fault::message()`, printed by `item::set_fault` when it is told to speak (`item.cpp:7840-7842`, `add_msg( m_bad, f_id.obj().message() )`). The data writes it as a translation
/// object or a plain string, and **a fault with no message is silent rather than absent** - the text is empty and the fault still exists.
#[test]
fn a_fault_carries_the_line_it_prints() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    if !path.is_dir() {
        return;
    }
    let faults = cdda_content::faults::load_paths(&[&path]).expect("the fault data loads");
    assert!(
        faults.len() > 50,
        "the catalogue is large: {}",
        faults.len()
    );
    // **The data in this version names no fault message at all**, which is why the field is proved from a record instead of from the catalogue: a fault that does name one is read, and a fault that
    // does not is silent rather than missing.
    let speaking = cdda_json::parse(
        br#"{"id":"test_fault","message":"Your %s has a bad case of the dents.","type":"fault"}"#,
    )
    .expect("valid json");
    let cdda_json::Value::Object(object) = speaking else {
        panic!("a fault is an object")
    };
    let read = cdda_content::faults::Fault::read(&object).expect("a fault reads");
    assert_eq!(
        read.message, "Your %s has a bad case of the dents.",
        "the line is read from the record"
    );
    assert!(
        faults.iter().all(|fault| fault.message.is_empty()),
        "and no fault in the catalogue names one here"
    );
    // And the faults a blow adds are in the catalogue with their types, which is what the pick filters on.
    assert!(
        faults
            .iter()
            .any(|fault| fault.fault_type == "mechanical_damage"),
        "there is at least one mechanical-damage fault to catch"
    );
}
