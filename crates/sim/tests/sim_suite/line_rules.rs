//! Which squares a straight line covers.
//!
//! Reference: `line_to` (`line.cpp:224-239`) over `bresenham` (`:26-71`). The port needs it for a cone's rays and a dash's walk, and both depend on the same two facts:
//! **the origin is excluded** and the destination is the last square emitted, and the shorter axis only moves when the accumulated error says so.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing, clippy::float_cmp)]

#[test]
fn a_line_walks_from_the_origin_to_the_destination_and_does_not_include_the_origin() {
    assert_eq!(
        cdda_sim::line::between((0, 0), (3, 0)),
        vec![(1, 0), (2, 0), (3, 0)],
        "**the origin is not in the line**, which is what lets a cone walk a ray without hitting its caster"
    );
    assert_eq!(
        cdda_sim::line::between((0, 0), (2, 2)),
        vec![(1, 1), (2, 2)],
        "a perfect diagonal steps both axes together"
    );
    assert_eq!(
        cdda_sim::line::between((0, 0), (4, 2)),
        vec![(1, 0), (2, 1), (3, 1), (4, 2)],
        "**and a slope of one half alternates**, which is `bresenham`'s error term doing the work: `t` starts at zero, so the first step is along the longer axis alone"
    );
    assert_eq!(
        cdda_sim::line::between((5, 5), (5, 5)),
        vec![(5, 5)],
        "a line to nowhere is the square it starts on, which is `line_to`'s own short circuit"
    );
}
