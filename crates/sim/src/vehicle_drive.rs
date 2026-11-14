//! What a vehicle's wheels resist rolling with: the first number of the driving model.

use bevy_ecs::prelude::*;

/// Gravity, as the reference stores it: a **float**.
///
/// Reference: `constexpr float accel_g = 9.81f` (`src/vehicle.h:232`). The type is load-bearing: `9.81f` widened to double is 9.8100004196167, and a port that
/// used the decimal 9.81 would be wrong in the eighth digit of every rolling coefficient. The reference widens it inside the product, so the port does too.
pub const ACCEL_G: f32 = 9.81;

/// The SAE J2452 ratio every rolling coefficient carries.
///
/// Reference: `sae_ratio` (`vehicle.cpp:4668`), whose comment quotes the SAE J2452 law `F_rr = N * C_rr * 0.000225 * ( v + 33.33 )`; the coefficient here is
/// that product without the speed term. The constant is the reference's own, not a rounded version of it.
pub const SAE_ROLLING: f64 = 0.000_225;

/// The reference's wheel-count correction, and the two numbers it is built from.
///
/// Reference: `vehicle.cpp:4657-4669` - `wheel_ratio` 1.25 over `base_wheels * wheel_ratio - base_wheels + wheels`, which mildly raises rolling resistance for
/// a vehicle with more than four wheels and lowers it for one with less.
const WHEEL_RATIO: f64 = 1.25;
/// See [`WHEEL_RATIO`].
const BASE_WHEELS: f64 = 4.0;

/// What a vehicle with no usable wheel resists rolling with.
///
/// Reference: `wheel_factor = 50` (`vehicle.cpp:4673`) when `wheelcache` is empty - a vehicle dragged on its frame is far harder to move than one on wheels.
pub const NO_WHEEL_FACTOR: f64 = 50.0;

/// What a vehicle's wheels resist rolling with, in newtons per metre per second.
///
/// Reference: `vehicle::coeff_rolling_drag` (`vehicle.cpp:4655-4686`): the wheel cache holds every **unbroken** part carrying `WHEEL` (`vehicle.cpp:6738-6740`),
/// their `rolling_resistance` is summed (each wheel's share of the mass is the comment's own note that this is an approximation), the sum is scaled by the
/// wheel-count correction, and the answer is `SAE_ROLLING * accel_g * wheel_factor * mass_in_kilograms`.
///
/// A vehicle with no wheels at all answers [`NO_WHEEL_FACTOR`], and a vehicle whose mass the port cannot compute answers `None` rather than a guess.
#[must_use]
pub fn rolling_drag(world: &World, vehicle: Entity) -> Option<f64> {
    let grams = crate::vehicle_mass::mass(world, vehicle);
    #[allow(clippy::cast_precision_loss)]
    let kilograms = grams as f64 / 1000.0;
    let count = world
        .get::<crate::vehicles::Vehicle>(vehicle)
        .map_or(0, |built| built.parts.len());
    let mut wheel_factor = 0.0;
    let mut wheels = 0_usize;
    for index in 0..count {
        let Some(part) = world
            .get::<crate::vehicles::Vehicle>(vehicle)
            .and_then(|built| built.parts.get(index))
        else {
            continue;
        };
        if part.broken {
            continue;
        }
        let Some(spec) = crate::vehicles::part_spec(world, vehicle, index) else {
            continue;
        };
        if !spec.flags.contains("WHEEL") {
            continue;
        }
        // The reference's sum widens each wheel's **float** rolling resistance to double (`vehicle.cpp:4678`), so the port must sum the float and not a decimal
        // that looks like it: four wheels of 1.9 sum to 7.5999999046 as floats and to 7.6 as decimals, which is a difference in the eighth digit of the answer.
        wheel_factor += f64::from(spec.rolling_resistance?);
        wheels += 1;
    }
    let factor = if wheels == 0 {
        NO_WHEEL_FACTOR
    } else {
        #[allow(clippy::cast_precision_loss)]
        let count = wheels as f64;
        wheel_factor * WHEEL_RATIO / (BASE_WHEELS * WHEEL_RATIO - BASE_WHEELS + count)
    };
    Some(SAE_ROLLING * f64::from(ACCEL_G) * factor * kilograms)
}
