//! **`has_species` asks what the creature is made of, by species id.**
//!
//! Reference: `f_has_species` (`condition.cpp:646-654`): `actor->has_species( species_id( name ) )`, which for a monster is whether its type's `species` list holds the id.
//! A person has none - `Character::has_species` answers false when there is no species list - so the same question about the survivor is always no.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_json::{Value, parse};
use std::path::Path;

/// The zombie's own type, read from the dump so the species comes from the data rather than a literal.
fn zombie_type() -> cdda_content::monsters::MonsterType {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
    let text = std::fs::read(path).expect("the monster dump exists");
    let value = parse(&text).expect("the dump is valid JSON");
    let Value::Object(root) = value else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(prototypes)) = root.get("prototypes") else {
        panic!("the dump has no prototypes");
    };
    cdda_content::monsters::load_objects(prototypes)
        .expect("the dump loads")
        .into_iter()
        .find(|one| one.id == "mon_zombie")
        .expect("the corpus has a zombie")
}

fn holds(world: &World, speaker: Entity, listener: Option<Entity>, text: &str) -> bool {
    let value = parse(text.as_bytes()).expect("json");
    let condition = cdda_content::dialogue::read_condition(&value).expect("the port reads the condition");
    cdda_sim::conditions::holds_with(world, speaker, listener, &condition)
}

#[test]
fn a_monster_has_the_species_its_type_states() {
    let monster_type = zombie_type();
    assert!(
        monster_type.species.contains("ZOMBIE"),
        "the dump says a zombie is of the ZOMBIE species"
    );
    let mut world = World::new();
    let zombie = cdda_sim::creature::spawn_monster_of_type(
        &mut world,
        cdda_sim::MonsterKindId(7),
        &monster_type,
        (1, 1, 0),
        100,
    );
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    assert!(
        holds(&world, zombie, None, r#"{ "u_has_species": "ZOMBIE" }"#),
        "**a zombie is of the ZOMBIE species**"
    );
    assert!(
        !holds(&world, zombie, None, r#"{ "u_has_species": "ROBOT" }"#),
        "and not of another"
    );
    assert!(
        !holds(&world, survivor, None, r#"{ "u_has_species": "ZOMBIE" }"#),
        "a person is of no species at all"
    );
}
