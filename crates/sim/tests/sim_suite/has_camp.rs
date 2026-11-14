//! **`u_has_camp` asks whether the player owns a camp anywhere on the overmap.**
//!
//! Reference: `f_u_has_camp` (`condition.cpp:1985-2001`): it walks the player's `camps` set, finds the camp on each tile, and answers true for the first one whose **owner** is
//! the player's faction - `your_followers`, the constant `avatar::get_faction` returns (`avatar.cpp:102`, `:1182-1185`).

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

#[test]
fn has_camp_answers_for_a_camp_the_player_owns() {
    let mut world = World::new();
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let value = cdda_json::parse(br#""u_has_camp""#).expect("json");
    let condition =
        cdda_content::dialogue::read_condition(&value).expect("**the port reads `u_has_camp`**");

    assert!(
        !cdda_sim::conditions::holds_with(&world, speaker, None, &condition),
        "**a survivor with no camp has none**"
    );

    // Somebody else's camp is not the player's, and the reference checks the owner rather than the tile.
    cdda_sim::camps::found(&mut world, (3, 4), "hells_raiders");
    assert!(
        !cdda_sim::conditions::holds_with(&world, speaker, None, &condition),
        "a camp owned by another faction is not the player's camp"
    );

    cdda_sim::camps::found(&mut world, (7, 8), cdda_sim::camps::PLAYERS_FACTION);
    assert!(
        cdda_sim::conditions::holds_with(&world, speaker, None, &condition),
        "**and one owned by the player is**"
    );
    assert_eq!(
        cdda_sim::camps::count(&world),
        2,
        "the two camps are two entities, not a list beside them"
    );
}
