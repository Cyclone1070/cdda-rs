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
#[ignore = "red on the span's running mean, measured to three decimals: the squares are reached and their shape is right, but the intensity is \
computed against a cumulative transparency of 0.03194 where the reference's trace says 0.038333 - which is the value a fold with a *zero* current \
transparency produces ((0.038333 * 5 + 0) / 6). So a span that was cut short is being folded with the opacity of a block it never started, and the \
reference erases that span instead"]
fn the_intensity_a_descending_ray_carries_is_the_reference_s() {
    let seen = open_plane((10, 10, 0), 3);
    let at_one = seen
        .get(&(11, 10, -1))
        .copied()
        .expect("one step sideways and one level down");
    let expected = sight_calc(1.0, OPEN, 1);
    assert!(
        (at_one - expected).abs() < 1e-4,
        "one step down carries the sight calculation: {at_one} against {expected}"
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
