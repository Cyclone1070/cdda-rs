//! **Being underwater is a state a creature is in, and the data asks about it by name.**
//!
//! Reference: `Character::set_underwater` (`character.cpp:12569-12574`) keeps `underwater`, `Character::is_underwater` answers it, and `f_is_underwater`
//! (`condition.cpp:2651`) exposes it to the data as `u_is_underwater` / `npc_is_underwater`. The flag is set when a creature dives (`game.cpp:12525`) or is inside a
//! `WATER_CUBE` tile (`game.cpp:12545-12549`) and cleared when it surfaces - so it is the creature's **own** state, not a property of the ground it stands on.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

fn holds(world: &World, speaker: Entity, listener: Option<Entity>, text: &str) -> bool {
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let condition =
        cdda_content::dialogue::read_condition(&value).expect("the port reads the condition");
    cdda_sim::conditions::holds_with(world, speaker, listener, &condition)
}

#[test]
fn a_creature_says_whether_it_is_underwater() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let diver = cdda_sim::creature::spawn_creature(&mut world, "the diver", (1, 0, 0));

    assert!(
        !holds(&world, survivor, None, r#""u_is_underwater""#),
        "**a creature that has not dived is not underwater**"
    );
    cdda_sim::creature::set_underwater(&mut world, diver, true);
    assert!(
        holds(&world, diver, None, r#""u_is_underwater""#),
        "and one that has is"
    );
    assert!(
        holds(&world, survivor, Some(diver), r#""npc_is_underwater""#),
        "**the `npc_` form asks the listener**, which is the diver"
    );
    assert!(
        !holds(&world, survivor, Some(survivor), r#""npc_is_underwater""#),
        "and not the speaker"
    );

    // Surfacing clears it, which is the other half of `set_underwater`.
    cdda_sim::creature::set_underwater(&mut world, diver, false);
    assert!(
        !holds(&world, diver, None, r#""u_is_underwater""#),
        "surfacing clears the state"
    );
}
