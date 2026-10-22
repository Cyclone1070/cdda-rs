//! The two things `scent_map::update` does besides the arithmetic: it **pauses** when nothing comes near, and it diffuses over a
//! **window** around the character rather than the whole map.
//!
//! Reference: `scent_map::update` (`src/scent_map.cpp:170-258`). The pause is the first three lines of the function: the clock is
//! reset whenever the character moves, and after a thousand turns in one place the map stops updating entirely - so a world where
//! nobody walks is not the same world as one where somebody does, even if neither has moved this turn.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::scent::{SCENT_PAUSE, ScentClock, ScentWindow, scent_window};

#[test]
fn the_clock_resets_when_the_character_moves() {
    let mut clock = ScentClock::default();
    assert!(
        clock.should_update((10, 10), 0),
        "the first turn always updates"
    );
    assert!(
        clock.should_update((10, 10), 500),
        "and so does a settled one before the pause"
    );
    assert!(clock.should_update((11, 10), 501), "moving resets it");
    assert!(
        clock.should_update((11, 10), 1_400),
        "which buys another thousand turns"
    );
}

#[test]
fn the_map_stops_updating_after_a_thousand_turns_in_one_place() {
    let mut clock = ScentClock::default();
    assert!(clock.should_update((5, 5), 0));
    assert!(
        clock.should_update((5, 5), SCENT_PAUSE),
        "exactly a thousand turns still updates"
    );
    assert!(
        !clock.should_update((5, 5), SCENT_PAUSE + 1),
        "one turn later it does not"
    );
    // Standing still does not reset the clock, so the pause holds; walking out of it does.
    assert!(!clock.should_update((5, 5), SCENT_PAUSE + 2));
    assert!(clock.should_update((6, 5), SCENT_PAUSE + 3));
}

#[test]
fn the_window_is_the_radius_the_reference_uses() {
    let window = ScentWindow::around((40, 40));
    assert_eq!(window.min_x(), 0);
    assert_eq!(window.max_x(), 80);
    assert_eq!(window.min_y(), 0);
    assert_eq!(window.max_y(), 80);
    assert!(window.holds((0, 0)) && window.holds((80, 80)));
    assert!(!window.holds((-1, 40)) && !window.holds((81, 40)));
    assert_eq!(window.side(), 81, "two radii and the centre");
}

#[test]
fn a_source_diffuses_over_the_window_and_loses_most_of_itself() {
    // A single square of scent in the middle: one turn later the centre keeps the lion's share, its four orthogonal neighbours have
    // some, and the corners have least - the weights are 10 for a plain square and 2 for a reducing one, so the spread is not a
    // flat average.
    let mut window = scent_window((40, 40));
    window.set(40, 40, 10_000);
    window.diffuse();
    let centre = window.get(40, 40);
    let east = window.get(41, 40);
    let corner = window.get(41, 41);
    // Worked from the reference's own expressions: nine plain squares are in play, so `squares_used` is 90 and the centre's
    // `(10*1000 - 90*100)` leaves a tenth of itself, plus what its three columns pass back in - `100 * 10 * 10000` - over 10000.
    // The result is a fifth of the source per turn, and each orthogonal neighbour gets a twentieth.
    assert_eq!(centre, 2_000, "the centre keeps a fifth");
    assert_eq!(east, 1_000, "an orthogonal neighbour takes a twentieth");
    // The diagonal gets the **same** as the orthogonal in one turn, which surprised me and is right: `squares_used` counts every
    // square in play, and with a full field of plain squares it is ninety for both of them, so the absorption term that would tell
    // them apart is zero. The difference shows only where fewer squares take part - the edge of the window - which is the next test.
    assert_eq!(
        corner, east,
        "a diagonal takes the same as an orthogonal here: {corner} vs {east}"
    );
    assert_eq!(
        window.get(42, 40),
        0,
        "two squares away is untouched in one turn"
    );
    assert_eq!(window.get(45, 40), 0, "and so is five");
}

#[test]
fn a_blocking_square_is_emptied_and_the_square_behind_it_gains_nothing() {
    let mut window = scent_window((40, 40));
    window.set(40, 40, 10_000);
    window.block(41, 40);
    window.diffuse();
    assert_eq!(window.get(41, 40), 0, "the wall holds none");
    assert_eq!(window.get(42, 40), 0, "and passes none through");
    assert!(window.get(40, 40) > 0, "the source still has its own");
}

#[test]
fn a_square_at_the_edge_of_the_window_keeps_more_because_fewer_squares_take_part() {
    // `squares_used` says how many squares share the scent, so the edge of the window - with fewer neighbours in play - keeps
    // **more** of itself than the middle does. I guessed the opposite and the numbers said otherwise: the `(90 - squares_used) / 5`
    // absorption term does bite at the edge, and it does not bite hard enough to reverse this.
    let mut window = scent_window((40, 40));
    window.set(40, 40, 10_000);
    window.set(0, 40, 10_000);
    window.diffuse();
    let middle = window.get(40, 40);
    let edge = window.get(0, 40);
    assert_eq!(
        (middle, edge),
        (2_000, 3_000),
        "middle {middle}, edge {edge}"
    );
}

#[test]
fn shifting_the_window_moves_what_it_holds_and_empties_what_it_leaves() {
    // Reference: `scent_map::shift` (`scent_map.cpp:82-99`), called when the reality bubble moves (`game.cpp:13125`). The values
    // travel with the map, and the squares the window uncovers are empty rather than stale.
    let mut window = scent_window((40, 40));
    window.set(40, 40, 1_000);
    window.set(41, 40, 500);
    // The convention is the reference's, and it is the opposite of the obvious one: a **positive** shift makes each destination read
    // from a **higher** coordinate (`grscent[x][y] = grscent[x + shift][y + shift]`), so the contents travel toward lower ones - the
    // map moved, the scent stayed where it was in the world.
    window.shift(2, 0);
    assert_eq!(
        window.get(38, 40),
        1_000,
        "the value stayed put in the world"
    );
    assert_eq!(window.get(39, 40), 500);
    assert_eq!(
        window.get(40, 40),
        0,
        "and the squares it no longer covers read as no scent"
    );
    assert_eq!(window.get(41, 40), 0);
}

#[test]
fn shifting_back_and_forth_returns_the_window_it_started_with() {
    // The iteration order is what makes this work: the reference walks the window from the side the data is moving towards, so a
    // square is always read before it is written over. Walking the other way would smear each value across the window.
    let mut window = scent_window((40, 40));
    for x in 30..50 {
        window.set(x, 40, x * 10);
    }
    let before: Vec<i32> = (30..50).map(|x| window.get(x, 40)).collect();
    window.shift(5, 0);
    window.shift(-5, 0);
    let after: Vec<i32> = (30..50).map(|x| window.get(x, 40)).collect();
    assert_eq!(before, after);
}

#[test]
fn a_shift_further_than_the_window_empties_it() {
    // Nothing survives a move that takes the whole window off the map: the reference's `inbounds` test sends every square's value
    // to zero, and a port that clamped the shift instead would leave a band of stale scent at the edge.
    let mut window = scent_window((40, 40));
    window.set(40, 40, 1_000);
    window.shift(100, 0);
    assert_eq!(window.get(42, 40), 0);
    assert_eq!(window.get(140, 40), 0);
}
