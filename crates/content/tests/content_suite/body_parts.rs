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

/// A part states what it contributes to each limb score, and how much of the body's limb type it is.
///
/// Reference: `body_part_type::load` (`bodypart.cpp:424-434`) reads `limb_scores` as `[ id, score, max? ]` with the max defaulting to the score, and `bodypart.cpp:331-370` reads
/// `limbtypes` as a map of limb type to weight - **defaulting to the plain `limb_type` at a weight of one** - and caches the heaviest as the part's primary type.
///
/// **The figures are the whole limb-score subsystem's input**: a torso's `balance` and an arm's `lift` are what the dodge and attack-speed chains add up, and a loader that dropped them
/// would leave those rules unable to answer for a real body. The dump is the oracle's own answer, so the test reads the corpus rather than the raw JSON.
#[test]
fn a_body_part_states_its_limb_scores() {
    let parts = loaded();
    let torso = part(&parts, "torso");
    let balance = torso
        .limb_scores
        .iter()
        .find(|(score, _, _)| score == "balance")
        .expect("a torso states a balance");
    assert!((balance.1 - 0.7).abs() < 1e-6, "**a torso's balance is 0.7**: {}", balance.1);
    assert!(
        (balance.2 - balance.1).abs() < 1e-6,
        "and its max is the score when the reference states no second figure"
    );

    let arm = part(&parts, "arm_l");
    let lift = arm.limb_scores.iter().find(|(score, _, _)| score == "lift").expect("an arm lifts");
    assert!((lift.1 - 0.5).abs() < 1e-6, "an arm's lift is half");
    let manip = arm.limb_scores.iter().find(|(score, _, _)| score == "manip").expect("an arm manipulates");
    assert!(
        (manip.1 - 0.1).abs() < 1e-6 && (manip.2 - 0.2).abs() < 1e-6,
        "**and `manip` states a max of its own**, which is why the dump carries three figures: {} {}",
        manip.1,
        manip.2
    );
    assert!(
        arm.limb_types.iter().any(|(kind, weight)| kind == "arm" && (*weight - 1.0).abs() < 1e-6),
        "an arm belongs to the arm limb type at a weight of one"
    );
    assert_eq!(arm.primary_limb_type.as_deref(), Some("arm"), "which is then its primary type");

    // Every part in the dump carries a score list and a type, which is what makes the sum answerable for a whole body.
    assert!(
        parts.iter().all(|part| !part.limb_types.is_empty()),
        "**every part belongs to some limb type**"
    );
}

/// A part states how much health it has, which is what "hurt" is measured against.
///
/// Reference: `body_part_type::base_hp` (`bodypart.h:291`), the figure `wound_adjusted_limb_value` divides a part's health by (`bodypart.cpp:969-970`). **Without it a part cannot say how
/// hurt it is**, so every limb score that a wound moves would be stuck at its whole value.
#[test]
fn a_body_part_states_its_health() {
    let parts = loaded();
    // **A stump has no health**, which the data states as zero and the reference keeps: it is what is left of a limb, not a limb, so the test says "every part a body can lose health
    // from" rather than "every part".
    let stumps: Vec<&str> = parts
        .iter()
        .filter(|part| part.base_hp == 0)
        .map(|part| part.id.as_str())
        .collect();
    assert_eq!(
        stumps,
        vec!["arm_stub_l", "arm_stub_r", "leg_stub_r", "leg_stub_l"],
        "**only the four stumps have no health**"
    );
    assert!(
        parts.iter().filter(|part| part.base_hp > 0).count() > 30,
        "and everything else states one"
    );
    assert_eq!(part(&parts, "head").base_hp, 60, "a head has sixty");
    assert_eq!(part(&parts, "torso").base_hp, 60, "and so does a torso");
}
