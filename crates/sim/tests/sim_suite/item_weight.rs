//! What an item weighs, contents included.
//!
//! Transcribed from `item::weight` (`src/item.cpp:7229-7300`).

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    Container, GroundItem, ItemTypeId, drop_item, insert_into, item_volume, item_volume_integral,
    item_weight, item_weight_integral,
};

fn grams(kind: ItemTypeId) -> cdda_sim::TypeWeight {
    // Type seven stands for the thirty-two `NO_DROP` types in the data; the loader hands the flag over the same way.
    if kind.0 == 7 {
        return cdda_sim::TypeWeight::hidden();
    }
    match kind.0 {
        // A battery, counted in charges: five grams a charge.
        2 => cdda_sim::TypeWeight::counted(5),
        // A rock or a box, one object whatever its charge count.
        1 => cdda_sim::TypeWeight::plain(100),
        9 => cdda_sim::TypeWeight::plain(500),
        _ => cdda_sim::TypeWeight::plain(0),
    }
}

fn box_holding(world: &mut World, contents: &[ItemTypeId]) -> Entity {
    let box_entity = drop_item(world, (5, 5), ItemTypeId(9), 1);
    world.entity_mut(box_entity).insert(Container::new(10_000));
    for kind in contents {
        let item = drop_item(world, (5, 5), *kind, 1);
        insert_into(world, box_entity, item, |_| cdda_sim::ItemFacts::solid(1)).expect("it fits");
    }
    box_entity
}

#[test]
fn a_plain_item_weighs_what_its_prototype_says() {
    let mut world = World::new();
    let rock = drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    assert_eq!(item_weight(&mut world, rock, grams), 100);
}

#[test]
fn a_stack_weighs_its_charges_times_the_prototype_weight() {
    let mut world = World::new();
    let cells = drop_item(&mut world, (5, 5), ItemTypeId(2), 40);
    assert_eq!(
        item_weight(&mut world, cells, grams),
        200,
        "forty cells at five grams"
    );
}

#[test]
fn what_is_inside_counts() {
    let mut world = World::new();
    let box_entity = box_holding(&mut world, &[ItemTypeId(1), ItemTypeId(1)]);
    assert_eq!(
        item_weight(&mut world, box_entity, grams),
        700,
        "500 of box and 200 of rock"
    );
}

#[test]
fn the_counting_goes_all_the_way_down() {
    let mut world = World::new();
    let inner = box_holding(&mut world, &[ItemTypeId(1)]);
    let outer = drop_item(&mut world, (5, 5), ItemTypeId(9), 1);
    world.entity_mut(outer).insert(Container::new(10_000));
    insert_into(&mut world, outer, inner, |_| cdda_sim::ItemFacts::solid(1)).expect("the inner box fits");
    assert_eq!(
        item_weight(&mut world, outer, grams),
        1100,
        "box, box, rock"
    );
}

#[test]
fn something_that_is_not_there_weighs_nothing() {
    let mut world = World::new();
    let rock = drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    world.despawn(rock);
    assert_eq!(item_weight(&mut world, rock, grams), 0);
}

#[test]
fn a_stack_of_zero_counts_as_one_rather_than_weighing_nothing() {
    // `item::weight` asks the prototype, and a zero-charge stack is still an object on the floor.
    let mut world = World::new();
    let empty = drop_item(&mut world, (5, 5), ItemTypeId(2), 0);
    assert_eq!(item_weight(&mut world, empty, grams), 5);
}

#[test]
fn charges_that_would_overflow_saturate_instead_of_wrapping() {
    let mut world = World::new();
    let huge = drop_item(&mut world, (5, 5), ItemTypeId(2), i32::MAX);
    assert_eq!(
        item_weight(&mut world, huge, grams),
        i64::from(i32::MAX) * 5
    );
    if let Some(mut item) = world.get_mut::<GroundItem>(huge) {
        item.charges = i32::MAX;
    }
    assert!(
        item_weight(&mut world, huge, |_| cdda_sim::TypeWeight::plain(i64::MAX)) > 0,
        "no wrap to a negative"
    );
}

#[test]
fn an_item_that_does_not_drop_weighs_nothing_contents_included() {
    // **The first branch of `item::weight`** (`src/item.cpp:7235-7239`): a `NO_DROP` item is not really there - the reference's own words are "items just for ease of
    // implementation" - so it answers zero **before** anything else is asked, contents and all. Thirty-two types in the data carry the flag.
    //
    // The order is the rule: a port that weighed the contents and then zeroed the base would leave a `NO_DROP` box holding a rock at a hundred grams, and one that zeroed
    // the whole tree afterwards would be right by accident. The test gives the box contents precisely so the two differ.
    let mut world = World::new();
    let box_entity = box_holding(&mut world, &[ItemTypeId(1)]);
    // The same box, first as an ordinary container: five hundred grams of box and a hundred of rock.
    assert_eq!(item_weight(&mut world, box_entity, grams), 600);
    // Then as a `NO_DROP` one: nothing at all, contents included.
    let ghost = drop_item(&mut world, (6, 6), ItemTypeId(7), 1);
    world.entity_mut(ghost).insert(Container::new(10_000));
    let rock = drop_item(&mut world, (6, 6), ItemTypeId(1), 1);
    insert_into(&mut world, ghost, rock, |_| cdda_sim::ItemFacts::solid(1)).expect("it fits");
    assert_eq!(
        item_weight(&mut world, ghost, grams),
        0,
        "a NO_DROP container weighs nothing, and neither does what is inside it"
    );
}

#[test]
fn a_maximally_damaged_item_weighs_its_integral_weight() {
    // **`item::weight( integral: true )`** (`src/item.cpp:7256-7261`): the same rule with a different number out of the type - `integral_weight` instead of `weight`. It
    // exists because a gun's *stock* and *barrel* do not stop weighing what they weigh when the gun is damaged past use, and two types in the data say so explicitly
    // (`robofac_gun_shotgun_breach` 1000 against 650, `retool_ar15_223rem_pistol` 1134 against 1819).
    //
    // Everything else is unchanged: the charges still multiply, the contents are still added, and a `NO_DROP` item is still nothing at all.
    let mut world = World::new();
    let gun = drop_item(&mut world, (5, 5), ItemTypeId(11), 1);
    assert_eq!(item_weight(&mut world, gun, integral_grams), 1000, "as made");
    assert_eq!(
        item_weight_integral(&mut world, gun, integral_grams),
        650,
        "and at the end of its life, which is lighter"
    );
    let other = drop_item(&mut world, (6, 6), ItemTypeId(12), 1);
    assert_eq!(item_weight(&mut world, other, integral_grams), 1134);
    assert_eq!(
        item_weight_integral(&mut world, other, integral_grams),
        1819,
        "this one is heavier, so the rule is a second number and not a fraction"
    );
}

/// The two types whose integral weight differs from their weight, and one that does not.
fn integral_grams(kind: ItemTypeId) -> cdda_sim::TypeWeight {
    match kind.0 {
        11 => cdda_sim::TypeWeight::with_integral(1000, 650),
        12 => cdda_sim::TypeWeight::with_integral(1134, 1819),
        7 => cdda_sim::TypeWeight::hidden(),
        other => grams(ItemTypeId(other)),
    }
}

#[test]
fn a_pile_of_charges_takes_up_its_volume_in_stacks_rounded_up() {
    // **`item::volume`'s charge rule** (`src/item.cpp:7546-7562`): a charge-counted item multiplies its volume by its charges and then **divides by the type's
    // `stack_size`, rounding up a millilitre**. The type's volume is the volume of a **whole stack** - a battery in the data is 100 ml for 100 charges - which is the part
    // that reads wrong until a test says it: one charge of that battery is one millilitre, not a hundred.
    //
    // Two things make this worth a gate rather than a comment: the divisor is the type's stack size and not the charges, and the rounding is **up**, which needs a type
    // whose stacks do not divide evenly - the second synthetic type is 1 ml in stacks of three, so four charges take two millilitres of room.
    let mut world = World::new();
    let one = drop_item(&mut world, (5, 5), ItemTypeId(20), 1);
    let hundred = drop_item(&mut world, (5, 5), ItemTypeId(20), 100);
    let hundred_one = drop_item(&mut world, (5, 5), ItemTypeId(20), 101);
    assert_eq!(item_volume(&mut world, one, stack_volume), 1, "one charge of a hundred-charge stack");
    assert_eq!(
        item_volume(&mut world, hundred, stack_volume),
        100,
        "a full stack is the type's own volume"
    );
    assert_eq!(
        item_volume(&mut world, hundred_one, stack_volume),
        101,
        "and one charge past it is a millilitre more"
    );
    let three = drop_item(&mut world, (6, 6), ItemTypeId(21), 3);
    let four = drop_item(&mut world, (6, 6), ItemTypeId(21), 4);
    assert_eq!(item_volume(&mut world, three, stack_volume), 1, "a whole stack of three");
    assert_eq!(
        item_volume(&mut world, four, stack_volume),
        2,
        "four charges in stacks of three round **up** to two millilitres, not down to one"
    );
    // An item that is not counted in charges ignores all of it: its volume is its volume.
    let rock = drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    assert_eq!(item_volume(&mut world, rock, stack_volume), 250);
}

/// Volume facts for the two synthetic types: one charge-counted in stacks of a hundred, one plain.
fn stack_volume(kind: ItemTypeId) -> cdda_sim::TypeVolume {
    match kind.0 {
        20 => cdda_sim::TypeVolume::charges(100, 100),
        21 => cdda_sim::TypeVolume::charges(1, 3),
        _ => cdda_sim::TypeVolume::plain(250),
    }
}

#[test]
fn a_collapsed_item_takes_up_its_integral_volume() {
    // **`item::volume( integral: true )`** (`src/item.cpp:7540-7544`): the same rule over the type's other volume, `integral_volume` instead of `volume`. It is the
    // volume an item takes when it is packed down - a folded frame, a drawn bowstring - and **120 types in the data differ**, 97 of them saying zero on purpose.
    //
    // The charge rule still applies, because the reference picks the number first and divides by the stack size afterwards.
    let mut world = World::new();
    let folded = drop_item(&mut world, (5, 5), ItemTypeId(30), 1);
    assert_eq!(item_volume(&mut world, folded, integral_volume), 4000, "as it stands");
    assert_eq!(
        item_volume_integral(&mut world, folded, integral_volume),
        500,
        "and packed down, which is the whole point of the second number"
    );
    let charges = drop_item(&mut world, (5, 5), ItemTypeId(31), 6);
    assert_eq!(item_volume(&mut world, charges, integral_volume), 2, "six charges of a 3 ml stack of nine");
    assert_eq!(
        item_volume_integral(&mut world, charges, integral_volume),
        1,
        "and packed down: 6 x 1 ml over a stack of nine, rounded up"
    );
}

/// A type whose two volumes differ, and one counted in charges whose two differ too.
fn integral_volume(kind: ItemTypeId) -> cdda_sim::TypeVolume {
    match kind.0 {
        30 => cdda_sim::TypeVolume::with_integral_volume(4000, 500),
        31 => cdda_sim::TypeVolume::charges_with_integral_volume(3, 1, 9),
        _ => cdda_sim::TypeVolume::plain(250),
    }
}
