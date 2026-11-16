//! How fast a vehicle can change its speed: the thrust the engines put on the road.

use bevy_ecs::prelude::*;

/// Metres per second to miles per hour, as a float - the same constant the top speed uses.
///
/// Reference: `static const float mps_to_miph = 2.23694f` (`vehicle_move.cpp:88`).
pub const MPS_TO_MIPH: f32 = 2.236_94;

/// Hundredths of a mile per hour to centimetres per second.
///
/// Reference: `vmiph_to_cmps` (`vehicle_move.cpp:106-109`): `vmiph / mps_to_miph`, truncated to an `int` - so a speed the reference cannot represent in whole
/// centimetres per second is rounded **down**, and every later division is over that smaller number.
#[must_use]
pub fn vmiph_to_cmps(vmiph: i32) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    let cmps = (f64::from(vmiph) / f64::from(MPS_TO_MIPH)) as i32;
    cmps
}

/// Centimetres per second back to hundredths of a mile per hour.
///
/// Reference: `cmps_to_vmiph` (`vehicle_move.cpp:101-104`): `cmps * mps_to_miph`, again truncated.
#[must_use]
pub fn cmps_to_vmiph(cmps: i32) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    let vmiph = (f64::from(cmps) * f64::from(MPS_TO_MIPH)) as i32;
    vmiph
}

/// How much speed the engines can add this turn, in hundredths of a mile per hour.
///
/// Reference: `vehicle::ground_acceleration` (`vehicle.cpp:4110-4129`):
///
/// - it asks about a speed at least a quarter of the vehicle's top speed, and never under a thousand vmiph, so a standing vehicle is still accelerating into a
///   target it can reach;
/// - the power-to-weight ratio is `power / weight`, and it is the **one** double division in the chain, truncated to an `int` immediately;
/// - `100 * 100 * ratio / cmps` is integer division, and the answer goes back through [`cmps_to_vmiph`].
///
/// `weight` is in kilograms as the reference computes it: `to_kilogram` is the mass in milligrams over a million (`units.h:363`), so the port takes the grams it
/// stores, multiplies by a thousand and divides by a million - the same double, not a rounded one.
#[must_use]
pub fn ground_acceleration(power_w: i32, mass_g: i64, max_velocity: i32, at_velocity: i32) -> i32 {
    let target = at_velocity.max(1000.max(max_velocity / 4));
    let cmps = vmiph_to_cmps(target);
    #[allow(clippy::cast_precision_loss)]
    let weight = (mass_g as f64 * 1000.0) / 1_000_000.0;
    #[allow(clippy::cast_possible_truncation)]
    let ratio = (f64::from(power_w) / weight) as i32;
    let accel_at_velocity = 100 * 100 * ratio / cmps;
    cmps_to_vmiph(accel_at_velocity)
}

/// What the engine can add to the vehicle's speed this turn, from where it stands.
///
/// Reference: `vehicle::current_acceleration` (`vehicle.cpp:4203-4206`) - `acceleration( here, fueled, std::abs( velocity ) )`, which for a ground vehicle is
/// [`ground_acceleration`] at its own speed. A vehicle whose engines are all off adds nothing: `engine_on` (`vehicle.cpp:4112`) is the reference's own switch, and
/// the port answers `0` when no engine can make power at all.
#[must_use]
pub fn current_acceleration(world: &World, vehicle: Entity) -> i32 {
    let Some(built) = world.get::<crate::vehicles::Vehicle>(vehicle) else {
        return 0;
    };
    let power = crate::vehicle_drive::total_power(world, vehicle);
    if power == 0 {
        return 0;
    }
    let Some(mass) = mass_of(world, vehicle) else {
        return 0;
    };
    // **A vehicle that weighs nothing cannot be accelerated.** The reference never sees one, because its item factory always knows a part's base item; a port world
    // without the item registry does, and dividing by that zero would make the acceleration saturate and then overflow the next multiply.
    if mass <= 0 {
        return 0;
    }
    let Some(max_velocity) = crate::vehicle_drive::max_ground_velocity(world, vehicle) else {
        return 0;
    };
    ground_acceleration(power, mass, max_velocity, built.velocity.abs())
}

/// The vehicle's mass in grams, or `None` if it is not in the store.
fn mass_of(world: &World, vehicle: Entity) -> Option<i64> {
    world.get::<crate::vehicles::Vehicle>(vehicle)?;
    Some(crate::vehicle_mass::mass(world, vehicle))
}

/// Everything one thrust step reads, in the reference's own units.
#[derive(Debug, Clone, Copy)]
pub struct ThrustInputs {
    /// The speed the vehicle is doing now, hundredths of a mile per hour, signed.
    pub velocity: i32,
    /// The speed the driver asked for; `vehicle::cruise_velocity`.
    pub cruise_velocity: i32,
    /// `1` to accelerate, `-1` to brake or reverse - `vehicle::thrust`'s `thd`.
    pub direction: i32,
    /// What the engines can add this turn: [`current_acceleration`].
    pub acceleration: i32,
    /// How much of the wheels' grip the ground gives back: `vehicle::k_traction`. `1.0` on open floor.
    pub traction: f32,
    /// `vehicle::max_velocity`, before traction scales it.
    pub max_velocity: i32,
    /// `vehicle::max_reverse_velocity`.
    pub max_reverse: i32,
    /// Whether any engine burns battery: reverse costs 60% with anything else (`vehicle_move.cpp:516-519`).
    pub battery_engine: bool,
}

/// The speed a vehicle is left at after one thrust step.
///
/// Reference: `vehicle::thrust` (`vehicle_move.cpp:453-620`), the part that changes `velocity`. The rest of the function is noise, fuel and engine damage, which are
/// their own rules; this is the arithmetic that decides how fast the vehicle is going next turn, in the reference's order:
///
/// - accelerating only happens **with** the direction of travel: a vehicle rolling forward that is told to brake is not thrusting, and gets the brake floor;
/// - the engine's acceleration is scaled by traction, a float multiply truncated to an `int`;
/// - maximum braking is a flat `20 mph/s` (`max_brake = 20 * 100`), added to the brake's share and not to the engine's;
/// - reverse is 60% unless a battery engine is fitted, and that multiply is a `double` truncated towards zero;
/// - the cruise clamp runs **after** the reverse penalty, so asking to reverse at `-2000` gives `-2000` even though the engine could only manage `-1200`;
/// - crossing zero stops the vehicle rather than rolling it backwards;
/// - otherwise velocity moves by the increment and is clamped between the reverse floor and `traction * max_velocity`.
#[must_use]
pub fn thrust_velocity(inputs: &ThrustInputs) -> i32 {
    let velocity = inputs.velocity;
    let thrusting = velocity == 0 || velocity.signum() == inputs.direction;
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let accel = (inputs.acceleration as f32 * inputs.traction) as i32;
    if thrusting && accel == 0 {
        return velocity;
    }
    let max_brake = 20 * 100;
    let mut increment = (accel + if thrusting { 0 } else { max_brake }) * inputs.direction;
    if inputs.direction == -1 && thrusting && !inputs.battery_engine {
        #[allow(clippy::cast_possible_truncation)]
        let reversed = (0.6 * f64::from(increment)) as i32;
        increment = reversed;
    }
    let max_velocity = max_velocity_with_traction(inputs);
    if accel != 0 {
        let effective_cruise = inputs.cruise_velocity.min(max_velocity);
        if inputs.direction > 0 {
            increment = increment.min(effective_cruise - velocity);
        } else {
            increment = increment.max(effective_cruise - velocity);
        }
    }
    let next = velocity + increment;
    if (velocity > 0 && next < 0) || (velocity < 0 && next > 0) {
        return 0;
    }
    if increment > 0 {
        velocity.max(next.min(max_velocity))
    } else {
        velocity.min(next.max(inputs.max_reverse))
    }
}

/// `traction * max_velocity`: the reference's `const int max_vel = traction * max_velocity( here );`.
fn max_velocity_with_traction(inputs: &ThrustInputs) -> i32 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let scaled = (inputs.traction * inputs.max_velocity as f32) as i32;
    scaled
}

/// One thrust step on a vehicle in the store, the way a driver asks for it.
///
/// Reference: `vehicle::thrust` (`vehicle_move.cpp:453-620`) as `gain_moves` calls it: the engine's acceleration, the top speed and the reverse floor come from the
/// vehicle's own parts, traction is `1.0` because broken ground is not ported yet, and the answer is written back to `Vehicle::velocity`. Returns whether the speed
/// changed, so a caller does not have to compare.
///
/// **The cruise comes from the vehicle**, not the caller: `cruise_velocity` is the speed the driver asked for, and it is what makes a thrust step stop at a chosen
/// speed instead of at the top speed.
pub fn thrust(world: &mut World, vehicle: Entity, direction: i32) -> bool {
    let Some(built) = world.get::<crate::vehicles::Vehicle>(vehicle) else {
        return false;
    };
    let velocity = built.velocity;
    let cruise_velocity = built.cruise_velocity;
    let acceleration = current_acceleration(world, vehicle);
    let Some(max_velocity) = crate::vehicle_drive::max_ground_velocity(world, vehicle) else {
        return false;
    };
    let battery_engine = crate::vehicle_drive::battery_engine(world, vehicle);
    let inputs = ThrustInputs {
        velocity,
        cruise_velocity,
        direction,
        acceleration,
        // Broken ground is `k_traction`'s job and is not ported; open floor is the reference's own 1.0.
        traction: 1.0,
        max_velocity,
        max_reverse: crate::vehicle_drive::max_reverse_velocity(max_velocity, battery_engine),
        battery_engine,
    };
    let next = thrust_velocity(&inputs);
    // **The engine's work is what it costs in fuel**, and the reference spends it right here (`vehicle_move.cpp:587`): the load this step asked for, plus whatever the
    // alternator drew, burned through the tanks. The alternator load is not ported, so it is zero here rather than a guess.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let accel = (acceleration as f32 * inputs.traction) as i32;
    let vel_inc = next - velocity;
    let load = drive_load(vel_inc, accel, 20 * 100, inputs.direction * vel_inc > 0);
    crate::vehicle_drive::burn_fuel(world, vehicle, load);
    if next == velocity {
        return false;
    }
    if let Some(mut built) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
        built.velocity = next;
    }
    true
}

/// The reference's `rolling_constant_to_variable`: the constant part of rolling drag.
///
/// Reference: `vehicles::rolling_constant_to_variable = 33.33` (`vehicle.h:85`) - the comment above the formula says 33.3, the constant is the number with two
/// decimals, and the port takes the constant.
pub const ROLLING_CONSTANT_TO_VARIABLE: f64 = 33.33;

/// Hundredths of a mile per hour to metres per second.
///
/// Reference: `vmiph_to_mps` (`vehicle_move.cpp:96-99`): `vmiph / mps_to_miph / mi_to_vmi`, where the divisions are **float** - the int is converted to float
/// first - and only the return widens to `double`. The port divides in `f32` for that reason and widens afterwards.
#[must_use]
pub fn vmiph_to_mps(vmiph: i32) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let speed = vmiph as f32;
    f64::from(speed / MPS_TO_MIPH / 100.0)
}

/// Metres per second back to hundredths of a mile per hour, truncated.
///
/// Reference: `mps_to_vmiph` (`vehicle_move.cpp:91-94`): `mps * mps_to_miph * mi_to_vmi`, a `double` multiply that lands on a whole number by truncation.
#[must_use]
pub fn mps_to_vmiph(mps: f64) -> i32 {
    let scaled = mps * f64::from(MPS_TO_MIPH) * 100.0;
    #[allow(clippy::cast_possible_truncation)]
    let answer = scaled as i32;
    answer
}

/// How much speed a vehicle loses this turn to air and rolling resistance.
///
/// Reference: `vehicle::slowdown` (`vehicle_move.cpp:112-153`), for a vehicle that is neither floating, flying nor skidding:
///
/// - air drag is `c_air * v^2`, with `v` in metres per second and `c_air` the coefficient `max_ground_velocity` already uses;
/// - rolling drag is `c_rolling * ( 33.33 + v )` - the constant is the low-speed floor that keeps a coaster slowing down;
/// - the force is divided by the mass **in kilograms**, and a vehicle of zero mass loses nothing rather than dividing by it;
/// - the answer goes back through [`mps_to_vmiph`], and never falls below one: a moving vehicle always sheds at least that much, which is what stops a coaster
///   from creeping forever.
///
/// **Named:** the reference's last subtraction is `static_drag()` - plows and towing - and a vehicle with neither has zero there, which is every vehicle in the
/// recording.
#[must_use]
pub fn slowdown_of(
    air_drag: f64,
    rolling_drag: f64,
    mass_g: i64,
    at_velocity: i32,
    engine_running: bool,
) -> i32 {
    let mps = vmiph_to_mps(at_velocity.abs());
    let mut drag = air_drag * mps * mps;
    if mass_g > 0 {
        drag += rolling_drag * (ROLLING_CONSTANT_TO_VARIABLE + mps);
    }
    #[allow(clippy::cast_precision_loss)]
    let kilograms = mass_g as f64 / 1000.0;
    let acceleration = if kilograms > 0.0 {
        drag / kilograms
    } else {
        0.0
    };
    let mut slowdown = mps_to_vmiph(acceleration);
    // `static_drag` (`vehicle.cpp:4961`): an engine that is **off** adds 1500 W of drag, which is subtracted from the slowdown - so a vehicle left out of gear
    // coasts down faster. Plows and towing are its other terms and are zero on every recorded vehicle.
    if !engine_running {
        slowdown += 1500;
    }
    slowdown.max(1)
}

/// How much speed a vehicle in the store loses this turn, from its own body and wheels.
///
/// Reference: `vehicle::slowdown` (`vehicle_move.cpp:112-153`) with the coefficients the vehicle's parts give. `None` when the vehicle is not in the store.
#[must_use]
pub fn slowdown(
    world: &World,
    vehicle: Entity,
    at_velocity: i32,
    engine_running: bool,
) -> Option<i32> {
    let air = crate::vehicle_air::coeff_air_drag(world, vehicle)?;
    let rolling = crate::vehicle_drive::rolling_drag(world, vehicle)?;
    let mass = crate::vehicle_mass::mass(world, vehicle);
    Some(slowdown_of(air, rolling, mass, at_velocity, engine_running))
}

/// One turn of coasting: the speed drag leaves, before anything is asked of the engine.
///
/// Reference: `vehicle::gain_moves`'s slowdown block (`vehicle.cpp:6488-6499`). A vehicle loses [`slowdown`] each turn; if that would take it past zero, a vehicle
/// under a driver's control is left creeping at one vmiph and everybody else **stops**, which is the reference's own `stop()`.
pub fn coast(world: &mut World, vehicle: Entity, driven: bool, engine_running: bool) -> bool {
    let Some(built) = world.get::<crate::vehicles::Vehicle>(vehicle) else {
        return false;
    };
    let velocity = built.velocity;
    if velocity == 0 {
        return false;
    }
    let Some(slowdown) = slowdown(world, vehicle, velocity, engine_running) else {
        return false;
    };
    let next = if slowdown > velocity.abs() {
        if built.cruise_velocity != 0 && driven {
            velocity.signum()
        } else {
            0
        }
    } else if velocity < 0 {
        velocity + slowdown
    } else {
        velocity - slowdown
    };
    if next == velocity {
        return false;
    }
    if let Some(mut built) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
        built.velocity = next;
    }
    true
}

/// The speed at which a vehicle covers one square in a turn: `vehicles::vmiph_per_tile` (`vehicle.h:86`).
pub const VMIPH_PER_TILE: f32 = 400.0;

/// What one square of movement costs in turn fraction, at a speed.
///
/// Reference: `vehicle::act_on_map` (`vehicle_move.cpp:1904`): `vmiph_per_tile / max( 0.0001f, |velocity| )`. A bicycle at 400 vmiph spends a whole turn on a
/// square; at 1,000 it spends 0.4 of one, which is why a fast vehicle takes several squares a turn.
#[must_use]
pub fn turn_cost(velocity: i32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let speed = velocity.abs() as f32;
    VMIPH_PER_TILE / speed.max(0.0001)
}

/// One square of movement if the vehicle can afford it, and the bookkeeping either way.
///
/// Reference: `vehicle::act_on_map` (`vehicle_move.cpp:1829-2010`), with the parts the port has:
///
/// - **below 20 vmiph a vehicle stops where it stands** and the attempt costs 0.321 of a turn (`vehicle_move.cpp:1887-1892`) - a bicycle "goes in reverse" more
///   slowly than that, which is the reference's own words for the threshold;
/// - a square costs [`turn_cost`], and a vehicle that cannot afford one keeps the fraction for next turn (`of_turn_carry`) instead of losing it;
/// - otherwise the cost is spent and the vehicle takes exactly one square along its facing, in the direction the speed's sign says.
///
/// The displacement goes through [`crate::vehicles::move_vehicle`], so the index and every rider move with it. Returns whether the vehicle changed square.
pub fn move_step(world: &mut World, vehicle: Entity) -> bool {
    // **The caller owns randomness.** The turn loop threads its own stream through [`move_step_with_rolls`]; this entry point exists for the callers that only
    // move a vehicle - the tests and the inspection tools - and it takes a fresh stream so a crash's spread still happens, deterministically.
    move_step_with_rolls(world, vehicle, &mut cdda_core_types::rng::Rolls::new(0))
}

/// Moves a vehicle one square, drawing from the game's own stream for the damage a crash spreads.
pub fn move_step_with_rolls(
    world: &mut World,
    vehicle: Entity,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> bool {
    let Some(built) = world.get::<crate::vehicles::Vehicle>(vehicle) else {
        return false;
    };
    let velocity = built.velocity;
    let rotation = built.rotation;
    let of_turn = built.of_turn;

    // **Too slow to move**: the reference stops the vehicle outright, spends 0.321 of the turn, and leaves it where it is.
    if velocity.abs() < 20 {
        if let Some(mut built) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
            built.velocity = 0;
            built.of_turn_carry = 0.0;
            built.of_turn = of_turn - 0.321;
        }
        return false;
    }

    let cost = turn_cost(velocity);
    if cost >= of_turn {
        // Cannot afford a square this call: the rest of the turn is carried rather than dropped.
        if let Some(mut built) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
            built.of_turn_carry = of_turn;
            built.of_turn = 0.0;
        }
        return false;
    }

    // The turn is spent **before** the square is taken, which is the reference's order: a move that cannot happen still costs the turn it tried to use.
    if let Some(mut built) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
        built.of_turn -= cost;
    }
    let (dx, dy) = crate::rotation::coord_translate(rotation, (0, 0), (1, 0));
    let step = if velocity < 0 { (-dx, -dy) } else { (dx, dy) };

    // **Anything in the way stops the vehicle where it stands.** The detection is the port's own [`crate::vehicles::first_collision`] - the rule that answers
    // what a part would hit - and `map::move_vehicle` calls `veh.stop()` when a collision leaves the vehicle unable to move (`map.cpp:948-950`): the square is
    // never entered and the speed is gone. Smashing the obstacle, the damage the crash does and the bounce off another vehicle are the collision rounds that
    // follow; this one only refuses to drive through.
    // **Two vehicles**: the map's arm (`map.cpp:1075-1200`). Speeds exchange along the line between the two hulls, the lost energy becomes damage, and each side's parts
    // take half of it. Perpendicular components and the ARMOR substitution are not ported yet and are named in `docs/parity.md`.
    if let Some(crate::vehicles::Collision::Vehicle {
        part,
        target,
        target_part,
    }) = crate::vehicles::first_collision(world, vehicle, step)
    {
        let mine = world.get::<crate::vehicles::Vehicle>(vehicle);
        let theirs = world.get::<crate::vehicles::Vehicle>(target);
        if let (Some(mine), Some(theirs)) = (mine, theirs) {
            let (v1, v2) = (f64::from(mine.velocity), f64::from(theirs.velocity));
            #[allow(clippy::cast_precision_loss)]
            let (m1, m2) = (
                crate::vehicle_mass::mass(world, vehicle) as f64,
                crate::vehicle_mass::mass(world, target) as f64,
            );
            // The reference measures the difference in miles an hour: `get_collision_factor( vel1_y/100 - vel2_y/100 )`.
            let e = collision_factor((v1 - v2) / 100.0);
            let (a1, a2) = exchanged(m1, v1, m2, v2, e);
            let damage = collision_damage(m1, v1, m2, v2, e);
            let share = collision_share(damage, 1);
            #[allow(clippy::cast_possible_truncation)]
            let settled = (a1.round() as i32, a2.round() as i32);
            if let Some(mut state) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
                state.velocity = settled.0;
            }
            if let Some(mut state) = world.get_mut::<crate::vehicles::Vehicle>(target) {
                state.velocity = settled.1;
            }
            crate::vehicles::damage(world, vehicle, part, share, "bash", false);
            crate::vehicles::damage(world, target, target_part, share, "bash", false);
        }
        return true;
    }
    if let Some(crate::vehicles::Collision::Terrain {
        part,
        square,
        bashable,
    }) = crate::vehicles::first_collision(world, vehicle, step)
    {
        // The crash dents the part that met the obstacle, once the data says what it is made of. Until a part carries a density the blow has no split to compute,
        // so the vehicle stops exactly as before and nothing is damaged - the recording is what fills the gap, not a guess here.
        dent_on_crash(world, vehicle, part, square, bashable, rolls);
        if let Some(mut built) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
            built.velocity = 0;
            built.of_turn_carry = 0.0;
        }
        return false;
    }

    let moved = crate::vehicles::move_vehicle(world, vehicle, step);
    // **Straining wears the engines**, straight after the step, which is where the reference does it (`vehicle_move.cpp:588-594`): the strain is read against the safe and
    // top speeds, turned into the capped figure, and spent on the caller's dice.
    let velocity_now = world
        .get::<crate::vehicles::Vehicle>(vehicle)
        .map_or(0, |built| built.velocity);
    let safe = crate::vehicle_drive::safe_ground_velocity(world, vehicle).unwrap_or(0);
    let top = crate::vehicle_drive::max_ground_velocity(world, vehicle).unwrap_or(0);
    let strained = crate::vehicle_drive::strain(velocity_now, safe, top);
    if strained > 0.0 {
        crate::vehicle_drive::wear_engines(world, vehicle, strained, rolls);
    }
    moved
}

/// The obstacle an unbreakable crash lands on: `veh_coll_other` (`vehicle_move.cpp:961-967`).
pub const UNBREAKABLE_MASS_KG: f64 = 1000.0;
/// Its restitution - `e = 0.10f` (`vehicle_move.cpp:964`).
pub const UNBREAKABLE_RESTITUTION: f64 = 0.10;
/// Its density - `part_dens = 80` (`vehicle_move.cpp:965`).
pub const UNBREAKABLE_DENSITY: f64 = 80.0;

/// The impulse a crash into an unbreakable obstacle produces, and the speed it leaves.
///
/// Reference: `vehicle::part_collision`'s loop (`vehicle_move.cpp:1030-1168`) for the `veh_coll_other` branch. The reference's own guard prints 61 for the car at
/// 1,000 and 247 at 2,000, and those are the numbers this returns - the blow **before** any part reduction or spreading. The second number is the **obstacle's**
/// share, `damage * ( 100 - k ) / 100`: about a quarter of the blow, because the material factor is clamped at 25.
#[must_use]
pub fn crash_impulse(mass_g: i64, velocity: i32, vpart_density: f64) -> (i32, i32, i32) {
    if velocity == 0 || mass_g <= 0 || vpart_density <= 0.0 {
        return (0, 0, velocity);
    }
    #[allow(clippy::cast_precision_loss)]
    let mass = mass_g as f64 / 1000.0;
    let (mass2, e) = (UNBREAKABLE_MASS_KG, UNBREAKABLE_RESTITUTION);
    let material_factor = ((UNBREAKABLE_DENSITY - vpart_density) * 0.5).clamp(-25.0, 25.0);
    let weight_factor = if mass >= mass2 {
        -25.0 * (mass.ln() - mass2.ln()) / mass.ln()
    } else {
        25.0 * (mass2.ln() - mass.ln()) / mass2.ln()
    };
    let k = (50.0 + material_factor + weight_factor).clamp(10.0, 90.0);
    #[allow(clippy::cast_precision_loss)]
    let mut speed = velocity as f32;
    let sign_before = speed.signum();
    let mut vel2 = 0.0_f64;
    let mut damage = 0.0_f64;
    for _ in 0..100 {
        let vel1 = f64::from(speed) / 100.0;
        let vel1_a = (mass * vel1 + mass2 * vel2 + e * mass2 * (vel2 - vel1)) / (mass + mass2);
        let vel2_a = (mass * vel1 + mass2 * vel2 + e * mass * (vel1 - vel2)) / (mass + mass2);
        let before = 0.5 * (mass * vel1 * vel1) + 0.5 * (mass2 * vel2 * vel2);
        let after = 0.5 * (mass * vel1_a * vel1_a) + 0.5 * (mass2 * vel2_a * vel2_a);
        let deformation = before - after;
        if deformation <= 0.0 {
            if vel1_a.abs() < vel1.abs() {
                #[allow(clippy::cast_possible_truncation)]
                let lower = (vel1_a * 90.0) as f32;
                speed = lower;
            }
            if vel2_a.abs() > vel2.abs() {
                vel2 = vel2_a;
            }
            continue;
        }
        damage += deformation / 400.0;
        #[allow(clippy::cast_possible_truncation)]
        let next = (vel1_a * 100.0) as f32;
        speed = next;
        if (speed.signum() - sign_before).abs() > f32::EPSILON {
            break;
        }
    }
    #[allow(clippy::cast_possible_truncation)]
    let impulse = (damage * k / 100.0) as i32;
    // The obstacle's share is the rest of the same blow (`vehicle_move.cpp:1066`), truncated the way the reference truncates it on its way into `bash`.
    #[allow(clippy::cast_possible_truncation)]
    let obstacle = (damage * (100.0 - k) / 100.0) as i32;
    #[allow(clippy::cast_possible_truncation)]
    let left = speed as i32;
    (impulse, obstacle, left)
}

/// Dents the part that a crash stopped the vehicle on, when the part says what it is made of.
///
/// The impulse comes from [`crash_impulse`], the split from the part's own density; without one the reference's loop has no `k` to compute and the vehicle simply
/// stops. Nothing else about the crash moves: the recording's eight rows are the gate for the loop, and this is the seam that will carry them into the store.
fn dent_on_crash(
    world: &mut World,
    vehicle: Entity,
    part: usize,
    square: (i32, i32, i32),
    bashable: bool,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Some(density) = part_density(world, vehicle, part) else {
        return;
    };
    let mass = crate::vehicle_mass::mass(world, vehicle);
    let Some(velocity) = world
        .get::<crate::vehicles::Vehicle>(vehicle)
        .map(|built| built.velocity)
    else {
        return;
    };
    let (impulse, obstacle, _) = crash_impulse(mass, velocity, density);
    if impulse > 0 {
        crate::vehicles::damage(world, vehicle, part, impulse, "bash", false);
        // **And the neighbourhood**: the reference follows the blow with `damage_all( coll_dmg / 2, coll_dmg, ... )` (`map.cpp:869`), which is why a real crash
        // dents more than the part that touched the wall.
        let impact = world
            .get::<crate::vehicles::Vehicle>(vehicle)
            .and_then(|built| built.parts.get(part))
            .map_or((0, 0), |entry| (entry.x, entry.y));
        damage_around(world, vehicle, impact, impulse, rolls);
        // **And the wall**: a bashable obstacle takes the rest of the split, `obj_dmg = dmg * ( 100 - k ) / 100`, through the map's own bash rule
        // (`vehicle_move.cpp:1074-1078`). `bash_square` reads the square, decides with the data and writes back what it broke.
        if bashable && obstacle > 0 {
            crate::bash::bash_square_with_rolls(world, square, obstacle, true, rolls);
        }
    }
}

/// What a part is made of, per litre, when the loaded data says.
fn part_density(world: &World, vehicle: Entity, part: usize) -> Option<f64> {
    let spec = crate::vehicles::part_spec(world, vehicle, part)?;
    spec.density
}

/// How much of a blow reaches a part at a given distance from the impact.
///
/// Reference: `vehicle::damage_all` (`vehicle.cpp:7674-7687`): `net_dmg = rng( dmg1, dmg2 ) / ( distance * distance )`, where the distance is
/// `1 + square_dist( mount, impact )` and only parts further than one square are touched; a part with a shock absorber on its mount loses the absorber's `bonus`
/// points from that, floored at zero. The draw is a parameter because it is random in the reference; the caller supplies it, so this rule stays a pure function and
/// the gate can pin it with a seed.
#[must_use]
pub fn spread_share(draw: i32, distance: i32, shock_absorber_bonus: i32) -> i32 {
    if distance <= 1 || draw <= 0 {
        return 0;
    }
    (draw / (distance * distance) - shock_absorber_bonus).max(0)
}

/// Spreads a blow across the parts around the point it landed on.
///
/// Reference: `vehicle::damage_all( here, coll_dmg / 2, coll_dmg, damage_bash, collision_point )` (`map.cpp:869`) and its loop (`vehicle.cpp:7674-7689`): every part
/// further than one square draws `rng( low, high )`, takes [`spread_share`] of it, and goes through [`crate::vehicles::damage`]. Returns how many parts it touched.
pub fn damage_around(
    world: &mut World,
    vehicle: Entity,
    impact: (i32, i32),
    coll_dmg: i32,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> usize {
    let (low, high) = (coll_dmg / 2, coll_dmg);
    let count = world
        .get::<crate::vehicles::Vehicle>(vehicle)
        .map_or(0, |built| built.parts.len());
    let mut touched = 0;
    for part in 0..count {
        let Some(built) = world.get::<crate::vehicles::Vehicle>(vehicle) else {
            break;
        };
        let Some(entry) = built.parts.get(part) else {
            continue;
        };
        let distance = 1 + (entry.x - impact.0).abs() + (entry.y - impact.1).abs();
        if distance <= 1 {
            continue;
        }
        let draw = rolls.rng(low.max(1), high.max(low.max(1)));
        let share = spread_share(draw, distance, 0);
        if share > 0 {
            crate::vehicles::damage(world, vehicle, part, share, "bash", false);
            touched += 1;
        }
    }
    touched
}

/// The two speeds after two vehicles collide along one axis.
///
/// The two speeds after two vehicles collide along one axis.
///
/// Reference: `map::move_vehicle`'s vehicle arm (`map.cpp:1106-1110`), exactly as written:
///
/// ```text
/// vel1_y_a = ( m2*vel2_y*(1+e) + vel1_y*( m1 - m2*e ) ) / ( m1 + m2 )
/// vel2_y_a = ( m1*vel1_y*(1+e) + vel2_y*( m2 - m1*e ) ) / ( m1 + m2 )
/// ```
///
/// Each body keeps its own component with its own mass and takes the other's through the restitution; the form conserves momentum for any two masses, and the test says
/// so. Force is transmitted only along the collision axis - the perpendicular components pass through untouched, which is the caller's business, not this function's.
///
/// The restitution is **not** a constant: the reference draws it from `get_collision_factor( vel1_y/100 - vel2_y/100 )` (`map.cpp:1101`), the difference of the two
/// speeds in miles an hour. That rule is not ported yet, so `e` is a parameter here.
#[must_use]
pub fn exchanged(m1: f64, v1: f64, m2: f64, v2: f64, e: f64) -> (f64, f64) {
    if m1 <= 0.0 || m2 <= 0.0 || (m1 + m2) <= 0.0 {
        return (v1, v2);
    }
    let total = m1 + m2;
    let first = (m1 * v1 + m2 * v2 + e * m2 * (v2 - v1)) / total;
    let second = (m1 * v1 + m2 * v2 + e * m1 * (v1 - v2)) / total;
    (first, second)
}

/// The damage one vehicle-vehicle collision deals, before it is split and shared out.
///
/// Reference: `map.cpp:1141-1146` - the deformation energy `d_E` between the speeds before and after the exchange, then `dmg = | d_E / 1000 / 2000 |`. **Two divisors, and
/// the second is 2000**, where the obstacle branch divides by 400: the two paths are not the same rule with a different mass.
#[must_use]
pub fn collision_damage(m1: f64, v1: f64, m2: f64, v2: f64, e: f64) -> i32 {
    let (a1, a2) = exchanged(m1, v1, m2, v2, e);
    let before = 0.5 * m1 * v1 * v1 + 0.5 * m2 * v2 * v2;
    let after = 0.5 * m1 * a1 * a1 + 0.5 * m2 * a2 * a2;
    #[allow(clippy::cast_possible_truncation)]
    let damage = ((before - after).abs() / 1000.0 / 2000.0) as i32;
    damage
}

/// The restitution of a vehicle-vehicle collision, from the difference of the two speeds.
///
/// Reference: `get_collision_factor` (`vehicle_move.cpp:1636-1643`): up to 31 miles an hour apart the factor falls linearly from **1** to **0.1**, and beyond that it
/// stays at 0.1. A gentle nudge is almost perfectly elastic; a hard hit is almost inelastic, which is why a fast crash dumps its energy into the parts instead of
/// bouncing the vehicles apart.
#[must_use]
pub fn collision_factor(delta_v: f64) -> f64 {
    let gap = delta_v.abs();
    if gap <= 31.0 {
        1.0 - (0.9 * gap) / 31.0
    } else {
        0.1
    }
}

/// What one colliding part pair takes of a vehicle-vehicle collision.
///
/// Reference: `map.cpp:1157-1169` - the damage is halved (`dmg_veh1 = dmg * 0.5f`) and then divided by the number of part pairs the two vehicles share
/// (`dmg1_part = dmg_veh1 / coll_parts_cnt`), so a collision across three pairs spreads one blow over six parts. Zero pairs is the reference's "bugs in the code" case and
/// answers nothing here rather than dividing by zero.
#[must_use]
pub fn collision_share(damage: i32, pairs: i32) -> i32 {
    if pairs <= 0 || damage <= 0 {
        return 0;
    }
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let share = (f64::from(damage) * 0.5 / f64::from(pairs)) as i32;
    share
}

/// How hard the engine works for one step, as the percentage the fuel rule takes.
///
/// Reference: `vehicle::thrust` (`vehicle_move.cpp:528-542`) - the load is the share of the acceleration the engine has to supply:
/// `1000 * |vel_inc| / accel` while thrusting, and `1000 * max(0, |vel_inc| - max_brake) / accel` while braking, because the brakes take the first 20 mph/s and the
/// engine takes only what is left. When there is no slope to fight (`desired_velocity` unknown) it is simply **1000 thrusting, 0 coasting**.
///
/// `vel_inc` is the speed change this step will try to make, already clamped to the effective cruise velocity by the caller.
#[must_use]
pub fn drive_load(vel_inc: i32, accel: i32, max_brake: i32, thrusting: bool) -> i32 {
    if accel <= 0 {
        return if thrusting { 1000 } else { 0 };
    }
    if thrusting {
        #[allow(clippy::cast_possible_truncation)]
        let load = 1000 * vel_inc.abs() / accel;
        load.clamp(0, 1000)
    } else {
        #[allow(clippy::cast_possible_truncation)]
        let load = 1000 * (vel_inc.abs() - max_brake).max(0) / accel;
        load.clamp(0, 1000)
    }
}

/// Moves every vehicle that is under way, on the turn's dice, and answers how many moved.
///
/// Reference: `map::vehmove` (`map.cpp:1090`) - one pass over the vehicles, each taking its own step. The port keeps that shape: a vehicle with no velocity is skipped rather
/// than stepped, and the dice are taken once for the whole pass and put back, so the stream a replay sees is the stream the turn used.
pub fn move_vehicles(world: &mut World) -> usize {
    let mut vehicles = Vec::new();
    let mut query = world.query::<(Entity, &crate::vehicles::Vehicle)>();
    for (entity, built) in query.iter(world) {
        if built.velocity != 0 {
            vehicles.push(entity);
        }
    }
    let mut rolls =
        crate::singletons::take_rolls(world).unwrap_or_else(|| cdda_core_types::rng::Rolls::new(0));
    let mut moved = 0;
    for vehicle in vehicles {
        if move_step_with_rolls(world, vehicle, &mut rolls) {
            moved += 1;
        }
    }
    crate::singletons::put_rolls(world, rolls);
    moved
}
