//! **`npc_allies` counts the people who follow the player.**
//!
//! Reference: `f_npc_allies` (`condition.cpp:1159-1165`) compares `g->allies().size()` with a number; `f_npc_allies_global` (`:1167-1179`) counts every NPC that is
//! `is_player_ally()`, **not** a hallucination and **not** dead. The port keeps one reality bubble and no hallucination, so the two questions answer the same count here - and the
//! doc says so rather than pretending otherwise.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

fn holds(world: &World, speaker: Entity, text: &str) -> bool {
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let condition =
        cdda_content::dialogue::read_condition(&value).expect("the port reads the condition");
    cdda_sim::conditions::holds_with(world, speaker, None, &condition)
}

/// A survivor and one person, who follows when `ally` says so.
fn a_person(world: &mut World, name: &str, ally: bool) -> Entity {
    let them = cdda_sim::creature::spawn_creature(world, name, (1, 0, 0));
    cdda_sim::npcs::make_npc(world, them, name);
    if ally {
        cdda_sim::npcs::make_ally(world, them);
    }
    them
}

#[test]
fn the_people_who_follow_the_player_are_counted() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::missions::set_avatar(&mut world, survivor);
    a_person(&mut world, "a follower", true);
    a_person(&mut world, "another follower", true);
    a_person(&mut world, "a stranger", false);

    assert!(
        holds(&world, survivor, r#"{ "npc_allies": 2 }"#),
        "**two people follow, so two is enough**"
    );
    assert!(
        holds(&world, survivor, r#"{ "npc_allies": 1 }"#),
        "and so is one"
    );
    assert!(
        !holds(&world, survivor, r#"{ "npc_allies": 3 }"#),
        "**but the stranger is not one of them**, so three is too many"
    );
    assert!(
        holds(&world, survivor, r#"{ "npc_allies_global": 2 }"#),
        "and the global count agrees, because this port has one bubble"
    );
    assert!(
        !holds(&world, survivor, r#"{ "npc_allies_global": 3 }"#),
        "there too"
    );
}
