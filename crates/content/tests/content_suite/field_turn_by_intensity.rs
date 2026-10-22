//! The turn a field takes depends on **how strong it is**.
//!
//! Reference: every processor reads `cur.get_intensity_level()` before it does anything (`field_processor_monster_spawn`,
//! `map_field.cpp:764`). A field at intensity three is not the field at intensity one: the data gives each level its own chance,
//! count and group, and a sweep that used one level's numbers for all of them would spawn at the wrong rate for every field that
//! has more than one level.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::fields::{self, FieldLevel, FieldTurn, FieldType};
use cdda_json::parse;

fn field_type() -> FieldType {
    // Two levels from the reference's own dump, chosen because they differ in everything a turn reads.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/prototypes/fields.json");
    let text = std::fs::read_to_string(path).expect("the field dump exists");
    let value = parse(text.as_bytes()).expect("the dump is valid JSON");
    let all = fields::from_dump(&value).expect("the dump loads");
    all.into_iter()
        .find(|field| field.levels.len() >= 2)
        .expect("a field with more than one level")
}

fn level_with(chance: i32, count: i32, group: &str) -> FieldLevel {
    FieldLevel {
        name: String::new(),
        sym: 0,
        color: String::new(),
        dangerous: false,
        transparent: true,
        move_cost: 0,
        upgrade_chance: 0,
        upgrade_duration: 0,
        spawn_chance: chance,
        spawn_count: count,
        spawn_radius: 0,
        spawn_group: group.to_owned(),
    }
}

fn turn_of(levels: Vec<FieldLevel>, intensity: i32) -> Option<FieldTurn> {
    let mut field = field_type();
    field.levels = levels;
    cdda_content::fields::turn_at(&field, intensity)
}

#[test]
fn each_intensity_hands_over_its_own_numbers() {
    let weak = level_with(50, 1, "GROUP_ZOMBIE");
    let strong = level_with(2, 4, "GROUP_GOO");
    let levels = vec![weak, strong];
    assert_eq!(
        turn_of(levels.clone(), 1).map(|turn| turn.spawn_chance),
        Some(50)
    );
    assert_eq!(
        turn_of(levels.clone(), 2).map(|turn| turn.spawn_count),
        Some(4)
    );
    assert_eq!(
        turn_of(levels, 2).map(|turn| turn.spawn_group),
        Some("GROUP_GOO".to_owned())
    );
}

#[test]
fn an_intensity_the_data_does_not_have_hands_over_nothing() {
    // Beyond the last level there is no field left to take a turn, and the reference cannot ask for one either: intensity is
    // capped at the type's maximum.
    let levels = vec![level_with(1, 1, "GROUP_ZOMBIE")];
    assert!(
        turn_of(levels.clone(), 0).is_none(),
        "intensity starts at one"
    );
    assert!(
        turn_of(levels.clone(), 2).is_none(),
        "there is no second level"
    );
    assert!(turn_of(levels, 1).is_some());
}

#[test]
fn the_intensity_of_a_real_field_selects_its_own_level() {
    // The same question against the reference's data rather than a fixture: for every field and every intensity, the turn comes
    // from that level and no other.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/prototypes/fields.json");
    let text = std::fs::read_to_string(path).expect("the field dump exists");
    let value = parse(text.as_bytes()).expect("the dump is valid JSON");
    let all = fields::from_dump(&value).expect("the dump loads");
    let mut checked = 0;
    for field in &all {
        for (index, level) in field.levels.iter().enumerate() {
            let intensity = i32::try_from(index).expect("a small index") + 1;
            let turn = fields::turn_at(field, intensity).expect("a level at this intensity");
            assert_eq!(
                turn.spawn_chance, level.spawn_chance,
                "{} at {intensity}",
                field.id
            );
            assert_eq!(
                turn.spawn_group, level.spawn_group,
                "{} at {intensity}",
                field.id
            );
            checked += 1;
        }
    }
    assert!(checked > 50, "only {checked} levels were checked");
}
