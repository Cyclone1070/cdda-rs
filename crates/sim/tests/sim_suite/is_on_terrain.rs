//! **`u_is_on_terrain` asks the ground under the actor, by id.**
//!
//! Reference: `f_is_on_terrain` (`condition.cpp:2124-2133`): `here.ter( pos ) == ter_id( name )` - the terrain **id** under the actor, compared exactly. It is the sibling of
//! `u_is_on_terrain_with_flag`, which the port already answers, and the difference is the rule: one compares a name, the other a flag.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

#[test]
fn the_ground_under_the_actor_is_asked_by_id() {
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [("t_grass".to_owned(), 1), ("t_dirt".to_owned(), 2)],
    );
    let grass = cdda_sim::terrain_types::code_of(&world, "t_grass").expect("grass is registered");
    let dirt = cdda_sim::terrain_types::code_of(&world, "t_dirt").expect("dirt is registered");
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::square::set_terrain(&mut world, (0, 0, 0), grass);

    let value = cdda_json::parse(br#"{ "u_is_on_terrain": "t_grass" }"#).expect("json");
    let condition = cdda_content::dialogue::read_condition(&value).expect("**the port reads `u_is_on_terrain`**");
    assert!(
        cdda_sim::conditions::holds_with(&world, speaker, None, &condition),
        "**the survivor stands on grass**"
    );

    // The same question about a terrain the survivor is not standing on.
    let other = cdda_json::parse(br#"{ "u_is_on_terrain": "t_dirt" }"#).expect("json");
    let other = cdda_content::dialogue::read_condition(&other).expect("the effect reads");
    assert!(
        !cdda_sim::conditions::holds_with(&world, speaker, None, &other),
        "and not on dirt"
    );

    // The `npc_` spelling asks the **listener**, which is the other half of every pair in the table.
    let listener = cdda_sim::creature::spawn_creature(&mut world, "the trader", (1, 0, 0));
    cdda_sim::square::set_terrain(&mut world, (1, 0, 0), dirt);
    let about_npc = cdda_json::parse(br#"{ "npc_is_on_terrain": "t_dirt" }"#).expect("json");
    let about_npc = cdda_content::dialogue::read_condition(&about_npc).expect("the pair reads");
    assert!(
        cdda_sim::conditions::holds_with(&world, speaker, Some(listener), &about_npc),
        "the trader stands on dirt"
    );
}
