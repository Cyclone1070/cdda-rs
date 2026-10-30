//! Missions a character has taken, and the one question the dialogue machine asks about them.
//!
//! Reference: `f_u_has_mission` (`condition.cpp:709-721`) walks `get_avatar().get_active_missions()` and answers true when one has
//! the type id the guard names. It is the biggest single refused leaf in the data - 281 occurrences, and 114 of the 3,708
//! condition objects.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::missions;
use std::collections::BTreeSet;

fn condition(text: &str) -> cdda_content::dialogue::Condition {
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    cdda_content::dialogue::read_condition(&value).expect("the port reads this guard")
}

#[test]
fn a_mission_is_a_component_and_the_condition_reads_it() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    missions::set_avatar(&mut world, survivor);
    let guard = condition(r#"{"u_has_mission": "MISSION_REACH_REFUGEE_CENTER"}"#);
    assert!(!cdda_sim::conditions::holds(&world, survivor, &guard));

    missions::accept(&mut world, survivor, "MISSION_REACH_REFUGEE_CENTER");
    assert!(cdda_sim::conditions::holds(&world, survivor, &guard));
    assert_eq!(
        missions::active_missions(&world, survivor),
        vec!["MISSION_REACH_REFUGEE_CENTER".to_owned()]
    );
    assert!(missions::finish(&mut world, survivor, "MISSION_REACH_REFUGEE_CENTER"));
    assert!(!cdda_sim::conditions::holds(&world, survivor, &guard));
}

/// The question is about the **player**, not about whoever is speaking.
///
/// Reference: `f_u_has_mission` reads `get_avatar()`, so an NPC asking it asks about the player's missions. The port states the
/// same thing through `missions::question_holder`, which prefers the avatar and falls back to the speaker when nobody has claimed
/// the role - a divergence, and the fallback is what a test uses when it has not built a player.
#[test]
fn the_question_asks_the_avatar_rather_than_the_speaker() {
    let mut world = World::new();
    let player = cdda_sim::creature::spawn_creature(&mut world, "the player", (0, 0, 0));
    let talker = cdda_sim::creature::spawn_creature(&mut world, "the talker", (0, 0, 0));
    missions::set_avatar(&mut world, player);
    missions::accept(&mut world, player, "MISSION_ISHERWOOD_CHRIS_1");

    let guard = condition(r#"{"u_has_mission": "MISSION_ISHERWOOD_CHRIS_1"}"#);
    assert!(
        cdda_sim::conditions::holds(&world, talker, &guard),
        "the talker's own mission list is empty, and the answer is still true"
    );
    assert_eq!(missions::question_holder(&world, talker), player);
    assert!(missions::active_missions(&world, talker).is_empty());

    // With nobody claiming the role the question falls back to the speaker, which is the divergence stated above.
    let mut world = World::new();
    let lonely = cdda_sim::creature::spawn_creature(&mut world, "the lonely", (0, 0, 0));
    assert_eq!(missions::question_holder(&world, lonely), lonely);
    assert!(missions::avatar(&world).is_none());
}

/// The mission ids the tests use are ids the data defines.
///
/// The trait tests caught an invented id the hard way; this one is written that way from the start.
#[test]
fn the_mission_ids_in_the_data_are_what_the_tests_use() {
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
                if matches!(object.get("type"), Some(cdda_json::Value::Str(kind)) if kind == "mission_definition")
                    && let Some(cdda_json::Value::Str(id)) = object.get("id")
                {
                    ids.insert(id.clone());
                }
            }
        }
    }
    println!("mission definitions in the data: {}", ids.len());
    assert!(
        ids.contains("MISSION_REACH_REFUGEE_CENTER"),
        "the test's mission is a real one"
    );
    assert!(ids.contains("MISSION_ISHERWOOD_CHRIS_1"), "and so is the second");
    assert!(ids.len() > 100, "the data defines a hundred-odd missions");
}
