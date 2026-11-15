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

/// The rolling term that grows with speed instead of staying constant.
///
/// Reference: `constexpr double rolling_constant_to_variable = 33.33` (`src/vehicle.h:85`) - the `33.33` of the SAE law's `( v + 33.33 )`. Unlike
/// [`ACCEL_G`] this one is a **double** in the reference, so it stays one here.
pub const ROLLING_CONSTANT_TO_VARIABLE: f64 = 33.33;

/// Metres per second to miles per hour, as a float.
///
/// Reference: `static const float mps_to_miph = 2.23694f` (`vehicle_move.cpp:88`), widened inside the product exactly like [`ACCEL_G`].
pub const MPS_TO_MIPH: f32 = 2.236_94;

/// Vehicle miles per hour per mile per hour: the reference counts speed in hundredths (`mi_to_vmi`, `vehicle_move.cpp:86`).
const MI_TO_VMI: f64 = 100.0;

/// Solves `a*v^3 + b*v^2 + c*v + d = 0` for its positive root.
///
/// Reference: `simple_cubic_solution` (`vehicle.cpp:4163-4201`), a static helper the top-speed rule calls with the drag coefficients. It is not a general
/// cubic solver - it is the reference's own rearrangement, and the branch it takes is part of the answer:
///
/// - `tricky_bit = q*q + t*t*t` is negative when the cubic has three real roots. The reference then goes through `std::complex` and keeps `p + real(term1+term2)`
///   when the imaginary part is under 2, which is its own sanity check; otherwise it prints a debug message and answers 0. That branch is transcribed rather than
///   simplified away, because a port that "knew better" would answer a different speed on exactly the vehicles that reach it.
/// - otherwise it is the sum of the two real cube roots.
///
/// # Panics
///
/// Never: an `a` of zero sends the divisions to infinity or NaN, which is what the reference's own arithmetic does, and the answer is then converted the same way.
#[allow(clippy::many_single_char_names)]
fn simple_cubic_solution(cube: f64, square: f64, linear: f64, constant: f64) -> f64 {
    let p = -square / (3.0 * cube);
    let q = p * p * p + (square * linear - 3.0 * cube * constant) / (6.0 * cube * cube);
    let r = linear / (3.0 * cube);
    let t = r - p * p;
    let tricky_bit = q * q + t * t * t;
    if tricky_bit < 0.0 {
        // The reference finds the cube root of `q ± i*sqrt(tricky_bit)` in the complex plane, by `std::pow(z, 1/3)`.
        let root = (-tricky_bit).sqrt();
        let (re1, im1) = cube_root_complex(q, root);
        let (re2, im2) = cube_root_complex(q, -root);
        let (re, im) = (re1 + re2, im1 + im2);
        if im < 2.0 { p + re } else { 0.0 }
    } else {
        let root = tricky_bit.sqrt();
        (q + root).cbrt() + (q - root).cbrt() + p
    }
}

/// The principal cube root of a complex number, by polar form - what `std::pow(z, 1.0/3.0)` computes.
fn cube_root_complex(re: f64, im: f64) -> (f64, f64) {
    let radius = re.hypot(im).cbrt();
    let angle = im.atan2(re) / 3.0;
    (radius * angle.cos(), radius * angle.sin())
}

/// The fastest a vehicle can go, in hundredths of a mile per hour.
///
/// Reference: `vehicle::max_ground_velocity` (`vehicle.cpp:4234-4247`): solve
/// `c_air * v^3 + c_rolling * v^2 + 33.33 * c_rolling * v - power = 0` for `v` in metres per second, then `mps_to_vmiph`.
///
/// The three inputs are separate arguments rather than read from a vehicle, because the rule is about the numbers: the air drag is a scan of the vehicle's
/// columns ([`super::vehicle_air`] when it is ported), the rolling drag is [`rolling_drag`], and the power is the engine sum. A vehicle with no engine asks for
/// 0 W and stands still.
#[must_use]
pub fn top_speed_vmiph(air_drag: f64, rolling_drag: f64, power_w: i32) -> i32 {
    let max_in_mps = simple_cubic_solution(
        air_drag,
        rolling_drag,
        rolling_drag * ROLLING_CONSTANT_TO_VARIABLE,
        -f64::from(power_w),
    );
    #[allow(clippy::cast_possible_truncation)]
    let speed = (max_in_mps * f64::from(MPS_TO_MIPH) * MI_TO_VMI) as i32;
    speed
}

/// What a vehicle's running engines together make, in watts.
///
/// Reference: `vehicle::total_power` (`vehicle.cpp:4052-4075`): every engine that is **on** - available and switched on (`is_engine_on`, `vehicle.cpp:1061-1064`) -
/// adds its `part_vpower_w`, and a vehicle with more than one running engine is not quite the sum: `pwr * 4 / ( 4 + count - 1 )`, because engines cannot all drive
/// the wheels at their best at once.
///
/// **Named, not ported:** the reference's fault multiplier (`REDUCE_ENG_POWER`), the `safe` mode's `m2c` cap, the alternator line (alternators have negative
/// power), the damage scaling and the animal/muscle/wind special cases. None of them can apply to the recorded vehicles, which are undamaged, unfaulted and
/// carry neither alternators nor muscles; each is a rule to port when something needs it.
#[must_use]
pub fn total_power(world: &World, vehicle: Entity) -> i32 {
    let count = world
        .get::<crate::vehicles::Vehicle>(vehicle)
        .map_or(0, |built| built.parts.len());
    let mut power = 0_i64;
    let mut engines = 0_i64;
    for index in 0..count {
        let Some(part) = world
            .get::<crate::vehicles::Vehicle>(vehicle)
            .and_then(|built| built.parts.get(index))
        else {
            continue;
        };
        if part.broken || !part.enabled {
            continue;
        }
        let Some(spec) = crate::vehicles::part_spec(world, vehicle, index) else {
            continue;
        };
        if !spec.flags.contains("ENGINE") {
            continue;
        }
        power += i64::from(part_power(&spec, part.hp));
        engines += 1;
    }
    let power = power.max(0);
    let power = if engines > 1 {
        power * 4 / (4 + engines - 1)
    } else {
        power
    };
    i32::try_from(power).unwrap_or(i32::MAX)
}

/// What one engine makes at the health it stands at.
///
/// Reference: `vehicle::part_vpower_w`'s last lines (`vehicle.cpp:1180-1187`): a healthy engine gives its `power`, and a damaged one
/// `power * ( damaged_power_factor + ( 1 - damaged_power_factor ) * health )`. `health` is `health_percent()`, which is `1 - damage_percent`
/// (`vehicle_part.cpp:193-206`) and therefore the fraction of the part's own durability that is left.
///
/// The arithmetic is the reference's to the last digit, and the digit matters: its power unit is an **integer of milliwatts** (`units_fwd.h:57`), so the scaling
/// lands on a whole milliwatt by truncation and only then divides by a thousand for whole watts. `1.0f - dpf` is a **float** subtraction widened by the double
/// `health_percent`; the recorded series (37,300 W at 0.925, 0.8125, 0.625, 0.4375, 0.325 and 0.25375) is what pins it.
fn part_power(spec: &cdda_content::vehicles::PartSpec, hp: i32) -> i32 {
    if spec.durability <= 0 {
        return spec.power;
    }
    let Some(factor) = spec.damaged_power_factor else {
        return spec.power;
    };
    let health = f64::from(hp) / f64::from(spec.durability);
    let one_minus = f64::from(1.0_f32 - factor);
    let scale = f64::from(factor) + one_minus * health;
    let milliwatts = i64::from(spec.power) * 1000;
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let scaled = (milliwatts as f64 * scale) as i64;
    #[allow(clippy::cast_possible_truncation)]
    let watts = (scaled / 1000) as i32;
    watts
}

/// The fastest a vehicle can go, in hundredths of a mile per hour, from its own body, wheels and engines.
///
/// Reference: `vehicle::max_ground_velocity` (`vehicle.cpp:4234-4247`) over `coeff_air_drag`, `coeff_rolling_drag` and `total_power`. `None` when the vehicle is not
/// in the store or its mass cannot be computed.
#[must_use]
pub fn max_ground_velocity(world: &World, vehicle: Entity) -> Option<i32> {
    let air = crate::vehicle_air::coeff_air_drag(world, vehicle)?;
    let rolling = rolling_drag(world, vehicle)?;
    Some(top_speed_vmiph(air, rolling, total_power(world, vehicle)))
}

/// The slowest a vehicle may roll backwards, in hundredths of a mile per hour.
///
/// Reference: `vehicle::max_reverse_velocity` (`vehicle.cpp:4289-4299`): an electric motor reverses as fast as it drives forward, and everything else manages a
/// quarter of its top speed. The division happens on the positive number and the sign is applied after - `-max_vel / 4` in C++ is `(-max_vel) / 4`, which truncates
/// towards zero, so a 1,871 vmiph bicycle reverses at 467.
#[must_use]
pub fn max_reverse_velocity(max_velocity: i32, battery_engine: bool) -> i32 {
    if battery_engine {
        -max_velocity
    } else {
        -max_velocity / 4
    }
}

/// Whether any engine on the vehicle is electric.
///
/// Reference: `vehicle::has_engine_type( fuel_type_battery, true )` (`vehicle.cpp:4289-4299`): the scan includes **broken** engines, because a wrecked electric motor
/// still makes the drivetrain one that can reverse. The fuel is the part's `engine_info::fuel_type` (`veh_type.h:130`), which the dump carries per part.
#[must_use]
pub fn battery_engine(world: &World, vehicle: Entity) -> bool {
    let Some(built) = world.get::<crate::vehicles::Vehicle>(vehicle) else {
        return false;
    };
    (0..built.parts.len()).any(|index| {
        let Some(spec) = crate::vehicles::part_spec(world, vehicle, index) else {
            return false;
        };
        spec.flags.contains("ENGINE") && spec.fuel_type == "battery"
    })
}

/// What a vehicle's engines make in **safe** mode, in watts.
///
/// Reference: `vehicle::total_power`'s `safe` branch (`vehicle.cpp:4052-4079`): each running engine contributes `vpower * m2c / 100` instead of its full `vpower` - and then
/// the same `pwr * 4 / ( 4 + count - 1 )` applies. `m2c` is the engine's own motor-to-crankshaft ratio, so the cap is data, not a constant.
///
/// `safe_ground_velocity` (`vehicle.cpp:4302-4310`) is then the top-speed cubic with this power: the safe speed is not a different formula, it is the same one asked
/// with less power. That is why one function serves both.
#[must_use]
pub fn safe_power(engines: &[(i32, i32)]) -> i32 {
    let mut power = 0_i64;
    let mut count = 0_i64;
    for &(watts, m2c) in engines {
        power += i64::from(watts) * i64::from(m2c) / 100;
        count += 1;
    }
    let power = power.max(0);
    let power = if count > 1 {
        power * 4 / (4 + count - 1)
    } else {
        power
    };
    #[allow(clippy::cast_possible_truncation)]
    let power = power.clamp(0, i64::from(i32::MAX)) as i32;
    power
}

/// What a vehicle's engines may give in safe mode, read from the store.
///
/// The store half of [`safe_power`]: every part that carries an `m2c` ratio contributes its power and that ratio, which is the reference's `engine_info` filter. An
/// engine with no ratio in the data is not an engine the port can cap, so it is left out rather than guessed at full power.
#[must_use]
pub fn safe_power_of(world: &World, vehicle: Entity) -> i32 {
    let count = world
        .get::<crate::vehicles::Vehicle>(vehicle)
        .map_or(0, |built| built.parts.len());
    let mut engines = Vec::new();
    for index in 0..count {
        let Some(spec) = crate::vehicles::part_spec(world, vehicle, index) else {
            continue;
        };
        if let Some(m2c) = spec.m2c {
            engines.push((spec.power, m2c));
        }
    }
    safe_power(&engines)
}
