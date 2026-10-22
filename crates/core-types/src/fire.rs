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

/// What one square next to a fire looks like, as far as spreading is concerned.
///
/// The four flags are the reference's own condition, kept together because they answer one question - would this square catch? - and
/// splitting them into separate types would only move the same four `bool`s somewhere else.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Neighbour {
    /// A fire is already burning there; the reference boosts those elsewhere and never re-lights them here.
    pub has_fire: bool,
    /// A spider web is there, which makes the fire much more eager.
    pub web: bool,
    /// The ground is `FLAMMABLE`.
    pub flammable: bool,
    /// The ground is `FLAMMABLE_ASH`.
    pub flammable_ash: bool,
    /// The ground is `FLAMMABLE_HARD`, which needs a hotter fire.
    pub flammable_hard: bool,
}

/// The squares a fire sets alight this turn.
///
/// Reference: the spread loop at `src/map_field.cpp:1285-1345`. Its shape is the rule:
///
/// * the loop starts at a **random** neighbour - `rng( 0, n - 1 )` - and wraps, because the reference wants a fire to prefer whichever
///   side it happens to think of first, and the order is part of the world;
/// * a neighbour is skipped on `one_in( intensity * 2 )`, so a bigger fire considers fewer neighbours per turn;
/// * the chance to take is `25 * ( intensity - 1 )`, and `50 + chance / 2` when a web is there - which is why a **single square of fire
///   can cross a web** but not a lawn;
/// * the fire's `power` is `intensity + one_in( 5 )`, and the ground must accept it: `FLAMMABLE` and `FLAMMABLE_ASH` need power two with a
///   coin toss, `FLAMMABLE_HARD` needs power three with one in five, and a web needs nothing at all.
///
/// Returns the indices of the neighbours that caught, in the order the loop considered them. The dice are spent identically whether the
/// answer is yes or no - which is the property a replay depends on.
#[must_use]
pub fn spread_fire(
    rolls: &mut crate::rng::Rolls,
    intensity: i32,
    can_spread: bool,
    neighbours: &[Neighbour],
) -> Vec<usize> {
    let mut caught = Vec::new();
    if neighbours.is_empty() {
        return caught;
    }
    let Some(count) = i32::try_from(neighbours.len()).ok() else {
        return caught;
    };
    let start = rolls.rng(0, count - 1);
    for step in 1..=count {
        let index = (start + step).rem_euclid(count);
        let Some(neighbour) = usize::try_from(index)
            .ok()
            .and_then(|index| neighbours.get(index))
        else {
            continue;
        };
        if rolls.one_in(intensity * 2) {
            continue;
        }
        if neighbour.has_fire {
            continue;
        }
        let chance = spread_chance(intensity, neighbour.web);
        let power = intensity + i32::from(rolls.one_in(5));
        // The reference draws the take roll and the terrain rolls in this order; each is drawn only when the condition before it holds,
        // which is why a fire's dice consumption depends on what it is standing next to.
        let takes_ground = (power >= 2 && neighbour.flammable && rolls.one_in(2))
            || (power >= 2 && neighbour.flammable_ash && rolls.one_in(2))
            || (power >= 3 && neighbour.flammable_hard && rolls.one_in(5));
        if !can_spread {
            continue;
        }
        if rolls.rng(1, 100) < chance && (neighbour.web || takes_ground) {
            caught.push(usize::try_from(index).unwrap_or(0));
        }
    }
    caught
}
