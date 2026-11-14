//! Bionics read their own rules, not a dialogue that happens to call one.
//!
//! The roadmap's P4 note records that `bionics.rs` was "ported, thinly gated - covered only through dialogue effects". These gates are that fix: they ask the module's own questions, so a change to
//! the list (a duplicate, an emptied component) fails here rather than somewhere downstream.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::World;

fn with_bionic(world: &mut World, name: &str) -> bevy_ecs::entity::Entity {
    let creature = cdda_sim::creature::spawn_creature(world, "the survivor", (0, 0, 0));
    cdda_sim::bionics::install(world, creature, name);
    creature
}

/// Installing the same bionic twice leaves one, and the list keeps the order they went in.
///
/// Reference: `Character::add_bionic` (`character.cpp`) inserts into a **set** of ids, so a second install of the same bionic changes nothing, and the game's own display walks that set in its own
/// order. The port keeps a list, so **duplicates are the one thing a list allows that the reference cannot**, and this is the gate that says so.
#[test]
fn a_bionic_installed_twice_is_still_one_bionic() {
    let mut world = World::new();
    let creature = with_bionic(&mut world, "bio_power_storage");
    cdda_sim::bionics::install(&mut world, creature, "bio_power_storage");
    cdda_sim::bionics::install(&mut world, creature, "bio_adrenaline");

    assert_eq!(
        cdda_sim::bionics::bionics_of(&world, creature),
        vec!["bio_power_storage".to_owned(), "bio_adrenaline".to_owned()],
        "**two installs of the same id are one entry**, and the order is the install order"
    );
    assert!(
        cdda_sim::bionics::has(&world, creature, "bio_adrenaline"),
        "the second one is there"
    );
}

/// `ANY` asks whether there is a bionic at all, and an unknown name never answers for another.
///
/// Reference: `f_has_bionics` (`condition.cpp:946-949`): `ANY` holds when the creature has a bionic **or a full power store** - the port has no power store, a divergence recorded in `parity.md` -
/// and any other name asks for that id in particular, so **`has( "bio_x" )` must not be satisfied by `bio_y`**.
#[test]
fn any_asks_whether_there_is_a_bionic_and_a_name_asks_for_that_one() {
    let mut world = World::new();
    let bare = cdda_sim::creature::spawn_creature(&mut world, "the untouched", (0, 0, 0));
    assert!(
        !cdda_sim::bionics::has(&world, bare, "ANY"),
        "a creature with no bionics answers no"
    );
    assert!(
        cdda_sim::bionics::bionics_of(&world, bare).is_empty(),
        "and its list is empty"
    );

    let wired = with_bionic(&mut world, "bio_power_storage");
    assert!(
        cdda_sim::bionics::has(&world, wired, "ANY"),
        "one bionic is enough for ANY"
    );
    assert!(
        cdda_sim::bionics::has(&world, wired, "bio_power_storage"),
        "and the name is found"
    );
    assert!(
        !cdda_sim::bionics::has(&world, wired, "bio_adrenaline"),
        "**a different name is not**: the list is not a keyword match"
    );
}
