//! The origin and scale legality matrix, and the operations it permits.
//!
//! Reference: `tests/coordinate_test.cpp` (`coordinate_strings`,
//! `coordinate_operations`, `coordinate_comparison`, `coordinate_hash`,
//! `combine_is_opposite_of_remain`, `coord_point_distances`,
//! `coord_point_midpoint`) and the rules in `src/coordinates.h`. The reference's
//! illegal combinations are compile errors in C++ templates; here they are
//! missing impls, so a program that tries one does not build.

use cdda_core_types::coords::{
    MapSquare, Overmap, OvermapTerrain, Point, Scale, Segment, Submap, combine, origin,
    project_bounds,
};

#[test]
fn a_point_carries_its_components_and_prints_them() {
    let abs = Point::<origin::Absolute, OvermapTerrain>::new(3, 4, 5);
    assert_eq!(abs.x(), 3);
    assert_eq!(abs.y(), 4);
    assert_eq!(abs.z(), 5);
    assert_eq!(abs.xy(), (3, 4));
    // The reference's two-dimensional point prints "(3,4)"; the port keeps one
    // three-dimensional point, which prints all three components.
    assert_eq!(abs.to_string(), "(3,4,5)");
}

#[test]
fn the_scale_matrix_is_the_one_the_reference_uses() {
    // Reference: `map_squares_per` (`coordinates.h:32-50`) with SEEX 12,
    // OMAPX 180 and SEG_SIZE 32.
    assert_eq!(MapSquare::MAP_SQUARES_PER, 1);
    assert_eq!(Submap::MAP_SQUARES_PER, 12);
    assert_eq!(OvermapTerrain::MAP_SQUARES_PER, 24);
    assert_eq!(Segment::MAP_SQUARES_PER, 768);
    assert_eq!(Overmap::MAP_SQUARES_PER, 4320);

    // One overmap cell, projected down, is the size of the coarser scale in
    // units of the finer one.
    let cell = Point::<origin::Absolute, Overmap>::new(1, 1, 0);
    assert_eq!(cell.project_to::<OvermapTerrain>().xy(), (180, 180));
    assert_eq!(cell.project_to::<Submap>().xy(), (360, 360));
    assert_eq!(cell.project_to::<MapSquare>().xy(), (4320, 4320));

    // Segments and overmaps are not multiples of one another - 180 terrain tiles
    // to an overmap against 32 to a segment - so projecting between them does not
    // build, and a segment lands on whole terrain tiles but never on a whole
    // overmap.
    assert_ne!(Overmap::MAP_SQUARES_PER % Segment::MAP_SQUARES_PER, 0);
    assert_ne!(Segment::MAP_SQUARES_PER % Overmap::MAP_SQUARES_PER, 0);
    let segment = Point::<origin::Absolute, Segment>::new(1, 1, 0);
    assert_eq!(segment.project_to::<OvermapTerrain>().xy(), (32, 32));
}

#[test]
fn a_remainder_is_measured_from_the_cell_it_falls_in() {
    // Absolute submap coordinates split by overmap terrain: the quotient is
    // absolute, the remainder is measured from the terrain tile itself, which is
    // the whole point of the origin-per-scale rule (`coordinates.h:795-810`).
    let sm = Point::<origin::Absolute, Submap>::new(-1, -25, 3);
    let split = sm.project_remain::<OvermapTerrain>();
    // A terrain tile is two submaps across, so -1 and -25 land in the cell at
    // (-1, -13) with an offset of one submap on each axis.
    assert_eq!(split.quotient.xy(), (-1, -13));
    assert_eq!(split.remainder.xy(), (1, 1));
    assert_eq!(split.quotient.z(), 3);

    // Recombining restores the source, and that is the only legal way to add the
    // two halves: their origins differ.
    assert_eq!(split.combine(), sm);
}

#[test]
fn combining_accepts_exactly_the_origin_the_coarse_scale_names() {
    // `abs_om` with a submap offset measured from the overmap: the fine origin
    // is the one an overmap names, so this is the combination the reference
    // allows. An overmap point with an overmap-terrain offset does not compile,
    // because `origin_from_scale( om )` is the overmap, not the terrain tile.
    let coarse = Point::<origin::Absolute, Overmap>::new(2, -1, 4);
    let fine = Point::<origin::Overmap, Submap>::new(1, 1, 0);
    let whole = combine(coarse, fine);
    // An overmap is 360 submaps across.
    assert_eq!(whole.xy(), (2 * 360 + 1, -360 + 1));
    assert_eq!(whole.z(), 4);
}

#[test]
fn split_and_combine_are_opposites_of_each_other() {
    // Reference: `combine_is_opposite_of_remain`. Every point of a sample, at
    // every legal coarse scale, survives the round trip.
    for x in [-50, -1, 0, 1, 23, 24, 4319, 4320] {
        for y in [-49, -24, -23, 0, 12, 767, 768] {
            let source = Point::<origin::Absolute, MapSquare>::new(x, y, -2);

            let by_submap = source.project_remain::<Submap>();
            assert_eq!(by_submap.combine(), source);

            let by_terrain = source.project_remain::<OvermapTerrain>();
            assert_eq!(by_terrain.combine(), source);

            let by_overmap = source.project_remain::<Overmap>();
            assert_eq!(by_overmap.combine(), source);
        }
    }
}

#[test]
fn a_segment_cannot_be_split_onto_and_does_not_divide_an_overmap() {
    // A segment names no origin, so a remainder cannot be measured from one
    // (`coordinates.h:53-65`); and 180 terrain tiles to an overmap against 32 to
    // a segment means the two are not multiples, so projecting between them is
    // refused as well (`coordinates.h:601-644`).
    let segment = Point::<origin::Absolute, Segment>::new(3, -2, 0);
    assert_eq!(segment.project_to::<MapSquare>().xy(), (3 * 768, -2 * 768));
    assert_eq!(segment.project_to::<OvermapTerrain>().xy(), (96, -64));
    assert_ne!(
        4320 % 768,
        0,
        "an overmap is not a whole number of segments"
    );
}

#[test]
fn a_cells_bounds_are_the_cell_and_the_cell_plus_one() {
    let cell = Point::<origin::Absolute, OvermapTerrain>::new(3, -1, 2);
    let (near, far) = project_bounds::<MapSquare, origin::Absolute, OvermapTerrain>(cell);
    assert_eq!(near.xy(), (72, -24));
    assert_eq!(far.xy(), (96, 0));
    assert_eq!(near.z(), 2);
    assert_eq!(far.z(), 2);
}

#[test]
fn distances_match_the_reference() {
    // Reference: `coord_point_distances` (`coordinate_test.cpp:553-574`).
    let origin_point = Point::<origin::Absolute, OvermapTerrain>::new(0, 0, 0);
    let ten = Point::<origin::Absolute, OvermapTerrain>::new(10, 10, 10);
    let flat = Point::<origin::Absolute, OvermapTerrain>::new(10, 10, 0);

    assert_eq!(origin_point.square_dist(flat), 10);
    assert_eq!(origin_point.square_dist(ten), 10);
    // Ten apart on two axes is 14.14, truncated to 14; on three it is 17.32,
    // truncated to 17.
    assert_eq!(origin_point.trig_dist(flat), 14);
    assert_eq!(origin_point.trig_dist(ten), 17);
    assert_eq!(origin_point.manhattan_dist(flat), 20);
}

#[test]
fn a_midpoint_truncates_toward_zero() {
    // Reference: `coord_point_midpoint` (`coordinate_test.cpp:575-584`).
    let a = Point::<origin::Absolute, OvermapTerrain>::new(2, 2, 2);
    let b = Point::<origin::Absolute, OvermapTerrain>::new(8, 17, 5);
    assert_eq!(a.midpoint(b).xy(), (5, 9));
    assert_eq!(a.midpoint(b).z(), 3);

    // The sign case: -1 and 0 average to -0.5, which truncates to 0 rather than
    // flooring to -1.
    let low = Point::<origin::Relative, MapSquare>::new(-1, -1, -1);
    let zero = Point::<origin::Relative, MapSquare>::new(0, 0, 0);
    assert_eq!(low.midpoint(zero).xy(), (0, 0));
}

#[test]
fn points_of_one_type_compare_and_hash_by_their_components() {
    use std::collections::HashSet;
    let a = Point::<origin::Absolute, MapSquare>::new(1, 2, 3);
    let b = Point::<origin::Absolute, MapSquare>::new(1, 2, 3);
    assert_eq!(a, b);
    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));

    // Ordering is by the components, as the reference compares the raw point.
    let c = Point::<origin::Absolute, MapSquare>::new(1, 2, 4);
    assert!(a < c);
    assert!(c > a);
}
