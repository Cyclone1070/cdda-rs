//! The rule that lets sight cross a level, gated on the reference's own numbers.
//!
//! Reference: `cast_horizontal_zlight_segment` (`src/shadowcasting.cpp:160-350`), transcribed in [`cdda_core_types::zlight`]. The two facts these gates pin are the
//! two the trace measured: a sideways ray **descends**, and a floor under it **stops it** - and the intensity it carries is the port's own `sight_calc`, so the
//! gate can say "the reference's 0.891366" and mean a number this port can compute.

#![allow(
    clippy::panic,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::float_cmp
)]

use cdda_core_types::shadowcasting::sight_calc;
use cdda_core_types::zlight::{DOWNWARD_SEGMENTS, Plane, Seen, cast_horizontal_segment};

/// The transparency the fixture's open squares carry: open air, already multiplied by the weather's sight penalty.
const OPEN: f32 = 0.038_333;

/// One segment of a downward cast over a plane with no floors.
fn open_plane(origin: (i32, i32, i32), radius: i32) -> Seen {
    let mut seen = Seen::new();
    for (transform, z) in DOWNWARD_SEGMENTS {
        cast_horizontal_segment(
            &Plane {
                transparency: &|_, _, _| OPEN,
                floor: &|_, _, _| false,
            },
            transform,
            z,
            origin,
            radius,
            (32, 32),
            1.0,
            &mut seen,
        );
    }
    seen
}

#[test]
fn a_sideways_ray_descends_one_level_per_step() {
    // The reference's own picture of this segment is "Down lateral": the ray walks sideways and drops. With a radius of three and nothing to stop it, the squares one
    // level down have to be reachable, and the first of them sits one step away - not three - because `delta.z` climbs with the sweep's major axis.
    let seen = open_plane((10, 10, 0), 3);
    let below: Vec<(i32, i32, i32)> = seen
        .keys()
        .copied()
        .filter(|(_, _, z)| *z < 0)
        .collect();
    assert!(
        !below.is_empty(),
        "the cast reaches the level below through open air: {seen:?}"
    );
    // Every square it reaches below is at most as deep as its own distance, and none is deeper than the radius.
    for (x, y, z) in &below {
        let distance = (x - 10).abs().max((y - 10).abs());
        assert!(
            -z <= distance,
            "({x},{y},{z}) is deeper than its distance of {distance}"
        );
    }
    // And the first square down is the one a step sideways reaches, not something deeper.
    assert!(
        seen.contains_key(&(11, 10, -1)),
        "one step sideways and one level down is seen"
    );
}

#[test]
fn the_intensity_of_a_descending_ray_falls_with_distance() {
    // What can be said without a measurement behind it: a ray that has gone further is dimmer. The **exact** intensity depends on the span's running mean of
    // transparency, and the reference's trace gives that only for its own world (`cum 0.038333` at distance three, which `the_reference_s_traced_intensity_is_what_this_rule_produces`
    // already pins). Comparing a synthetic plane against a guessed running mean would be a gate on the port's own arithmetic agreeing with itself.
    let seen = open_plane((10, 10, 0), 4);
    let near = seen
        .get(&(11, 10, -1))
        .copied()
        .expect("one step sideways and one level down");
    let far = seen
        .get(&(14, 10, -1))
        .copied()
        .expect("four steps sideways and one level down");
    assert!(
        near > far,
        "a square further away is dimmer: {near} at one step against {far} at four"
    );
    assert!(
        (0.0..=1.0).contains(&near) && (0.0..=1.0).contains(&far),
        "intensities are fractions of full sight: {near}, {far}"
    );
}

#[test]
fn the_reference_s_traced_intensity_is_what_this_rule_produces() {
    // The measurement, turned into an assertion: `zlight HORIZ/SEG 75,72,-1 dist 3 intensity 0.891366 cum 0.038333`. A rule that produced a different number would
    // be a different rule, whatever the shape of the squares it reached.
    let traced = sight_calc(1.0, OPEN, 3);
    assert!(
        (traced - 0.891_366).abs() < 1e-5,
        "distance three through the open air: {traced}"
    );
    let four = sight_calc(1.0, OPEN, 4);
    assert!(
        (four - 0.857_844).abs() < 1e-5,
        "and distance four: {four}"
    );
}

#[test]
fn a_floor_is_a_ceiling_when_the_ray_comes_from_above() {
    // The rule that keeps a cellar dark: a square below the origin is blocked when the square **above** it has a floor (`shadowcasting.cpp:283-296`), because from
    // underneath you are looking at the underside of a floor. Cutting the hole out of that floor is what lets the same plane be seen.
    let mut roofed = Seen::new();
    let mut holed = Seen::new();
    for (transform, z) in DOWNWARD_SEGMENTS {
        cast_horizontal_segment(
            &Plane {
                transparency: &|_, _, _| OPEN,
                floor: &|_, _, level| level == 0,
            },
            transform,
            z,
            (10, 10, 0),
            3,
            (32, 32),
            1.0,
            &mut roofed,
        );
        cast_horizontal_segment(
            &Plane {
                transparency: &|_, _, _| OPEN,
                // A hole at one square: the rest of the level above is floored.
                floor: &|x, y, level| level == 0 && (x, y) != (11, 10),
            },
            transform,
            z,
            (10, 10, 0),
            3,
            (32, 32),
            1.0,
            &mut holed,
        );
    }
    assert!(
        roofed.keys().all(|(_, _, z)| *z >= 0),
        "a solid floor over the whole level hides everything below it: {roofed:?}"
    );
    assert!(
        holed.contains_key(&(11, 10, -1)),
        "the hole is what the ray gets through: {holed:?}"
    );
}

/// A grid of squares that are open, walled or dimmed by a deterministic pattern.
fn patterned_grid(side: usize) -> cdda_core_types::shadowcasting::TransparencyGrid {
    let mut cells = Vec::with_capacity(side * side);
    for x in 0..side {
        for y in 0..side {
            // A pattern with walls, corners and open runs, so a cast that mishandles a corner is caught rather than lost in noise.
            let cell = if (x * 7 + y * 3) % 11 == 0 {
                0.0
            } else if (x + y) % 5 == 0 {
                OPEN
            } else {
                1.0
            };
            cells.push(cell);
        }
    }
    cdda_core_types::shadowcasting::TransparencyGrid::new(side, cells)
}

#[test]
fn the_flat_sweep_and_the_recursive_cast_are_different_algorithms() {
    // **A finding, and a tripwire.** The reference builds its seen cache with `cast_zlight` (`lightmap.cpp:1079`); `castLight`, the recursive cast the port's
    // `Level::visibility` uses, is the **lightmap** cast (`castLightAll`, `lightmap.cpp:988`). They sweep differently - whole planes of spans against one octant row at a
    // time - and over the same grid they do not agree about squares just past a corner.
    //
    // This gate pins the divergence rather than asserting it away: the port's visibility is the lightmap algorithm, which is not the one the reference fills a
    // `seen_cache` with. The day the port switches, this gate fails and has to be turned into an equality test - which is exactly the reminder wanted.
    let side = 21_u32;
    let grid = patterned_grid(side as usize);
    let origin = (10, 10);
    let radius = 8;
    let recursive = cdda_core_types::shadowcasting::cast_light(&grid, origin, radius);
    let mut swept = Seen::new();
    for (transform, _) in DOWNWARD_SEGMENTS {
        cast_horizontal_segment(
            &Plane {
                transparency: &|x, y, _| grid.at(x, y),
                floor: &|_, _, _| false,
            },
            transform,
            0,
            (origin.0, origin.1, 0),
            radius,
            (i32::try_from(side).expect("a small grid"), i32::try_from(side).expect("a small grid")),
            1.0,
            &mut swept,
        );
    }
    let mut disagreements = 0;
    let span = i32::try_from(side).expect("a small grid");
    for x in 0..span {
        for y in 0..span {
            let Ok(index) = usize::try_from(y * span + x) else {
                continue;
            };
            let recursive_saw = recursive.get(index).copied().unwrap_or(false);
            if recursive_saw != swept.contains_key(&(x, y, 0)) {
                disagreements += 1;
            }
        }
    }
    // The count is pinned as a *range*: a few squares differ because the algorithms differ, and a count of zero would mean this grid stopped exercising the difference.
    assert!(
        disagreements > 0,
        "the two casts are not the same algorithm, and this grid no longer shows it"
    );
    assert!(
        disagreements < side * side / 2,
        "they agree about most squares even where they differ: {disagreements} of {}",
        side * side
    );
}
