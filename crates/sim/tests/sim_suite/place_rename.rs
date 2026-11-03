//! A place can be renamed, and gets its own name back when the timer runs out.
//!
//! Reference: the `place_override` dialogue effect schedules `OVERRIDE_PLACE` with the new name inside the event (`npctalk.cpp:4803-4818`), and
//! `display::current_position_text` answers that name **while the event is pending** and the real name after it (`display.cpp:1243-1246`). The port had the effect and
//! nowhere to put the name, so it scheduled an event with an empty payload and renamed nothing.

use bevy_ecs::prelude::*;

/// A rename lasts exactly as long as the event that carries it.
#[test]
fn a_rename_lasts_until_its_event_comes_due() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::calendar::set_turn(&mut world, 1_000);

    let effect = cdda_content::dialogue::Effect::PlaceOverride {
        place: cdda_content::dialogue::StringSource::Literal("Safe House".to_owned()),
        fallback: None,
        length: cdda_content::dialogue::EffectDuration::Turns(Some(600)),
        key: None,
    };
    let applied = cdda_sim::dialogue_effects::run_effects(&mut world, survivor, None, &[effect]);
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "the effect is answered");
    assert_eq!(
        cdda_sim::timed_events::text_of(&world, "OVERRIDE_PLACE").as_deref(),
        Some("Safe House"),
        "and the place wears the new name at once"
    );

    // Reference: the event is due at `turn + length + 1_seconds`, and the plus one is a turn.
    cdda_sim::calendar::set_turn(&mut world, 1_600);
    assert_eq!(
        cdda_sim::timed_events::text_of(&world, "OVERRIDE_PLACE").as_deref(),
        Some("Safe House"),
        "one turn before it is due, the new name still stands"
    );
    cdda_sim::calendar::set_turn(&mut world, 1_601);
    assert_eq!(
        cdda_sim::timed_events::text_of(&world, "OVERRIDE_PLACE"),
        None,
        "and the moment it comes due the place is itself again"
    );
}
