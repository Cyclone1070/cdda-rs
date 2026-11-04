//! Throwing something: the spread rules, the landing rule and the item's move, wired together.
//!
//! Reference: `Character::throw_item` (`ranged.cpp:1467-1600`) - the spread from `throwing_dispersion`, a roll of it, `projectile_attack_roll` for the aim, and the miss
//! branch of `projectile_attack` for the square. The port has every one of those rules; this is the wire that spends them.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;

/// A thrower holding one rock, with the item table that says what a rock is.
fn a_thrower_with_a_rock(world: &mut World) -> (Entity, Entity) {
    let kinds = [
        cdda_sim::item_registry::Kind {
            id: "rock",
            volume_ml: 250,
            weight_g: 500,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: -2,
        },
        cdda_sim::item_registry::Kind {
            id: "anvil",
            volume_ml: 5_000,
            weight_g: 20_000,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: -2,
        },
    ];
    cdda_sim::item_registry::set_item_types(world, &kinds);
    cdda_sim::singletons::put_rolls(world, cdda_core_types::rng::Rolls::new(1_234));
    let thrower = cdda_sim::creature::spawn_creature(world, "the thrower", (0, 0, 0));
    world
        .entity_mut(thrower)
        .insert(cdda_sim::creature::Strength { base: 8 });
    let names = cdda_sim::item_registry::snapshot(world);
    let rock = cdda_sim::drop_item(
        world,
        (0, 0),
        names.id_of("rock").expect("a rock"),
        1,
    );
    world.entity_mut(rock).insert(cdda_sim::creature::Carried(thrower));
    (thrower, rock)
}

#[test]
fn a_thrown_rock_leaves_the_hand_and_lands_near_where_it_was_aimed() {
    let mut world = World::new();
    let (thrower, rock) = a_thrower_with_a_rock(&mut world);
    let landed = cdda_sim::throwing::throw_at(&mut world, thrower, rock, (10, 0, 0));
    let cdda_sim::throwing::Toss::Landed { at, .. } = landed else {
        panic!("the rock is thrown: {landed:?}");
    };
    assert!(
        world.get::<cdda_sim::creature::Carried>(rock).is_none(),
        "a thrown thing is no longer carried"
    );
    let position = world.get::<cdda_sim::Position>(rock).copied();
    assert_eq!(
        position.map(|place| (place.x, place.y, place.z)),
        Some(at),
        "and it stands on the square the throw says"
    );
    // Ten squares out with a 500 g rock at skill zero: the spread is 2500, and the aim rules bound where that lands.
    assert!(
        i32::abs(at.0) <= 13 && i32::abs(at.1) <= 13,
        "a throw ten squares out does not leave the neighbourhood: {at:?}"
    );
}

#[test]
fn throwing_at_your_own_square_changes_nothing_and_spends_no_dice() {
    let mut world = World::new();
    let (thrower, rock) = a_thrower_with_a_rock(&mut world);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(8_181));
    assert_eq!(
        cdda_sim::throwing::throw_at(&mut world, thrower, rock, (0, 0, 0)),
        cdda_sim::throwing::Toss::AtYourself,
        "a throw at your own square is refused, as the reference refuses a shot at its own square"
    );
    assert!(
        world.get::<cdda_sim::creature::Carried>(rock).is_some(),
        "and the rock stays in the hand"
    );
}

#[test]
fn a_thing_nobody_is_holding_cannot_be_thrown() {
    let mut world = World::new();
    let (thrower, rock) = a_thrower_with_a_rock(&mut world);
    world
        .entity_mut(rock)
        .remove::<cdda_sim::creature::Carried>();
    assert_eq!(
        cdda_sim::throwing::throw_at(&mut world, thrower, rock, (5, 5, 0)),
        cdda_sim::throwing::Toss::NotCarried,
        "and the port says so rather than throwing something that is on the floor"
    );
}

/// What a thrown thing is worth as a blow: the arm's strength, thinned by inexperience, and capped by the thing's own weight.
///
/// Reference: `Character::thrown_item_adjusted_damage` (`ranged.cpp:1411-1428`): `arm_strength / 2` times `(skill / 10) * 0.85 + 0.15`, and the reference's own comment says
/// what the cap is - "up to str/2 or weight/100g (lower), so 10 str is 5 damage before multipliers". At skill zero the multiplier is 0.15, so a novice with eight arms
/// throws for 0.6 however heavy the rock; at full skill it is the whole 4.0.
#[test]
fn a_thrown_thing_s_damage_is_the_arm_thinned_by_inexperience_and_capped_by_weight() {
    use cdda_sim::throwing::damage_of_throw;

    assert!(
        (damage_of_throw(8, 0, 500) - 0.6).abs() < 1e-9,
        "a novice's arm gives a fifth of itself: {}",
        damage_of_throw(8, 0, 500)
    );
    assert!(
        (damage_of_throw(8, 10, 500) - 4.0).abs() < 1e-9,
        "and a practised one gives all of it: {}",
        damage_of_throw(8, 10, 500)
    );
    // Ten strength is five damage at full skill, which is the number the reference's comment names.
    assert!(
        (damage_of_throw(10, 10, 100_000) - 5.0).abs() < 1e-9,
        "ten strength is five damage before the weight cap: {}",
        damage_of_throw(10, 10, 100_000)
    );
    // And the weight caps it: 100 g is worth one point, so a 300 g pebble cannot do more than three however strong the arm.
    assert!(
        (damage_of_throw(20, 10, 300) - 3.0).abs() < 1e-9,
        "a light thing cannot carry a heavy arm's blow: {}",
        damage_of_throw(20, 10, 300)
    );
}

/// A rock thrown at a creature hurts it, and the hurt is the same number the rule names.
///
/// The landing square is read from a first throw and the monster is put **there**, because the aim rules spread a novice's throw and a monster standing on the aim is not the
/// same as a monster standing where the rock went. Skill ten makes the blow four points rather than a novice's 0.6, which a whole-number blow can actually show.
#[test]
fn a_rock_that_lands_on_a_creature_hurts_it() {
    let mut world = World::new();
    let (thrower, rock) = a_thrower_with_a_rock(&mut world);
    cdda_sim::skills_of::set_level(&mut world, thrower, "throw", 10);
    let cdda_sim::throwing::Toss::Landed { at: first, .. } =
        cdda_sim::throwing::throw_at(&mut world, thrower, rock, (10, 0, 0))
    else {
        panic!("the rock is thrown");
    };

    // A fresh world with the same seed and the same throw, and a creature standing where the first rock landed.
    let mut world = World::new();
    let (thrower, rock) = a_thrower_with_a_rock(&mut world);
    cdda_sim::skills_of::set_level(&mut world, thrower, "throw", 10);
    let target = cdda_sim::creature::spawn_creature(&mut world, "the target", first);
    world
        .entity_mut(target)
        .insert(cdda_sim::creature::Health { hp: 100, max_hp: 100 });
    let landed = cdda_sim::throwing::throw_at(&mut world, thrower, rock, (10, 0, 0));
    let cdda_sim::throwing::Toss::Landed { at, hit, .. } = landed else {
        panic!("the rock is thrown: {landed:?}");
    };
    assert_eq!(at, first, "the same seed lands the same rock on the same square");
    assert_eq!(
        hit,
        Some((target, 4)),
        "a full-skill arm throws four points of hurt"
    );
    assert_eq!(
        cdda_sim::creature::health_of(&world, target),
        Some(96),
        "and the creature is down by exactly that much"
    );
}

/// Armour takes the edge off a thrown blow, and can stop it entirely.
///
/// Reference: `Creature::deal_projectile_attack` resolves its impact through `absorb_hit` (`creature.cpp:1300-1400`), and `monster::absorb_hit` (`monster.cpp:2085`)
/// subtracts the target's resistance, never below zero. The fixture's rock has no thrown damage of its own, so the blow is the arm's four points and the armour is the only
/// thing between them.
#[test]
fn armour_takes_the_edge_off_a_thrown_blow_and_never_heals() {
    for (armour, expected) in [(0, 4), (3, 1), (4, 0), (100, 0)] {
        let mut world = World::new();
        let (thrower, rock) = a_thrower_with_a_rock(&mut world);
        cdda_sim::skills_of::set_level(&mut world, thrower, "throw", 10);
        let cdda_sim::throwing::Toss::Landed { at, .. } =
            cdda_sim::throwing::throw_at(&mut world, thrower, rock, (10, 0, 0))
        else {
            panic!("the rock is thrown");
        };

        let mut world = World::new();
        let (thrower, rock) = a_thrower_with_a_rock(&mut world);
        cdda_sim::skills_of::set_level(&mut world, thrower, "throw", 10);
        let target = cdda_sim::creature::spawn_creature(&mut world, "the target", at);
        world
            .entity_mut(target)
            .insert(cdda_sim::creature::Health { hp: 100, max_hp: 100 })
            .insert(cdda_sim::creature::Armour { bash: armour, cut: 0, stab: 0 });
        let cdda_sim::throwing::Toss::Landed { hit, .. } =
            cdda_sim::throwing::throw_at(&mut world, thrower, rock, (10, 0, 0))
        else {
            panic!("the rock is thrown");
        };
        assert_eq!(
            hit,
            Some((target, expected)),
            "armour {armour} leaves {expected} of the four-point blow"
        );
        assert_eq!(
            cdda_sim::creature::health_of(&world, target),
            Some(100 - expected),
            "and the creature is down by that much, never healed by its armour"
        );
    }
}

/// A heavy thing landing makes a sound, and the boundary is **strictly over** five hundred grams.
///
/// Reference: `Character::throw_item` gives a projectile the `HEAVY_HIT` effect when it weighs more than **500 grams** (`ranged.cpp:1522-1524`) - `weight > 500_gram`,
/// not `>=` - and the landing branch answers it with a thud of volume **8** (`ballistics.cpp:180-186`). The fixture's rock is exactly 500 grams, so it is the case that
/// proves the boundary: it lands in silence, and a stone of five kilograms does not.
#[test]
fn a_thing_over_half_a_kilo_landing_makes_a_sound_and_five_hundred_grams_does_not() {
    use cdda_sim::throwing::{Toss, throw_at};

    // Exactly five hundred grams: silent, because the reference's test is `>`.
    let mut world = World::new();
    let (thrower, rock) = a_thrower_with_a_rock(&mut world);
    let Toss::Landed { .. } = throw_at(&mut world, thrower, rock, (10, 0, 0)) else {
        panic!("the rock is thrown");
    };
    assert!(
        cdda_sim::sounds::heard_this_turn(&mut world).is_empty(),
        "five hundred grams exactly is not a heavy hit"
    );

    // Five kilograms: a thud, at the square it stopped on.
    let mut world = World::new();
    let (thrower, rock) = a_thrower_with_a_rock(&mut world);
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rock",
            volume_ml: 2_000,
            weight_g: 5_000,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: -2,
        }],
    );
    let Toss::Landed { at, .. } = throw_at(&mut world, thrower, rock, (10, 0, 0)) else {
        panic!("the rock is thrown");
    };
    let heard = cdda_sim::sounds::heard_this_turn(&mut world);
    assert_eq!(heard.len(), 1, "one thing landed: {heard:?}");
    assert_eq!(
        heard.first().map(|sound| sound.position),
        Some(at),
        "the thud is where it stopped"
    );
    assert_eq!(
        heard.first().map(|sound| sound.volume),
        Some(8),
        "volume eight, which is the reference's own"
    );
}

/// A heavy thing landing in water splashes louder than one landing on the floor, and a light thing says nothing either way.
///
/// Reference: the landing branch of `projectile_attack` (`ballistics.cpp:180-186`) makes volume **10** with a `splash!` when the square carries `TFLAG_LIQUID` and volume
/// **8** with a thud otherwise - and both live inside the `HEAVY_HIT` test, so a light thing landing in water is as silent as one landing on grass.
#[test]
fn a_heavy_landing_in_water_is_louder_than_one_on_the_floor() {
    use cdda_sim::throwing::landing_noise;

    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    // Four arguments: x, y, the terrain code, and the code an empty square starts as.
    level.set_terrain(0, 0, 1, 0);
    level.set_terrain(1, 0, 2, 0);
    cdda_sim::load_level(&mut world, &level);
    cdda_sim::terrain_types::set_types(
        &mut world,
        [
            ("t_floor".to_owned(), 1u32),
            ("t_water_sh".to_owned(), 2u32),
        ],
    );
    cdda_sim::terrain_types::set_flags(
        &mut world,
        [("t_water_sh".to_owned(), vec!["LIQUID".to_owned()])],
    );

    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 0, 0)),
        Some(2),
        "the water is on the square the test thinks it is"
    );
    assert!(
        cdda_sim::terrain_types::has_flag(&world, 2, "LIQUID"),
        "and its type carries the flag"
    );
    // Five kilograms on the floor is the thud, and on water the splash.
    assert_eq!(landing_noise(&world, (0, 0, 0), 5_000), Some(8), "a thud on the floor");
    assert_eq!(landing_noise(&world, (1, 0, 0), 5_000), Some(10), "a splash in the water");
    // Exactly five hundred grams is not heavy, on either.
    assert_eq!(landing_noise(&world, (0, 0, 0), 500), None, "five hundred grams on the floor");
    assert_eq!(landing_noise(&world, (1, 0, 0), 500), None, "five hundred grams in the water");
}

/// A rock thrown at a creature is judged against **that creature's** size, not against a whole tile.
///
/// Reference: `projectile_attack` takes `target_critter->ranged_target_size()` (`ballistics.cpp:255-264`), which for an ordinary creature is `occupied_tile_fraction` of its size. Before this
/// round the port passed `1.0` for every throw - a target that fills its tile - so **every creature was as easy to hit as a hulk**.
#[test]
fn a_rock_thrown_at_a_creature_is_judged_against_the_size_of_that_creature() {
    let mut world = World::new();
    let (thrower, rock) = a_thrower_with_a_rock(&mut world);
    // A small thing standing on the square the rock is aimed at, with a size the port's own table can price.
    let beast = cdda_sim::creature::spawn_creature(&mut world, "a small thing", (3, 0, 0));
    world.entity_mut(beast).insert(cdda_sim::creature::Creature {
        name: "a small thing".to_owned(),
        size: cdda_sim::CreatureSize::Tiny,
    });
    assert!(
        !cdda_sim::creature::creature_at(&world, (3, 0, 0)).is_empty(),
        "the small thing is on the square the rock is aimed at, which is what makes this the creature case"
    );

    let tossed = cdda_sim::throwing::throw_at(&mut world, thrower, rock, (3, 0, 0));
    assert!(
        matches!(tossed, cdda_sim::throwing::Toss::Landed { .. }),
        "the rock is thrown at the small thing: {tossed:?}"
    );

    // The differential is the proof: the **same throw**, from the same hand, on the same dice, at the same range - once at a creature and once at bare ground. Only the size the miss is
    // measured against differs, so a port that still passed a whole tile for every throw would report the same number twice.
    let cdda_sim::throwing::Toss::Landed { missed_by: at_creature, .. } = tossed else {
        panic!("the rock landed");
    };
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1_234));
    let rock_kind = {
        let names = cdda_sim::item_registry::snapshot(&mut world);
        names.id_of("rock").expect("a rock")
    };
    let second = cdda_sim::drop_item(&mut world, (0, 0), rock_kind, 1);
    world.entity_mut(second).insert(cdda_sim::creature::Carried(thrower));
    let at_ground = cdda_sim::throwing::throw_at(&mut world, thrower, second, (3, 1, 0));
    let cdda_sim::throwing::Toss::Landed { missed_by: at_nothing, .. } = at_ground else {
        panic!("the second rock landed");
    };
    assert!(
        at_creature > at_nothing,
        "the same throw is worse against a small creature ({at_creature}) than against bare ground ({at_nothing})"
    );
}
