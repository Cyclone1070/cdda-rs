//! **A bare string effect is a string effect, not a program call.**
//!
//! Reference: `talk_effect_t::parse_string_effect` (`npctalk.cpp:8157-8341`): a string in an effect list is looked up in the legacy table of string effects - and
//! `"open_dialogue"` is one of them (`:8321-8325`), becoming the same rule as `{ "open_dialogue": … }` with no topic. The port read every bare string as a call to a program of
//! that name, so thirteen programs called `run_eocs(no program named open_dialogue)` for an effect the data meant directly.

#![allow(clippy::expect_used, clippy::panic)]

#[test]
fn the_string_spelling_of_open_dialogue_is_the_effect_not_a_call() {
    let value = cdda_json::parse(br#""open_dialogue""#).expect("json");
    let effect =
        cdda_content::dialogue::read_effect(&value).expect("**a bare string effect reads**");
    assert_eq!(
        effect.kind(),
        "open_dialogue",
        "the string names the effect, which is the same rule `{{ \"open_dialogue\": … }}` writes"
    );
    assert_ne!(
        effect.kind(),
        "run_eocs",
        "and it is not a call to a program called open_dialogue"
    );

    // A name that is not in the reference's string-effect table is an unknown effect, not a program call: `parse_string_effect` throws for one.
    let unknown = cdda_json::parse(br#""no_such_string_effect""#).expect("json");
    assert!(
        cdda_content::dialogue::read_effect(&unknown).is_none(),
        "**an unknown string effect refuses the program**, which is what the reference's throw does to the definition"
    );
}
