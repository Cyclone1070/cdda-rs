//! The turn a field takes, read from the field's own data instead of hand-built.
//!
//! The spawner, the upgrade and the decay are all numbers the type carries per intensity level. Handing them over by hand in every
//! test is how a port drifts: the rule stays right and the numbers behind it are wrong. This gate reads them out of the reference's
//! own field definitions.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::fields::{self, FieldType};
use cdda_json::parse;

fn fields() -> Vec<FieldType> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/prototypes/fields.json");
    let text = std::fs::read_to_string(path).expect("the field dump exists");
    let value = parse(text.as_bytes()).expect("the dump is valid JSON");
    fields::from_dump(&value).expect("the dump loads")
}

#[test]
fn a_level_hands_over_its_own_numbers() {
    // Every spawning level in the data has to hand over the chance, the count, the group and the radius it was written with - a
    // turn built from anything else is a different field.
    let fields = fields();
    let mut spawning = 0;
    for field in &fields {
        for level in &field.levels {
            let turn = level.turn();
            assert_eq!(turn.spawn_chance, level.spawn_chance, "{} chance", field.id);
            assert_eq!(turn.spawn_count, level.spawn_count, "{} count", field.id);
            assert_eq!(turn.spawn_group, level.spawn_group, "{} group", field.id);
            assert_eq!(turn.spawn_radius, level.spawn_radius, "{} radius", field.id);
            assert_eq!(
                turn.upgrade_chance, level.upgrade_chance,
                "{} upgrade",
                field.id
            );
            if turn.spawn_chance > 0 && turn.spawn_count > 0 {
                spawning += 1;
                assert!(
                    !turn.spawn_group.is_empty(),
                    "{} spawns but names no group",
                    field.id
                );
            }
        }
    }
    assert_eq!(spawning, 2, "the data has two spawning intensity levels");
}

#[test]
fn a_field_that_spawns_nothing_says_so() {
    let fields = fields();
    let smoke = fields
        .iter()
        .find(|field| field.id == "fd_smoke")
        .expect("fd_smoke");
    for level in &smoke.levels {
        let turn = level.turn();
        assert_eq!(turn.spawn_chance, 0);
        assert_eq!(turn.spawn_count, 0);
        assert!(turn.spawn_group.is_empty());
    }
}
