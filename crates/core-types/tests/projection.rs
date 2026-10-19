//! Projection between coordinate scales.
//!
//! Reference: `src/coordinates.h:32` (scale factors), `:601-644` (projection),
//! `src/point.h:146` (rounding). Scaling *down* floors toward negative infinity
//! — it does not truncate toward zero — and the vertical coordinate is never
//! scaled.

use cdda_core_types::coords::{MapSquare, OvermapTerrain, Point, Relative};

#[test]
fn scaling_down_floors_toward_negative_infinity() {
    // 23 map squares is still the first terrain tile; 24 starts the next one.
    assert_eq!(
        Point::<Relative, MapSquare>::new(23, 23, 0)
            .project_to::<OvermapTerrain>()
            .xy(),
        (0, 0)
    );
    assert_eq!(
        Point::<Relative, MapSquare>::new(24, 24, 0)
            .project_to::<OvermapTerrain>()
            .xy(),
        (1, 1)
    );
    // The distinguishing case: truncation would give (0, 0) here.
    assert_eq!(
        Point::<Relative, MapSquare>::new(-1, -1, 0)
            .project_to::<OvermapTerrain>()
            .xy(),
        (-1, -1)
    );
    assert_eq!(
        Point::<Relative, MapSquare>::new(-24, -25, 0)
            .project_to::<OvermapTerrain>()
            .xy(),
        (-1, -2)
    );
}

#[test]
fn projection_keeps_the_vertical_coordinate() {
    let up = Point::<Relative, OvermapTerrain>::new(2, 3, -3).project_to::<MapSquare>();
    assert_eq!(up.xy(), (48, 72));
    assert_eq!(up.z(), -3);

    let down = Point::<Relative, MapSquare>::new(2, 3, 7).project_to::<OvermapTerrain>();
    assert_eq!(down.xy(), (0, 0));
    assert_eq!(down.z(), 7);
}

#[test]
fn remainder_is_measured_from_the_quotient_cell() {
    // Reference: `project_remain` (`coordinates.h:796-810`) returns the quotient
    // at the coarse scale and the offset inside that cell at the fine scale.
    let p = Point::<Relative, MapSquare>::new(-1, -25, 3);
    let split = p.project_remain::<OvermapTerrain>();

    assert_eq!(split.quotient.xy(), (-1, -2));
    assert_eq!(split.remainder.xy(), (23, 23));
    assert_eq!(split.quotient.z(), 3);

    // The remainder is never negative, and recombining restores the source.
    let (qx, qy) = split.quotient.xy();
    let (rx, ry) = split.remainder.xy();
    let back = Point::<Relative, MapSquare>::new(qx * 24 + rx, qy * 24 + ry, 3);
    assert_eq!(back.xy(), p.xy());
}
