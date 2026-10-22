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

/// What growing to `intensity` costs, in **turns** of the fire's life.
///
/// Reference: `cur.set_field_age( cur.get_field_age() + 10_minutes * cur.get_field_intensity() )` - the intensity is read **after** it has
/// risen, so growing to three costs thirty minutes rather than twenty. A turn is a second in the reference, so ten minutes is six hundred
/// turns: the units are the whole content of this function, and my first version of it said "ten minutes per level" while returning ten,
/// which is a factor of sixty in the wrong direction.
#[must_use]
pub fn growth_cost(intensity: i32) -> i64 {
    10 * 60 * i64::from(intensity)
}

/// What a fire with nothing to burn loses, in **turns** of its life, on its turn.
///
/// Reference: `cur.mod_field_age( 10_seconds * cur.get_field_intensity() )` - **ten seconds**, not ten minutes, per level; with a turn to
/// the second that is ten turns per level. A raging fire still dies faster than a small one when it stands on bare stone, but at a
/// sixtieth of the rate the growth cost spends fuel.
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

/// Where a fire ends up after one turn of the rules that need nothing but its own state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FireTurn {
    /// The fire's age after the turn, in turns; negative is fuel remaining.
    pub age: i64,
    /// Its intensity after the turn.
    pub intensity: i32,
    /// Whether it grew this turn.
    pub grew: bool,
}

/// One turn of a fire: spend or gain fuel, then grow if it has earned it.
///
/// Reference: `field_processor_fd_fire`, the fuel accounting at `src/map_field.cpp:1122-1131` and the growth at `:1222-1257`, in that
/// order - the reference spends what it burned before it decides whether the flames are hot enough to grow.
///
/// * `time_added` is fuel the fire consumed this turn, in turns. It is **subtracted** from the age, because a negative age is fuel.
/// * With nothing consumed and terrain that can burn, the fire loses [`dying_out_cost`] instead: it is standing on nothing.
/// * Then, if it has fuel (`age < 0`), is not already raging, and the cap from [`growth_cap`] allows it, it grows one level and pays
///   [`growth_cost`].
#[must_use]
pub fn fire_turn(
    age: i64,
    intensity: i32,
    adjacent_fires: i32,
    can_burn: bool,
    time_added: i64,
) -> FireTurn {
    let mut age = age;
    if time_added != 0 {
        age -= time_added;
    } else if can_burn {
        age += dying_out_cost(intensity);
    }
    let mut intensity = intensity;
    let mut grew = false;
    if age < 0 && intensity < 3 {
        let cap = growth_cap(age, adjacent_fires);
        if intensity < cap && age < 0 {
            intensity += 1;
            age += growth_cost(intensity);
            grew = true;
        }
    }
    FireTurn {
        age,
        intensity,
        grew,
    }
}
