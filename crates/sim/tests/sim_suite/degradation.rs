//! The degradation a damaged item is born with, and the bar that shows it.
//!
//! Reference: `item::rand_degradation` (`src/item.cpp:903-909`) and `item::degradation_symbol` (`src/item.cpp:6918-6941`).

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::rng::{Recording, Rolls};
use cdda_sim::{degradation_symbol, rand_degradation};

/// A recording of exactly one die, so the test pins the bounds the reference asks for as well as the answer.
fn one_roll(low: i32, high: i32, drawn: i32) -> Rolls {
    let text = format!("int {low} {high} {drawn}\n");
    Rolls::replaying(Recording::parse(&text).expect("one die parses"))
}

#[test]
fn a_damaged_item_rolls_its_degradation_between_nothing_and_its_own_damage() {
    // **The die is asked between zero and the item's damage**, not between zero and the maximum: a thing that took 1,000 of 4,000 cannot be more degraded than it is
    // damaged. The replay asserts those bounds, so a port that asked for the wrong range fails here rather than rolling differently.
    let mut rolls = one_roll(0, 1_000, 1_000);
    assert_eq!(
        rand_degradation(&mut rolls, 1_000, 50),
        1_000,
        "the top of the range: 1,000 * 50 / 50"
    );

    // **Truncation, not rounding.** 999 * 50 / 50 is 999; 1 * 50 / 50 is 1; but 7 * 50 / 60 is 5.83 and the reference's float-to-int conversion answers 5.
    let mut rolls = one_roll(0, 1_000, 7);
    assert_eq!(
        rand_degradation(&mut rolls, 1_000, 60),
        5,
        "5.83 truncates to 5"
    );

    // **A type that does not degrade rolls nothing at all**, and the recording proves it: a second draw would fail to find a die.
    let mut rolls = Rolls::replaying(Recording::parse("").expect("an empty recording"));
    assert_eq!(
        rand_degradation(&mut rolls, 1_000, 0),
        0,
        "no increments, no degradation"
    );
}

#[test]
fn the_bar_beside_an_item_has_five_steps_and_none_for_a_type_that_does_not_degrade() {
    // The reference divides the maximum by five to get a step (`max_damage() / 5`), then asks which step the degradation is in.
    let max = 4_000;
    let steps = [
        (0, "█"),
        (799, "█"),
        (800, "▇"),
        (1_600, "▅"),
        (2_400, "▃"),
        (3_200, "▁"),
        (4_000, "▁"),
    ];
    for (degradation, expected) in steps {
        assert_eq!(
            degradation_symbol(degradation, max, 50),
            expected,
            "at {degradation} of {max}"
        );
    }
    // **Nothing at all for a type that cannot degrade**, whatever its state says - the type check is the last thing the reference does.
    assert_eq!(
        degradation_symbol(3_000, max, 0),
        "",
        "an item that cannot degrade has no bar"
    );
    // And a zero maximum, which only a charge-counted pile has, divides by one rather than by zero.
    assert_eq!(
        degradation_symbol(0, 0, 50),
        "█",
        "a zero slice is treated as one, so nothing divides by zero"
    );
}
