//! How long cutting up a corpse takes.
//!
//! Reference: `butcher_time_to_cut` (`butchery.cpp:382-487`). **The rule was already ported** - `butcher_time_to_cut` in `lib.rs`, beside the activity that spends it - and this file exists because
//! round 1376 wrote a second copy of it in a new module without finding the first. The duplicate is deleted; these are the numbers it should have brought to the rule that was already there.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp, clippy::indexing_slicing)]

use cdda_sim::{ButcherAction, ButcherState, CreatureSize, butcher_time_to_cut};

/// A butcher with no practice, no helpers, and a corpse with no flags: everything the gates add is set here.
fn state(size: CreatureSize, action: ButcherAction) -> ButcherState {
    ButcherState {
        size,
        action,
        tool_factor: 0,
        field_dressed: false,
        field_dress_failed: false,
        quartered: false,
        instant_bleed: false,
        butchering_basic: 0.0,
        butchering_advanced: 0.0,
        skinning_basic: 0.0,
        helpers: 0,
    }
}

/// The base time is the body's size, and nothing else.
///
/// **The table is in turns and the answer is in moves**, which the multiplication by `100 - factor` bridges: with no blade a cat is `150 * 100`, not `150`. `Bleed` is the action that reads no other
/// rule, so the five sizes show through it unchanged.
#[test]
fn a_bigger_body_takes_longer_to_cut() {
    let bleed = |size: CreatureSize| butcher_time_to_cut(state(size, ButcherAction::Bleed));
    assert_eq!(bleed(CreatureSize::Tiny), 15_000, "a cat is a hundred and fifty turns, which is fifteen thousand moves");
    assert_eq!(bleed(CreatureSize::Small), 30_000, "a dog is three hundred turns");
    assert_eq!(bleed(CreatureSize::Medium), 45_000, "a person is four hundred and fifty turns");
    assert_eq!(bleed(CreatureSize::Large), 60_000, "a cow is six hundred turns");
    assert_eq!(
        bleed(CreatureSize::Huge),
        180_000,
        "something you need a ladder for is eighteen hundred turns"
    );
}

/// The tool's quality takes a percentage off, and **the floor is a quarter of the time rather than none of it**.
///
/// Reference: `butchery.cpp:412-413` - `time_to_cut *= std::max( 25, 100 - factor )`, whose own comment says "at factor 50 it's 50, at factor 75 it's 25". A blade past quality seventy-five buys
/// nothing more, which is why a master's knife does not make a huge corpse free.
#[test]
fn a_better_blade_cuts_faster_until_it_stops_mattering() {
    let medium = |quality: i32| {
        let mut found = state(CreatureSize::Medium, ButcherAction::Bleed);
        found.tool_factor = quality;
        butcher_time_to_cut(found)
    };
    assert_eq!(medium(0), 45_000, "no blade gives the whole hundred times the table");
    assert_eq!(medium(1), 44_550, "**one point of quality is one percent off the hundred**: 450 * 99");
    assert_eq!(medium(50), 22_500, "half the time at quality fifty");
    assert_eq!(medium(75), 11_250, "**the floor: a quarter of the time at quality seventy-five**");
    assert_eq!(medium(90), 11_250, "and no quality past seventy-five buys anything");
}

/// The action decides the multiplier, two actions have **floors** of their own, and the proficiency tail multiplies what is left.
///
/// Reference: `butchery.cpp:421-487`. **The tail is why an untrained butcher is not at the base time**, and **each penalty is a truncation**: the reference holds `prof_skin_penalty` in an `int`, so the
/// half-point an untrained skinner costs is **zero** rather than a half. That makes a quick cut `0.7 * 3 + 0.15 * 1 + 0.15` and a full butchery `0.4 * 3 + 0.4 * 1 + 0.2`.
#[test]
fn the_action_decides_the_multiplier_and_the_floors_hold() {
    let medium = |action: ButcherAction| butcher_time_to_cut(state(CreatureSize::Medium, action));
    assert_eq!(medium(ButcherAction::Bleed), 45_000, "a bleed reads no other rule at all");
    assert_eq!(medium(ButcherAction::Quarter), 11_250, "a quartering is a quarter of the base when that is over the floor");
    assert_eq!(medium(ButcherAction::Dismember), 4_500, "and a dismembering is a tenth");
    assert_eq!(medium(ButcherAction::Dissect), 270_000, "a dissection is six times the base");
    assert_eq!(medium(ButcherAction::Quick), 107_999, "**a quick cut is 45,000 * 2.4, truncated from 108,000 by the float**");
    assert_eq!(medium(ButcherAction::FieldDress), 270_000, "a field dressing is twice the base, then times three");
    assert_eq!(
        medium(ButcherAction::Skin),
        90_000,
        "**a skinning is twice the base and then times one**, because the half-point skin penalty truncates to zero"
    );
    assert_eq!(medium(ButcherAction::Full), 486_000, "**a full butchery of an undressed body is six times the base, then times 1.8**");

    // The two floors: a body small enough that a tenth of its time is under 600 moves waits 600 anyway.
    let tiny = |action: ButcherAction, quality: i32| {
        let mut found = state(CreatureSize::Tiny, action);
        found.tool_factor = quality;
        butcher_time_to_cut(found)
    };
    assert_eq!(tiny(ButcherAction::Dismember, 0), 1_500, "a cat's tenth is fifteen hundred, over the floor");
    assert_eq!(tiny(ButcherAction::Dismember, 75), 600, "**but a cat with a good blade is under the floor, so the floor answers**");
    assert_eq!(tiny(ButcherAction::Quarter, 75), 1_200, "and a quartered cat would be under four hundred and fifty, so twelve hundred answers");
}

/// A field-dressed corpse is quicker to finish, and a quartered one quicker again.
///
/// Reference: `butchery.cpp:431-436` for the dressing, `:468-470` for the quartering - which is divided out **last**, after every multiplier and before the proficiencies.
#[test]
fn dressing_and_quartering_both_shorten_the_work() {
    let mut dressed = state(CreatureSize::Medium, ButcherAction::Full);
    dressed.field_dressed = true;
    assert_eq!(butcher_time_to_cut(dressed), 324_000, "**four times the base, then times 1.8**: 450 * 100 * 4 * 1.8");

    let mut failed = state(CreatureSize::Medium, ButcherAction::Full);
    failed.field_dressed = true;
    failed.field_dress_failed = true;
    assert_eq!(
        butcher_time_to_cut(failed),
        486_000,
        "**a failed dressing is as slow as none at all**, which is the reference's own `||`"
    );

    let whole = butcher_time_to_cut(state(CreatureSize::Medium, ButcherAction::Dissect));
    let mut quartered = state(CreatureSize::Medium, ButcherAction::Dissect);
    quartered.quartered = true;
    assert_eq!(whole, 270_000, "a dissection of a whole body is six times the base");
    assert_eq!(
        butcher_time_to_cut(quartered),
        67_500,
        "**and a quartered one is a quarter of that**"
    );
}

/// Helpers shorten the work by a tenth each, and a butcher who bleeds instantly takes one move.
///
/// Reference: `butchery.cpp:426-430` and the last line before the return, `time_to_cut *= ( 1.0f - helpers / 10.0f )`. **The fraction is applied as a fraction**: truncating it first would delete the
/// work rather than shorten it.
#[test]
fn helpers_shorten_the_work_and_an_instant_bleed_takes_a_move() {
    let mut helped = state(CreatureSize::Medium, ButcherAction::Bleed);
    helped.helpers = 3;
    assert_eq!(
        butcher_time_to_cut(helped),
        31_499,
        "**three helpers take three tenths off**: 45,000 * 0.7, truncated a move short by the float - the reference truncates the same way"
    );

    let mut bleeding = state(CreatureSize::Medium, ButcherAction::Bleed);
    bleeding.instant_bleed = true;
    assert_eq!(butcher_time_to_cut(bleeding), 1, "a butcher who bleeds instantly needs a single move");
}
