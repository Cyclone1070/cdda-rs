//! **What a placed vehicle finds in its cargo.**
//!
//! Reference: `vehicle::place_spawn_items` (`vehicle.cpp:6379-6450`). A prototype lists spawns per square - a chance of a batch, the items a batch draws, and the item groups it
//! draws from - and placing the vehicle fills the cargo part at that square.
//!
//! **The dice are the caller's.** Every count and every item is a roll, so the rule is written to take a [`Rolls`] and nothing here decides a value on its own; a test can hand it a
//! recording and get the reference's own session back.

use cdda_core_types::rng::Rolls;

/// **A count rounded by dice**: the whole part, plus one if a die falls inside the fraction.
///
/// Reference: `roll_remainder` (`rng.cpp:136-147`). The reference asks the dice **only when there is a fraction** - `value > integ` - so a whole number consumes no roll, which
/// matters for a replay that must stay in step with the reference's own stream.
#[must_use]
pub fn roll_remainder(value: f64, rolls: &mut Rolls) -> i32 {
    let whole = value.trunc();
    let fraction = value - whole;
    let rolled = if value > 0.0 && value > whole && rolls.x_in_y(fraction, 1.0) {
        whole + 1.0
    } else if value < 0.0 && value < whole && rolls.x_in_y(-fraction, 1.0) {
        whole - 1.0
    } else {
        whole
    };
    #[expect(clippy::cast_possible_truncation, reason = "a count of items is small")]
    let count = rolled as i32;
    count
}

/// **How many batches a spawn produces**: its chance, scaled by the `ITEM_SPAWNRATE` option, and rounded by the dice.
///
/// Reference: `vehicle.cpp:6405` - `roll_remainder( spawn.chance * std::max( spawn_rate, 1.0f ) / 100.0f )`. The option is floored at one, so a spawn rate below one cannot
/// silence a spawn that has a chance.
#[must_use]
pub fn batch_count(chance: i32, spawn_rate: f64, rolls: &mut Rolls) -> i32 {
    let listed = f64::from(chance);
    roll_remainder(listed * spawn_rate.max(1.0) / 100.0, rolls)
}
