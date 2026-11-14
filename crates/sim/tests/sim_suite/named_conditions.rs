//! **A condition can be given a name and asked for later by that name.**
//!
//! Reference: `f_set_condition` (`npctalk.cpp:5870-5881`) stores a condition under a name in `dialogue::conditionals`, and `f_get_condition` (`condition.cpp:1941-1948`) answers
//! it through `evaluate_conditional` (`npctalk.cpp:2670-2674`) - which answers **false for a name nobody set**, rather than refusing. The data leans on both: a caravan encounter
//! sets `random_enc_condition` from its own condition and a later program asks for it by name.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

fn effect(text: &str) -> cdda_content::dialogue::Effect {
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    cdda_content::dialogue::read_effect(&value).expect("the port reads the effect")
}

fn holds(world: &World, speaker: Entity, text: &str) -> bool {
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let condition =
        cdda_content::dialogue::read_condition(&value).expect("the port reads the condition");
    cdda_sim::conditions::holds_with(world, speaker, None, &condition)
}

/// A world with one zombie and the two halves of the pair.
fn a_zombie() -> (World, Entity) {
    use cdda_json::{Value, parse};
    use std::path::Path;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
    let text = std::fs::read(path).expect("the monster dump exists");
    let value = parse(&text).expect("the dump is valid JSON");
    let Value::Object(root) = value else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(prototypes)) = root.get("prototypes") else {
        panic!("the dump has no prototypes");
    };
    let monster_type = cdda_content::monsters::load_objects(prototypes)
        .expect("the dump loads")
        .into_iter()
        .find(|one| one.id == "mon_zombie")
        .expect("the corpus has a zombie");
    let mut world = World::new();
    let zombie = cdda_sim::creature::spawn_monster_of_type(
        &mut world,
        cdda_sim::MonsterKindId(3),
        &monster_type,
        (1, 0, 0),
        100,
    );
    (world, zombie)
}

#[test]
fn a_condition_is_stored_under_a_name_and_asked_for_later() {
    let (mut world, zombie) = a_zombie();
    let asked = r#"{ "get_condition": "is_a_zombie" }"#;

    assert!(
        !holds(&world, zombie, asked),
        "**a name nobody has set answers no**, which is `evaluate_conditional`'s own answer for an unknown key"
    );

    let setting =
        effect(r#"{ "set_condition": "is_a_zombie", "condition": { "u_has_species": "ZOMBIE" } }"#);
    let applied = cdda_sim::dialogue_effects::run_effects(
        &mut world,
        zombie,
        None,
        std::slice::from_ref(&setting),
    );
    assert_eq!(
        applied,
        cdda_sim::dialogue_effects::Applied::Done,
        "the effect runs"
    );

    assert!(
        holds(&world, zombie, asked),
        "**and once it is set, the question answers the condition that was stored**"
    );
    // The stored condition is the one that was given, not a constant: ask the same name about something the stored condition says is false.
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    assert!(
        !holds(&world, survivor, asked),
        "a person is not of the ZOMBIE species, so the same name answers no for one"
    );
}
