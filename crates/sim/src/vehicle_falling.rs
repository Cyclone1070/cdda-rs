//! **Whether a vehicle is held up by the ground under it.**
//!
//! Reference: `vehicle::check_falling_or_floating` (`vehicle_move.cpp:2059-2135`), which the reference calls after a move settles. It answers three questions at once and the port
//! keeps them in one struct, because the reference answers them in one pass and each decides a different consequence: whether the hull falls, whether it floats, and whether it
//! sits in deep water.
//!
//! The rule has three levels, and the **first** of them that decides is the reference's order:
//!
//! 1. **Wheels.** If at least half the wheels stand on something, the vehicle is not falling and is not in water - nothing else is asked.
//! 2. **The parts.** Otherwise the hull falls only if **no** part is held up. The reference walks its parts clearing the flag as soon as one is supported and skipping the rest
//!    (`if( !is_falling ) { continue; }`, `vehicle_move.cpp:2118-2121`), and because the flag can only go from falling to not, the answer is "any part will do" - which is
//!    looser than "every part must be held up" and worth pinning, since a port that tightened it would disagree with the reference on hulls that hang over an edge.
//! 3. **Water**, counted over every part: two thirds of the hull in deep water is *in deep water*, half of it in water at all is *in water*.
//!
//! A wheel on water is **not** supported, while a hull over water **is** - the reference passes `water_supports` false for the wheels and true for the parts
//! (`vehicle_move.cpp:2090-2100`), which is the same rule asked two ways.

use bevy_ecs::prelude::World;

/// How far down the world goes, in levels - the reference's `OVERMAP_DEPTH` (`map_scale_constants.h:47`). The bottom is held up by the world itself.
pub const OVERMAP_DEPTH: i32 = 10;

/// How tall one level is, in metres - the reference's `tile_height` (`vehicle_move.cpp:84`). A `float` there, and kept as one here, because the fall's arithmetic is single precision
/// all the way to the square root and the last digit of the answer depends on it.
pub const TILE_HEIGHT_METRES: f32 = 4.0;

/// Gravity, in metres per second squared - the reference's literal `g` (`vehicle_move.cpp:1871`).
pub const GRAVITY: f32 = 9.8;

/// Metres per second in one mile per hour - the reference's `mps_to_miph` (`vehicle_move.cpp:88`).
pub const METRES_PER_SECOND_IN_A_MILE_PER_HOUR: f32 = 2.236_94;

/// Vehicle speed is in **hundredths of a mile per hour**, which is what the `v` in `vmiph` stands for.
pub const HUNDREDTHS: f32 = 100.0;

/// Metres per second as the reference's vehicle speed, **truncating towards zero** exactly as its `int` return does - which for a fall's negative velocity means -1980 and not -1981.
///
/// Reference: `mps_to_vmiph` (`vehicle_move.cpp:91-94`), whose product is a `double` and whose return type is `int`.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "the reference's own `int` return truncates this exact product; the truncation is the rule"
)]
pub fn metres_per_second_to_vmiph(metres_per_second: f32) -> i32 {
    (f64::from(metres_per_second)
        * f64::from(METRES_PER_SECOND_IN_A_MILE_PER_HOUR)
        * f64::from(HUNDREDTHS)) as i32
}

/// The reference's vehicle speed as metres per second.
///
/// Reference: `vmiph_to_mps` (`vehicle_move.cpp:96-99`), which divides a `double` twice - so the port widens before it divides rather than staying in single precision.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "the reference stores the conversion in a `float` (vehicle_move.cpp:1873), and that narrowing is exactly what the round-trip test measures"
)]
pub fn vmiph_to_metres_per_second(vmiph: i32) -> f32 {
    (f64::from(vmiph) / f64::from(METRES_PER_SECOND_IN_A_MILE_PER_HOUR) / f64::from(HUNDREDTHS))
        as f32
}

/// **How fast a falling hull is going after another level of fall** - `v_2 = -sqrt( 2 * d * g + v_1^2 )` (`vehicle_move.cpp:1869-1877`).
///
/// The sign is the reference's: a fall's velocity is **negative**, and the square root drops whatever sign the previous one had. The arithmetic is `float` throughout, as the
/// reference's is, so the conversion back to hundredths of a mile per hour truncates the same digit.
#[must_use]
pub fn falling_velocity(previous: i32) -> i32 {
    let old = vmiph_to_metres_per_second(previous);
    let fall = (2.0_f32 * TILE_HEIGHT_METRES * GRAVITY + old * old).sqrt();
    metres_per_second_to_vmiph(-fall)
}

/// **Whether a hull is due to fall this move** - the reference's `should_fall = is_falling || vertical_velocity != 0` (`vehicle_move.cpp:1863`).
///
/// The second half is what keeps a hull falling once it has started: a frame that lands on the ground stops being flagged, but a hull already moving downwards keeps its velocity
/// until the landing rule zeros it, so the test is a velocity as well as a flag.
#[must_use]
pub fn should_fall(is_falling: bool, vertical_velocity: i32) -> bool {
    is_falling || vertical_velocity != 0
}

/// **What holds a vehicle up**, as the reference asks it once per move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Support {
    /// Whether nothing holds the hull up, which makes it fall.
    pub falling: bool,
    /// Whether at least half the hull stands in water.
    pub in_water: bool,
    /// Whether two thirds of the hull stands in **deep** water.
    pub in_deep_water: bool,
}

/// Whether the square a vehicle would step onto carries a terrain or furniture flag.
///
/// Reference: `map::has_flag_ter_or_furn` (`map.cpp`), asked of the square's terrain and then of its furniture - either one carrying the flag is answer enough.
fn square_has_flag(world: &World, square: (i32, i32, i32), flag: &str) -> bool {
    if crate::square::terrain_at(world, square)
        .is_some_and(|code| crate::terrain_types::has_flag(world, code, flag))
    {
        return true;
    }
    crate::square::furniture_at(world, square)
        .is_some_and(|code| crate::furniture_types::has_flag(world, code, flag))
}

/// **Whether something standing at this square can hold up what is above it.**
///
/// Reference: `map::supports_above` (`map.cpp:2662-2679`) - an impassable terrain holds anything up, furniture with a negative move cost does the same, and so does a vehicle
/// already standing there.
#[must_use]
pub fn supports_above(world: &World, square: (i32, i32, i32)) -> bool {
    if crate::square::terrain_at(world, square)
        .is_some_and(|code| crate::terrain_types::move_cost_of(world, code) == 0)
    {
        return true;
    }
    if crate::square::furniture_at(world, square)
        .is_some_and(|code| crate::furniture_types::move_cost_of(world, code) < 0)
    {
        return true;
    }
    // A vehicle standing on the square holds it up as surely as a wall does - the reference asks `veh_at`, and a hull is its own floor.
    crate::singletons::index(world)
        .map_or(&[][..], |index| {
            index.at_level(square.0, square.1, square.2)
        })
        .iter()
        .any(|entity| world.get::<crate::vehicles::Vehicle>(*entity).is_some())
}

/// **Whether a square holds up something standing on it**, asked for a wheel or for a hull.
///
/// Reference: the `has_support` lambda (`vehicle_move.cpp:2070-2087`): the bottom of the world always holds, water holds only when the caller says so, a square that is not open
/// air holds, and open air holds only through what stands under it.
#[must_use]
pub fn has_support(world: &World, square: (i32, i32, i32), water_supports: bool) -> bool {
    if square.2 == -OVERMAP_DEPTH {
        return true;
    }
    if square_has_flag(world, square, "SWIMMABLE") {
        return water_supports;
    }
    if !square_has_flag(world, square, "NO_FLOOR") {
        return true;
    }
    supports_above(world, (square.0, square.1, square.2 - 1))
}

/// **What holds the vehicle up**, by the reference's own three passes.
#[must_use]
pub fn check_support(world: &World, vehicle: bevy_ecs::prelude::Entity) -> Support {
    let Some(hull) = world.get::<crate::vehicles::Vehicle>(vehicle) else {
        return Support::default();
    };
    let Some(origin) = world.get::<crate::Position>(vehicle).copied() else {
        return Support::default();
    };
    let at = |part: &crate::vehicles::VehiclePart| (origin.x + part.x, origin.y + part.y, origin.z);

    // **First pass: the wheels.** Half of them is enough, and when it is, nothing else is asked.
    let wheel_indices = crate::vehicles::wheels(world, vehicle);
    let supported = wheel_indices
        .iter()
        .filter(|index| {
            hull.parts
                .get(**index)
                .is_some_and(|part| has_support(world, at(part), false))
        })
        .count();
    if supported > 0 && supported.saturating_mul(2) >= wheel_indices.len() {
        return Support::default();
    }

    // A vehicle with no parts at all is a dirty record rather than a falling one.
    if hull.parts.is_empty() {
        return Support::default();
    }

    // **Second pass: the parts**, and the water is counted over the same squares as the pass goes. The flag is only lowered, never raised: the reference skips the support test
    // once a part has been held up (`if( !is_falling ) { continue; }`), so **one** supported part is enough however many of the others hang over open air.
    let mut falling = true;
    let mut deep_water = 0_usize;
    let mut water = 0_usize;
    for part in &hull.parts {
        let square = at(part);
        if square_has_flag(world, square, "DEEP_WATER") {
            deep_water += 1;
        }
        if square_has_flag(world, square, "SWIMMABLE") {
            water += 1;
        }
        if falling {
            falling = !has_support(world, square, true);
        }
    }
    let count = hull.parts.len();
    Support {
        falling,
        in_water: water.saturating_mul(2) >= count,
        in_deep_water: deep_water.saturating_mul(3) >= count.saturating_mul(2),
    }
}

/// **Recomputes whether the hull is held up and stores the answer on it** - the reference's `veh.check_falling_or_floating()` (`map.cpp:953`), which `map::move_vehicle` calls
/// once a step has settled.
///
/// Landing is **not** this function's business: the reference zeroes `vertical_velocity` in its vertical collision, so a hull that comes to rest is put down by the impact rather
/// than by a support test. Setting the flag here and leaving the velocity alone is what lets a hull that has already started down keep its speed for the next level.
pub fn settle(world: &mut World, vehicle: bevy_ecs::prelude::Entity) {
    let support = check_support(world, vehicle);
    if let Some(mut hull) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
        hull.is_falling = support.falling;
    }
}

/// **Whether something solid is in the way of the next level down**, which is what a fall lands on.
///
/// Reference: `vehicle::collision`'s downward case (`vehicle_move.cpp:728-733`, `744-760`) - a descending hull first meets what is below it, and a part that cannot pass sets the
/// vertical velocity to zero and with it clears `is_falling` (`vehicle_move.cpp:803-807`). What "solid" means here is [`supports_above`]: ground nothing can walk onto, furniture with a
/// negative move cost, or another vehicle. A level that is simply **open air** is not in the way, which is why a hull falls through several of them.
#[must_use]
pub fn will_land(world: &World, vehicle: bevy_ecs::prelude::Entity) -> bool {
    let Some(hull) = world.get::<crate::vehicles::Vehicle>(vehicle) else {
        return false;
    };
    let Some(origin) = world.get::<crate::Position>(vehicle).copied() else {
        return false;
    };
    hull.parts
        .iter()
        .any(|part| supports_above(world, (origin.x + part.x, origin.y + part.y, origin.z - 1)))
}
