//! **Where a target is, and how hard to steer at it.**
//!
//! Reference: `vehicle::get_angle_from_targ` (`vehicle.cpp:888-900`) and the static `get_turn_from_angle` (`vehicle.cpp:746-767`). Both are pure geometry, and both are read by two
//! rules the port has already met: `do_towing_move` (`:7118`) and `drive_to_local_target` (`:798`).
//!
//! The angle is `atan2( cross, dot )` between the hull's facing and the vector to the target, so it is **signed**: positive is a target to the hull's left, negative to its right.
//! The turn is a **ladder of degrees**, not a proportional answer, and its bands are asymmetric at the edges - `> 10`, `<= 45`, `< 180` - which is why they are tables here and not
//! comparisons invented at the call site.

/// **The angle to a target from a hull's facing**, in degrees - `vehicle::get_angle_from_targ` (`vehicle.cpp:888-900`).
///
/// `face` is the hull's facing vector and `target` the vector from the hull to the target, both in the map's own units. The cross product gives the sine's sign and the dot the cosine,
/// so the answer is in `(-180, 180]` degrees and **zero means dead ahead**.
#[must_use]
pub fn angle_from_target(face: (f64, f64), target: (f64, f64)) -> f64 {
    let (face_x, face_y) = face;
    let (target_x, target_y) = target;
    let cross = face_x * target_y - target_x * face_y;
    let dot = face_x * target_x + face_y * target_y;
    cross.atan2(dot).to_degrees()
}

/// **How hard to steer, as a quarter-turn count** - the static `get_turn_from_angle` (`vehicle.cpp:746-767`).
///
/// The bands, in the reference's own order and with its own edges:
///
/// | Angle | Forward | Reverse |
/// |---|---|---|
/// | `> 10`, `<= 45` | 1 | **4** |
/// | `> 45`, `<= 90` | 3 | 3 |
/// | `> 90`, `< 180` | 4 | **1** |
/// | `> -10`, `>= -45` | -1 | **-4** |
/// | `>= -90`, `< -45` | -3 | -3 |
/// | `< -90`, `> -180` | -4 | **-1** |
/// | exactly ±180 **and on the target** | 0 | 0 |
/// | anything else | 0 | 0 |
///
/// The reverse column is why this cannot be a sign flip: a reversing vehicle steers **away** from the target's side, so the far bands swap their extremes while the middle band is the
/// same either way.
// **The comparisons are the reference's own**, including exact equality where it has one: `angle == 180_degrees` is a comparison of a value the reference produced the same way, and a
// tolerant version would answer differently on the exact edge the reference spends a branch on.
#[allow(clippy::float_cmp)]
#[must_use]
pub fn turn_from_angle(angle: f64, on_target: bool, reverse: bool) -> i32 {
    if angle > 10.0 && angle <= 45.0 {
        if reverse { 4 } else { 1 }
    } else if angle > 45.0 && angle <= 90.0 {
        3
    } else if angle > 90.0 && angle < 180.0 {
        if reverse { 1 } else { 4 }
    } else if (-45.0..-10.0).contains(&angle) {
        if reverse { -4 } else { -1 }
    } else if (-90.0..-45.0).contains(&angle) {
        -3
    } else if angle < -90.0 && angle > -180.0 {
        if reverse { -1 } else { -4 }
    } else if angle.abs() == 180.0 && on_target {
        // Exactly on the button: keep driving, the next path point will be picked up.
        0
    } else {
        0
    }
}

/// **The steering quantum**: every steering decision lands on a multiple of fifteen degrees - `vehicles::steer_increment` (`vehicle.h:88`), `15_degrees`.
///
/// It is a **quantum, not a speed**: a hull does not turn by an arbitrary angle, it turns by whole increments, and every rule that rounds a heading in the reference rounds it to this.
pub const STEER_INCREMENT_DEGREES: f64 = 15.0;

/// **An angle brought into `[0, 360)`** - the reference's `normalize( units::angle )`, which every heading passes through before it is rounded or compared.
#[must_use]
pub fn normalise(degrees: f64) -> f64 {
    let wrapped = degrees % 360.0;
    if wrapped < 0.0 {
        wrapped + 360.0
    } else {
        wrapped
    }
}

/// **An angle rounded to the nearest steering quantum** - `round_to_multiple_of( val, of )` (`units_utility.h:42-47`), whose body is `lround( val / of ) * of`: the division is rounded
/// **half away from zero**, not truncated, so seven and a half degrees rounds up to fifteen and -7.5 rounds down to -15.
#[must_use]
pub fn round_to_steer_increment(degrees: f64) -> f64 {
    let multiple = (degrees / STEER_INCREMENT_DEGREES).round();
    multiple * STEER_INCREMENT_DEGREES
}

/// **The facing a towed hull is steered to** - `vehicle::selfdrive` (`vehicle_move.cpp:1331-1338`), which turns by `trn * steer_increment` when the turn is not zero. The turn is the
/// quarter-turn count [`turn_from_angle`] answers, so a hull whose cable asks for "one quarter" turns exactly fifteen degrees - it does not aim at the target, it steps toward it.
#[must_use]
pub fn steered_facing(current_degrees: f64, turn: i32) -> f64 {
    let step = f64::from(turn) * STEER_INCREMENT_DEGREES;
    normalise(current_degrees + step)
}

/// **The heading an immediate-path check runs along** - `vehicle::immediate_path` (`vehicle.cpp:725-731`): the hull's face plus a rotation, **normalised and then rounded to the steering
/// quantum**, and the clamp happens after the wrap, so a heading of 359 degrees and a rotation of 4 rounds to zero rather than to 372.
#[must_use]
pub fn immediate_path_heading(face_degrees: f64, rotate_degrees: f64) -> f64 {
    let adjusted = normalise(face_degrees + rotate_degrees);
    let rounded = round_to_steer_increment(adjusted);
    normalise(rounded)
}
