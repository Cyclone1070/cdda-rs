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
