//! The body's parts, read from the reference's own data.
//!
//! Source: `corpus/anatomy.json` is `--dump-anatomy`, which writes every `body_part_type` with the two numbers
//! `anatomy::select_body_part` weighs (`hit_size`, `hit_difficulty`) and the two flags that decide what a short
//! attacker can reach (`LIMB_UPPER`, `LIMB_LOWER`). A test that built its own parts would prove the arithmetic
//! and say nothing about the data.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use cdda_content::body_parts::BodyPartType;
use cdda_json::{Value, parse};
use std::path::Path;

fn loaded() -> Vec<BodyPartType> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/anatomy.json");
    let text = std::fs::read(path).expect("the anatomy dump exists");
    let value = parse(&text).expect("the dump is valid JSON");
    let Value::Object(root) = value else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(parts)) = root.get("parts") else {
        panic!("the dump has no parts");
    };
    cdda_content::body_parts::load_objects(parts).expect("the dump loads")
}

fn part<'a>(parts: &'a [BodyPartType], id: &str) -> &'a BodyPartType {
    parts
        .iter()
        .find(|part| part.id == id)
        .unwrap_or_else(|| panic!("the corpus has no {id}"))
}

#[test]
fn a_torso_is_the_biggest_target_and_the_head_is_harder_to_reach() {
    let parts = loaded();
    assert!(
        parts.len() > 10,
        "the dump holds {} parts, which is too few to be the corpus",
        parts.len()
    );

    let torso = part(&parts, "torso");
    assert_eq!(torso.hit_size, 36.0, "a torso is thirty-six");
    assert_eq!(torso.hit_difficulty, 1.0, "and no harder than average to aim at");
    assert!(!torso.upper && !torso.lower, "it is neither high nor low");

    let head = part(&parts, "head");
    assert_eq!(head.hit_size, 4.0, "a head is four");
    assert!(
        head.hit_difficulty > 1.0,
        "and harder than a torso to land on: {}",
        head.hit_difficulty
    );
    assert!(head.upper, "and out of reach of a short attacker");

    let eyes = part(&parts, "eyes");
    assert_eq!(eyes.hit_size, 0.5, "eyes are half a unit");
    assert!(eyes.upper, "and high");
}

#[test]
fn the_upper_and_lower_flags_come_from_the_data_not_from_the_name() {
    let parts = loaded();
    let arms = part(&parts, "arm_l");
    assert!(
        !arms.upper,
        "an arm is not marked high in this data, which is the sort of thing a name-based guess would get wrong"
    );
    let legs = part(&parts, "leg_l");
    assert!(legs.lower, "a leg is marked low");

    // The null part is the appendix: zero size, so it can never be picked by the weighted rule.
    let none = part(&parts, "bp_null");
    assert_eq!(none.hit_size, 0.0, "the null part has no size at all");
}
