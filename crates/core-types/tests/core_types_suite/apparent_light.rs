//! What the player can make of one square, given the light map and their own eyes.
//!
//! Reference: `map::apparent_light_at` (`src/lightmap.cpp:801-857`). The quadrant arithmetic that decides *how much* light reaches a square lives in
//! `apparent_light_helper`; this is the decision tree that turns its answers into a `lit_level`, and every input below is something that helper or the light cache
//! hands over.
//!
//! The order of the branches **is** the behaviour. Clairvoyance beats everything, including an opaque wall. A square beyond the player's unimpaired range is blank
//! unless it is itself a light source. And being obstructed does not make a square blank by itself - enough light getting through makes it `BRIGHT_ONLY`, provided it
//! is brighter than the ambient light outside.

#![allow(clippy::expect_used)]

use cdda_core_types::light::{Apparent, Eyes, LitLevel, apparent_light};

fn eyes() -> Eyes {
    Eyes {
        dist: 5,
        clairvoyance: 0,
        unimpaired_range: 60,
        vision_threshold: 3.5,
    }
}

fn seen(light: f32) -> Apparent {
    Apparent {
        obstructed: false,
        absolute_obstructed: false,
        light,
        light_source: 0.0,
        camera: 0.0,
        ambient: 5.0,
    }
}

#[test]
fn clairvoyance_sees_through_anything() {
    let mut eyes = eyes();
    eyes.clairvoyance = 10;
    eyes.dist = 10;
    let mut square = seen(0.0);
    square.obstructed = true;
    assert_eq!(apparent_light(&eyes, &square), LitLevel::Bright);

    eyes.dist = 11;
    assert_eq!(
        apparent_light(&eyes, &square),
        LitLevel::Blank,
        "one square past the range and it is gone"
    );
}

#[test]
fn beyond_the_unimpaired_range_only_a_light_source_itself_shows() {
    let mut eyes = eyes();
    eyes.unimpaired_range = 3;
    eyes.dist = 4;

    let mut far = seen(0.0);
    assert_eq!(apparent_light(&eyes, &far), LitLevel::Blank);

    far.light_source = 5.0;
    assert_eq!(
        apparent_light(&eyes, &far),
        LitLevel::BrightOnly,
        "a light seen at a distance, not a square to look at"
    );

    far.absolute_obstructed = true;
    assert_eq!(
        apparent_light(&eyes, &far),
        LitLevel::Blank,
        "and nothing at all if the light cannot get through"
    );
}

#[test]
fn an_obstructed_square_is_bright_only_when_the_light_beats_the_sky() {
    let mut square = seen(20.0);
    square.obstructed = true;
    square.ambient = 5.0;
    assert_eq!(
        apparent_light(&eyes(), &square),
        LitLevel::BrightOnly,
        "too hazy to see detail, bright enough to see light"
    );

    square.ambient = 25.0;
    assert_eq!(
        apparent_light(&eyes(), &square),
        LitLevel::Blank,
        "the same light under a brighter sky tells the player nothing"
    );
}

#[test]
fn an_unobstructed_square_reads_off_the_light_in_order() {
    let mut square = seen(11.0);
    assert_eq!(
        apparent_light(&eyes(), &square),
        LitLevel::Bright,
        "brighter than a light source means a light source"
    );

    square.light = 11.0;
    square.light_source = 0.0;
    assert_eq!(apparent_light(&eyes(), &square), LitLevel::Bright);

    square.light = 9.0;
    square.light_source = 0.0;
    assert_eq!(
        apparent_light(&eyes(), &square),
        LitLevel::Low,
        "lit is 10; nine is below it and above the threshold"
    );

    square.light_source = 1.0;
    assert_eq!(
        apparent_light(&eyes(), &square),
        LitLevel::Bright,
        "a light source in the square itself wins over the light map"
    );

    square.light_source = 0.0;
    square.light = 2.0;
    assert_eq!(
        apparent_light(&eyes(), &square),
        LitLevel::Blank,
        "below the threshold is nothing at all"
    );
}

#[test]
fn the_boundaries_are_a_strict_greater_and_an_inclusive_threshold() {
    // `> LIGHT_AMBIENT_LIT` for lit and `>= vision_threshold` for low (`lightmap.cpp:851-856`): exactly ten is **not** lit, and exactly the threshold still counts.
    let mut square = seen(10.0);
    square.light_source = 0.0;
    assert_eq!(
        apparent_light(&eyes(), &square),
        LitLevel::Low,
        "exactly at the lit threshold, and lit needs more"
    );

    square.light = 3.5;
    assert_eq!(
        apparent_light(&eyes(), &square),
        LitLevel::Low,
        "exactly at the vision threshold"
    );

    square.light = 3.49;
    assert_eq!(
        apparent_light(&eyes(), &square),
        LitLevel::Blank,
        "a hundredth below it"
    );
}

#[test]
fn a_camera_light_substitutes_for_the_players_own_eyes() {
    let mut eyes = eyes();
    eyes.unimpaired_range = 3;
    eyes.dist = 4;
    let mut square = seen(4.0);
    square.camera = 0.5;
    assert_eq!(
        apparent_light(&eyes, &square),
        LitLevel::Low,
        "a camera at the square puts it back in range"
    );
}
