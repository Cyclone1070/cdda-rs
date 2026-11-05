//! Weather in the ear, and the signal a noise sends to a horde.
//!
//! Reference: `sounds::process_sounds` (`src/sounds.cpp:470`) subtracts the weather's `sound_attn` from every cluster's volume, and
//! `get_signal_for_hordes` (`:432-458`) turns that reduced volume into a signal for hordes on the overmap. Both are small rules with
//! sharp edges: the subtraction is unguarded, so a quiet noise in a storm is a **negative** volume, and the signal has a floor, a cap
//! and a minimum volume below which nothing is sent at all.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::sound::{heard_volume, horde_signal};

#[test]
fn weather_takes_its_toll_from_every_noise() {
    assert_eq!(heard_volume(20, 0), 20, "calm weather changes nothing");
    assert_eq!(heard_volume(20, 5), 15);
}

#[test]
fn a_quiet_noise_in_a_storm_is_negative_and_not_clamped() {
    // The reference subtracts without a guard, so the volume can go below zero. Clamping here would be a silent difference: the horde
    // rule below tests `> 60`, so a negative volume and a zero behave the same *there* - but a caller that reports the volume it heard
    // would print a different number.
    assert_eq!(heard_volume(3, 10), -7);
}

#[test]
fn only_a_loud_noise_reaches_a_horde() {
    // `vol_hordes` must clear sixty; below that nothing is signalled at all.
    assert_eq!(horde_signal(60, 0, 0), 0, "sixty exactly is not enough");
    let signal = horde_signal(61, 0, 0);
    assert_eq!(signal, 8, "past the volume gate the floor of eight applies");
    assert_eq!(
        horde_signal(130, 0, 0),
        11,
        "a hundred and thirty over twelve rounds up to eleven"
    );
    assert_eq!(
        horde_signal(1_000, 0, 0),
        26,
        "and nothing gets louder than twenty-six"
    );
}

#[test]
fn weather_and_depth_both_reduce_what_a_horde_hears() {
    // Weather comes off first, then underground levels divide by twice their depth.
    // Eighty less twenty is exactly sixty, and the gate is "greater than sixty", so that one sends nothing - a mistake I made in this
    // very test a moment after writing the case above it. Eighty-one is the first volume that gets through.
    assert_eq!(
        horde_signal(80, 20, 0),
        0,
        "exactly sixty after the weather is still not enough"
    );
    assert_eq!(
        horde_signal(81, 20, 0),
        8,
        "but sixty-one clears the gate and its floor"
    );
    // 150 / 12 is 12.5 and the reference takes `std::ceil`, so the signal is 13 - the rounding is part of the rule, and rounding down
    // would leave a horde slightly less interested than the reference has it.
    assert_eq!(
        horde_signal(300, 0, -1),
        13,
        "one level down halves three hundred to a hundred and fifty"
    );
    assert_eq!(
        horde_signal(300, 0, -2),
        8,
        "two levels down quarter it to seventy-five, which floors at eight"
    );
    assert_eq!(
        horde_signal(300, 0, -5),
        0,
        "five levels down divide it to thirty, which is not enough"
    );
}
