//! The smoke a fire leaves.
//!
//! Reference: the smoke block at the end of `field_processor_fd_fire` (`src/map_field.cpp:1407-1425`). Two conditions decide whether any smoke is
//! made, and where it goes depends on what is overhead: **up** if the square above has no floor, otherwise **here**, thicker.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::fire::{Smoke, smoke_from_fire};
use cdda_core_types::rng::Rolls;

#[test]
fn a_suppressed_fire_makes_no_smoke_and_spends_no_dice() {
    // `SUPPRESS_SMOKE` is tested first, so the short-circuit is part of the world: a fireplace in a sealed room must not shift the dice stream
    // for everything after it.
    let mut rolls = Rolls::new(3);
    let expected = rolls.clone();
    assert_eq!(
        smoke_from_fire(&mut rolls, 3, 100, 0, true, true, true),
        None
    );
    assert_eq!(rolls, expected, "no dice were spent");
}

#[test]
fn smoke_goes_up_when_there_is_a_hole_above_it() {
    // With plenty of smoke produced and a windless sky, the first two conditions pass almost always; the smoke lands on the square above, at a
    // strength between one and the fire's own.
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..200 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        if let Some(Smoke { up, intensity }) =
            smoke_from_fire(&mut rolls, 3, 100, 0, false, true, true)
        {
            assert!(up, "seed {seed} put the smoke in the fire's own square");
            assert!((1..=3).contains(&intensity), "strength {intensity}");
            return;
        }
    }
    panic!("two hundred seeds and no smoke");
}

#[test]
fn smoke_stays_where_the_fire_is_when_it_cannot_rise() {
    // No floor above means the smoke cannot go up even though it is a higher square, so it settles on the fire itself - and thicker: the fire's
    // own intensity, not a roll between one and it.
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..200 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        if let Some(Smoke { up, intensity }) =
            smoke_from_fire(&mut rolls, 3, 100, 0, false, true, false)
        {
            assert!(!up);
            assert_eq!(intensity, 3, "the fire's own strength, not a roll");
            return;
        }
    }
    panic!("two hundred seeds and no smoke");
}

#[test]
fn a_weak_fire_makes_smoke_only_sometimes() {
    // The last condition is `rng( 3, 35 ) < intensity * 10`: at intensity one only three through nine of thirty-three values pass, so a small fire
    // smokes rarely - and the count is what a caller sees.
    let mut smoky = 0;
    for attempt in 0..400_u32 {
        // Spread, not counted up: `uniform_int` maps a small engine value to a small answer, and a small seed's first draw is small, so `0, 1, 2, ...` samples the low
        // end of the distribution instead of the chance.
        let seed = crate::spread_seed(attempt);
        let mut rolls = Rolls::new(seed);
        if smoke_from_fire(&mut rolls, 1, 100, 0, false, true, true).is_some() {
            smoky += 1;
        }
    }
    let mut raging = 0;
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..400 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        if smoke_from_fire(&mut rolls, 3, 100, 0, false, true, true).is_some() {
            raging += 1;
        }
    }
    assert!(
        smoking_rate(smoky) < smoking_rate(raging),
        "{smoky} of 400 versus {raging} of 400"
    );
    assert!(raging > 100, "a raging fire smokes often: {raging} of 400");
}

fn smoking_rate(count: usize) -> f64 {
    f64::from(u32::try_from(count).unwrap_or(u32::MAX)) / 400.0
}

#[test]
fn wind_makes_smoke_less_likely() {
    // `rng( 0, 100 - windpower ) > smoke` refuses the smoke: the smaller the range, the likelier the roll is small enough to pass, so smoke is *more* likely in wind
    // for a given smoke count - and the reference's own comment says the opposite. This test states what the arithmetic does.
    //
    // The seeds are **spread**, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small, so `0, 1, 2, ...`
    // would report that a one-in-five chance passes almost always.
    let mut still = 0;
    let mut windy = 0;
    for attempt in 0..400_u32 {
        let seed = crate::spread_seed(attempt);
        let mut rolls = Rolls::new(seed);
        if smoke_from_fire(&mut rolls, 2, 20, 0, false, false, false).is_some() {
            still += 1;
        }
        let mut rolls = Rolls::new(seed);
        if smoke_from_fire(&mut rolls, 2, 20, 90, false, false, false).is_some() {
            windy += 1;
        }
    }
    assert!(windy > still, "windy {windy}, still {still}");
    assert!(
        still > 20,
        "and a still fire still smokes sometimes: {still} of 400"
    );
}
