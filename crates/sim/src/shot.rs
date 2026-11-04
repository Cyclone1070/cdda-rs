//! What a gunshot costs the hands that fire it, in action points.
//!
//! Reference: `Character::fire_gun` charges `time_to_attack( *this, *gun_id ) + RAS_time( *this, ammo )` (`src/ranged.cpp:1060`) and calls `mod_moves( -attack_moves )` once the burst is over
//! (`ranged.cpp:1197`). The two terms are separate rules and the port keeps them separate:
//!
//! * `time_to_attack` (`ranged.cpp:2299-2308`) is the **skill's** data, not the gun's: a straight line from `base_time` down by `time_reduction_per_level` a level of the gun's own skill, never
//!   below `min_time`. A rifle's line is `50 - 3 a level, floor 20`; a pistol's is `30 - 1 a level, floor 20`. **A gun that states no skill therefore costs its skill's figure, which is why the
//!   numbers belong to the skill and not to the weapon.**
//! * `RAS_time` (`ranged.cpp:2310-2322`) is how long the round takes to *reach* the chamber: nothing while the shooter has wind, and two moves a percent once it drops below twenty-five -
//!   plus the reload option's own moves, which is ammunition's rule and not this one.
//!
//! Neither term is written to the store here: the port spends moves in one place (`creature::spend_moves`) and reads the rule here.

/// The moves a shot costs from the skill alone: `time_to_attack` (`src/ranged.cpp:2299-2308`).
///
/// The three numbers are the skill's `time_to_attack` block - `min_time`, `base_time` and `time_reduction_per_level` - and the reference rounds the line before it takes the floor, which matters
/// only for a reduction that is not a whole number.
///
/// The reference's `skill_used->time_to_attack()` is read from the skill registry, which the port does **not** carry yet: the values arrive as arguments here so that the rule can be gated
/// on its arithmetic, and the loader that fills them is named in `docs/parity.md` rather than guessed.
#[must_use]
pub fn time_to_attack(base_time: i32, reduction_per_level: i32, min_time: i32, skill: i32) -> i32 {
    (base_time - reduction_per_level * skill).max(min_time)
}

/// How much longer a shot takes while the shooter is out of wind: the stamina half of `RAS_time` (`src/ranged.cpp:2310-2322`).
///
/// The reference computes `sta_percent = 100 * stamina / stamina_max` and adds `( 25 - sta_percent ) * 2` only **below** twenty-five - so a shooter at exactly a quarter wind pays nothing, and
/// one at no wind pays fifty moves. **It is a cliff, not a slope**: the term is zero for three quarters of the bar and then rises two moves a percent.
#[must_use]
pub fn winded_cost(stamina_percent: i32) -> i32 {
    if stamina_percent < 25 {
        (25 - stamina_percent) * 2
    } else {
        0
    }
}

/// The whole cost of a shot: the skill's line plus the ammunition's reach, which is `RAS_time`'s own sum.
///
/// Reference: `attack_moves = time_to_attack( *this, *gun_id ) + RAS_time( *this, ammo )` (`src/ranged.cpp:1060`). The reload option's moves are the caller's business - they belong to the
/// round being fetched, not to the shot being fired - so they arrive as `reload_moves` and are added here exactly as the reference adds them.
#[must_use]
pub fn cost_of_a_shot(
    base_time: i32,
    reduction_per_level: i32,
    min_time: i32,
    skill: i32,
    stamina_percent: i32,
    reload_moves: i32,
) -> i32 {
    time_to_attack(base_time, reduction_per_level, min_time, skill)
        + winded_cost(stamina_percent)
        + reload_moves
}
