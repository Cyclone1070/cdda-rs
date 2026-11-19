//! **A step onto a trap springs it.** Reference: `game::walk_move` ends with `here.creature_on_trap( u )` (`game.cpp:11356-11362`), which reaches `trap::trigger` - so the trap
//! under the square a creature walked into does what it does, and a trap it did not walk into does nothing.
//!
//! The `eocs` action is the one the port has (see `trap_spring`), which makes this the wiring round: a program the trap names must run because a creature stepped there.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;

fn a_trap() -> cdda_content::traps::Trap {
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
        action: Some("eocs".to_owned()),
        eocs: vec!["EOC_mark".to_owned()],
        trigger_u: None,
        trigger_npc: None,
        map_regen: None,
    }
}

/// A floor of one terrain, a creature on it, and the marking program installed.
fn a_world() -> (World, Entity) {
    let mut world = World::new();
    let mut table = cdda_sim::terrain_table::TerrainTable::default();
    table.insert(1, true);
    table.insert_move_cost(1, 2);
    cdda_sim::singletons::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain: table,
            furniture: cdda_sim::terrain_table::TerrainTable::new(),
        },
    );
    let mut level = cdda_core_types::level::Level::new();
    for y in -10..=10 {
        for x in -10..=10 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let who = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    // A step is paid for out of the creature's moves, and a refused payment is no step at all.
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Moves { left: 10_000 });
    let program = cdda_json::parse(
        br#"{"type": "effect_on_condition", "id": "EOC_mark", "effect": [{"u_message": "sprung"}]}"#
            .as_slice(),
    )
    .expect("json");
    let read = cdda_content::dialogue::read_program(&program).expect("the program reads");
    cdda_sim::eoc_registry::set_programs(&mut world, &[read]);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap()]);
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, who)
}

/// **Walking onto a trap springs it**: the program runs and the trap is gone.
#[test]
fn a_step_onto_a_trap_springs_it() {
    let (mut world, who) = a_world();
    assert!(cdda_sim::square::set_trap(&mut world, (1, 0, 0), "tr_test"));
    assert_eq!(
        cdda_sim::walk::walk(&mut world, who, 1, 0),
        cdda_sim::creature::StepOutcome::Moved,
        "the step happens"
    );
    assert_eq!(
        cdda_sim::messages::lines(&world),
        vec!["sprung".to_owned()],
        "**the trap under the square the creature walked into fired**"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (1, 0, 0)),
        None,
        "and it is gone"
    );
}

/// **Stepping somewhere else does not spring it.** The trap is sprung by the square the creature **entered**, not by the one it left or by any trap in sight.
#[test]
fn a_step_elsewhere_leaves_the_trap_alone() {
    let (mut world, who) = a_world();
    assert!(cdda_sim::square::set_trap(&mut world, (1, 0, 0), "tr_test"));
    assert_eq!(
        cdda_sim::walk::walk(&mut world, who, 0, 1),
        cdda_sim::creature::StepOutcome::Moved,
        "the creature steps north instead"
    );
    assert!(
        cdda_sim::messages::lines(&world).is_empty(),
        "**nothing fired**"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (1, 0, 0)),
        Some("tr_test".to_owned()),
        "and the trap is still set for whoever walks there"
    );
}

/// **A refused step does not spring the trap either**: there was no move, so there is no square entered.
#[test]
fn a_refused_step_does_not_spring_a_trap() {
    let (mut world, who) = a_world();
    assert!(cdda_sim::square::set_trap(&mut world, (1, 0, 0), "tr_test"));
    // A wall on the same square stops the step, which is refused before the trap could ever be reached.
    cdda_sim::square::set_terrain(&mut world, (1, 0, 0), 0);
    assert_ne!(
        cdda_sim::walk::walk(&mut world, who, 1, 0),
        cdda_sim::creature::StepOutcome::Moved,
        "the step is refused"
    );
    assert!(
        cdda_sim::messages::lines(&world).is_empty(),
        "and nothing fires"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (1, 0, 0)),
        Some("tr_test".to_owned()),
        "the trap is untouched"
    );
}
