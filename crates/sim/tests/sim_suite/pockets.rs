//! What goes in which pocket.
//!
//! Reference: `item_pocket::can_contain` (`item_pocket.cpp:1435-1610`). The order of the checks is part of the rule:
//! a holster that already has a pistol says "full" before it says "too heavy", and the one-item limit beats the room
//! left, because a holster is shaped for one thing whatever its capacity says.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_sim::pockets::{Contents, Fits, Pocket, fits};

fn pocket(capacity_ml: i32, max_weight_g: i32) -> Pocket {
    Pocket {
        capacity_ml,
        max_weight_g,
        max_item_ml: None,
        holster: false,
    }
}

#[test]
fn a_thing_that_fits_goes_in() {
    assert_eq!(
        fits(pocket(2_000, 5_000), Contents::default(), 1_500, 1_000),
        Fits::Yes
    );
}

#[test]
fn a_thing_bigger_than_the_room_left_does_not() {
    let half_full = Contents {
        count: 1,
        volume_ml: 1_200,
        weight_g: 100,
    };
    assert_eq!(
        fits(pocket(2_000, 5_000), half_full, 900, 100),
        Fits::NoRoom,
        "1,200 + 900 is more than 2,000"
    );
    assert_eq!(
        fits(pocket(2_000, 5_000), half_full, 800, 100),
        Fits::Yes,
        "and 1,200 + 800 is exactly the pocket"
    );
}

#[test]
fn a_thing_too_heavy_for_the_pocket_does_not() {
    assert_eq!(
        fits(pocket(2_000, 500), Contents::default(), 100, 600),
        Fits::TooHeavy
    );
    assert_eq!(
        fits(pocket(2_000, 500), Contents::default(), 100, 500),
        Fits::Yes,
        "exactly the limit is inside it"
    );
}

#[test]
fn a_pocket_shaped_for_one_thing_refuses_a_second_whatever_the_room() {
    let holster = Pocket {
        capacity_ml: 5_000,
        max_weight_g: 5_000,
        max_item_ml: None,
        holster: true,
    };
    let one_inside = Contents {
        count: 1,
        volume_ml: 10,
        weight_g: 10,
    };
    assert_eq!(
        fits(holster, one_inside, 10, 10),
        Fits::AlreadyFull,
        "a holster holds a pistol and not a pistol and a pen, however small the pen"
    );
    assert_eq!(fits(holster, Contents::default(), 10, 10), Fits::Yes);
}

#[test]
fn the_one_thing_limit_comes_before_the_room_left() {
    // A pocket that takes a litre and no single thing over a hundred millilitres: a two-hundred-millilitre thing is
    // refused for its own size even though the pocket is empty and has room, which is what `max_item_volume` means.
    let narrow = Pocket {
        capacity_ml: 1_000,
        max_weight_g: 5_000,
        max_item_ml: Some(100),
        holster: false,
    };
    assert_eq!(
        fits(narrow, Contents::default(), 200, 10),
        Fits::TooBigForThePocket
    );
    assert_eq!(fits(narrow, Contents::default(), 100, 10), Fits::Yes);
}
