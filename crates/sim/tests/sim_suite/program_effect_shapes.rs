//! **A program's effect may be written three ways, and all three mean the same thing.**
//!
//! Reference: `talk_effect_t::load_effect` (`npctalk.cpp:8348-8386`): the member is a **string** (a named string effect), an **object** (one sub-effect) or an **array** of
//! either. The port read only the array, so every program that wrote `"effect": { "run_eocs": [ ... ] }` was refused whole - eleven of them, which is what the census's effect
//! ranking was pointing at when it blamed `run_eocs`.

#![allow(clippy::expect_used, clippy::panic)]

#[test]
fn a_program_reads_whether_its_effect_is_one_object_or_a_list() {
    let object = r#"{
        "type": "effect_on_condition",
        "id": "EOC_ONE_EFFECT",
        "condition": { "not": { "u_has_effect": "fearparalyze" } },
        "effect": { "run_eocs": [ "EOC_B" ] }
    }"#;
    let value = cdda_json::parse(object.as_bytes()).expect("json");
    let program =
        cdda_content::dialogue::read_program(&value).expect("**a single effect object reads**");
    assert_eq!(
        program.effects.len(),
        1,
        "and it is one effect, exactly as an array of one would be"
    );
    assert_eq!(
        program.effects.first().expect("one effect").kind(),
        "run_eocs",
        "the effect the data wrote"
    );

    let list = r#"{
        "type": "effect_on_condition",
        "id": "EOC_ONE_EFFECT",
        "condition": { "not": { "u_has_effect": "fearparalyze" } },
        "effect": [ { "run_eocs": [ "EOC_B" ] } ]
    }"#;
    let value = cdda_json::parse(list.as_bytes()).expect("json");
    let program = cdda_content::dialogue::read_program(&value).expect("the array shape reads");
    assert_eq!(
        program.effects.len(),
        1,
        "the list of one reads to the same program"
    );
}
