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

/// The fastest a vehicle can go **without straining**, in hundredths of a mile per hour.
///
/// Reference: `vehicle::safe_ground_velocity` (`vehicle.cpp:4302-4310`) - the same cubic as [`max_ground_velocity`], asked with the engines capped at their own `m2c`
/// ratio ([`safe_power_of`]) instead of their full power. It is not a second formula: the safe speed is the top speed of a weaker engine.
#[must_use]
pub fn safe_ground_velocity(world: &World, vehicle: Entity) -> Option<i32> {
    let air = crate::vehicle_air::coeff_air_drag(world, vehicle)?;
    let rolling = rolling_drag(world, vehicle)?;
    Some(top_speed_vmiph(air, rolling, safe_power_of(world, vehicle)))
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
        // **On only**: the reference's `is_engine_on` (`vehicle.cpp:1061-1064`) is `enabled && !removed && ENGINE`, and a switched-off engine makes nothing. The port
        // reads the part's own `enabled` component; `removed` and the fuel test (`engine_fuel_left`) are not ported yet, and are named as gaps.
        let switched_on = world
            .get::<crate::vehicles::Vehicle>(vehicle)
            .and_then(|built| built.parts.get(index))
            .is_some_and(|part| part.enabled);
        if let Some(m2c) = spec
            .m2c
            .filter(|_| switched_on && engine_fuel_left(world, vehicle, index))
        {
            engines.push((spec.power, m2c));
        }
    }
    safe_power(&engines)
}

/// Whether an engine has any fuel of the kind it wants.
///
/// Reference: `vehicle::engine_fuel_left` (`vehicle.cpp`), which is `fuel_left( vp.fuel_current() )` - and `fuel_current` answers the part's chosen fuel
/// (`vehicle_part::ammo_pref`) when it has one, else the first of its `fuel_opts` that the vehicle actually holds. A tank with no charges holds nothing, so an engine
/// fed from it is dry.
#[must_use]
pub fn engine_fuel_left(world: &World, vehicle: Entity, index: usize) -> bool {
    let Some(part) = world
        .get::<crate::vehicles::Vehicle>(vehicle)
        .and_then(|built| built.parts.get(index))
    else {
        return false;
    };
    let preferred = part.fuel_preference.clone();
    if !preferred.is_empty() {
        return crate::vehicles::fuel_left(world, vehicle, &preferred) > 0;
    }
    let Some(spec) = crate::vehicles::part_spec(world, vehicle, index) else {
        return false;
    };
    spec.fuel_opts
        .iter()
        .any(|fuel| crate::vehicles::fuel_left(world, vehicle, fuel) > 0)
}

/// How hard a vehicle is working past its safe speed, from 0 to 1.
///
/// Reference: `vehicle::strain` (`vehicle.cpp:4967-4982`): nothing at a standstill, zero while it is inside its safe speed **in either direction**, and beyond that the
/// share of the way from the safe speed to the maximum - `( |v| - sv ) / ( mv - sv )`, with `mv` nudged one above `sv` when the two coincide so the division always has a
/// slope. That nudge is the reference's own guard, not a port invention.
#[must_use]
pub fn strain(velocity: i32, safe: i32, maximum: i32) -> f64 {
    if velocity == 0 {
        return 0.0;
    }
    let safe_speed = safe;
    let mut fastest = maximum;
    if fastest <= safe_speed {
        fastest = safe_speed + 1;
    }
    if velocity < safe_speed && velocity > -safe_speed {
        return 0.0;
    }
    f64::from(velocity.abs() - safe_speed) / f64::from(fastest - safe_speed)
}

/// What a vehicle burns per fuel type, in watts, from the engines that are running.
///
/// Reference: `vehicle::fuel_usage` (`vehicle.cpp:5196-5207`) - for every engine, the fuel it currently burns (`fuel_current`) collects that engine's
/// `energy_consumption`. Perpetual engines - muscle, wind, sunlight - are skipped by `is_perpetual_type` (`vehicle.cpp:5189-5195`), because they burn nothing.
///
/// Takes `(fuel, watts)` pairs rather than a vehicle, so the rule is arithmetic: the caller decides which engines are running, this sums them by fuel.
#[must_use]
pub fn fuel_usage(engines: &[(String, i32)]) -> std::collections::BTreeMap<String, i32> {
    let mut usage = std::collections::BTreeMap::new();
    for (fuel, watts) in engines {
        if fuel.is_empty() || *watts <= 0 {
            continue;
        }
        *usage.entry(fuel.clone()).or_insert(0) += watts;
    }
    usage
}

/// Burns fuel until it has given `wanted_j`, answering the energy it actually gave.
///
/// Reference: `vehicle::drain_energy` (`vehicle.cpp:5209-5224`) walks **every** part and lets each take what it can of the fuel asked for, stopping when the want is
/// met. It works in energy, not charges, so a part can give up a charge's worth or part of one; the port's tanks hold whole charges, so it takes them whole and reports
/// the energy those charges held - the same walk, with the granularity the store actually has.
#[must_use]
pub fn drain_energy(
    tanks: &mut [(String, i32)],
    fuel: &str,
    per_charge_j: f64,
    wanted_j: f64,
) -> f64 {
    if per_charge_j <= 0.0 || wanted_j <= 0.0 {
        return 0.0;
    }
    let mut given = 0.0;
    for (kind, charges) in tanks.iter_mut() {
        if *charges <= 0 || kind != fuel {
            continue;
        }
        let needed = (wanted_j - given) / per_charge_j;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let whole = needed.ceil() as i32;
        let taken = whole.min(*charges);
        *charges -= taken;
        #[allow(clippy::cast_precision_loss)]
        {
            given += f64::from(taken) * per_charge_j;
        }
        if given >= wanted_j {
            break;
        }
    }
    given
}

/// The energy a vehicle wants to burn for one second of running.
///
/// Reference: `vehicle::consume_fuel` (`vehicle.cpp:5235-5240`) - `usage * 1s * load * ( 1 + strain^2 * 100 ) / 1000`, then the leftover from the previous turn is
/// subtracted, because a turn cannot burn part of a charge and the rest carries over. Powers are watts and one second of watts is joules, so the product is the energy the
/// engines ask for.
#[must_use]
pub fn fuel_to_consume(usage_w: i32, load: i32, strain: f64, remainder_j: f64) -> f64 {
    let asked = f64::from(usage_w) * f64::from(load) * (1.0 + strain * strain * 100.0) / 1000.0;
    asked - remainder_j
}

/// One turn of burning for every fuel the vehicle uses.
///
/// Reference: `vehicle::consume_fuel` (`vehicle.cpp:5226-5246`): for each fuel, the energy asked for is [`fuel_to_consume`]; the leftover from the last turn comes off; if
/// there is energy to give, [`drain_energy`] gives it and the shortfall **becomes** the new leftover (`drained - asked`), and if the leftover was already more than the
/// ask, the unused part carries forward unchanged (`-to_consume`). Both branches keep the fraction a turn could not burn, which is why a car does not lose fuel to rounding.
#[must_use]
pub fn consume_fuel(
    usage: &std::collections::BTreeMap<String, i32>,
    load: i32,
    strain: f64,
    remainder: &std::collections::BTreeMap<String, f64>,
    tanks: &mut [(String, i32)],
    per_charge_j: f64,
) -> std::collections::BTreeMap<String, f64> {
    let mut carried = std::collections::BTreeMap::new();
    for (fuel, watts) in usage {
        let leftover = remainder.get(fuel).copied().unwrap_or(0.0);
        let asked = fuel_to_consume(*watts, load, strain, leftover);
        if asked > 0.0 {
            let drained = drain_energy(tanks, fuel, per_charge_j, asked);
            carried.insert(fuel.clone(), drained - asked);
        } else {
            carried.insert(fuel.clone(), -asked);
        }
    }
    carried
}

/// What a vehicle's running engines ask for, per fuel, read from the store.
///
/// The store half of [`fuel_usage`]: an engine contributes when it is **on** and **fuelled** (`is_engine_on` and `engine_fuel_left`), burns the fuel it chose or the first
/// of its options, and asks for its `energy_consumption_w`. A part with no fuel options is not an engine and contributes nothing.
#[must_use]
pub fn fuel_usage_of(world: &World, vehicle: Entity) -> std::collections::BTreeMap<String, i32> {
    let Some(built) = world.get::<crate::vehicles::Vehicle>(vehicle) else {
        return std::collections::BTreeMap::new();
    };
    let count = built.parts.len();
    let mut engines = Vec::new();
    for index in 0..count {
        let Some(part) = world
            .get::<crate::vehicles::Vehicle>(vehicle)
            .and_then(|built| built.parts.get(index))
        else {
            continue;
        };
        if !part.enabled || !engine_fuel_left(world, vehicle, index) {
            continue;
        }
        let Some(spec) = crate::vehicles::part_spec(world, vehicle, index) else {
            continue;
        };
        let fuel = if part.fuel_preference.is_empty() {
            spec.fuel_opts.first().cloned()
        } else {
            Some(part.fuel_preference.clone())
        };
        let Some(fuel) = fuel else { continue };
        // **A perpetual fuel never runs out**, so its engine never asks to burn any - the reference's `is_perpetual_type`, asked of the fuel item's flags.
        let perpetual = crate::item_registry::kind_by_name(world, &fuel)
            .is_some_and(|kind| crate::item_registry::kind_has_flag(world, kind, "PERPETUAL"));
        if perpetual {
            continue;
        }
        engines.push((fuel, spec.energy_consumption_w));
    }
    fuel_usage(&engines)
}

/// Whether a fuel is one that never runs out - the reference's `is_perpetual_type` (`vehicle.cpp:1056-1059`).
///
/// It is a property of the **fuel item**, not of the engine: `item( vp.info().fuel_type ).has_flag( flag_PERPETUAL )`. Muscle, wind, sunlight and water carry it, so an
/// engine burning them asks for no fuel at all. The port takes the flags rather than a world, so the rule is a question about data and the caller decides where it looks.
#[must_use]
pub fn is_perpetual(fuel_flags: &[String]) -> bool {
    fuel_flags.iter().any(|flag| flag == "PERPETUAL")
}
