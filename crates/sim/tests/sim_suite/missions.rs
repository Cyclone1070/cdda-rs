//! Missions a character has taken, and the one question the dialogue machine asks about them.
//!
//! Reference: `f_u_has_mission` (`condition.cpp:709-721`) walks `get_avatar().get_active_missions()` and answers true when one has
//! the type id the guard names. It is the biggest single refused leaf in the data - 281 occurrences, and 114 of the 3,708
//! condition objects.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

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
    assert!(missions::finish(
        &mut world,
        survivor,
        "MISSION_REACH_REFUGEE_CENTER"
    ));
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
    assert!(
        ids.contains("MISSION_ISHERWOOD_CHRIS_1"),
        "and so is the second"
    );
    assert!(ids.len() > 100, "the data defines a hundred-odd missions");
}

/// An offer lands on the creature being talked to, not on the one talking.
///
/// Reference: `f_offer_mission` (`npctalk.cpp:5990-5998`) takes `d.actor( d.has_beta )->get_npc()` - the **other** side of the conversation - and adds a mission of each
/// named type to it. The data spells the member as one id or a list, and the reference refuses anything else (`npctalk.cpp:5981-5989`).
#[test]
fn a_mission_is_offered_to_the_creature_being_talked_to() {
    let mut world = World::new();
    let talker = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let listener = cdda_sim::creature::spawn_creature(&mut world, "the mechanic", (1, 0, 0));

    let text = r#"{"offer_mission": ["MISSION_FIX_GENERATOR", "MISSION_FUEL_RUN"]}"#;
    let Ok(value) = cdda_json::parse(text.as_bytes()) else {
        panic!("json");
    };
    let Some(one) = cdda_content::dialogue::read_effect(&value) else {
        panic!("the port reads this effect");
    };
    let applied =
        cdda_sim::dialogue_effects::run_effects(&mut world, talker, Some(listener), &[one]);
    assert_eq!(
        applied,
        cdda_sim::dialogue_effects::Applied::Done,
        "the offer is answered"
    );
    assert_eq!(
        missions::offers(&world, listener),
        vec![
            "MISSION_FIX_GENERATOR".to_owned(),
            "MISSION_FUEL_RUN".to_owned()
        ],
        "and the creature being talked to is the one offering them"
    );
    assert!(
        missions::offers(&world, talker).is_empty(),
        "the talker offers nothing"
    );
}

/// **The mission store answers what a mission is.** The port's `Missions` component holds ids in a set; this registry holds the definitions those ids name, so a rule with an
/// id - `clear_mission` with the mission it is clearing - can ask what it leads to. Reference: `mission_type::get` (`src/mission.h`, `mission.cpp`).
///
/// The lighthouse chain is the data's own: `MISSION_lighthouse_man_1` leads to `_2`, which leads to `_3`, and every link is stated in the data rather than in this test.
#[test]
fn the_mission_store_answers_what_a_mission_is_and_what_follows_it() {
    let mut world = World::new();
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let roots = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = roots.iter().map(std::path::PathBuf::as_path).collect();
    let loaded = cdda_content::mission_types::load_paths(&paths).expect("the missions load");
    cdda_sim::mission_types::set_mission_types(&mut world, loaded);
    let kind = cdda_sim::mission_types::kind_named(&world, "MISSION_lighthouse_man_1")
        .expect("the first lighthouse mission is in the store");
    assert_eq!(
        kind.id, "MISSION_lighthouse_man_1",
        "**the store finds it by id**"
    );
    assert_eq!(
        cdda_sim::mission_types::follow_up(&world, "MISSION_lighthouse_man_1").as_deref(),
        Some("MISSION_lighthouse_man_2"),
        "**and knows the mission that follows it**"
    );
    assert_eq!(
        cdda_sim::mission_types::follow_up(&world, "MISSION_lighthouse_man_5"),
        None,
        "**the last link in the chain leads nowhere**"
    );
    assert_eq!(
        cdda_sim::mission_types::kind_named(&world, "MISSION_no_such_thing"),
        None,
        "**and a mission the data does not define is nothing, not an error**"
    );
    let ids = cdda_sim::mission_types::ids(&world);
    assert_eq!(ids.len(), 306, "**the store lists all 306 missions**");
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted, "**in id order**");
}
