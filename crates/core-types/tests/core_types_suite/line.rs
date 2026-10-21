//! What a line between two cells yields, pinned by the reference's own cases.
//!
//! Reference: `tests/line_test.cpp` (`line_to_boundaries`, `line_to_regression`,
//! `squares_closer_to_test`) and the canonical implementation it compares against
//! (`tests/line_test.cpp:17-88`), which is the same walk `src/line.cpp` performs.

use cdda_core_types::line::Point;
use cdda_core_types::line::line_to;

#[test]
fn a_line_always_ends_on_its_target() {
    // `tests/line_test.cpp:416-437`: every target in a square, and every start
    // offset the tie-break can take, must still land on the target. Upstream also
    // warns rather than fails when it does not; here it fails.
    let started = std::time::Instant::now();
    let mut checked = 0;
    for x in -60..60 {
        for y in -60..60 {
            let target = Point::new(x, y);
            let doubled_x = x.abs() * 2;
            let doubled_y = y.abs() * 2;
            let dominant = doubled_x.max(doubled_y);
            let minor = doubled_x.min(doubled_y);
            let ideal_start_offset = minor - (dominant / 2);
            let sign = i32::from(ideal_start_offset > 0) - i32::from(ideal_start_offset < 0);
            let max_start_offset = ideal_start_offset.abs() * 2 + 1;
            for k in -1..=max_start_offset {
                let line = line_to(Point::new(0, 0), target, k * sign);
                checked += 1;
                assert_eq!(
                    line.last().copied(),
                    Some(target),
                    "target {x},{y} with t = {} ended at {:?}",
                    k * sign,
                    line.last()
                );
            }
        }
    }
    println!(
        "line walk: {checked} target/offset pairs land on the target in {:.2}s",
        started.elapsed().as_secs_f64()
    );
}

#[test]
fn a_line_steps_to_neighbours_and_never_includes_its_origin() {
    // The shape of the walk, which the boundary sweep cannot see: each point is
    // one cell from the last, the origin is not in the list, and a straight run
    // is exactly the cells between the ends.
    let line = line_to(Point::new(0, 0), Point::new(3, 3), 0);
    assert_eq!(
        line,
        vec![Point::new(1, 1), Point::new(2, 2), Point::new(3, 3)]
    );

    let straight = line_to(Point::new(2, 5), Point::new(6, 5), 0);
    assert_eq!(
        straight,
        vec![
            Point::new(3, 5),
            Point::new(4, 5),
            Point::new(5, 5),
            Point::new(6, 5)
        ]
    );

    for (from, to) in [
        (Point::new(0, 0), Point::new(7, 3)),
        (Point::new(-4, 9), Point::new(6, -2)),
        (Point::new(5, 5), Point::new(5, -8)),
    ] {
        let line = line_to(from, to, 0);
        let mut previous = from;
        for point in &line {
            let step = (point.x - previous.x)
                .abs()
                .max((point.y - previous.y).abs());
            assert_eq!(
                step, 1,
                "{point:?} follows {previous:?} on {from:?}->{to:?}"
            );
            previous = *point;
        }
        assert_eq!(previous, to);
        assert!(!line.contains(&from), "the origin is not part of the line");
    }
}

#[test]
fn the_error_term_decides_the_first_step_on_a_tie() {
    // Reference values, printed from the canonical walk in `tests/line_test.cpp`.
    // A two-by-one line is a tie, and the starting error picks the side it leans
    // to - which is exactly what a bullet through a corner depends on.
    assert_eq!(
        line_to(Point::new(0, 0), Point::new(2, 1), 0),
        vec![Point::new(1, 0), Point::new(2, 1)]
    );
    assert_eq!(
        line_to(Point::new(0, 0), Point::new(2, 1), 1),
        vec![Point::new(1, 1), Point::new(2, 1)]
    );
    // Steeper than wide leans the other way.
    assert_eq!(
        line_to(Point::new(0, 0), Point::new(1, 2), 0),
        vec![Point::new(0, 1), Point::new(1, 2)]
    );
    assert_eq!(
        line_to(Point::new(0, 0), Point::new(4, 1), 0),
        vec![
            Point::new(1, 0),
            Point::new(2, 1),
            Point::new(3, 1),
            Point::new(4, 1)
        ]
    );
    assert_eq!(
        line_to(Point::new(0, 0), Point::new(1, 4), 0),
        vec![
            Point::new(0, 1),
            Point::new(1, 2),
            Point::new(1, 3),
            Point::new(1, 4)
        ]
    );
}

#[test]
fn a_line_to_itself_is_one_cell_not_none() {
    // The walk enters its body once whatever the difference is, so the degenerate
    // line yields the cell it never left.
    assert_eq!(
        line_to(Point::new(4, 4), Point::new(4, 4), 0),
        vec![Point::new(4, 4)]
    );
    assert_eq!(
        line_to(Point::new(5, 5), Point::new(5, 5), 3),
        vec![Point::new(5, 5)]
    );
}
