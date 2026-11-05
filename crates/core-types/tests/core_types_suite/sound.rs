//! How far a noise carries, and who hears it.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::sound::{SoundEvent, heard, sound_distance};

#[test]
fn distance_is_measured_in_steps_where_a_diagonal_is_one() {
    assert_eq!(sound_distance((0, 0), (3, 0)), 3);
    assert_eq!(sound_distance((0, 0), (3, 4)), 4, "not the hypotenuse");
    assert_eq!(sound_distance((5, 5), (5, 5)), 0);
}

#[test]
fn a_noise_carries_twice_its_volume_in_squares() {
    assert!(heard(10, 19), "volume ten reaches nineteen squares");
    assert!(!heard(10, 21), "and not twenty-one");
    assert!(
        !heard(10, 20),
        "the edge is exclusive: the test is strictly greater"
    );
}

#[test]
fn a_listener_far_away_hears_nothing_at_all() {
    let shout = SoundEvent {
        source: (0, 0),
        volume: 4,
    };
    assert!(shout.heard_from((6, 0)).is_some());
    assert!(
        shout.heard_from((20, 0)).is_none(),
        "four times two is eight squares"
    );
}

#[test]
fn a_nearer_listener_hears_it_louder_than_a_further_one() {
    let shout = SoundEvent {
        source: (0, 0),
        volume: 20,
    };
    let near = shout.heard_from((2, 0)).expect("close enough");
    let far = shout.heard_from((18, 0)).expect("still close enough");
    assert!(near > far, "{near} should be louder than {far}");
    assert_eq!(near, 19);
}
