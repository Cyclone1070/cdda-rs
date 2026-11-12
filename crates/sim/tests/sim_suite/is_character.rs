//! **`is_character` asks whether a talker is a person - and `is_avatar` asks whether it is the player.**
//!
//! Reference: `f_is_character` (`condition.cpp:1331-1336`) answers `get_const_character() != nullptr`, which is true for the avatar **and** for every NPC and false for a
//! monster. `f_is_avatar` (`condition.cpp`) is the narrower question. The two were one answer in this port until a data site (`{"and": ["npc_is_character", ...]}`) needed
//! the difference.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

fn holds(world: &World, speaker: Entity, listener: Option<Entity>, text: &str) -> bool {
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let condition = cdda_content::dialogue::read_condition(&value).expect("the port reads the condition");
    cdda_sim::conditions::holds_with(world, speaker, listener, &condition)
}

#[test]
fn a_person_is_a_character_and_the_player_is_the_avatar() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::missions::set_avatar(&mut world, survivor);
    let trader = cdda_sim::creature::spawn_creature(&mut world, "the trader", (1, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, trader, "trader");

    assert!(holds(&world, survivor, None, r#""u_is_character""#), "**the player is a character**");
    assert!(holds(&world, survivor, None, r#""u_is_avatar""#), "and the avatar");
    assert!(
        holds(&world, survivor, Some(trader), r#""npc_is_character""#),
        "**an NPC is a character too**, which is the whole point of the condition: the player is one and so is the trader"
    );
    assert!(
        holds(&world, trader, None, r#""u_is_character""#),
        "the trader, speaking, is a character"
    );
    assert!(
        !holds(&world, trader, None, r#""u_is_avatar""#),
        "but not the avatar"
    );
    let about_the_trader = r#"{"and": [ "has_beta", "npc_is_character" ]}"#;
    assert!(
        holds(&world, survivor, Some(trader), about_the_trader),
        "and the `and` the data writes - the shape that exposed the conflation - reads"
    );

    // **A monster is not a person**, which is the other half of the rule: `get_const_character` is null for one.
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "a zombie", (2, 0, 0));
    cdda_sim::creature::set_kind(&mut world, zombie, cdda_sim::MonsterKindId(0));
    assert!(
        !holds(&world, zombie, None, r#""u_is_character""#),
        "a zombie is nobody's character"
    );
}
