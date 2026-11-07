//! How long cutting up a corpse takes, and what the cuts are called.
//!
//! Reference: `butcher_time_to_cut` (`butchery.cpp:382-487`). Every number here is the reference's own: a body's size sets the base, the tool's quality factor takes a percentage off it, the
//! handful of actions multiply it, a quartered corpse is quicker, and **the quality can never take more than seventy-five percent off** - which is the one line a port is most likely to smooth
//! into a straight percentage.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp, clippy::indexing_slicing)]

use cdda_sim::CreatureSize;
use cdda_sim::butchery::{ButcherAction, time_to_cut};

/// The base time is the body's size, and nothing else: `150`, `300`, `450`, `600`, `1800` **turns**, which with no blade is a hundred times as many **moves**.
#[test]
fn a_bigger_body_takes_longer_to_cut() {
    let quick = |size: CreatureSize| time_to_cut(size, 0, ButcherAction::Quick, false, false);
    assert_eq!(quick(CreatureSize::Tiny), 15_000, "a cat is a hundred and fifty turns, which is fifteen thousand moves");
    assert_eq!(quick(CreatureSize::Small), 30_000, "a dog is three hundred turns");
    assert_eq!(quick(CreatureSize::Medium), 45_000, "a person is four hundred and fifty turns");
    assert_eq!(quick(CreatureSize::Large), 60_000, "a cow is six hundred turns");
    assert_eq!(
        quick(CreatureSize::Huge),
        180_000,
        "something you need a ladder for is eighteen hundred turns"
    );
}

/// The tool's quality takes a percentage off, and **the floor is twenty-five percent of the time rather than none of it**.
///
/// Reference: `butchery.cpp:412-413` - `time_to_cut *= std::max( 25, 100 - factor )`, whose own comment says "at factor 50 it's 50, at factor 75 it's 25". A blade with quality ten buys
/// nothing more, which is why a master's knife does not make a huge corpse free.
#[test]
fn a_better_blade_cuts_faster_until_it_stops_mattering() {
    let medium = |quality: i32| time_to_cut(CreatureSize::Medium, quality, ButcherAction::Quick, false, false);
    assert_eq!(medium(0), 45_000, "no blade gives the whole hundred times the table");
    assert_eq!(medium(1), 44_550, "**one point of quality is one percent off the hundred**: 450 * 99");
    assert_eq!(medium(50), 22_500, "half the time at quality fifty");
    assert_eq!(medium(75), 11_250, "**the floor: a quarter of the time at quality seventy-five**");
    assert_eq!(medium(90), 11_250, "and no quality past seventy-five buys anything");
}

/// The action decides the multiplier, and two of the actions have **floors** rather than multipliers alone.
///
/// Reference: `butchery.cpp:421-465` - a full butchery is six times the base when the corpse has not been field dressed, a quartering is a quarter of the time but **never under 1,200 moves**, and a
/// dismembering is a tenth but **never under 600**. The floors are the reference's own, and they are what keeps a tiny body from being cut into pieces in a single move.
#[test]
fn the_action_decides_the_multiplier_and_the_floors_hold() {
    let medium = |action: ButcherAction| time_to_cut(CreatureSize::Medium, 0, action, false, false);
    assert_eq!(medium(ButcherAction::Quick), 45_000, "a quick cut is the base");
    assert_eq!(medium(ButcherAction::Bleed), 45_000, "and so is a bleed, for a hand that can bleed instantly");
    assert_eq!(medium(ButcherAction::FieldDress), 90_000, "field dressing is twice the base");
    assert_eq!(medium(ButcherAction::Skin), 90_000, "and so is skinning");
    assert_eq!(medium(ButcherAction::Full), 270_000, "**a full butchery of an undressed body is six times the base**");
    assert_eq!(
        time_to_cut(CreatureSize::Medium, 0, ButcherAction::Full, true, false),
        180_000,
        "**and four times once the corpse has been field dressed**"
    );
    assert_eq!(medium(ButcherAction::Dissect), 270_000, "a dissection is six times the base, and the longest of them all");
    assert_eq!(medium(ButcherAction::Quarter), 11_250, "a quartering is a quarter of the base when that is over the floor");
    assert_eq!(medium(ButcherAction::Dismember), 4_500, "and a dismembering is a tenth");

    // **The floors**: a body small enough that a tenth of its time is under 600 moves waits 600 anyway.
    assert_eq!(
        time_to_cut(CreatureSize::Tiny, 0, ButcherAction::Dismember, false, false),
        1_500,
        "a cat's tenth is fifteen hundred, over the floor"
    );
    assert_eq!(
        time_to_cut(CreatureSize::Tiny, 75, ButcherAction::Dismember, false, false),
        600,
        "**but a cat with a good blade is under the floor, so the floor answers**"
    );
    assert_eq!(
        time_to_cut(CreatureSize::Tiny, 75, ButcherAction::Quarter, false, false),
        1_200,
        "and a quartered cat would be under the four-hundred-and-fifty floor, so twelve hundred answers"
    );
}

/// A corpse already quartered is quicker again, and the quartering is taken off **whatever the action left**.
///
/// Reference: `butchery.cpp:468-470` - `if( corpse_item.has_flag( flag_QUARTERED ) ) time_to_cut /= 4;`, after every multiplier and before the proficiencies.
#[test]
fn a_quartered_corpse_is_quicker_again() {
    let whole = time_to_cut(CreatureSize::Huge, 0, ButcherAction::Full, false, false);
    let quartered = time_to_cut(CreatureSize::Huge, 0, ButcherAction::Full, false, true);
    assert_eq!(whole, 1_080_000, "a huge body's full butchery is six times eighteen hundred turns");
    assert_eq!(quartered, 270_000, "**and a quartered one is a quarter of that**");
    assert_eq!(
        time_to_cut(CreatureSize::Medium, 50, ButcherAction::Dissect, false, true),
        33_750,
        "the quarter comes off after the blade and the action both: 450 * 50 * 6 / 4"
    );
}
