//! Traits as a component, and the dialogue condition that reads them.
//!
//! Reference: `f_has_trait` (`condition.cpp:580-586`) asks `has_trait` of the actor the member names; `Character::has_trait`
//! (`character.cpp`) reads the character's own set. Measured against the data, `u_has_trait` is the second biggest guard the port
//! could not answer - 36 of 692 programs were refused because of it.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_sim::traits;
use std::collections::BTreeSet;

fn condition(text: &str) -> cdda_content::dialogue::Condition {
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    cdda_content::dialogue::read_condition(&value).expect("the port reads this guard")
}

#[test]
fn a_trait_is_a_component_and_the_condition_reads_it() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    assert!(!traits::has_trait(&world, survivor, "HALLUCINATION"));
    assert!(!cdda_sim::conditions::holds(
        &world,
        survivor,
        &condition(r#"{"u_has_trait": "HALLUCINATION"}"#)
    ));

    traits::add_trait(&mut world, survivor, "HALLUCINATION");
    assert!(cdda_sim::conditions::holds(
        &world,
        survivor,
        &condition(r#"{"u_has_trait": "HALLUCINATION"}"#)
    ));
    assert_eq!(
        traits::traits_held(&world, survivor),
        vec!["HALLUCINATION".to_owned()],
        "one trait, held once"
    );
    assert!(traits::remove_trait(&mut world, survivor, "HALLUCINATION"));
    assert!(!traits::remove_trait(&mut world, survivor, "HALLUCINATION"));
    assert!(traits::traits_held(&world, survivor).is_empty());
}

#[test]
fn the_npc_form_asks_the_listener_and_says_nothing_without_one() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let ally = cdda_sim::creature::spawn_creature(&mut world, "the ally", (0, 0, 0));
    traits::set_traits(&mut world, ally, &["MARTIAL_ARTS4"]);

    let guard = condition(r#"{"npc_has_trait": "MARTIAL_ARTS4"}"#);
    assert!(cdda_sim::conditions::holds_with(
        &world,
        survivor,
        Some(ally),
        &guard
    ));
    assert!(
        !cdda_sim::conditions::holds_with(&world, survivor, None, &guard),
        "with nobody listening the question has no answer, which is false rather than a guess"
    );
}

#[test]
fn a_program_guarded_by_a_trait_runs_only_when_the_trait_is_there() {
    use cdda_sim::dialogue_effects::Applied;
    use cdda_sim::eoc_registry;
    use cdda_sim::messages;

    let mut world = World::new();
    let definition = cdda_json::parse(
        br#"{"type": "effect_on_condition", "id": "EOC_PARKOUR",
            "condition": {"u_has_trait": "HALLUCINATION"},
            "effect": [{"u_message": "you vault the fence"}]}"#,
    )
    .expect("json");
    let program = cdda_content::dialogue::read_program(&definition).expect("readable");
    eoc_registry::set_programs(&mut world, &[program]);
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    assert_eq!(
        eoc_registry::run_program(&mut world, survivor, None, "EOC_PARKOUR", 0),
        Applied::Done
    );
    assert!(messages::lines(&world).is_empty(), "no trait, no message");

    traits::add_trait(&mut world, survivor, "HALLUCINATION");
    eoc_registry::run_program(&mut world, survivor, None, "EOC_PARKOUR", 0);
    assert_eq!(messages::lines(&world), vec!["you vault the fence".to_owned()]);
}

/// The data's whole trait ids, so the ids the tests use are real ones rather than inventions.
#[test]
fn the_trait_ids_in_the_data_are_what_the_tests_use() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    let mut ids: BTreeSet<String> = BTreeSet::new();
    let mut stack = vec![data];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(std::ffi::OsStr::to_str) != Some("json") {
                continue;
            }
            let Some(shared) = cdda_content::read_json(&path) else {
                continue;
            };
            let cdda_json::Value::Array(ref items) = *shared else {
                continue;
            };
            for item in items {
                let cdda_json::Value::Object(object) = item else {
                    continue;
                };
                if matches!(object.get("type"), Some(cdda_json::Value::Str(kind)) if kind == "mutation")
                    && let Some(cdda_json::Value::Str(id)) = object.get("id")
                {
                    ids.insert(id.clone());
                }
            }
        }
    }
    println!("mutations in the data: {}", ids.len());
    // The first version of this test used invented ids (`TRAIT_PARKOUR`), and this assertion is what caught it: the data's
    // mutations are spelled bare, without the `TRAIT_` prefix the reference's `trait_id` adds in code. A test that invents an id
    // proves the port handles a trait nobody has.
    assert!(ids.contains("HALLUCINATION"), "the test's trait is a real one");
    assert!(ids.contains("MARTIAL_ARTS4"), "and so is the second");
    assert!(ids.len() > 400, "the data defines the mutations a trait set would hold");
}

/// A program can give and take a trait, which is how the data's mutations happen.
///
/// Reference: `f_add_trait` (`npctalk.cpp:3498-3508`) calls `set_mutation`; `f_remove_trait` (3536-3543) calls `unset_mutation`. These
/// are the first effects that change a creature rather than say something, and they are 139 of the data's 1,612 effect entries
/// (36 `u_add_trait`, 103 `u_lose_trait`).
#[test]
fn a_program_can_give_and_take_a_trait() {
    use cdda_sim::dialogue_effects::{run_effects, Applied};
    use cdda_sim::messages;

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let ally = cdda_sim::creature::spawn_creature(&mut world, "the ally", (0, 0, 0));

    let program = vec![
        cdda_content::dialogue::read_effect(
            &cdda_json::parse(br#"{"u_add_trait": "HALLUCINATION"}"#.as_slice()).expect("json"),
        )
        .expect("readable"),
        cdda_content::dialogue::read_effect(
            &cdda_json::parse(br#"{"npc_add_trait": "MARTIAL_ARTS4"}"#.as_slice()).expect("json"),
        )
        .expect("readable"),
        cdda_content::dialogue::read_effect(
            &cdda_json::parse(br#"{"u_message": "done"}"#.as_slice()).expect("json"),
        )
        .expect("readable"),
    ];
    assert_eq!(
        run_effects(&mut world, survivor, Some(ally), &program),
        Applied::Done
    );
    assert!(traits::has_trait(&world, survivor, "HALLUCINATION"));
    assert!(
        traits::has_trait(&world, ally, "MARTIAL_ARTS4"),
        "the `npc_` spelling gives it to the listener"
    );
    assert_eq!(messages::lines(&world), vec!["done".to_owned()]);

    let lose = vec![
        cdda_content::dialogue::read_effect(
            &cdda_json::parse(br#"{"u_lose_trait": "HALLUCINATION"}"#.as_slice()).expect("json"),
        )
        .expect("readable"),
    ];
    assert_eq!(
        run_effects(&mut world, survivor, Some(ally), &lose),
        Applied::Done
    );
    assert!(!traits::has_trait(&world, survivor, "HALLUCINATION"));

    // Losing a trait it does not have is still a finished effect - `unset_mutation` does nothing and says nothing.
    assert_eq!(
        run_effects(&mut world, survivor, Some(ally), &lose),
        Applied::Done
    );

    // An `npc_` effect with nobody listening cannot be done, and says so rather than changing the speaker by mistake.
    let to_nobody = vec![
        cdda_content::dialogue::read_effect(
            &cdda_json::parse(br#"{"npc_lose_trait": "MARTIAL_ARTS4"}"#.as_slice()).expect("json"),
        )
        .expect("readable"),
    ];
    assert_eq!(
        run_effects(&mut world, survivor, None, &to_nobody),
        Applied::CannotAnswer
    );
    assert!(
        traits::has_trait(&world, ally, "MARTIAL_ARTS4"),
        "and the ally keeps the trait it had"
    );
}
