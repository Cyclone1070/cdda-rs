//! What happens to a group's items that do not fit the container the group names.
//!
//! Reference: `put_into_container` (`src/item_group.cpp:115-171`). The rule is sharper than the names suggest: every drawn item is taken out of the group's list, then only
//! the ones that ended up in the container's `excess` pile are put back. So
//!
//! | `on_overflow` | what the player finds |
//! |---|---|
//! | `spill` | the item, loose on the ground |
//! | `discard` | nothing - it is gone on purpose |
//! | `none` | **nothing either**, and a diagnostic, because the data was wrong |
//!
//! A last item that is a **corpse** is forced in whatever its volume says (`ctr.is_corpse()`), which is why the reference has a branch for it rather than treating it as a
//! failed fit.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_core_types::rng::Rolls;
use cdda_sim::{
    Container, GroundItem, ItemFacts, ItemTypeId, Overflow, Position, drop_item, fill_container,
};

const BOX: ItemTypeId = ItemTypeId(9);
const ROCK: ItemTypeId = ItemTypeId(1);

/// Every item is 100 ml; a corpse is 100 ml too, so the volume is never the reason for the branch.
fn facts(_item: Entity) -> ItemFacts {
    ItemFacts::solid(100)
}

fn make_box(world: &mut World, capacity_ml: i64) -> Entity {
    let box_entity = drop_item(world, (5, 5), BOX, 1);
    world
        .entity_mut(box_entity)
        .insert(Container::new(capacity_ml));
    box_entity
}

fn loose(world: &mut World) -> usize {
    let mut query = world.query::<(&Position, &GroundItem, Option<&cdda_sim::Inside>)>();
    query
        .iter(world)
        .filter(|(at, item, inside)| {
            (at.x, at.y) == (5, 5) && inside.is_none() && item.kind == ROCK
        })
        .count()
}

#[test]
fn everything_fits_and_nothing_is_left_over() {
    let mut world = World::new();
    let box_entity = make_box(&mut world, 500);
    let rocks = vec![ROCK, ROCK];
    let filled = fill_container(
        &mut world,
        box_entity,
        &rocks,
        cdda_sim::Fill {
            overflow: Overflow::Spill,
            container_is_corpse: false,
        },
        facts,
        &mut Rolls::new(5),
    );
    assert_eq!(filled.inside, 2, "both rocks went in");
    assert_eq!(filled.left_over, 0);
    assert_eq!(loose(&mut world), 0);
}

#[test]
fn spill_leaves_the_leftovers_on_the_ground() {
    let mut world = World::new();
    let box_entity = make_box(&mut world, 150);
    let rocks = vec![ROCK, ROCK, ROCK];
    let filled = fill_container(
        &mut world,
        box_entity,
        &rocks,
        cdda_sim::Fill {
            overflow: Overflow::Spill,
            container_is_corpse: false,
        },
        facts,
        &mut Rolls::new(5),
    );
    assert_eq!(filled.inside, 1, "one rock fits");
    assert_eq!(filled.left_over, 2);
    assert_eq!(
        loose(&mut world),
        2,
        "and the two that did not are on the square"
    );
}

#[test]
fn discard_loses_the_leftovers_on_purpose() {
    let mut world = World::new();
    let box_entity = make_box(&mut world, 150);
    let rocks = vec![ROCK, ROCK, ROCK];
    let filled = fill_container(
        &mut world,
        box_entity,
        &rocks,
        cdda_sim::Fill {
            overflow: Overflow::Discard,
            container_is_corpse: false,
        },
        facts,
        &mut Rolls::new(5),
    );
    assert_eq!(filled.inside, 1);
    assert_eq!(filled.left_over, 2, "the player is told what it cost");
    assert_eq!(loose(&mut world), 0, "and nothing is left on the ground");
}

#[test]
fn none_loses_the_leftovers_too_because_the_data_was_wrong() {
    // The reference reports a debug message and then drops them, because `none` means "the data should have fitted" rather than "keep it somewhere".
    let mut world = World::new();
    let box_entity = make_box(&mut world, 150);
    let rocks = vec![ROCK, ROCK, ROCK];
    let filled = fill_container(
        &mut world,
        box_entity,
        &rocks,
        cdda_sim::Fill {
            overflow: Overflow::None,
            container_is_corpse: false,
        },
        facts,
        &mut Rolls::new(5),
    );
    assert_eq!(filled.inside, 1);
    assert_eq!(filled.left_over, 2);
    assert_eq!(
        loose(&mut world),
        0,
        "the same outcome as a discard, and a diagnostic"
    );
}

/// The run is shuffled before anything is offered, so it is not always the last drawn items that are rejected.
///
/// Reference: `src/item_group.cpp:127-129`, `std::shuffle( items.end() - num_items, items.end(), rng_get_engine() )`. The port shuffles with the same standard-library
/// algorithm over the same engine, which the core-types gate pins against 36 recorded permutations - so all this test has to show is that the shuffle is *reached*: with
/// a run too big for the box, which items land inside has to depend on the seed.
#[test]
fn which_items_land_inside_depends_on_the_seed() {
    let rock = ItemTypeId(1);
    let plank = ItemTypeId(2);
    let pipe = ItemTypeId(3);
    let run = [rock, plank, pipe];
    let mut kinds_seen = std::collections::BTreeSet::new();
    for attempt in 1..=8 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut world = World::new();
        let box_entity = make_box(&mut world, 150);
        fill_container(
            &mut world,
            box_entity,
            &run,
            cdda_sim::Fill {
                overflow: Overflow::Spill,
                container_is_corpse: false,
            },
            facts,
            &mut Rolls::new(seed),
        );
        let boxed = cdda_sim::contents(&mut world, box_entity)
            .into_iter()
            .filter_map(|entity| world.get::<GroundItem>(entity).map(|item| item.kind))
            .collect::<Vec<_>>();
        assert_eq!(boxed.len(), 1, "the box takes one of the three");
        if let Some(&only) = boxed.first() {
            kinds_seen.insert(only);
        }
    }
    assert!(
        kinds_seen.len() > 1,
        "the shuffle decides which item goes in, so different seeds have to put different ones in - they all put {kinds_seen:?}"
    );
}

/// A corpse takes everything, whatever its pockets say.
///
/// Reference: `put_into_container` (`src/item_group.cpp:137-145`): when `can_contain_directly` fails the code asks whether the **container** is a corpse, and if it is,
/// `force_insert_item` puts the item in regardless of volume. Corpses are the one container the data never sizes correctly, so the reference stops asking - and a port
/// that kept asking would spill a dead man's pockets onto the pavement.
#[test]
fn a_corpse_container_takes_what_would_not_fit_anywhere_else() {
    let mut world = World::new();
    let corpse = make_box(&mut world, 50);
    let rocks = vec![ROCK, ROCK, ROCK];
    let filled = fill_container(
        &mut world,
        corpse,
        &rocks,
        cdda_sim::Fill {
            overflow: Overflow::Spill,
            container_is_corpse: true,
        },
        facts,
        &mut Rolls::new(5),
    );
    assert_eq!(filled.inside, 3, "all three went in");
    assert_eq!(filled.left_over, 0, "and nothing was left over");
    assert_eq!(loose(&mut world), 0, "nothing on the ground either");
}

#[test]
fn an_ordinary_container_of_the_same_size_spills_two_of_them() {
    // The same fixture without the corpse flag, so the test above is about the flag and not about the arithmetic.
    let mut world = World::new();
    let box_entity = make_box(&mut world, 50);
    let rocks = vec![ROCK, ROCK, ROCK];
    let filled = fill_container(
        &mut world,
        box_entity,
        &rocks,
        cdda_sim::Fill {
            overflow: Overflow::Spill,
            container_is_corpse: false,
        },
        facts,
        &mut Rolls::new(5),
    );
    assert_eq!(filled.inside, 0, "not even one fits in 50 ml");
    assert_eq!(filled.left_over, 3);
    assert_eq!(loose(&mut world), 3);
}
