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
    // **The caller owns randomness**, the same way [`move_step`] leaves it to its turn: this entry point takes a fresh stream so a thrust still wears an engine,
    // deterministically, for the tests and the inspection tools.
    thrust_with_rolls(
        world,
        vehicle,
        direction,
        &mut cdda_core_types::rng::Rolls::new(0),
    )
}

/// One press of the accelerator, drawing from the game's own stream for the damage a hard run does to the engines.
///
/// Reference: `vehicle::thrust` (`vehicle_move.cpp:470-600`): the engines that are on and fuelled burn, and then `strn = int( strain * strain * 100 )` goes to
/// `do_engine_damage` for **every** engine (`:591-595`). The dice arrive as an argument so the turn's one stream decides, exactly like the wear rule itself.
pub fn thrust_with_rolls(
    world: &mut World,
    vehicle: Entity,
    direction: i32,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> bool {
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
    // **Then the engines wear, if the throttle asked too much of them.** Reference: `vehicle_move.cpp:591-595`, where the strain is squared, scaled by a hundred,
    // truncated, and handed to every engine; the port's `wear_engines` draws the same two dice per engine from the stream the turn owns.
    let safe = crate::vehicle_drive::safe_ground_velocity(world, vehicle).unwrap_or(max_velocity);
    let strain = crate::vehicle_drive::strain(next, safe, max_velocity);
    crate::vehicle_drive::wear_engines(world, vehicle, strain, rolls);
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
    slowdown_with_skid(
        air_drag,
        rolling_drag,
        mass_g,
        at_velocity,
        engine_running,
        1.0,
    )
}

/// The same rule with the skid factor at hand: [`skid_drag_factor`] multiplies the **rolling** drag, so a sliding hull scrubs speed off without changing its air resistance.
///
/// Reference: `vehicle::slowdown` (`vehicle_move.cpp:112-153`), whose literal is `f_rolling_drag * skid_factor`.
#[must_use]
pub fn slowdown_with_skid(
    air_drag: f64,
    rolling_drag: f64,
    mass_g: i64,
    at_velocity: i32,
    engine_running: bool,
    skid_factor: f64,
) -> i32 {
    let mps = vmiph_to_mps(at_velocity.abs());
    let mut drag = air_drag * mps * mps;
    if mass_g > 0 {
        drag += rolling_drag * (ROLLING_CONSTANT_TO_VARIABLE + mps) * skid_factor;
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

/// The speed a hull that cannot stand on its wheels is left with.
///
/// Reference: `map::move_vehicle` (`map.cpp:909`): `veh.velocity -= std::clamp( veh.velocity, -2000, 2000 )`, which takes exactly twenty miles an hour off anything faster and
/// stops anything slower dead. Sign-preserving, because the clamp is applied to the same value that is subtracted.
#[must_use]
pub fn drag_without_wheels(velocity: i32) -> i32 {
    velocity - velocity.clamp(-2_000, 2_000)
}

/// Wears the speed off a hull that is being dragged, and chews the ground it is dragged over.
///
/// Reference: `map::move_vehicle` (`map.cpp:903-914`): when the vehicle cannot stand on its wheels, is not vertical, is neither floating nor flying, and is at ground level, its
/// speed takes [`drag_without_wheels`] and **every square it covers that is dirt or grass becomes a dirt mound**. The reference writes the mound by terrain id; the port asks the
/// terrain name table where those three names live and leaves any other square alone.
///
/// **The vertical, watercraft and flying exemptions are not ported** - the store has no vertical travel, no floating hull and no flight yet, and each is named in
/// `docs/parity.md` rather than assumed false.
pub fn wheelless_drag(world: &mut World, vehicle: Entity) -> bool {
    let Some(built) = world.get::<crate::vehicles::Vehicle>(vehicle) else {
        return false;
    };
    if built.parts.iter().all(|part| !part.broken) {
        // Every part intact is a vehicle that may still be on its wheels; the wheel rule decides properly.
    }
    if crate::vehicles::valid_wheel_config(world, vehicle) {
        return false;
    }
    let Some(at) = world.get::<crate::Position>(vehicle).copied() else {
        return false;
    };
    if at.z != 0 {
        return false;
    }
    let velocity = world
        .get::<crate::vehicles::Vehicle>(vehicle)
        .map_or(0, |built| built.velocity);
    let dragged = drag_without_wheels(velocity);
    if let Some(mut built) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
        built.velocity = dragged;
    }
    let Some(dirt) = crate::terrain_types::code_of(world, "t_dirt") else {
        return true;
    };
    let Some(grass) = crate::terrain_types::code_of(world, "t_grass") else {
        return true;
    };
    let Some(mound) = crate::terrain_types::code_of(world, "t_dirtmound") else {
        return true;
    };
    for square in crate::vehicles::occupied_squares(world, vehicle) {
        let Some(code) = crate::square::terrain_at(world, square) else {
            continue;
        };
        if code == dirt || code == grass {
            crate::square::set_terrain(world, square, mound);
        }
    }
    true
}

/// How much harder the rolling drag bites when the hull is sliding rather than rolling.
///
/// Reference: `vehicle::slowdown` (`vehicle_move.cpp:123-126`): `skid_factor = 1 + 24 * std::abs( units::sin( face.dir() - move.dir() ) )`. It multiplies the **rolling** drag
/// and nothing else - air resistance does not care which way the hull is pointing - which is why a side-on skid scrubs speed off while a straight one costs no more than usual.
///
/// The factor is one at nought and a hundred and eighty degrees, and twenty-five when the vehicle is travelling at right angles to its nose.
#[must_use]
pub fn skid_drag_factor(face_dir: i32, move_dir: i32) -> f64 {
    let between = f64::from((face_dir - move_dir).rem_euclid(360)).to_radians();
    1.0 + 24.0 * between.sin().abs()
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
    // Reference: the factor is built from the hull's facing and the direction it last moved, which the store keeps apart precisely so this angle can be asked for.
    let skid = world
        .get::<crate::vehicles::Vehicle>(vehicle)
        .map_or(1.0, |built| {
            skid_drag_factor(
                crate::vehicles::floor_to_heading(built.rotation),
                built.move_dir,
            )
        });
    Some(slowdown_with_skid(
        air,
        rolling,
        mass,
        at_velocity,
        engine_running,
        skid,
    ))
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
    // **A hull on its belly wears its speed off before it is asked to move.** Reference: `map::move_vehicle` runs the wheel-less drag at the top of the move (`map.cpp:903-914`), so
    // the speed this step is planned with is the speed after the drag - and the ground it is sitting on is already chewed by the time the step is attempted.
    wheelless_drag(world, vehicle);
    let Some(built) = world.get::<crate::vehicles::Vehicle>(vehicle) else {
        return false;
    };
    let velocity = built.velocity;
    let of_turn = built.of_turn;
    let skidding = built.skidding;
    let move_dir = built.move_dir;

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
    // **Where a vehicle travels depends on whether it is sliding.** Reference: `vehicle::act_on_map` (`vehicle_move.cpp:1969-1983`) picks the move vector from three
    // cases - a skidding vehicle keeps going along its last move, a turning one steps along its **wheels**, and a straight one along its hull - and only afterwards does
    // the hull face the way it went (`map.cpp:934-939`), which is why a steering press turns the *next* step and not the current one.
    let heading = f64::from(if skidding {
        move_dir
    } else {
        wheel_direction(world, vehicle)
    });
    let (dx, dy) = crate::rotation::coord_translate(heading, (0, 0), (1, 0));
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
        // **Two hulls meeting, on the line between their centres of mass.** Reference: `map.cpp:1075-1146` - the port used to exchange the *speeds* in one dimension, which
        // could not tell a head-on hit from a side hit and left a glancing blow's sideways motion out entirely. This is the reference's rule; the pair count is the one part
        // each the port's collision finds, not the reference's whole list (`map.cpp:1157-1169`), which is its own named gap.
        let damage = vehicle_impact(world, vehicle, target);
        let share = collision_share(damage, 1);
        crate::vehicles::damage(world, vehicle, part, share, "bash", false);
        crate::vehicles::damage(world, target, target_part, share, "bash", false);
        // **And the mover stops for this turn**, which is the reference's `return nullptr` "to let the hit vehicle move away" (`map.cpp:898-901`) rather than a destroyed
        // vehicle - round 172 looked for the cause of that null for a whole round.
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
        let prev_velocity = world
            .get::<crate::vehicles::Vehicle>(vehicle)
            .map_or(0, |built| built.velocity);
        let part_dmg = dent_on_crash(world, vehicle, part, square, bashable, rolls);
        // **A hard enough blow lines a turn up for the next move** - the reference's `coll_turn` (`vehicle_move.cpp:1208-1225`), spent in `map.cpp:938-941` when the vehicle
        // can move again. A crash that cannot skid draws nothing, which is why the dice are asked here and not later.
        if let Some(degrees) = skid_turn(prev_velocity, 0, part_dmg, rolls)
            && let Some(mut built) = world.get_mut::<crate::vehicles::Vehicle>(vehicle)
        {
            built.coll_turn = degrees;
        }
        // **The wall stops the car the way the reference's `veh.stop()` does** (`map.cpp:948-950`), which includes putting the wheel back on the hull's facing.
        stop_vehicle(world, vehicle);
        return false;
    }

    // **A lined-up turn is spent on the move, not before it.** Reference: `map.cpp:938-941` sets `skidding` and turns the hull only on an accepted move, which is why a vehicle
    // held still by another wall keeps its `coll_turn` for the attempt that succeeds.
    let mut coll_turn_just_spent = None;
    if let Some(mut built) = world.get_mut::<crate::vehicles::Vehicle>(vehicle)
        && built.coll_turn != 0
    {
        let lined_up = built.coll_turn;
        coll_turn_just_spent = Some(lined_up);
        built.coll_turn = 0;
        built.skidding = true;
        built.rotation = (built.rotation + f64::from(lined_up)).rem_euclid(360.0);
    }
    // Reference: `vehicle::turn` moves `turn_dir` with the hull, so the wheels a skid is following are the wheels the crash turned.
    if let Some(coll_turn) = coll_turn_just_spent
        && let Some(mut wheels) = world.get_mut::<Wheels>(vehicle)
    {
        wheels.turn_dir = (wheels.turn_dir + coll_turn).rem_euclid(360);
    }
    let moved = crate::vehicles::move_vehicle(world, vehicle, step);
    // **The leftover turn decays a degree a step.** Reference: `map.cpp:917-928`, which runs before the move is accepted, so the decay happens whether or not the square is
    // taken - and `decayed_turn` is the same rule the port already had and nothing called.
    if let Some(mut built) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
        built.last_turn = decayed_turn(built.last_turn, 1);
    }
    // **The hull faces where the step went, unless it is skidding.** Reference: `map.cpp:934-939` sets `veh.face` to the direction of travel on an accepted move and takes
    // `veh.turn_dir` while skidding. A backwards step is covered by this rather than by a flip of its own: the step already points the way the vehicle travels, so the heading of
    // that step is the answer in both directions.
    if moved {
        let skidding = world
            .get::<crate::vehicles::Vehicle>(vehicle)
            .is_some_and(|built| built.skidding);
        let wheel = wheel_direction(world, vehicle);
        // **The facing is the direction the vehicle was travelling, not the square it landed on.** Reference: `map::move_vehicle` assigns `veh.face = facing`,
        // and `facing` is the `tileray` the step was taken from (`map.cpp:934-937`) - so a hull pointed at 330 degrees faces 330 even on the step where the
        // tileray's own rounding takes it one square east. The discrete step and the heading are two different numbers.
        #[allow(clippy::cast_possible_truncation)]
        let travel = heading.round() as i32;
        if let Some(mut built) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
            built.rotation = f64::from(facing_after_move(skidding, wheel, travel));
            // **`move` is the direction travelled, `face` is the hull.** Reference: `map.cpp:942` sets `veh.move = facing` on every accepted move, and the two are different
            // exactly while a skid carries the hull where the wheels point - which is the angle `possibly_recover_from_skid` measures.
            built.move_dir = travel;
        }
        // **And a skid may end here.** Reference: `map::move_vehicle` calls `possibly_recover_from_skid` right after an accepted move (`map.cpp:958-961`), which is the only
        // moment the two headings have just been written - so the recovery sees the step that was taken, not the one before it.
        recover_from_skid(world, vehicle, rolls);
    }
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
) -> i32 {
    let Some(density) = part_density(world, vehicle, part) else {
        return 0;
    };
    let mass = crate::vehicle_mass::mass(world, vehicle);
    let Some(velocity) = world
        .get::<crate::vehicles::Vehicle>(vehicle)
        .map(|built| built.velocity)
    else {
        return 0;
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
    impulse
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

/// The turn a smashed part gives the vehicle, and whether the collision was a skid.
///
/// Reference: `vehicle_move.cpp:1208-1226`, in the reference's own order:
///
/// ```text
/// turn_amount = rng( 1, 3 ) * sqrt( part_dmg ); turn_amount /= 15; if < 1 then 1; *= 15; if > 120 then 120
/// turn_roll   = rng( 0, 100 )
/// skid        = turn_roll < | ( prev_velocity - coll_velocity ) / 100.0f * 2.0f |
/// direction   = one_in( 2 ) ? turn_amount : -turn_amount
/// ```
///
/// **The order of the draws is the rule**, not an implementation detail: the amount, then the roll that decides, then the direction - and the direction is only drawn when the
/// roll says the vehicle skids, which is why a collision that cannot skid leaves the dice where the reference leaves them.
///
/// `None` is "this was a collision, not a skid"; `Some(degrees)` is the turn to add, signed.
#[must_use]
pub fn skid_turn(
    prev_velocity: i32,
    coll_velocity: i32,
    part_dmg: i32,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> Option<i32> {
    let raw = f64::from(rolls.rng(1, 3)) * f64::from(part_dmg.max(0)).sqrt();
    #[allow(clippy::cast_possible_truncation)]
    let mut amount = raw as i32;
    amount /= 15;
    if amount < 1 {
        amount = 1;
    }
    amount *= 15;
    if amount > 120 {
        amount = 120;
    }
    let roll = rolls.rng(0, 100);
    let lost = f64::from(prev_velocity - coll_velocity) / 100.0 * 2.0;
    if f64::from(roll) < lost.abs() {
        // Reference: `one_in( 2 )` picks the side, and it is drawn **after** the roll that decided the skid.
        let side = if rolls.rng(1, 2) == 1 { 1 } else { -1 };
        return Some(side * amount);
    }
    None
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

/// A vehicle's velocity as a vector: the direction it is travelling in, times its speed.
///
/// Reference: `vehicle::velo_vec` (`vehicle_move.cpp:1601-1611`) - a **skidding** vehicle's velocity points along `move`, everything else along `face`. This is what makes the
/// exchange depend on where the vehicles were pointed rather than only on how fast they were going.
#[must_use]
fn velocity_vector(rotation: f64, move_dir: i32, skidding: bool, velocity: i32) -> (f64, f64) {
    let degrees = if skidding {
        f64::from(move_dir)
    } else {
        rotation
    };
    let radians = degrees.to_radians();
    (
        radians.cos() * f64::from(velocity),
        radians.sin() * f64::from(velocity),
    )
}

/// The centre of mass in the vehicle's **rotated** frame, which is what the collision axis is drawn from.
///
/// Reference: `vehicle::rotated_center_of_mass` (`vehicle.cpp:3829-3835`) reads `mass_center_precalc`, the centre computed from the parts' precalculated (post-rotation)
/// positions. The port's [`crate::vehicle_mass::center_of_mass`] is the unrotated one, so it is rotated here through the same `coord_translate` the parts go through.
#[must_use]
fn rotated_center_of_mass(world: &World, vehicle: Entity) -> (f64, f64) {
    let rotation = world
        .get::<crate::vehicles::Vehicle>(vehicle)
        .map_or(0.0, |built| built.rotation);
    let (x, y) = crate::vehicle_mass::center_of_mass(world, vehicle);
    let (rotated_x, rotated_y) = crate::rotation::coord_translate(rotation, (0, 0), (x, y));
    (f64::from(rotated_x), f64::from(rotated_y))
}

/// **Two hulls meeting.** The reference splits both velocities along the line between the two vehicles' centres of mass, exchanges only that component, keeps the sideways
/// component as it was, and drops both machines' movement points into a shared average.
///
/// Reference: `map::vehicle_vehicle_collision` (`map.cpp:1014-1200`). The exchange is a partially elastic collision -
/// `v1' = ( m2 v2 (1+e) + v1 (m1 - m2 e) ) / (m1 + m2)` - with the restitution `e` from [`collision_factor`]. The **perpendicular component is untouched**, which is why a
/// glancing hit sends both hulls sideways instead of stopping them, and why this cannot be the one-dimensional exchange the port used to run.
///
/// The new speed is the magnitude of the resulting vector, signed by whether it still points the way the hull faces; the reference assigns that float to an `int`, so the
/// fraction is truncated rather than rounded. Returns the deformation energy as damage, before it is shared out.
pub fn vehicle_impact(world: &mut World, vehicle: Entity, target: Entity) -> i32 {
    let (Some(at), Some(them)) = (
        world.get::<crate::Position>(vehicle).copied(),
        world.get::<crate::Position>(target).copied(),
    ) else {
        return 0;
    };
    let Some(mine) = world.get::<crate::vehicles::Vehicle>(vehicle) else {
        return 0;
    };
    let Some(theirs) = world.get::<crate::vehicles::Vehicle>(target) else {
        return 0;
    };
    let (rotation, of_turn, velocity) = (mine.rotation, mine.of_turn, mine.velocity);
    let (their_rotation, their_of_turn, their_velocity) =
        (theirs.rotation, theirs.of_turn, theirs.velocity);
    let move_dir = mine.move_dir;
    let their_move_dir = theirs.move_dir;
    let (skidding, their_skidding) = (mine.skidding, theirs.skidding);

    // **Kilogram, not gram**: the reference divides `to_kilogram( total_mass )`, and the energy below is in those units.
    #[allow(clippy::cast_precision_loss)]
    let m1 = crate::vehicle_mass::mass(world, vehicle) as f64 / 1000.0;
    #[allow(clippy::cast_precision_loss)]
    let m2 = crate::vehicle_mass::mass(world, target) as f64 / 1000.0;
    if m1 <= 0.0 || m2 <= 0.0 {
        return 0;
    }

    let w1 = velocity_vector(rotation, move_dir, skidding, velocity);
    let w2 = velocity_vector(
        their_rotation,
        their_move_dir,
        their_skidding,
        their_velocity,
    );
    let c1 = rotated_center_of_mass(world, vehicle);
    let c2 = rotated_center_of_mass(world, target);
    #[allow(clippy::cast_precision_loss)]
    let axis = (
        (f64::from(at.x) + c1.0) - (f64::from(them.x) + c2.0),
        (f64::from(at.y) + c1.1) - (f64::from(them.y) + c2.1),
    );
    let length = axis.0.hypot(axis.1);
    if length == 0.0 {
        return 0;
    }
    let (ax, ay) = (axis.0 / length, axis.1 / length);
    // The perpendicular axis is the collision axis turned a quarter turn: `collision_axis_x = collision_axis_y.rotated( M_PI / 2 )`.
    let (bx, by) = (-ay, ax);

    let first_y = ax * w1.0 + ay * w1.1;
    let first_x = bx * w1.0 + by * w1.1;
    let second_y = ax * w2.0 + ay * w2.1;
    let second_x = bx * w2.0 + by * w2.1;
    let e = collision_factor(first_y / 100.0 - second_y / 100.0);

    // **The exchange is per axis**, and `exchanged` is that algebra: the reference's `v1' = ( m2 v2 (1+e) + v1 (m1 - m2 e) ) / (m1 + m2)` is the same expression multiplied
    // out, so one rule serves the head-on case and the along-the-axis case with no duplicate.
    let (first_y_after, second_y_after) = exchanged(m1, first_y, m2, second_y, e);
    let final_first = (
        ax * first_y_after + bx * first_x,
        ay * first_y_after + by * first_x,
    );
    let final_second = (
        ax * second_y_after + bx * second_x,
        ay * second_y_after + by * second_x,
    );

    let speed = |vector: (f64, f64), degrees: f64| {
        let radians = degrees.to_radians();
        let magnitude = vector.0.hypot(vector.1);
        let facing = vector.0 * radians.cos() + vector.1 * radians.sin();
        let signed = if facing < 0.0 { -magnitude } else { magnitude };
        #[allow(clippy::cast_possible_truncation)]
        let whole = signed as i32;
        whole
    };
    let settled = (
        speed(final_first, rotation),
        speed(final_second, their_rotation),
    );
    // **The movement points are pooled and shared unevenly on purpose**: the vehicle that was hit is given the larger share (`map.cpp:1133-1140`), so the pair does not sit
    // frozen against each other for a turn.
    let average = their_of_turn.midpoint(of_turn).max(0.1);
    if let Some(mut state) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
        state.velocity = settled.0;
        state.of_turn = average * 0.9;
    }
    if let Some(mut state) = world.get_mut::<crate::vehicles::Vehicle>(target) {
        state.velocity = settled.1;
        state.of_turn = average * 1.1;
    }

    let before = 0.5 * m1 * (w1.0 * w1.0 + w1.1 * w1.1) + 0.5 * m2 * (w2.0 * w2.0 + w2.1 * w2.1);
    let after = 0.5 * m1 * (final_first.0 * final_first.0 + final_first.1 * final_first.1)
        + 0.5 * m2 * (final_second.0 * final_second.0 + final_second.1 * final_second.1);
    collision_damage(before - after)
}

/// The deformation energy of a collision, as damage: `| d_E / 1000 / 2000 |`.
///
/// Reference: `map.cpp:1144` - **two divisors, and the second is 2000**, where the obstacle branch divides by 400: the two paths are not the same rule with a different mass.
#[must_use]
pub fn collision_damage(lost_energy: f64) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    let damage = (lost_energy.abs() / 1000.0 / 2000.0) as i32;
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

/// Which way the throttle is held, given where the driver wants to be and where the car is.
///
/// Reference: `vehicle::gain_moves` (`vehicle.cpp`) calls `thrust( here, sgn( cruise_velocity - velocity ) )` for a vehicle in control with a cruise set, so the throttle
/// points at the **cruise**, not at the current heading: a car that is over its cruise is told to slow by thrusting the other way. Equal speeds mean no throttle at all.
#[must_use]
pub fn cruise_direction(cruise_velocity: i32, velocity: i32) -> i32 {
    (cruise_velocity - velocity).signum()
}

/// Lets every driver's hand act on the wheel, and answers how many vehicles were driven.
///
/// Reference: `vehicle::gain_moves` - a vehicle with a driver in control **and** a cruise set thrusts towards the cruise; everything else does nothing here and either
/// burns idle fuel or sleeps. The port asks the store who is driving ([`crate::vehicles::driver_of`]) and which way the throttle points, and leaves the thrust itself to
/// [`thrust`], which is where the fuel burns.
pub fn drive_vehicles(world: &mut World) -> usize {
    let mut vehicles = Vec::new();
    let mut query = world.query::<(Entity, &crate::vehicles::Vehicle)>();
    for (entity, built) in query.iter(world) {
        if built.cruise_velocity != 0 {
            vehicles.push(entity);
        }
    }
    // **The game's own stream, taken and put back**, exactly as `move_vehicles` does: a hard run wears engines, and the dice that decide it belong to the turn rather than
    // to whichever subsystem asked first.
    let mut rolls =
        crate::singletons::take_rolls(world).unwrap_or_else(|| cdda_core_types::rng::Rolls::new(0));
    let mut driven = 0;
    for vehicle in vehicles {
        if crate::vehicles::driver_of(world, vehicle).is_none() {
            continue;
        }
        let velocity = world
            .get::<crate::vehicles::Vehicle>(vehicle)
            .map_or(0, |built| built.velocity);
        let cruise = world
            .get::<crate::vehicles::Vehicle>(vehicle)
            .map_or(0, |built| built.cruise_velocity);
        // **A vehicle that cannot stand on its wheels does not move at all.** Reference: `vehicle::thrust` (`vehicle_move.cpp:473-479`) stops the vehicle and says so before it
        // asks for any power, so a hull with no wheels, one wheel that is not stable, or its weight off its wheels stays where it is however hard the driver asks.
        if !crate::vehicles::valid_wheel_config(world, vehicle) {
            stop_vehicle(world, vehicle);
            crate::messages::push(world, "The vehicle doesn't have enough wheels to move!");
            continue;
        }
        let direction = cruise_direction(cruise, velocity);
        if direction == 0 {
            continue;
        }
        if thrust_with_rolls(world, vehicle, direction, &mut rolls) {
            driven += 1;
        }
    }
    crate::singletons::put_rolls(world, rolls);
    driven
}

/// One press of the cruise control, and where the cruise ends up.
///
/// Reference: `vehicle::cruise_thrust` (`vehicle_move.cpp:635-675`), transplanted rule for rule:
///
/// - **The safe speed wins if the step crosses it**: stepping past `safe` stops exactly on it, so the driver cannot tap past the speed the engine is happy with.
/// - **Stepping down from a limit moves one first**, so the rounding below lands on a multiple of the step instead of sticking at the limit.
/// - **The cruise rounds down to a multiple of the step**, always towards zero - the reference's own comment says the result is "always equal to the original or closer to
///   zero, even if negative".
/// - **And the caps apply last**: no cruise above the top speed, none below the deepest reverse.
#[must_use]
pub fn cruise_thrust(
    cruise_velocity: i32,
    amount: i32,
    safe_velocity: i32,
    max_velocity: i32,
    max_reverse_velocity: i32,
) -> i32 {
    if amount == 0 {
        return cruise_velocity;
    }
    let step = amount.abs();
    let mut cruise = cruise_velocity;
    let next = cruise.saturating_add(amount);
    if (cruise < safe_velocity && safe_velocity < next)
        || (cruise > safe_velocity && safe_velocity > next)
    {
        cruise = safe_velocity;
    } else {
        if (amount < 0 && (cruise == safe_velocity || cruise == max_velocity))
            || (amount > 0 && cruise == max_reverse_velocity)
        {
            cruise += if amount < 0 { -1 } else { 1 };
        } else {
            cruise = next;
        }
        cruise = cruise / step * step;
    }
    cruise.clamp(max_reverse_velocity, max_velocity)
}

/// How far a steering step moves the wheel, in degrees.
///
/// Reference: `vehicles::steer_increment = 15_degrees` (`vehicle.h:88`) - the direction is only ever kept on a multiple of it.
pub const STEER_INCREMENT_DEGREES: i32 = 15;

/// One press of the wheel, on the vehicle itself.
///
/// Reference: `vehicle::turn` (`vehicle_move.cpp:676-685`), rule for rule:
///
/// ```text
/// if deg == 0: return
/// if velocity < 0 and not REVERSE_STEERING: deg = -deg
/// last_turn = deg
/// turn_dir = normalize( turn_dir + deg )
/// turn_dir = round_to_multiple_of( turn_dir, steer_increment )
/// ```
///
/// **`last_turn` is the press, not the rounded wheel**: the hull still owes what the driver asked for, and spends it a degree at a time as it moves. `REVERSE_STEERING` is
/// an option the port has not loaded, so the reference's default - off, which flips a press made in reverse - is what this does.
pub fn turn(world: &mut World, vehicle: Entity, degrees: i32) {
    if degrees == 0 {
        return;
    }
    let velocity = world
        .get::<crate::vehicles::Vehicle>(vehicle)
        .map_or(0, |built| built.velocity);
    let press = if velocity < 0 { -degrees } else { degrees };
    let current = wheel_direction(world, vehicle);
    let next = steered_direction(current, press, 0);
    world.entity_mut(vehicle).insert(Wheels { turn_dir: next });
    if let Some(mut built) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
        built.last_turn = press;
    }
}

/// Where the wheels point after a steering step.
///
/// Reference: `vehicle::turn` (`vehicle_move.cpp:678-685`): a reversing vehicle steers the other way unless the `REVERSE_STEERING` option is on (the option is not ported;
/// its default is off, which is the behaviour here), the result is normalised into one full turn, and then **rounded to a multiple of fifteen degrees** so the wheel is
/// never left between clicks.
#[must_use]
pub fn steered_direction(current_degrees: i32, degrees: i32, velocity: i32) -> i32 {
    let step = if velocity < 0 { -degrees } else { degrees };
    let turned = (current_degrees + step).rem_euclid(360);
    let increment = f64::from(turned) / f64::from(STEER_INCREMENT_DEGREES);
    #[allow(clippy::cast_possible_truncation)]
    let clicks = increment.round() as i32;
    (clicks * STEER_INCREMENT_DEGREES).rem_euclid(360)
}

/// What a driver is asking of the vehicle this turn.
///
/// Reference: the reference keeps this in the avatar's own input handling - `cruise_thrust( here, -acceleration * thr_amount )` and `veh.turn( turn_delta )` are called
/// straight from the character's action code (`vehicle_move.cpp:1330-1340`, `1530-1550`), so the intent lives with the person rather than with the car. The port keeps that:
/// the component sits on the **creature**, and a turn applies it to the vehicle the creature is driving.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, bevy_ecs::component::Component)]
pub struct Controls {
    /// How many vmiph to press the cruise by this turn, positive forward.
    pub cruise: i32,
    /// How far to turn the wheel this turn, in degrees.
    pub steer: i32,
}

/// Applies every driver's intent to the vehicle they are driving, and answers how many were heard.
///
/// Reference: the `cruise_thrust` and `turn` calls in the character's action handling. A creature with no [`Controls`] - or one not sitting in a vehicle - asks for nothing,
/// and a press of zero changes nothing. The steering is applied to the vehicle's wheel direction as the reference keeps it; turning the hull is
/// [`crate::rotation`]'s job and is not called here.
pub fn apply_controls(world: &mut World) -> usize {
    let mut intents = Vec::new();
    let mut query = world.query::<(Entity, &Controls, &crate::vehicles::Riding)>();
    for (creature, controls, riding) in query.iter(world) {
        intents.push((creature, *controls, riding.vehicle));
    }
    let mut heard = 0;
    for (creature, controls, vehicle) in intents {
        if crate::vehicles::driver_of(world, vehicle) != Some(creature) {
            continue;
        }
        if controls.cruise != 0 {
            let Some(safe) = crate::vehicle_drive::safe_ground_velocity(world, vehicle) else {
                continue;
            };
            let Some(maximum) = crate::vehicle_drive::max_ground_velocity(world, vehicle) else {
                continue;
            };
            let battery = crate::vehicle_drive::battery_engine(world, vehicle);
            let reverse = crate::vehicle_drive::max_reverse_velocity(maximum, battery);
            let current = world
                .get::<crate::vehicles::Vehicle>(vehicle)
                .map_or(0, |built| built.cruise_velocity);
            let next = cruise_thrust(current, controls.cruise, safe, maximum, reverse);
            if let Some(mut built) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
                built.cruise_velocity = next;
            }
            heard += 1;
        }
        if controls.steer != 0 {
            turn(world, vehicle, controls.steer);
            heard += 1;
        }
    }
    heard
}

/// Where a vehicle's wheels point, in degrees from forward.
///
/// Reference: `vehicle::turn_dir` (`vehicle.h`) - kept beside the hull's facing because **the wheel turns before the hull does**, and the steering rule rounds it to a multiple
/// of `steer_increment`. It is a component rather than a field on [`crate::vehicles::Vehicle`] because most vehicles never have one set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, bevy_ecs::component::Component)]
pub struct Wheels {
    /// The wheel direction in degrees, always a multiple of [`STEER_INCREMENT_DEGREES`].
    pub turn_dir: i32,
}

/// Where a vehicle's wheels point, reading zero when they have never been turned.
#[must_use]
pub fn wheel_direction(world: &bevy_ecs::world::World, vehicle: Entity) -> i32 {
    world
        .get::<Wheels>(vehicle)
        .map_or(0, |wheels| wheels.turn_dir)
}

/// Whether a turn direction lands between the four axes rather than on one.
///
/// Reference: `vehicle::precalculate_vehicle_turning` (`vehicle_move.cpp:1655`) - `std::lround( to_degrees( new_turn_dir ) ) % 90 == 45`. The reference asks it to know
/// whether the vehicle's movement this step is diagonal, which decides how the wheels meet a rail; the port keeps the rule and will use it for the same reason when rails
/// arrive. The modulo is on the **normalised** angle, so a negative heading answers the same as its positive twin.
#[must_use]
pub fn is_diagonal_turn(degrees: i32) -> bool {
    degrees.rem_euclid(360) % 90 == 45
}

/// The speed a skid recovers to, or `None` when the hull stays sliding.
///
/// Reference: `vehicle::possibly_recover_from_skid` (`vehicle_move.cpp:1563-1590`), in the reference's order:
///
/// ```text
/// if last_turn > 13_degrees: return                       // the initial skid is not recoverable
/// dot = move_vec().dot_product( face_vec() )
/// if fabs( dot ) * 100 > dice( 9, 20 ):
///     skidding = false
///     velocity *= dot
///     if dot < -0.8: velocity *= -1                       // pointed backwards: travel towards the nose
/// ```
///
/// **`dice( 9, 20 )` is nine twenty-sided dice, not a uniform range**: the reference's `dice( number, sides )` sums that many rolls, so the threshold is a bell around 94.5 and
/// a skid straight down the nose - a hundred - beats it about half the time. Drawing it as one flat number would make recovery far too likely.
///
/// The nine draws happen **only** when the turn is small enough to allow a chance at all, which is why the guard comes first: an unrecoverable skid must not move the stream.
#[must_use]
pub fn skid_recovery(
    last_turn: i32,
    move_dir: i32,
    face_dir: i32,
    velocity: i32,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> Option<i32> {
    if last_turn > 13 {
        return None;
    }
    let between = f64::from((move_dir - face_dir).rem_euclid(360));
    let dot = between.to_radians().cos();
    let mut threshold = 0;
    for _ in 0..9 {
        threshold += rolls.rng(1, 20);
    }
    if dot.abs() * 100.0 > f64::from(threshold) {
        #[allow(clippy::cast_possible_truncation)]
        let mut recovered = (f64::from(velocity) * dot) as i32;
        if dot < -0.8 {
            recovered *= -1;
        }
        return Some(recovered);
    }
    None
}

/// Ends a skid that the dice allow, after an accepted move.
///
/// Reference: `map::move_vehicle`'s call (`map.cpp:958-961`) - the recovery runs once per moved step, on the headings the step has just written, so it measures the move that
/// happened rather than the one before it. A vehicle that is not skidding is left alone, and one that stays sliding keeps its speed.
fn recover_from_skid(world: &mut World, vehicle: Entity, rolls: &mut cdda_core_types::rng::Rolls) {
    let Some(built) = world.get::<crate::vehicles::Vehicle>(vehicle) else {
        return;
    };
    if !built.skidding {
        return;
    }
    let (last_turn, move_dir, velocity) = (built.last_turn, built.move_dir, built.velocity);
    let face_dir = crate::vehicles::floor_to_heading(built.rotation);
    let Some(recovered) = skid_recovery(last_turn, move_dir, face_dir, velocity, rolls) else {
        return;
    };
    if let Some(mut built) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
        built.velocity = recovered;
        built.skidding = false;
        built.move_dir = crate::vehicles::floor_to_heading(built.rotation);
    }
}

/// Which way a vehicle faces once its step has been accepted.
///
/// Reference: `map::move_vehicle` (`map.cpp:934-939`): on a successful move `veh.face = facing` - the direction the step travelled - and only a **skidding** vehicle takes
/// `veh.turn_dir` instead, because a skid carries the hull where the wheels point rather than where the driver aimed it. Both numbers are headings in degrees, normalised,
/// and the answer is the heading the hull now has.
#[must_use]
pub fn facing_after_move(skidding: bool, turn_dir: i32, travel_dir: i32) -> i32 {
    if skidding {
        turn_dir.rem_euclid(360)
    } else {
        travel_dir.rem_euclid(360)
    }
}

/// How much of a turn is left in the hull after one accepted step.
///
/// Reference: `map::move_vehicle` (`map.cpp:922-928`): `last_turn -= last_turn_dec` after the step, and `if( last_turn < last_turn_dec ) last_turn = 0`. So a turn smaller than
/// one decrement is gone in a single step, and a bigger one is spent over several - which is what makes a car keep curving for a moment after the wheel is centred.
#[must_use]
pub fn decayed_turn(last_turn: i32, decrement: i32) -> i32 {
    // **A turn left in either direction walks back towards zero**, and is cleared the moment it would cross it: the reference adds the decrement to a negative turn and
    // subtracts it from a positive one, with its own guard on each side.
    if last_turn == 0 {
        return 0;
    }
    let walks_back = last_turn < 0;
    let walked = if walks_back {
        last_turn + decrement
    } else {
        last_turn - decrement
    };
    let crossed = if walks_back {
        walked > -decrement
    } else {
        walked < decrement
    };
    if crossed { 0 } else { walked }
}

/// Brings a vehicle to a full stop, and puts its wheel back where the hull points.
///
/// Reference: `vehicle::stop` (`vehicle_move.cpp:687-698`) zeroes the speed, clears the skid, drops the turn and the carried movement, and aligns `move` with `face`; the
/// stopped branch of `vehicle::thrust` (`vehicle_move.cpp:455-458`) additionally puts `turn_dir` back on the facing, which is why a parked car's wheel is straight even if it
/// was left mid-turn.
pub fn stop_vehicle(world: &mut World, vehicle: Entity) {
    let facing = {
        let Some(mut built) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) else {
            return;
        };
        built.velocity = 0;
        built.of_turn_carry = 0.0;
        // **A stop clears the skid but not the turn it lined up.** Reference: `vehicle::stop` sets `skidding = false` (`vehicle_move.cpp:691`) and says nothing about
        // `coll_turn`, which the next accepted move still spends - a car that stopped against a wall is not a car that forgot being shoved aside.
        built.skidding = false;
        built.last_turn = 0;
        built.rotation
    };
    #[allow(clippy::cast_possible_truncation)]
    let facing_degrees = facing.round() as i32;
    world.entity_mut(vehicle).insert(Wheels {
        turn_dir: facing_degrees.rem_euclid(360),
    });
}
