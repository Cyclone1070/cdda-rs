//! **Noticing a trap in time.** Reference: `Character::avoid_trap` (`character.cpp:12182-12203`) and `trap::can_see` (`trap.cpp:319-331`): a contest of `dice( 3, round( dex +
//! dodge * 1.5 ) )` against `dice( 3, avoidance )` when the creature has noticed the trap, or `dice( 6, avoidance )` when it has not, and a trap that is dodged is announced and
//! remembered.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;

fn a_trap() -> cdda_content::traps::Trap {
    cdda_content::traps::Trap {
        id: "tr_test".to_owned(),
        // The data's trap names carry no article - the message adds one ("You've spotted a %1$s!").
        name: "bear trap".to_owned(),
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

/// A world with a floor, a creature, and the marking program installed.
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
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(7));
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, who)
}

/// **A creature that rolls higher steps over the trap**, is told so, and remembers it from then on.
#[test]
fn a_noticed_trap_is_stepped_over_announced_and_remembered() {
    let (mut world, who) = a_world();
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Dexterity { base: 100 });
    let mut trap = a_trap();
    trap.avoidance = 1;
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        !cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**the creature steps over it**"
    );
    assert_eq!(
        cdda_sim::messages::lines(&world),
        vec!["You've spotted a bear trap!".to_owned()],
        "and is told what it spotted"
    );
    assert!(
        cdda_sim::trap_avoidance::knows(&world, who, (0, 0, 0)),
        "**and remembers it**, which is what makes the next roll three dice instead of six"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        Some("tr_test".to_owned()),
        "the trap is still set"
    );
}

/// **A creature that rolls lower sets it off**, and is told nothing about it.
#[test]
fn a_trap_the_creature_cannot_beat_fires() {
    let (mut world, who) = a_world();
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Dexterity { base: 1 });
    let mut trap = a_trap();
    trap.avoidance = 99;
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**the trap fires**"
    );
    assert_eq!(
        cdda_sim::messages::lines(&world),
        vec!["sprung".to_owned()],
        "the trap's program ran, and no spotting message was added"
    );
    assert!(
        !cdda_sim::trap_avoidance::knows(&world, who, (0, 0, 0)),
        "nothing was remembered"
    );
}

/// **An `UNDODGEABLE` trap is not stepped over**, however quick the creature is. Reference: `map.cpp:9932` skips the avoidance branch for the flag.
#[test]
fn an_undodgeable_trap_is_not_avoided() {
    let (mut world, who) = a_world();
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Dexterity { base: 100 });
    let mut trap = a_trap();
    trap.avoidance = 1;
    trap.flags.insert("UNDODGEABLE".to_owned());
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**it fires anyway**"
    );
}

/// **A monster does not avoid traps.** Reference: `game.cpp:12221` calls `creature_on_trap( *c, false )`, so the whole avoidance branch is skipped.
#[test]
fn a_creature_that_may_not_avoid_does_not() {
    let (mut world, who) = a_world();
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Dexterity { base: 100 });
    let mut trap = a_trap();
    trap.avoidance = 1;
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**no roll is made, so the trap fires**"
    );
}

/// **A trap that is always invisible is never spotted**, so it is never announced and never remembered. Reference: `trap::can_see` answers false for it (`trap.cpp:325-327`).
#[test]
fn an_always_invisible_trap_is_not_announced() {
    let (mut world, who) = a_world();
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Dexterity { base: 100 });
    let mut trap = a_trap();
    trap.avoidance = 1;
    trap.always_invisible = true;
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        !cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "the creature still steps over it"
    );
    assert!(
        cdda_sim::messages::lines(&world).is_empty(),
        "**but is not told what it stepped over**"
    );
    assert!(
        !cdda_sim::trap_avoidance::knows(&world, who, (0, 0, 0)),
        "and cannot remember it"
    );
}

/// **A creature with no dexterity gets no roll**, so the trap fires: the port's hand-made creatures have no stats until something gives them some, and a dodge that happened
/// because the component was missing would be the port inventing a rule.
#[test]
fn a_creature_with_no_stats_does_not_dodge() {
    let (mut world, who) = a_world();
    let mut trap = a_trap();
    trap.avoidance = 0;
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**no body, no dodge**"
    );
}

/// **A trap the creature has already noticed is easier to beat**: three dice against its avoidance instead of six.
///
/// The numbers are chosen so the answer cannot wobble: a creature with one dexterity rolls three one-sided dice, so its roll is **3**, and a trap with avoidance **1** rolls
/// either three ones (**3**) or six ones (**6**). Unnoticed, `3 >= 6` is false and the trap fires; noticed, `3 >= 3` is true and the creature steps over it. This is the only
/// case that separates the two die counts, and it separates them without depending on a seed.
#[test]
fn a_noticed_trap_rolls_three_dice_instead_of_six() {
    let (mut world, who) = a_world();
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Dexterity { base: 1 });
    let mut trap = a_trap();
    trap.avoidance = 1;
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));

    // Unnoticed: six dice against its avoidance, and the creature loses.
    assert!(
        !cdda_sim::trap_avoidance::can_see(&world, who, &a_trap_with(1), (0, 0, 0)),
        "the creature has not noticed the trap"
    );
    assert!(
        cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**unnoticed, the trap fires**"
    );

    // Noticed: the same trap on the same square, now known - three dice, and the creature wins.
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    cdda_sim::trap_avoidance::remember(&mut world, who, (0, 0, 0));
    assert!(
        !cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**noticed, the creature steps over it**"
    );
}

/// A trap with the given avoidance, for the `can_see` question above.
fn a_trap_with(avoidance: i32) -> cdda_content::traps::Trap {
    let mut trap = a_trap();
    trap.avoidance = avoidance;
    trap
}
