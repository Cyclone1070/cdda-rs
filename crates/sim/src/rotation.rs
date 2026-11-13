//! The reference's `tileray`, reduced to the one question a vehicle's square asks: where does a mount land when the vehicle faces a direction.
//!
//! Reference: `tileray` (`tileray.cpp:1-160`) is the line iterator the drawing code uses, and a vehicle borrows it twice. `vehicle::precalc_mounts` (`vehicle.cpp:3643-3660`) walks
//! every distinct mount through `coord_translate`, and `coord_translate` (`vehicle.cpp:3598-3615`) advances a fresh ray by `mount.x - pivot.x`, then offsets it sideways by
//! `mount.y - pivot.y` with `ortho_dx`/`ortho_dy`. The result is the square the part stands on, rounded to whole squares the way the reference rounds it.
//!
//! **This is a transcription, not a design.** The leftover counter, the quadrant signs and the rounding are the reference's own arithmetic, because the port has to land on the
//! same squares the reference lands on at 45 degrees, where no exact answer exists.

/// The sign of x for each quadrant, as the reference tabulates it (`tileray.cpp:14`).
const SX: [i32; 4] = [1, -1, -1, 1];
/// The sign of y for each quadrant (`tileray.cpp:15`).
const SY: [i32; 4] = [1, 1, -1, -1];

/// One ray from the reference's `tileray`, built from a facing in degrees.
struct TileRay {
    /// The x step of the last `advance`, after the quadrant signs are applied.
    dx: i32,
    /// The y step of the last `advance`, after the quadrant signs are applied.
    dy: i32,
    /// The absolute x of the ray's direction vector, in hundredths of a square.
    abs_x: i32,
    /// The absolute y of the ray's direction vector, in hundredths of a square.
    abs_y: i32,
    /// The normalized direction, in radians, because that is what the reference stores and divides.
    direction: f64,
    /// The Bresenham remainder, carried between steps of one `advance`.
    leftover: i32,
}

impl TileRay {
    /// A ray for a facing in degrees, with the reference's own rounding of the direction vector.
    #[allow(clippy::cast_possible_truncation)]
    fn new(degrees: f64) -> Self {
        // `units::from_degrees` is `v * M_PI / 180`, and `normalize` is `fmod` plus a turn for a negative result (`units_utility.cpp:10-18`).
        let radians = degrees * std::f64::consts::PI / 180.0;
        let mut direction = radians % (2.0 * std::f64::consts::PI);
        if direction < 0.0 {
            direction += 2.0 * std::f64::consts::PI;
        }
        // `rl_vec2d::as_point` rounds each component to the nearest whole number (`line.cpp:848-854`).
        let x = (direction.cos() * 100.0).round() as i32;
        let y = (direction.sin() * 100.0).round() as i32;
        Self {
            dx: 0,
            dy: 0,
            abs_x: x.abs(),
            abs_y: y.abs(),
            direction,
            leftover: 0,
        }
    }

    /// Whether the ray runs mostly south-north, which decides which axis steps and which one is the sideways offset.
    fn mostly_vertical(&self) -> bool {
        self.abs_x <= self.abs_y
    }

    /// The quadrant of the facing: 0 for east, 1 south, 2 west, 3 north.
    #[allow(clippy::cast_possible_truncation)]
    fn quadrant(&self) -> usize {
        let quarter = (self.direction / std::f64::consts::FRAC_PI_2).floor() as i64;
        usize::try_from(quarter.rem_euclid(4)).unwrap_or(0)
    }

    /// The x sign of this ray's quadrant; the tables are four long and the quadrant is 0..3 by construction.
    fn sign_x(&self) -> i32 {
        SX.get(self.quadrant()).copied().unwrap_or(1)
    }

    /// The y sign of this ray's quadrant.
    fn sign_y(&self) -> i32 {
        SY.get(self.quadrant()).copied().unwrap_or(1)
    }

    /// Moves the ray `num` squares along its direction, keeping the reference's Bresenham remainder.
    fn advance(&mut self, num: i32) {
        self.dx = 0;
        self.dy = 0;
        if num == 0 {
            return;
        }
        let steps = num.abs();
        let vertical = self.mostly_vertical();
        if self.abs_x != 0 && self.abs_y != 0 {
            for _ in 0..steps {
                if vertical {
                    self.leftover += self.abs_x;
                    if self.leftover >= self.abs_y {
                        self.dx += 1;
                        self.leftover -= self.abs_y;
                    }
                } else {
                    self.leftover += self.abs_y;
                    if self.leftover >= self.abs_x {
                        self.dy += 1;
                        self.leftover -= self.abs_x;
                    }
                }
            }
        }
        if vertical {
            self.dy = steps;
        } else {
            self.dx = steps;
        }
        self.dx *= self.sign_x();
        self.dy *= self.sign_y();
        if num < 0 {
            self.dx = -self.dx;
            self.dy = -self.dy;
        }
    }

    /// The x offset `od` squares sideways, which is zero on a mostly horizontal ray.
    fn ortho_dx(&self, od: i32) -> i32 {
        if self.mostly_vertical() {
            od * -self.sign_y()
        } else {
            0
        }
    }

    /// The y offset `od` squares sideways, which is zero on a mostly vertical ray.
    fn ortho_dy(&self, od: i32) -> i32 {
        if self.mostly_vertical() {
            0
        } else {
            od * self.sign_x()
        }
    }
}

/// The square a mount lands on when its vehicle faces a direction, relative to the vehicle's origin.
///
/// Reference: `vehicle::coord_translate` (`vehicle.cpp:3598-3615`). The pivot is the mount the vehicle's position is anchored at, which `map::add_vehicle` always sets to the
/// origin (`mapgen.cpp:7543-7550`).
#[must_use]
pub fn coord_translate(degrees: f64, pivot: (i32, i32), mount: (i32, i32)) -> (i32, i32) {
    let mut ray = TileRay::new(degrees);
    ray.advance(mount.0 - pivot.0);
    (
        ray.dx + ray.ortho_dx(mount.1 - pivot.1),
        ray.dy + ray.ortho_dy(mount.1 - pivot.1),
    )
}
