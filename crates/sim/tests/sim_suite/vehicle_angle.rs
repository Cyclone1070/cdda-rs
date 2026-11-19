//! **Where a target is, and how hard to steer at it.**
//!
//! Reference: `vehicle::get_angle_from_targ` (`vehicle.cpp:888-900`) and `get_turn_from_angle` (`vehicle.cpp:746-767`). Both are pure geometry, so the gate is a table of cases
//! rather than a world: the reference's own bands, with its own edges.

#![allow(clippy::float_cmp)]

use cdda_sim::vehicle_angle::{
    STEER_INCREMENT_DEGREES, angle_from_target, immediate_path_heading, normalise,
    round_to_steer_increment, steered_facing, turn_from_angle,
};

/// **The angle is signed, zero is dead ahead, and the quadrants come from the cross and dot products.** Reference: `get_angle_from_targ` - `atan2( cross, dot )` between the hull's facing
/// and the vector to the target.
#[test]
fn an_angle_is_signed_and_zero_is_dead_ahead() {
    let north = (0.0, 1.0);
    // Dead ahead.
    assert_eq!(angle_from_target(north, (0.0, 5.0)), 0.0, "straight ahead");
    // A target to the left is positive: the cross product is positive there.
    assert_eq!(
        angle_from_target(north, (-5.0, 5.0)),
        45.0,
        "**north-west of a north-facing hull is forty-five degrees left**"
    );
    // A target to the right is negative.
    assert_eq!(
        angle_from_target(north, (5.0, 5.0)),
        -45.0,
        "north-east is forty-five degrees right"
    );
    // Straight left and straight right.
    assert_eq!(angle_from_target(north, (-5.0, 0.0)), 90.0, "due left");
    assert_eq!(angle_from_target(north, (5.0, 0.0)), -90.0, "due right");
    // **Directly behind is one hundred and eighty, and which one is the sign of a zero**: the cross product here is `-0.0`, and `atan2( -0.0, -1 )` is **-180** in both Rust and the
    // reference's C++ - `units::atan2` is `std::atan2`. The turn ladder treats `180` and `-180` alike, which is why the ambiguity has never mattered.
    let behind = angle_from_target(north, (0.0, -5.0));
    assert_eq!(
        behind.abs(),
        180.0,
        "directly behind is a half turn either way"
    );
}

/// **The steering ladder, band by band, in both directions.** Reference: `get_turn_from_angle` (`vehicle.cpp:746-767`) - the edges are `> 10`, `<= 45`, `< 180`, and the reverse
/// column swaps the far bands' extremes while leaving the middle band alone.
#[test]
fn the_turn_ladder_has_the_references_own_edges() {
    // **Forward: left positive.** Ten degrees or less is nothing; just past ten starts a quarter turn; past forty-five a three-quarter; past ninety the hardest; and exactly a hundred and
    // eighty is nothing only when the hull is standing on the target.
    assert_eq!(turn_from_angle(0.0, false, false), 0, "dead ahead");
    assert_eq!(
        turn_from_angle(10.0, false, false),
        0,
        "**ten is still nothing**: `> 10`"
    );
    assert_eq!(
        turn_from_angle(11.0, false, false),
        1,
        "just past ten steers one"
    );
    assert_eq!(
        turn_from_angle(45.0, false, false),
        1,
        "**forty-five is still one**: `<= 45`"
    );
    assert_eq!(turn_from_angle(45.5, false, false), 3, "past it, three");
    assert_eq!(
        turn_from_angle(90.0, false, false),
        3,
        "ninety is still three: `<= 90`"
    );
    assert_eq!(turn_from_angle(90.5, false, false), 4, "past it, four");
    assert_eq!(
        turn_from_angle(179.5, false, false),
        4,
        "and it holds to just short of behind"
    );
    assert_eq!(
        turn_from_angle(180.0, false, false),
        0,
        "**exactly behind and off target steers nothing**"
    );
    assert_eq!(
        turn_from_angle(180.0, true, false),
        0,
        "and exactly behind while standing on the target also steers nothing"
    );

    // **The right side is the mirror until the far band.**
    assert_eq!(turn_from_angle(-10.0, false, false), 0, "-10 is nothing");
    assert_eq!(
        turn_from_angle(-11.0, false, false),
        -1,
        "just past it, one the other way"
    );
    assert_eq!(turn_from_angle(-45.0, false, false), -1, "-45 is still one");
    assert_eq!(turn_from_angle(-45.5, false, false), -3, "past it, three");
    assert_eq!(
        turn_from_angle(-90.0, false, false),
        -3,
        "-90 is still three"
    );
    assert_eq!(turn_from_angle(-90.5, false, false), -4, "past it, four");

    // **Reverse swaps the far bands and leaves the middle one**: a reversing hull steers away from the target's side.
    assert_eq!(
        turn_from_angle(20.0, false, true),
        4,
        "**twenty left in reverse is four**"
    );
    assert_eq!(
        turn_from_angle(60.0, false, true),
        3,
        "sixty is three either way"
    );
    assert_eq!(
        turn_from_angle(120.0, false, true),
        1,
        "**a hundred and twenty left in reverse is one**"
    );
    assert_eq!(
        turn_from_angle(-20.0, false, true),
        -4,
        "and twenty right is minus four"
    );
    assert_eq!(
        turn_from_angle(-60.0, false, true),
        -3,
        "sixty right is minus three either way"
    );
    assert_eq!(
        turn_from_angle(-120.0, false, true),
        -1,
        "a hundred and twenty right is minus one"
    );
}

/// **Steering happens in fifteen-degree steps.** Reference: `vehicles::steer_increment` (`vehicle.h:88`), `round_to_multiple_of` (`units_utility.h:42-47`), `vehicle::selfdrive`'s turn
/// (`vehicle_move.cpp:1331-1338`) and `vehicle::immediate_path`'s rounded heading (`vehicle.cpp:725-731`).
#[test]
fn steering_happens_in_fifteen_degree_steps() {
    assert_eq!(
        STEER_INCREMENT_DEGREES, 15.0,
        "the quantum is fifteen degrees"
    );

    // **Normalising wraps into [0, 360)** from either side.
    assert_eq!(normalise(370.0), 10.0, "past a full turn wraps");
    assert_eq!(
        normalise(-10.0),
        350.0,
        "and before zero wraps the other way"
    );
    assert_eq!(normalise(0.0), 0.0, "zero stays zero");
    assert_eq!(normalise(360.0), 0.0, "and a full turn is zero");

    // **Rounding is half away from zero**, which is `lround` and not a truncation.
    assert_eq!(
        round_to_steer_increment(7.4),
        0.0,
        "seven and a bit rounds down"
    );
    assert_eq!(
        round_to_steer_increment(7.5),
        15.0,
        "**seven and a half rounds up**"
    );
    assert_eq!(
        round_to_steer_increment(22.0),
        15.0,
        "twenty-two rounds to fifteen"
    );
    assert_eq!(
        round_to_steer_increment(23.0),
        30.0,
        "and twenty-three to thirty"
    );
    assert_eq!(
        round_to_steer_increment(-7.5),
        -15.0,
        "and it is away from zero on the other side too"
    );

    // **A towed hull steps by whole increments**, so a quarter turn is fifteen degrees and never the angle to the target.
    assert_eq!(steered_facing(0.0, 1), 15.0, "one step left");
    assert_eq!(steered_facing(0.0, -1), 345.0, "one step right wraps");
    assert_eq!(
        steered_facing(350.0, 4),
        50.0,
        "**four steps from three hundred and fifty wrap past north**"
    );
    assert_eq!(
        steered_facing(90.0, 0),
        90.0,
        "no turn leaves the facing alone"
    );

    // **The immediate-path heading is normalised, then rounded, then normalised again** - so the wrap comes before the rounding, and 359 with a rotation of 4 lands on zero.
    assert_eq!(
        immediate_path_heading(359.0, 4.0),
        0.0,
        "**three hundred and fifty-nine plus four is zero**"
    );
    assert_eq!(
        immediate_path_heading(0.0, 7.0),
        0.0,
        "seven rounds to nothing"
    );
    assert_eq!(
        immediate_path_heading(0.0, 8.0),
        15.0,
        "eight rounds to fifteen"
    );
    assert_eq!(
        immediate_path_heading(90.0, 0.0),
        90.0,
        "a heading already on the quantum stays"
    );
}
