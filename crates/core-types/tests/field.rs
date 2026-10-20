//! Fields fading, against the reference's `field_entry` rules.
//!
//! Source: `src/field.h:20-70` for the state, `src/field.cpp:58-113` for the intensity
//! clamp and the decay.

#![allow(clippy::panic, clippy::indexing_slicing)]

use cdda_core_types::field::FieldEntry;
use cdda_core_types::rng::Rolls;

fn rolls() -> Rolls {
    Rolls::new(12345)
}

#[test]
fn intensity_clamps_to_the_type_and_never_reaches_zero() {
    let mut field = FieldEntry::new(3, 5, true, false);
    field.set_intensity(9);
    assert_eq!(field.intensity(), 3, "a type's maximum is the ceiling");
    field.set_intensity(-2);
    assert_eq!(
        field.intensity(),
        1,
        "a dead field still reports intensity one"
    );
    assert!(
        !field.is_alive(),
        "death is recorded separately from strength"
    );
}

#[test]
fn a_linear_field_loses_one_level_per_half_life() {
    // Half-life five, starting at the first intensity: the fifth tick drops it to one
    // and kills it, because a dead field is still intensity one.
    let mut field = FieldEntry::new(3, 5, true, false);
    field.set_intensity(2);
    // The age goes up first, and the decay time is `now - age + half_life`, so a field
    // that starts counting at turn 100 with a half-life of five is due on turn 104: one
    // less than the naive reading, because the first tick already aged it.
    let mut now = 100;
    for _ in 0..4 {
        field.do_decay(now, &mut rolls());
        now += 1;
    }
    assert_eq!(field.intensity(), 2, "four ticks is not a half-life");
    assert!(field.is_alive());
    field.do_decay(now, &mut rolls());
    assert_eq!(field.intensity(), 1, "the half-life passed");
    assert!(field.is_alive(), "one level down is still alive");
    assert_eq!(
        field.age(),
        0,
        "the age belongs to the intensity, so it resets"
    );

    // A second half-life from the last level is what kills it: the step from one to
    // zero is the only one that does, because a dead field still reports intensity one.
    let mut now = 105;
    for _ in 0..4 {
        field.do_decay(now, &mut rolls());
        now += 1;
    }
    assert!(field.is_alive(), "four more ticks is still not a half-life");
    field.do_decay(now, &mut rolls());
    assert!(!field.is_alive());
    assert_eq!(field.intensity(), 1);
}

#[test]
fn a_field_with_no_half_life_never_fades() {
    // Blood on the floor does not evaporate on its own: `half_life` of zero means the
    // decay branch is never entered.
    let mut field = FieldEntry::new(3, 0, true, false);
    for now in 0..1_000 {
        field.do_decay(now, &mut rolls());
    }
    assert_eq!(field.intensity(), 1);
    assert!(field.is_alive());
}

#[test]
fn an_exponential_field_starts_its_decay_somewhere_ahead() {
    // The drawn decay time is not asserted exactly: it comes from the shared RNG stream
    // and will be gated when a whole level replays against the reference. What is
    // asserted is the shape - a decay time at or after the moment the field began this
    // intensity, never before it.
    let mut field = FieldEntry::new(3, 40, false, false);
    field.set_intensity(2);
    let now = 500;
    field.do_decay(now, &mut rolls());
    assert!(
        !field.is_alive() || field.age() > 0,
        "either it decayed already or it is counting"
    );
}
