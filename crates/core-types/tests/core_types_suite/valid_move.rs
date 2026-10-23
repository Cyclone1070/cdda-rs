//! `map::valid_move` step by step, for the one case a field needs: a step between two levels (`src/map.cpp:2490-2585`).
//!
//! A gas asks permission to fall and to rise, and the answer is this rule. Its shape is not obvious from the name: the *upper* square's floor decides whether a
//! fall is possible at all, an open-air square above is a ledge that overrides that floor, and `bash` short-circuits everything after those checks. Since the
//! gas calls it with `bash = true, flying = true`, the vehicle and creature halves of the rule are out of scope here - and are recorded as such below.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::movement::{StepFacts, can_step_vertically};

fn floor() -> StepFacts {
    StepFacts { move_cost: 2, open_air: false, no_floor_water: false, goes_down: false }
}

fn open_air() -> StepFacts {
    StepFacts { move_cost: 2, open_air: true, no_floor_water: false, goes_down: false }
}

fn wall() -> StepFacts {
    StepFacts { move_cost: 0, open_air: false, no_floor_water: false, goes_down: false }
}

#[test]
fn a_floor_above_forbids_the_fall() {
    // `has_floor( up_p ) && !up_ter.GOES_DOWN && !up_is_ledge` - the ordinary case: standing on a floor, nothing goes down, so nobody falls.
    assert!(!can_step_vertically(floor(), floor()));
}

#[test]
fn a_stair_down_lets_the_fall_through() {
    let mut stair = floor();
    stair.goes_down = true;
    assert!(can_step_vertically(stair, floor()));
}

#[test]
fn open_air_above_is_a_ledge_and_lets_the_fall_through() {
    // The ledge is a workaround the reference keeps on purpose (`map.cpp:2519-2521`): mapgen does not always make a drop location, and a square with no floor is
    // one whatever is under it.
    assert!(can_step_vertically(open_air(), floor()));
}

#[test]
fn a_ledge_over_rock_still_allows_the_step() {
    // The one branch that reads backwards at first: `!up_is_ledge && down_ter.movecost == 0` refuses a solid square below **unless** the upper square is a
    // ledge. The reference says why (`map.cpp:2519-2521`): mapgen does not always leave a valid drop location, and this forces at least one z-level drop rather
    // than trapping the player on a roof with nothing underneath.
    assert!(can_step_vertically(open_air(), wall()));
}

#[test]
fn a_wall_above_forbids_the_rise() {
    assert!(!can_step_vertically(floor(), wall()));
}

#[test]
fn open_air_above_accepts_the_rise() {
    // Rising, the open air is the *destination*. The rule cannot tell up from down - it reads the upper and lower squares and nothing else - so the caller has to
    // say which is which, which is why this function takes them by direction rather than by order of travel.
    assert!(can_step_vertically(open_air(), floor()));
}

#[test]
fn a_water_surface_is_no_floor_and_so_the_fall_goes_through() {
    // `has_floor` refuses `NO_FLOOR` *and* `NO_FLOOR_WATER`, while `is_open_air` reads `NO_FLOOR` alone. Water is therefore no floor even though it is not a
    // ledge: the two flags part company here, and only the floor test is what stops a fall.
    let water = StepFacts { move_cost: 2, open_air: false, no_floor_water: true, goes_down: false };
    assert!(can_step_vertically(water, floor()));
}
