//! Every singleton setter, on a **fresh** store, read back.
//!
//! This file exists because of a bug: `singletons::set_weather` wrote only to a component that already existed, so on a new world it
//! did nothing and said nothing. That is the fourth instance of the same failure mode - a rule that never matches, a number nothing
//! re-derives, a table entry that is not a running spell, and a setter that only updated what was already there - and each one was
//! found by a test that **wrote something and then asked the store what it held**.
//!
//! So the shape is swept rather than spot-checked: every setter that installs data into the store is called on an empty world, and
//! the component it claims to install has to be there afterwards. A setter added later without a line here is a setter nobody has
//! read back.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;

/// Whether the world now carries the component a setter claims to have installed.
fn carries<T: Component + Default, M: Component + Default>(world: &mut World) -> bool {
    let holder = cdda_sim::singletons::singleton::<T, M>(world);
    world.get::<T>(holder).is_some()
}

#[test]
fn every_singleton_setter_installs_what_it_claims_to() {
    let mut world = World::new();

    // `set_field_rules`, `set_prototypes` and `set_climate` install types the module keeps private, so they are exercised by the
    // modules that own them; the sweep covers every setter whose installed type the store will hand back.

    cdda_sim::singletons::set_weather(&mut world, cdda_sim::weather::Weather::clear());
    assert!(
        carries::<cdda_sim::weather::Weather, cdda_sim::singletons::TheWeather>(&mut world),
        "the sky survives a fresh store - this is the line that failed when the bug was found"
    );
    assert_eq!(
        cdda_sim::singletons::weather(&world).map(cdda_sim::weather::Weather::sight_penalty),
        Some(1.0),
        "and it can be read back"
    );

    cdda_sim::effect_registry::set_effect_types(&mut world, &[]);
    assert!(
        carries::<cdda_sim::effect_registry::EffectTypes, cdda_sim::effect_registry::TheEffectTypes>(
            &mut world
        ),
        "effect types survive a fresh store"
    );

    cdda_sim::emit_registry::set_emits(&mut world, &[]);
    assert!(
        carries::<cdda_sim::emit_registry::Emits, cdda_sim::emit_registry::TheEmits>(&mut world),
        "emissions survive a fresh store"
    );

    cdda_sim::eoc_registry::set_programs(&mut world, &[]);
    assert!(
        carries::<cdda_sim::eoc_registry::Programs, cdda_sim::eoc_registry::ThePrograms>(&mut world),
        "programs survive a fresh store"
    );

    cdda_sim::field_registry::set_field_names(&mut world, &[]);
    assert!(
        carries::<cdda_sim::field_registry::FieldNames, cdda_sim::field_registry::TheFieldNames>(
            &mut world
        ),
        "field names survive a fresh store"
    );

    cdda_sim::item_registry::set_item_types(&mut world, &[]);
    assert!(
        carries::<cdda_sim::item_registry::ItemTypes, cdda_sim::item_registry::TheItemTypes>(
            &mut world
        ),
        "item types survive a fresh store"
    );

    cdda_sim::monster_registry::set_monster_types(&mut world, &[]);
    assert!(
        carries::<cdda_sim::monster_registry::MonsterTypes, cdda_sim::monster_registry::TheMonsterTypes>(
            &mut world
        ),
        "monster types survive a fresh store"
    );

    let anyone = cdda_sim::creature::spawn_creature(&mut world, "anyone", (0, 0, 0));
    cdda_sim::dialogue_values::set(&mut world, anyone, "x", cdda_sim::dialogue_values::Value::Number(1.0));
    assert_eq!(
        cdda_sim::dialogue_values::number_of(&world, anyone, "x"),
        Some(1.0),
        "a value written to a character's own map reads back"
    );

    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    assert!(
        cdda_sim::singletons::take_rolls(&mut world).is_some(),
        "the dice survive a fresh store - this is the second setter the sweep found, and it had the same bug as the sky"
    );
}

/// The same sweep for the setters that write **per square** or **per creature** rather than into a singleton.
///
/// A per-square setter that silently no-ops would be harder to notice than a singleton one, because a square is usually written
/// once and read by a rule several turns later. Each of these writes to an empty store and then reads the value back through the
/// module's own accessor - the only way to see a write path that quietly declined to write.
#[test]
fn every_square_and_creature_setter_reads_back() {
    let mut world = World::new();

    cdda_sim::field_passage::set_passage(
        &mut world,
        (0, 0, 0),
        cdda_sim::field_passage::FieldPassage {
            impassable: true,
            permeable: false,
        },
    );
    assert!(
        !cdda_sim::field_passage::may_enter(&world, (0, 0, 0)),
        "an impassable square is impassable after being told so"
    );

    let facts = cdda_sim::TerrainFacts {
        flammable: true,
        ..cdda_sim::TerrainFacts::default()
    };
    cdda_sim::fire_facts::set_fire_facts(&mut world, (1, 0, 0), facts);
    assert!(
        cdda_sim::fire_facts::fire_facts_at(&world, (1, 0, 0)).flammable,
        "fire facts survive on a fresh square"
    );

    // The scent grid has no extent until a level gives it one, so the honest answer on a fresh store is "nothing stored" rather
    // than a write that silently vanishes. This is the one setter in the sweep whose refusal is correct: it declines because the
    // world has no squares, not because the write path is broken.
    cdda_sim::scent::set_scent_at(&mut world, (2, 0), 7);
    assert_eq!(
        cdda_sim::scent::scent_at(&world, (2, 0)),
        0,
        "a world with no level has no grid to hold scent, and says so by answering zero"
    );

    cdda_sim::add_field_strength(&mut world, (3, 0, 0), cdda_core_types::field::FieldId(13), 2);
    assert_eq!(
        cdda_sim::field_strength_at(&world, (3, 0, 0), cdda_core_types::field::FieldId(13)),
        2,
        "a field placed on a fresh square reads back at its strength"
    );
    // And a strength above the field **type's** level count is clamped rather than stored. The cap comes from the data - a field
    // with ten levels holds ten - so the test installs two fields with different counts and checks both.
    cdda_sim::field_registry::set_field_names(
        &mut world,
        &[("fd_null", 0, 1), ("fd_blood", 1, 3), ("fd_ten", 2, 10)],
    );
    let blood = cdda_core_types::field::FieldId(1);
    let ten = cdda_core_types::field::FieldId(2);
    cdda_sim::add_field_strength(&mut world, (3, 1, 0), blood, 99);
    assert_eq!(
        cdda_sim::field_strength_at(&world, (3, 1, 0), blood),
        3,
        "a field with three levels clamps at three"
    );
    cdda_sim::add_field_strength(&mut world, (3, 2, 0), ten, 99);
    assert_eq!(
        cdda_sim::field_strength_at(&world, (3, 2, 0), ten),
        10,
        "and a field with ten levels clamps at ten, which a constant three could never have said"
    );
    cdda_sim::add_field_strength(&mut world, (3, 3, 0), ten, 7);
    assert_eq!(
        cdda_sim::field_strength_at(&world, (3, 3, 0), ten),
        7,
        "a strength under the cap is stored as asked"
    );

    cdda_sim::sounds::make_sound(&mut world, (4, 0, 0), 12);
    assert_eq!(
        cdda_sim::sounds::heard_this_turn(&mut world).len(),
        1,
        "a sound made on a fresh store is heard"
    );

    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (5, 0, 0));
    cdda_sim::creature::set_health(&mut world, survivor, 12);
    assert_eq!(
        cdda_sim::creature::health_of(&world, survivor),
        Some(12),
        "health set on a fresh creature reads back"
    );

    cdda_sim::creature::set_moves(&mut world, survivor, 40);
    assert_eq!(
        cdda_sim::creature::moves_left(&world, survivor),
        40,
        "moves set on a fresh creature read back"
    );

    cdda_sim::traits::set_traits(&mut world, survivor, &["HALLUCINATION"]);
    assert!(cdda_sim::traits::has_trait(&world, survivor, "HALLUCINATION"));

    cdda_sim::missions::accept(&mut world, survivor, "MISSION_REACH_REFUGEE_CENTER");
    assert!(cdda_sim::missions::has_mission(
        &world,
        survivor,
        "MISSION_REACH_REFUGEE_CENTER"
    ));

    cdda_sim::applied_effects::add(
        &mut world,
        survivor,
        cdda_sim::effects::ActiveEffect {
            id: "effect_bleed".to_owned(),
            intensity: 1,
            duration: 100,
            permanent: false,
        },
    );
    assert!(cdda_sim::applied_effects::has(&world, survivor, "effect_bleed"));

    cdda_sim::weather::set_id(&mut world, "rain");
    assert_eq!(cdda_sim::weather::id_of(&world), Some("rain".to_owned()));
}
