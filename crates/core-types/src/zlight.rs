//! Sight that crosses a level.
//!
//! Reference: `cast_horizontal_zlight_segment` (`src/shadowcasting.cpp:160-350`), one of the twelve segments `cast_zlight` runs
//! (`shadowcasting.cpp:533-666`). The reference's own comment on the eight horizontal segments reads "Down lateral" and then draws the shape: a ray that walks
//! sideways also **descends**, one level per step of the span it is sweeping. That is the whole cross-level rule, and it is why a character standing beside a hole
//! sees the floor below it dimly rather than not at all.
//!
//! The measurement that pinned the mechanism (round 537): `CDDA_ZLIGHT_TRACE` on the reference, watching the start location of the fixed world, printed
//!
//! ```text
//! zlight HORIZ/SEG 75,72,-1 dist 3 intensity 0.891366 cum 0.038333
//! zlight HORIZ/SEG 76,72,-1 dist 4 intensity 0.857844 cum 0.038333
//! ```
//!
//! Both numbers fall out of the port's own `sight_calc(1.0, 0.038333, 3)` and `(…, 4)`, so the arithmetic is the arithmetic already ported - what was missing was
//! the **plane**: the sweep runs over `(minor horizontal, z)` at a fixed distance along the major horizontal axis, not over `(minor, major)` at one level.
//!
//! This module is that plane and nothing else. It does not yet replace the per-level cast; it is the rule, gated on its own, so the wiring can be checked against
//! something that already reproduces the reference's numbers.

use crate::light::TRANSPARENCY_OPEN_AIR;
use crate::shadowcasting::{Slope, Span, accumulate_transparency, sight_calc, sight_check, split_span};

/// The smallest and largest level the reference casts over.
///
/// Reference: `OVERMAP_DEPTH` and `OVERMAP_HEIGHT` (`shadowcasting.cpp:176-177`).
pub const MIN_Z: i32 = -10;

/// See [`MIN_Z`].
pub const MAX_Z: i32 = 10;

/// What a square of light meets on its way.
///
/// Reference: the three `array_of_grids_of` caches `cast_horizontal_zlight_segment` reads - the transparency of the square it is entering, and the floor cache of
/// the level **above** a square below the origin (`shadowcasting.cpp:283-296`).
pub struct Plane<'a> {
    /// How much light gets through the square at `(x, y, z)`; zero is solid.
    pub transparency: &'a dyn Fn(i32, i32, i32) -> f32,
    /// Whether `(x, y, z)` has a floor. A square below the origin whose **ceiling** is a floor is blocked.
    pub floor: &'a dyn Fn(i32, i32, i32) -> bool,
}

/// Where the seen cache is written: one value per square, the brightest of everything that reached it.
pub type Seen = std::collections::BTreeMap<(i32, i32, i32), f32>;

/// Runs one horizontal z-segment.
///
/// Reference: `cast_horizontal_zlight_segment<xx, xy, yx, yy, z_transform>` (`shadowcasting.cpp:160-350`). The four transforms map the sweep's own coordinates
/// to the map's, exactly as `castLight`'s do; `z_transform` is what makes the ray descend (`-1`) on the way down and climb (`+1`) on the way up.
///
/// Returns how many squares it wrote, which is what a caller wants to know when it is checking that a segment ran at all.
///
#[allow(
    clippy::too_many_arguments,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    clippy::float_cmp
)]
pub fn cast_horizontal_segment(
    plane: &Plane<'_>,
    transform: [i32; 4],
    z_transform: i32,
    origin: (i32, i32, i32),
    radius: i32,
    bounds: (i32, i32),
    numerator: f32,
    seen: &mut Seen,
) -> usize {
    let [xx, xy, yx, yy] = transform;
    let mut written = 0;
    let mut new_start_minor = Slope::new(1, 1);
    let mut middle_open = TRANSPARENCY_OPEN_AIR;
    // One span covering the whole plane to begin with, cut down as opacity appears - the reference's `cata::list<span<T>> spans = { { ... } }`
    // (`shadowcasting.cpp:190-196`).
    let mut spans = vec![Span {
        start_major: Slope::new(0, 1),
        end_major: Slope::new(1, 1),
        start_minor: Slope::new(0, 1),
        end_minor: Slope::new(1, 1),
        cumulative_value: TRANSPARENCY_OPEN_AIR,
        skip_first_row: false,
        skip_first_column: false,
    }];
    for distance in 1..=radius {
        let dy = distance;
        let mut current_transparency = 0.0_f32;
        let mut index = 0;
        while index < spans.len() {
            let mut started_block = false;
            let mut removed = false;
            for dz in 0..=distance {
                // The major axis of this sweep is **z**: the reference's slopes are built from `delta.z()` against `delta.y()`
                // (`shadowcasting.cpp:216-218`), which is the line that makes a sideways ray a descending one.
                let trailing_major = Slope::new(dz * 2 - 1, dy * 2 + 1);
                let leading_major = Slope::new(dz * 2 + 1, dy * 2 - 1);
                let z = origin.2 + dz * z_transform;
                if !(MIN_Z..=MAX_Z).contains(&z) {
                    continue;
                }
                let span = spans[index];
                if span.start_major > leading_major {
                    continue;
                }
                if span.skip_first_row && span.start_major == leading_major {
                    continue;
                }
                if span.end_major < trailing_major {
                    break;
                }
                let mut started_span = false;
                for dx in 0..=distance {
                    let x = origin.0 + dx * xx + dy * xy;
                    let y = origin.1 + dx * yx + dy * yy;
                    let trailing_minor = Slope::new(dx * 2 - 1, dy * 2 + 1);
                    let leading_minor = Slope::new(dx * 2 + 1, dy * 2 - 1);
                    if x < 0 || y < 0 || x >= bounds.0 || y >= bounds.1 {
                        continue;
                    }
                    if span.start_minor > leading_minor {
                        continue;
                    }
                    if span.skip_first_column && span.start_minor == leading_minor {
                        continue;
                    }
                    if span.end_minor < trailing_minor {
                        break;
                    }
                    let mut new_transparency = (plane.transparency)(x, y, z);
                    // **A floor is a ceiling when looked at from below.** The reference tests the floor cache of the level *above* the square when the square is
                    // below the origin, and of the square itself when it is above (`shadowcasting.cpp:283-296`): from underneath a floor you see its underside,
                    // which is a solid.
                    let mut floor_block = false;
                    if z < origin.2 {
                        if (plane.floor)(x, y, z + 1) {
                            floor_block = true;
                            new_transparency = 0.0;
                        }
                    } else if z > origin.2 && (plane.floor)(x, y, z) {
                        floor_block = true;
                        new_transparency = 0.0;
                    }
                    if !started_block {
                        started_block = true;
                        current_transparency = new_transparency;
                    }
                    // **All three of the sweep's coordinates are the ray's distance.** The reference measures `rl_dist( zero, delta )` with `delta` carrying the
                    // major axis in `z` (`shadowcasting.cpp:301`), so a ray that has dropped a level is further away than its horizontal travel suggests -
                    // dropping `dz` here makes every descending square too bright, which the visibility fixture showed as 78 squares brighter than the reference.
                    let dist = rl_distance(dx, dy).max(dz);
                    let last_intensity = sight_calc(numerator, spans[index].cumulative_value, dist);
                    if !floor_block {
                        let slot = seen.entry((x, y, z)).or_insert(0.0);
                        if last_intensity > *slot {
                            *slot = last_intensity;
                            written += 1;
                        }
                    }
                    if !started_span {
                        new_start_minor = leading_minor;
                        started_span = true;
                    }
                    if new_transparency == current_transparency {
                        new_start_minor = leading_minor;
                        continue;
                    }
                    // The split may insert spans before the current one, in which case the index has to move with it. It can also leave the current span as the
                    // last one, so the index has to be checked before the next read rather than assumed to point at a span.
                    index += split_span(
                        &mut spans,
                        index,
                        &mut current_transparency,
                        new_transparency,
                        distance,
                        &mut new_start_minor,
                        trailing_major,
                        leading_major,
                        trailing_minor,
                        leading_minor,
                    );
                    if index >= spans.len() {
                        break;
                    }
                }
                if index >= spans.len() {
                    break;
                }
                if !sight_check(current_transparency) {
                    spans[index].start_major = leading_major;
                }
                _ = dz;
            }
            // The reference's span-list tail: drop the span when the sweep never started or ended opaque, otherwise fold what it crossed into its running mean
            // and move on (`shadowcasting.cpp:330-349`).
            if index >= spans.len() {
                break;
            }
            let span = spans[index];
            let keep = started_block && sight_check(current_transparency);
            if keep {
                spans[index].cumulative_value =
                    accumulate_transparency(span.cumulative_value, current_transparency, distance);
                index += 1;
            } else {
                spans.remove(index);
                removed = true;
            }
            let _ = removed;
        }
        let _ = &mut middle_open;
    }
    written
}

/// The reference's `rl_dist` for a sweep coordinate: a diagonal step counts as one.
///
/// Reference: `rl_dist( tripoint_rel_ms::zero, delta )` with the default metric (`shadowcasting.cpp:301`), which is Chebyshev distance - the squares a character
/// can reach in that many moves.
#[must_use]
pub fn rl_distance(dx: i32, dy: i32) -> i32 {
    dx.abs().max(dy.abs())
}

/// Which eight horizontal segments the reference runs for a downward cast, in its own order.
///
/// Reference: `cast_zlight`'s `vertical_direction::DOWN` branch (`shadowcasting.cpp:540-585`), each comment there drawing the ray it casts.
pub const DOWNWARD_SEGMENTS: [([i32; 4], i32); 8] = [
    ([0, 1, 1, 0], -1),
    ([1, 0, 0, 1], -1),
    ([0, -1, 1, 0], -1),
    ([-1, 0, 0, 1], -1),
    ([0, 1, -1, 0], -1),
    ([1, 0, 0, -1], -1),
    ([0, -1, -1, 0], -1),
    ([-1, 0, 0, -1], -1),
];

/// Runs one vertical z-segment: the ray that drops or climbs along a diagonal.
///
/// Reference: `cast_vertical_zlight_segment<x, y, z>` (`shadowcasting.cpp:368-553`). Its plane is `(x, y)` at a **fixed** level `origin.z + distance * z_transform`,
/// with the sweep's major axis being `y`, which is the difference from [`cast_horizontal_segment`] where the level is the major axis. The four of these close the
/// corners the eight horizontal ones cannot reach: a shaft straight below a character is seen by these, not by a segment that walks sideways.
#[allow(
    clippy::too_many_arguments,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    clippy::float_cmp
)]
pub fn cast_vertical_segment(
    plane: &Plane<'_>,
    transform: [i32; 3],
    z_transform: i32,
    origin: (i32, i32, i32),
    radius: i32,
    bounds: (i32, i32),
    numerator: f32,
    seen: &mut Seen,
) -> usize {
    let [x_transform, y_transform, _] = transform;
    let mut written = 0;
    let mut new_start_minor = Slope::new(1, 1);
    let mut spans = vec![Span {
        start_major: Slope::new(0, 1),
        end_major: Slope::new(1, 1),
        start_minor: Slope::new(0, 1),
        end_minor: Slope::new(1, 1),
        cumulative_value: TRANSPARENCY_OPEN_AIR,
        skip_first_row: false,
        skip_first_column: false,
    }];
    for distance in 1..=radius {
        let dz = distance;
        let z = origin.2 + dz * z_transform;
        if !(MIN_Z..=MAX_Z).contains(&z) {
            continue;
        }
        let mut current_transparency = 0.0_f32;
        let mut index = 0;
        while index < spans.len() {
            let mut started_block = false;
            for dy in 0..=distance {
                let trailing_major = Slope::new(dy * 2 - 1, dz * 2 + 1);
                let leading_major = Slope::new(dy * 2 + 1, dz * 2 - 1);
                let y = origin.1 + dy * y_transform;
                if y < 0 || y >= bounds.1 {
                    continue;
                }
                let span = spans[index];
                if span.start_major > leading_major {
                    continue;
                }
                if span.skip_first_row && span.start_major == leading_major {
                    continue;
                }
                if span.end_major < trailing_major {
                    break;
                }
                let mut started_span = false;
                for dx in 0..=distance {
                    let x = origin.0 + dx * x_transform;
                    let trailing_minor = Slope::new(dx * 2 - 1, dz * 2 + 1);
                    let leading_minor = Slope::new(dx * 2 + 1, dz * 2 - 1);
                    if x < 0 || x >= bounds.0 {
                        continue;
                    }
                    if span.start_minor > leading_minor {
                        continue;
                    }
                    if span.end_minor < trailing_minor {
                        break;
                    }
                    let mut new_transparency = (plane.transparency)(x, y, z);
                    let mut floor_block = false;
                    if z < origin.2 {
                        if (plane.floor)(x, y, z + 1) {
                            floor_block = true;
                            new_transparency = 0.0;
                        }
                    } else if z > origin.2 && (plane.floor)(x, y, z) {
                        floor_block = true;
                        new_transparency = 0.0;
                    }
                    if !started_block {
                        started_block = true;
                        current_transparency = new_transparency;
                    }
                    let dist = rl_distance(dx, dy).max(dz);
                    let last_intensity = sight_calc(numerator, spans[index].cumulative_value, dist);
                    if !floor_block {
                        let slot = seen.entry((x, y, z)).or_insert(0.0);
                        if last_intensity > *slot {
                            *slot = last_intensity;
                            written += 1;
                        }
                    }
                    if !started_span {
                        new_start_minor = leading_minor;
                        started_span = true;
                    }
                    if new_transparency == current_transparency {
                        new_start_minor = leading_minor;
                        continue;
                    }
                    index += split_span(
                        &mut spans,
                        index,
                        &mut current_transparency,
                        new_transparency,
                        distance,
                        &mut new_start_minor,
                        trailing_major,
                        leading_major,
                        trailing_minor,
                        leading_minor,
                    );
                    if index >= spans.len() {
                        break;
                    }
                }
                if index >= spans.len() {
                    break;
                }
                if !sight_check(current_transparency) {
                    spans[index].start_major = leading_major;
                }
            }
            if index >= spans.len() {
                break;
            }
            let span = spans[index];
            if started_block && sight_check(current_transparency) {
                spans[index].cumulative_value =
                    accumulate_transparency(span.cumulative_value, current_transparency, distance);
                index += 1;
            } else {
                spans.remove(index);
            }
        }
    }
    written
}

/// Which way the reference lets a sight cast travel.
///
/// Reference: `enum class vertical_direction` (`shadowcasting.h:32-36`) - it has **no** "one level only" value: the level the character stands on is cast as the
/// `distance`-zero rows of the down and up segments, which is why [`cast_zlight`] never needs a flat cast of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalDirection {
    /// Only upward.
    Up,
    /// Only downward.
    Down,
    /// Both.
    Both,
}

/// The four x/y diagonal transforms of the vertical segments, in the reference's order.
///
/// Reference: `cast_zlight`'s four `cast_vertical_zlight_segment<x, y, ±1>` calls in each branch (`shadowcasting.cpp:610-625` and `676-686`).
pub const VERTICAL_TRANSFORMS: [[i32; 3]; 4] = [[1, 1, -1], [1, -1, -1], [-1, 1, -1], [-1, -1, -1]];

/// Casts sight over a character's own level **and** the levels on both sides of it.
///
/// Reference: `cast_zlight` (`shadowcasting.cpp:560-687`): eight horizontal segments and four vertical ones per direction, all writing into the same cache as a
/// maximum. The origin's own level is covered by the segments' zero-distance rows, so a caller that runs this gets the flat cast for free.
///
/// Returns how many squares it improved, summed over the segments.
#[allow(clippy::too_many_arguments)]
pub fn cast_zlight(
    plane: &Plane<'_>,
    origin: (i32, i32, i32),
    radius: i32,
    bounds: (i32, i32),
    numerator: f32,
    direction: VerticalDirection,
    seen: &mut Seen,
) -> usize {
    let mut written = 0;
    let sideways = |z_transform: i32, seen: &mut Seen| {
        let mut count = 0;
        for (transform, _) in DOWNWARD_SEGMENTS {
            count += cast_horizontal_segment(
                plane,
                transform,
                z_transform,
                origin,
                radius,
                bounds,
                numerator,
                seen,
            );
        }
        count
    };
    let dropping = |z_transform: i32, seen: &mut Seen| {
        let mut count = 0;
        for transform in VERTICAL_TRANSFORMS {
            count += cast_vertical_segment(
                plane, transform, z_transform, origin, radius, bounds, numerator, seen,
            );
        }
        count
    };
    if matches!(direction, VerticalDirection::Down | VerticalDirection::Both) {
        written += sideways(-1, seen);
        written += dropping(-1, seen);
    }
    if matches!(direction, VerticalDirection::Up | VerticalDirection::Both) {
        written += sideways(1, seen);
        written += dropping(1, seen);
    }
    written
}

/// Whether a character can see past its own level at all.
///
/// Reference: `lightmap.cpp:1060-1068`. The cast starts as `BOTH`; when the level the character stands on has **no floor gaps** (`no_floor_gaps`, the summary bit of
/// the floor cache) it becomes `UP` - a sealed floor means nothing below can be seen, while the sky above still can.
#[must_use]
pub fn direction_to_cast(origin_level_has_no_floor_gaps: bool) -> VerticalDirection {
    if origin_level_has_no_floor_gaps {
        VerticalDirection::Up
    } else {
        VerticalDirection::Both
    }
}
