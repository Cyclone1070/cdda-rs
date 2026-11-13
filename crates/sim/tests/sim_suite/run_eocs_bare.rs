//! **A call by bare name is the ordinary shape**, and a call to a program nobody installed refuses where it runs.
//!
//! Reference: `f_run_eocs` (`npctalk.cpp:512-541`): every entry of `run_eocs` is a `str_or_var`, so `{"run_eocs": ["EOC_X"]}` and `{"run_eocs": "EOC_X"}` are both a call by
//! name. The census reported eleven programs blocked on this shape, so the shape is pinned here before the census is believed.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;

#[test]
fn a_call_by_bare_name_reads_and_refuses_when_the_program_is_missing() {
    let mut world = World::new();
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    for text in [
        r#"{ "run_eocs": ["EOC_FEAR_PARALYZE2"] }"#,
        r#"{ "run_eocs": "EOC_FEAR_PARALYZE2" }"#,
    ] {
        let value = cdda_json::parse(text.as_bytes()).expect("json");
        let effect = cdda_content::dialogue::read_effect(&value).expect("**a call by bare name reads**");
        assert_eq!(effect.kind(), "run_eocs", "and it is the call the data wrote");
        assert_eq!(
            cdda_sim::dialogue_effects::run_effects(&mut world, speaker, None, std::slice::from_ref(&effect)),
            cdda_sim::dialogue_effects::Applied::CannotAnswer,
            "a call to a program nobody installed is a refusal, not a silent success"
        );
    }
}
