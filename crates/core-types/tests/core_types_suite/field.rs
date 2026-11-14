//! Fields fading, against the reference's `field_entry` rules.
//!
//! Source: `src/field.h:20-70` for the state, `src/field.cpp:58-113` for the intensity
//! clamp and the decay.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::field::FieldEntry;
use cdda_core_types::rng::{Recording, Rolls};

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

#[test]
fn a_field_decays_on_the_reference_schedule_when_the_recording_says_so() {
    // `initialize_decay` draws its exponential from the engine itself (`src/field.cpp:86-88`), so the value is not a die in the dice trace and a replay
    // has to be handed it separately. The draw is **truncated to whole turns**, which is what makes a short-lived field die on its first decay: a draw
    // under one turn puts `decay_time` in the past and the level goes at once.
    let recording = Recording::parse("drawn decay.turns 1\n").expect("one drawn value");
    let mut rolls = Rolls::replaying(recording);
    let mut entry = FieldEntry::new(3, 10, false, false);
    entry.set_intensity(2);
    entry.set_age(1);
    // age becomes two, so `decay_time` is now - 2 + 1: one turn in the past.
    entry.do_decay(5_000, &mut rolls);
    assert_eq!(
        entry.intensity(),
        1,
        "the level went, and the field survived"
    );
    assert_eq!(entry.age(), 0, "and the age restarted with the new level");
}

#[test]
fn a_recorded_draw_of_three_turns_keeps_the_field_alive() {
    let recording = Recording::parse("drawn decay.turns 3\n").expect("one drawn value");
    let mut rolls = Rolls::replaying(recording);
    let mut entry = FieldEntry::new(3, 10, false, false);
    entry.set_intensity(2);
    entry.set_age(1);
    // decay_time is now - 2 + 3, one turn in the future: nothing happens yet.
    entry.do_decay(5_000, &mut rolls);
    assert_eq!(entry.intensity(), 2, "still standing");
    assert_eq!(entry.age(), 2, "and a turn older");
}

#[test]
fn setting_the_age_restarts_the_decay_schedule() {
    // `field_entry::set_field_age` resets `decay_time` unless told not to (`src/field.h:70-78`), and everything that touches a field's age goes through
    // it: `gas_spread_to` sets the source's and the newborn's age, and `do_decay`'s own level loss sets the age back to zero. So a field that spreads
    // draws a **fresh** decay on its next turn instead of keeping the deadline it had - and a schedule that is never reset cannot be passed while the
    // harness's calendar stands still, which is why a field the reference buried stayed alive in the port.
    let recording =
        Recording::parse("drawn decay.turns 40\ndrawn decay.turns 1\n").expect("two drawn values");
    let mut rolls = Rolls::replaying(recording);
    let mut entry = FieldEntry::new(3, 10, false, false);
    entry.set_intensity(2);
    entry.set_age(1);
    // Far in the future: nothing happens.
    entry.do_decay(5_000, &mut rolls);
    assert_eq!(
        entry.intensity(),
        2,
        "the first draw put the deadline ahead"
    );
    // Something else sets the age, so the deadline is thrown away and drawn again.
    entry.set_age(0);
    entry.do_decay(5_000, &mut rolls);
    assert_eq!(
        entry.intensity(),
        1,
        "the second draw was spent, and it landed in the past"
    );
}

#[test]
fn every_decay_draws_a_fresh_deadline() {
    // Measured, not assumed: `CDDA_DECAY_TRACE` on the reference shows `decay_time 0` at the entry of **every** `do_decay`, so `initialize_decay` runs
    // each time and the deadline it computed last turn is never reused. A field therefore dies on the turn its draw is no longer than its age - the
    // extinguisher's risen cloud draws one turn at age two and goes at once. A port that cached the deadline kept that cloud alive for the rest of the
    // session, because a frozen clock can never pass a deadline that is ahead of it.
    let recording =
        Recording::parse("drawn decay.turns 40\ndrawn decay.turns 1\n").expect("two drawn");
    let mut rolls = Rolls::replaying(recording);
    let mut entry = FieldEntry::new(3, 10, false, false);
    entry.set_intensity(2);
    entry.set_age(1);
    // The first draw is forty turns out: nothing happens.
    entry.do_decay(5_000, &mut rolls);
    assert_eq!(
        entry.intensity(),
        2,
        "the first draw put the deadline ahead"
    );
    // The second decay must spend the second draw, not reuse the first: one turn at age three is in the past.
    entry.do_decay(5_000, &mut rolls);
    assert_eq!(
        entry.intensity(),
        1,
        "the second draw was spent and it landed in the past"
    );
}
