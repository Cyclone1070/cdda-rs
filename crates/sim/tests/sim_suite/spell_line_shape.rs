//! The **line** shape: a band from the caster to where the spell was aimed.
//!
//! Reference: `spell_effect::spell_effect_line` (`magic_spell_effect.cpp:308-444`) with `spell_detail::line_iterable`, `side_of` and `build_line` (`:101-157`). The tests
//! below are transcribed cases rather than a dump: the port has no recording of a line's squares, so each case is worked out from the reference's own geometry and stated
//! with the arithmetic beside it.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use std::collections::BTreeSet;

/// Every square a line covers, from a source at the origin.
fn line_to(target: (i32, i32), width: i32) -> BTreeSet<(i32, i32)> {
    let open = |_: (i32, i32, i32)| true;
    cdda_sim::spell_line::line((0, 0), target, width, &open, 0)
        .into_iter()
        .map(|(x, y, _)| (x, y))
        .collect()
}

/// A set of squares, for comparing against what the reference's geometry gives.
fn set(squares: &[(i32, i32)]) -> BTreeSet<(i32, i32)> {
    squares.iter().copied().collect()
}

#[test]
fn a_line_with_no_width_is_the_straight_path_from_the_caster_to_the_aim_point() {
    // `delta = (3,0)` is axis aligned, so the two legs are empty and only the middle path is walked: the origin is erased, and the walk stops at the target because the
    // band is bounded by perpendiculars through the source and the target.
    assert_eq!(
        line_to((3, 0), 0),
        set(&[(1, 0), (2, 0), (3, 0)]),
        "**a three-square line east covers exactly those squares, and not the caster**"
    );
    assert_eq!(
        line_to((0, -2), 0),
        set(&[(0, -1), (0, -2)]),
        "and the same rule runs south"
    );
    assert!(
        line_to((0, 0), 4).is_empty(),
        "a line aimed at its own square covers nothing"
    );
}

/// **A width widens the band without lengthening it**, and an odd width leans clockwise.
#[test]
fn a_widened_line_is_a_band_and_the_extra_rank_goes_to_the_clockwise_side() {
    let widened = line_to((3, 0), 2);
    // **The band is bounded by perpendiculars through the source and the target**, so the caster's own column is inside it: the squares at `( 0, +-1 )` are covered, and
    // only the caster's own square is erased. That is not a guess - it is what `between_or_on` says, since a point exactly on the near perpendicular is "between or on".
    assert_eq!(
        widened,
        set(&[
            (0, -1),
            (0, 1),
            (1, -1),
            (1, 0),
            (1, 1),
            (2, -1),
            (2, 0),
            (2, 1),
            (3, -1),
            (3, 0),
            (3, 1),
        ]),
        "**a width of two is a three-rank band**, four long and with the caster's column counted"
    );
    // Reference: `ccw_len = aoe / 2` and `cw_len = aoe - ccw_len`, so three ranks splits two and one - and the odd one lies on the clockwise side of an eastward line.
    let uneven = line_to((3, 0), 3);
    assert!(
        uneven.contains(&(2, 2)) && !uneven.contains(&(2, -2)),
        "**an odd width leans clockwise**: (2,2) is covered and (2,-2) is not, which is `cw_len = 3 - 1`"
    );
}

/// **A wall truncates the line**, and a line only ever covers its own level.
#[test]
fn an_impassable_square_stops_the_line_where_it_stands() {
    let walled = |at: (i32, i32, i32)| at.0 != 2;
    let covered = cdda_sim::spell_line::line((0, 0), (3, 0), 0, &walled, 0);
    assert_eq!(
        covered,
        vec![(1, 0, 0)],
        "**the square before the wall is the last one covered**, and the wall itself is not"
    );
    let upstairs = cdda_sim::spell_line::line((0, 0), (3, 0), 0, &|_| true, 1);
    assert!(
        upstairs.iter().all(|(_, _, z)| *z == 1),
        "a line holds the level it was cast on"
    );
}
