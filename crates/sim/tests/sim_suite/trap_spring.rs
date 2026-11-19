//! **What a trap does when a creature steps on it.** Reference: `trapfunc::eocs` (`trapfunc.cpp:382-400`), reached from `trap::trigger`: the trap's programs run with the creature
//! as the speaker and the trap's own square in the invocation's context as `trap_location`, and the trap is **removed** afterwards.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;

fn a_trap(action: &str, programs: &[&str]) -> cdda_content::traps::Trap {
    cdda_content::traps::Trap {
        id: "tr_test".to_owned(),
        name: "a test trap".to_owned(),
        symbol: i32::from(b'^'),
        color: cdda_core_types::colors::color_from_string("red").expect("a colour"),
        visibility: 0,
        avoidance: 0,
        difficulty: 0,
        radius: 0,
        benign: false,
        always_invisible: false,
        flags: std::collections::BTreeSet::new(),
        action: Some(action.to_owned()),
        eocs: programs.iter().map(|name| (*name).to_owned()).collect(),
    }
}

/// A world with ground, a creature on it, and one program registered under `EOC_mark`.
fn a_world() -> (World, Entity) {
    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -10..=20 {
        for y in -10..=20 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let who = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let program = cdda_json::parse(
        br#"{"type": "effect_on_condition", "id": "EOC_mark", "effect": [{"u_message": "sprung"}]}"#
            .as_slice(),
    )
    .expect("json");
    let read = cdda_content::dialogue::read_program(&program).expect("the program reads");
    cdda_sim::eoc_registry::set_programs(&mut world, &[read]);
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, who)
}

/// **A trap whose action is `eocs` runs its programs and is gone afterwards.**
#[test]
fn a_sprung_trap_runs_its_programs_and_is_removed() {
    let (mut world, who) = a_world();
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("eocs", &["EOC_mark"])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**the trap springs**"
    );
    assert_eq!(
        cdda_sim::messages::lines(&world),
        vec!["sprung".to_owned()],
        "its program ran"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        None,
        "**and the trap is gone**, the way a sprung trap is"
    );
    assert!(
        !cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "springing it again finds nothing"
    );
}

/// **The program is handed the trap's square in its context**, under the name `trap_location`, which is how the telepad's own program knows where it is.
#[test]
fn the_program_is_handed_the_traps_square() {
    let (mut world, who) = a_world();
    // A program that teleports to whatever square the trap hands it: if the context were not set, the effect would find nothing and answer `CannotAnswer`.
    let program = cdda_json::parse(
        br#"{"type": "effect_on_condition", "id": "EOC_hop", "effect": [
            {"u_teleport": {"context_val": "trap_location"}, "force": true}
        ]}"#
        .as_slice(),
    )
    .expect("json");
    let read = cdda_content::dialogue::read_program(&program).expect("the program reads");
    cdda_sim::eoc_registry::set_programs(&mut world, &[read]);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("eocs", &["EOC_hop"])]);
    assert!(cdda_sim::square::set_trap(&mut world, (3, 4, 0), "tr_test"));
    assert!(cdda_sim::trap_spring::spring(&mut world, who, (3, 4, 0)));
    assert_eq!(
        world
            .get::<cdda_sim::Position>(who)
            .map(|at| (at.x, at.y, at.z)),
        Some((3, 4, 0)),
        "**the creature is on the trap's square**, teleported there by the context value the trap set"
    );
}

/// **A trap whose action is not `eocs` is left alone.** The named actions are their own rules and are not ported; half-triggering one - removing the trap without doing what
/// it does - would be worse than leaving it set.
#[test]
fn a_trap_the_port_cannot_act_on_stays_where_it_is() {
    let (mut world, who) = a_world();
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("bubble", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        !cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "nothing happens"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        Some("tr_test".to_owned()),
        "**and the trap is still set**"
    );
    assert!(
        cdda_sim::messages::lines(&world).is_empty(),
        "no program ran"
    );
}
