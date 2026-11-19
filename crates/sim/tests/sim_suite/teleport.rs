//! Where a teleport lands, and what happens when it lands somewhere solid.
//!
//! Reference: `teleport::teleport_creature` and `teleport::teleport_to_point` (`teleport.cpp:152-260`). The draws are the store's own, so a gate can hold them still and pin the
//! arithmetic: an angle of zero is `cos( 0 )` of one and `sin( 0 )` of zero, which puts the creature exactly `distance` squares east whatever the platform's libm does.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// A world of one floor code with move cost two, so every square inside it is standable and nothing outside has terrain at all.
fn a_world() -> (World, Entity) {
    let mut world = World::new();
    let mut table = cdda_sim::terrain_table::TerrainTable::default();
    table.insert(1, true);
    // A square's standability is its move cost, the same question a route asks: without one the search refuses the square and so does a teleport.
    table.insert_move_cost(1, 2);
    cdda_sim::singletons::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain: table,
            furniture: cdda_sim::terrain_table::TerrainTable::default(),
        },
    );
    let mut level = cdda_core_types::level::Level::new();
    for y in -10..=10 {
        for x in -10..=10 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(1));
    let who = world.spawn(cdda_sim::Position { x: 0, y: 0, z: 0 }).id();
    (world, who)
}

#[test]
fn a_teleport_draws_an_angle_and_a_distance_and_moves_the_creature_exactly_there() {
    let (mut world, who) = a_world();
    // The reference draws `rng( 0, 360 )` and then `rng( min, max )`; with a generator that answers the bottom of every range, the angle is zero and the distance is the
    // minimum, which is the one pair whose arithmetic cannot differ between platforms.
    let outcome = cdda_sim::teleport::creature(&mut world, who, 3, 5, true);
    assert_eq!(
        outcome,
        cdda_sim::teleport::Teleport::Moved {
            from: (0, 0, 0),
            to: (3, 0, 0)
        },
        "**an angle of zero sends the creature east by the distance drawn**, and the distance drawn is the minimum"
    );
    assert_eq!(
        world
            .get::<cdda_sim::Position>(who)
            .map(|at| (at.x, at.y, at.z)),
        Some((3, 0, 0)),
        "and the store says so"
    );
}

/// **A safe teleport into a wall refuses; an unsafe one is death.**
///
/// Reference: `teleport::teleport_to_point` (`teleport.cpp:198-260`): the same square is no teleport, a solid landing returns false with a message when the jump is safe, and
/// without safety the creature takes 9,999 damage to the torso - the reference's way of saying there is nothing left to find.
#[test]
fn landing_in_a_wall_refuses_if_safe_and_kills_if_not() {
    let (mut world, who) = a_world();
    // A square with no terrain at all is impassable, which is how the port answers the reference's `impassable`.
    let outside = (100, 100, 0);
    assert_eq!(
        cdda_sim::teleport::to_point(&mut world, who, outside, true),
        cdda_sim::teleport::Teleport::Refused,
        "**a safe teleport into solid rock does nothing at all**"
    );
    assert_eq!(
        world
            .get::<cdda_sim::Position>(who)
            .map(|at| (at.x, at.y, at.z)),
        Some((0, 0, 0)),
        "and the creature has not moved"
    );
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Health { hp: 40, max_hp: 40 });
    assert_eq!(
        cdda_sim::teleport::to_point(&mut world, who, outside, false),
        cdda_sim::teleport::Teleport::Died,
        "an unsafe one kills"
    );
    assert!(
        cdda_sim::creature::health_of(&world, who).is_some_and(|hp| hp <= 0),
        "**and the creature is dead where it stood**, not moved and not merely hurt"
    );
    assert_eq!(
        cdda_sim::teleport::to_point(&mut world, who, (0, 0, 0), true),
        cdda_sim::teleport::Teleport::Unchanged,
        "and a teleport to the square it is already on is no teleport"
    );
}

#[test]
fn a_short_range_teleport_spell_moves_its_caster() {
    let mut world = World::new();
    let mut table = cdda_sim::terrain_table::TerrainTable::default();
    table.insert(1, true);
    table.insert_move_cost(1, 2);
    cdda_sim::singletons::set_prototypes(
        &mut world,
        cdda_sim::Prototypes {
            terrain: table,
            furniture: cdda_sim::terrain_table::TerrainTable::default(),
        },
    );
    let mut level = cdda_core_types::level::Level::new();
    for y in -10..=10 {
        for x in -10..=10 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(1));
    let caster = world.spawn(cdda_sim::Position { x: 0, y: 0, z: 0 }).id();
    let spell = cdda_content::spells::Spell {
        id: "blink".to_owned(),
        name: "Blink".to_owned(),
        effect_name: "short_range_teleport".to_owned(),
        effect_str: String::new(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: cdda_content::spells::Scaling::default(),
        range: cdda_content::spells::Scaling {
            start: Some(4.0),
            increment: Some(0.0),
            end: Some(4.0),
        },
        area: cdda_content::spells::Scaling::default(),
        energy: cdda_content::spells::Scaling::default(),
        duration: cdda_content::spells::Scaling::default(),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        valid_targets: ["hostile".to_owned()].into_iter().collect(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags: std::collections::BTreeSet::new(),
    };
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the spell moved its caster"
    );
    assert_eq!(
        world
            .get::<cdda_sim::Position>(caster)
            .map(|at| (at.x, at.y, at.z)),
        Some((4, 0, 0)),
        "**a range of four and no area is a jump of exactly four squares**"
    );
}

/// **A target above the world or below it is refused.** Reference: `teleport::teleport_to_point` answers `false` when the absolute z is beyond `OVERMAP_HEIGHT` or
/// `-OVERMAP_DEPTH` (`teleport.cpp:181-186`), before any landing rule runs - so a target off the top of the world cannot be "landed on" at all.
#[test]
fn a_target_outside_the_world_s_height_is_refused() {
    let (mut world, who) = a_world();
    let too_high = cdda_sim::falling::OVERMAP_HEIGHT + 1;
    assert_eq!(
        cdda_sim::teleport::to_point(&mut world, who, (0, 0, too_high), false),
        cdda_sim::teleport::Teleport::Refused,
        "**above the world is refused, not landed on**"
    );
    let too_deep = -cdda_sim::falling::OVERMAP_DEPTH - 1;
    assert_eq!(
        cdda_sim::teleport::to_point(&mut world, who, (0, 0, too_deep), false),
        cdda_sim::teleport::Teleport::Refused,
        "and so is below it"
    );
}

/// **A forced landing takes the nearest square it can stand on.** Reference: `teleport::teleport_to_point` (`teleport.cpp:215-223`) takes `closest_points_first( target, 5 )`,
/// drops the target itself, and picks the first square that is `passable_through`. When nothing within five squares is passable it keeps the solid target.
#[test]
fn a_forced_landing_takes_the_nearest_open_square() {
    let (mut world, _who) = a_world();
    // A wall in the middle of open floor: every neighbour within one square is open, so the nearest landing is one step away.
    cdda_sim::square::set_terrain(&mut world, (3, 3, 0), 2);
    let landing = cdda_sim::teleport::forced_landing(&mut world, (3, 3, 0));
    let Some((x, y, z)) = landing else {
        panic!("**there are open squares within five, so one is chosen**");
    };
    assert_eq!(z, 0, "the landing stays on the same level");
    let distance = (x - 3).abs().max((y - 3).abs());
    assert!(
        (1..=5).contains(&distance),
        "**the landing is within five squares: {distance}**"
    );
    assert!((x, y) != (3, 3), "**and it is not the wall itself**");
}

/// **A creature wearing a teleport lock cannot be teleported.** Reference: `teleport::teleport_to_point` refuses when the teleportee wears an item or carries an effect with
/// `DIMENSIONAL_ANCHOR`, or an effect with `TELEPORT_LOCK` (`teleport.cpp:191-201`) - the "strange, inwards force".
#[test]
fn a_teleport_lock_refuses_the_jump() {
    let (mut world, who) = a_world();
    let mut kind = cdda_sim::effect_registry::bare("effect_teleport_lock");
    kind.flags.insert("TELEPORT_LOCK".to_owned());
    cdda_sim::effect_registry::set_effect_types(&mut world, &[kind]);
    cdda_sim::applied_effects::add(
        &mut world,
        who,
        cdda_sim::effects::ActiveEffect {
            id: "effect_teleport_lock".to_owned(),
            intensity: 1,
            duration: 10,
            permanent: false,
        },
    );

    assert_eq!(
        cdda_sim::teleport::to_point(&mut world, who, (5, 5, 0), false),
        cdda_sim::teleport::Teleport::Refused,
        "**the lock holds: nothing happens**"
    );
}

/// **A creature wearing an anchored item cannot be teleported either.** Reference: the same guard asks `worn_with_flag( json_flag_DIMENSIONAL_ANCHOR )` beside the two effect
/// checks (`teleport.cpp:191-201`), so the anchor works whether it is a thing you wear or a thing that has been done to you.
#[test]
fn a_worn_dimensional_anchor_refuses_the_jump() {
    let (mut world, who) = a_world();
    world.spawn((
        cdda_sim::creature::WornBy(who),
        cdda_sim::creature::WornFlags(vec!["DIMENSIONAL_ANCHOR".to_owned()]),
    ));

    assert_eq!(
        cdda_sim::teleport::to_point(&mut world, who, (5, 5, 0), false),
        cdda_sim::teleport::Teleport::Refused,
        "**the anchor holds: nothing happens**"
    );
}

/// **A rider is put down before the teleport happens.** Reference: `teleport_to_point` unboards a character who is in a vehicle before it looks at the landing square
/// (`teleport.cpp:203-206`), so a teleported driver is no longer sitting in a seat that stayed behind.
#[test]
fn a_teleport_unboards_the_rider_first() {
    let (mut world, who) = a_world();
    let car = world.spawn_empty().id();
    world.entity_mut(who).insert(cdda_sim::vehicles::Riding {
        vehicle: car,
        part: 0,
    });

    assert!(
        matches!(
            cdda_sim::teleport::to_point(&mut world, who, (5, 5, 0), true),
            cdda_sim::teleport::Teleport::Moved { .. }
        ),
        "the teleport still happens"
    );
    assert!(
        world.get::<cdda_sim::vehicles::Riding>(who).is_none(),
        "**and the rider is out of the vehicle**"
    );
}

/// **A forced teleport into a solid square lands on the nearest open one** rather than refusing or dying - `teleport.cpp:215-223`. `forced_landing` already knew how to find
/// that square; until now nothing asked it, so a forced jump into a wall was still a refusal and the rule was a function without a caller.
#[test]
fn a_forced_teleport_into_a_wall_lands_beside_it() {
    let (mut world, who) = a_world();
    cdda_sim::square::set_terrain(&mut world, (3, 3, 0), 2);
    let outcome = cdda_sim::teleport::to_point_with(&mut world, who, (3, 3, 0), true, true, false);
    let cdda_sim::teleport::Teleport::Moved { to, .. } = outcome else {
        panic!("**a forced teleport does not refuse a solid landing**: {outcome:?}");
    };
    assert!((to.0, to.1) != (3, 3), "it does not land in the wall");
    let distance = (to.0 - 3).abs().max((to.1 - 3).abs());
    assert!(
        (1..=5).contains(&distance),
        "and it lands within five squares of where it aimed: {distance}"
    );
    assert_eq!(
        world
            .get::<cdda_sim::Position>(who)
            .map(|at| (at.x, at.y, at.z)),
        Some(to),
        "the creature really is there"
    );
}

/// **A `force_safe` teleport resolves a solid landing the same way a forced one does** - the reference tests `if( force || force_safe )` (`teleport.cpp:214-225`), so the flag
/// is a second way to the same landing rule and not a third behaviour.
#[test]
fn a_force_safe_teleport_into_a_wall_lands_beside_it() {
    let (mut world, who) = a_world();
    cdda_sim::square::set_terrain(&mut world, (3, 3, 0), 2);
    let outcome = cdda_sim::teleport::to_point_with(&mut world, who, (3, 3, 0), true, false, true);
    assert!(
        matches!(outcome, cdda_sim::teleport::Teleport::Moved { .. }),
        "**force_safe is enough on its own**: {outcome:?}"
    );
}

/// **A `force_safe` teleport lands on a square nobody is standing on**, and a plain one does not care - `teleport.cpp:252-270`: with `force_safe` and a creature already at the
/// target, the reference walks outward to the nearest square that is passable and empty. Without the flag it telefrags, which is a rule the port has not written.
#[test]
fn a_force_safe_teleport_lands_where_nobody_stands() {
    let (mut world, who) = a_world();
    let other = cdda_sim::creature::spawn_creature(&mut world, "other", (3, 3, 0));
    cdda_sim::PositionIndex::rebuild(&mut world);
    cdda_sim::PositionIndex::rebuild(&mut world);
    let outcome = cdda_sim::teleport::to_point_with(&mut world, who, (3, 3, 0), true, false, true);
    let cdda_sim::teleport::Teleport::Moved { to, .. } = outcome else {
        panic!("the teleport moves: {outcome:?}");
    };
    assert!(
        (to.0, to.1) != (3, 3),
        "**it does not land on top of the other creature**"
    );
    assert!(
        (to.0 - 3).abs().max((to.1 - 3).abs()) <= 5,
        "and it is within five squares of the target: {to:?}"
    );
    assert_eq!(
        world
            .get::<cdda_sim::Position>(other)
            .map(|at| (at.x, at.y, at.z)),
        Some((3, 3, 0)),
        "the creature that was there has not moved"
    );
}

/// **`force_safe` overrules a dimensional anchor** exactly as `force` does - the reference's guard is `if( !force && !force_safe && ... )` (`teleport.cpp:188-201`), so the flag
/// is a bypass and not merely a landing choice.
#[test]
fn force_safe_pushes_a_locked_creature_through() {
    let (mut world, who) = a_world();
    let mut kind = cdda_sim::effect_registry::bare("effect_teleport_lock");
    kind.flags.insert("TELEPORT_LOCK".to_owned());
    cdda_sim::effect_registry::set_effect_types(&mut world, &[kind]);
    cdda_sim::applied_effects::add(
        &mut world,
        who,
        cdda_sim::effects::ActiveEffect {
            id: "effect_teleport_lock".to_owned(),
            intensity: 1,
            duration: 10,
            permanent: false,
        },
    );
    assert!(
        matches!(
            cdda_sim::teleport::to_point_with(&mut world, who, (4, 4, 0), true, false, false),
            cdda_sim::teleport::Teleport::Refused
        ),
        "without the flag the lock refuses the jump"
    );
    assert!(
        matches!(
            cdda_sim::teleport::to_point_with(&mut world, who, (4, 4, 0), true, false, true),
            cdda_sim::teleport::Teleport::Moved { .. }
        ),
        "**and with it the creature goes anyway**"
    );
}
