//! What fire does when it is not burning anything: grows, spreads, and dies.
//!
//! Reference: `field_processor_fd_fire` (`src/map_field.cpp:945-1360`). The processor as a whole needs items, terrain flags, weather and
//! vehicles; the four rules here need none of them, and they are the ones that decide a fire's shape. Their constants are the reference's,
//! and their comparisons are the reference's strictness - "three neighbours" means `>= 3`, and fifty minutes of fuel means age `< -50`.

/// The highest intensity a fire can reach, given how long it has burned (negative age is fuel) and how many fires touch it.
///
/// Reference: `src/map_field.cpp:1222-1257`. The comments in the reference say "1 for a lone fire, 2 for at least 1 neighbor, 3 for at least
/// 2 neighbors", and the code checks **three** and **seven**. The code is the rule.
#[must_use]
pub fn growth_cap(age_minutes: i64, adjacent_fires: i32) -> i32 {
    if age_minutes < -500 {
        return 3;
    }
    let mut cap = 1 + i32::from(adjacent_fires >= 3) + i32::from(adjacent_fires >= 7);
    if cap < 2 && age_minutes < -50 {
        cap = 2;
    }
    cap
}

/// What growing to `intensity` costs, in minutes of the fire's life.
///
/// Reference: `cur.set_field_age( cur.get_field_age() + 10_minutes * cur.get_field_intensity() )` - the intensity is read **after** it has
/// risen, so growing to three costs thirty minutes rather than twenty.
#[must_use]
pub fn growth_cost(intensity: i32) -> i64 {
    10 * i64::from(intensity)
}

/// What a fire with nothing to burn loses, in minutes of its life, on its turn.
///
/// Reference: `cur.mod_field_age( 10_seconds * cur.get_field_intensity() )` - ten minutes per level, so a raging fire dies faster than a
/// small one when it is standing on bare stone.
#[must_use]
pub fn dying_out_cost(intensity: i32) -> i64 {
    10 * i64::from(intensity)
}

/// The chance in a hundred that a fire jumps to a neighbour, given a web in the way.
///
/// Reference: `spread_chance = 25 * ( intensity - 1 )`, and `50 + spread_chance / 2` when a web is there. A fire at intensity one cannot
/// spread at all, which is what keeps a single square of flame from crossing a field.
#[must_use]
pub fn spread_chance(intensity: i32, web: bool) -> i32 {
    let chance = 25 * (intensity - 1);
    if web { 50 + chance / 2 } else { chance }
}
