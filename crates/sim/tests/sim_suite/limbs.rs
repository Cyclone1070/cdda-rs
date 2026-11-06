//! How a body's condition and load change what its parts are worth.
//!
//! Reference: `bodypart::wound_adjusted_limb_value`, `encumb_adjusted_limb_value` and `skill_adjusted_limb_value` (`src/bodypart.cpp:967-996`), which
//! `bodypart::get_limb_score` (`:998-1018`) applies in that order before summing a creature's score.

/// A hurt part is worth less in proportion, and only below three quarters of its health.
///
/// Reference: `bodypart::wound_adjusted_limb_value` (`bodypart.cpp:967-976`) - above three quarters of its health a part is worth its whole figure, and below it the figure is scaled by
/// `health / 0.75`, so a part at a **quarter** of its health is worth **a third** of what it was.
#[test]
fn a_hurt_part_is_worth_less() {
    use cdda_sim::limbs::wound_adjusted_limb_value;

    #[allow(clippy::float_cmp)]
    {
        assert_eq!(wound_adjusted_limb_value(1.0, 100, 100), 1.0, "a whole part is worth its whole figure");
        assert_eq!(wound_adjusted_limb_value(1.0, 76, 100), 1.0, "**and three quarters is still whole**");
    }
    assert!((wound_adjusted_limb_value(1.0, 75, 100) - 1.0).abs() < 1e-9, "the threshold itself is whole");
    assert!(
        (wound_adjusted_limb_value(1.0, 50, 100) - 2.0 / 3.0).abs() < 1e-9,
        "**half health leaves two thirds**"
    );
    assert!(
        (wound_adjusted_limb_value(1.0, 25, 100) - 1.0 / 3.0).abs() < 1e-9,
        "and a quarter of the health is a third of the score"
    );
    assert!((wound_adjusted_limb_value(0.7, 0, 100) - 0.0).abs() < 1e-9, "a ruined part is worth nothing");
}

/// Load costs a part a twentieth per point past its threshold, and an overloaded limb is worth nothing.
///
/// Reference: `bodypart::encumb_adjusted_limb_value` (`bodypart.cpp:978-990`): **zero** when the limb is over its limit, otherwise `val * 19 / ( 19 + enc / 3 )` past the part's threshold -
/// a curve that starts at one, is a **twentieth** down at three points, and never reaches zero, which is the reference's own comment.
#[test]
fn a_loaded_part_is_worth_less() {
    use cdda_sim::limbs::encumb_adjusted_limb_value;

    #[allow(clippy::float_cmp)]
    {
        assert_eq!(encumb_adjusted_limb_value(1.0, 0, 0, false), 1.0, "nothing in the way");
        assert_eq!(encumb_adjusted_limb_value(1.0, 3, 3, false), 1.0, "**and up to the threshold is free**");
    }
    assert!(
        (encumb_adjusted_limb_value(1.0, 6, 3, false) - 19.0 / 20.0).abs() < 1e-9,
        "**three points over the threshold is a twentieth**"
    );
    assert_eq!(
        encumb_adjusted_limb_value(1.0, 99, 3, true),
        0.0,
        "an overencumbered limb is worth nothing at all"
    );
    // The curve never reaches zero in the reference's own arithmetic, which is why the over-limit flag exists.
    assert!(encumb_adjusted_limb_value(1.0, 100_000, 3, false) > 0.0);
}

/// Skill never lowers a part's figure, and the reference's own arithmetic says why.
///
/// Reference: `bodypart::skill_adjusted_limb_value` (`bodypart.cpp:992-996`):
///
/// ```text
/// const float mitigated_score = val * ( 1.0f + ( skill / 10.0f ) );
/// return std::min( val, mitigated_score );
/// ```
///
/// **The minimum is taken against a figure that is larger than `val` for every non-negative skill**, so the answer is always `val` - the function does nothing. It reads like a penalty and is
/// inert, which is exactly the kind of thing a port must copy rather than "fix": the reference's behaviour is the behaviour, and a port that made skill help would answer differently from the
/// game it is a port of. It is recorded here so that nobody has to rediscover it.
#[test]
fn skill_never_lowers_a_limb_value() {
    use cdda_sim::limbs::skill_adjusted_limb_value;

    #[allow(clippy::float_cmp)]
    {
        assert_eq!(skill_adjusted_limb_value(1.0, 0), 1.0, "a beginner");
        assert_eq!(skill_adjusted_limb_value(1.0, 5), 1.0, "**and a master: the reference's minimum keeps the value**");
    }
    assert_eq!(skill_adjusted_limb_value(0.7, 10), 0.7, "whatever figure is asked about");
}

/// A creature's score is the sum of its parts, each weighted by how much of a limb type it is.
///
/// Reference: `Character::get_limb_score` (`character_modifier.cpp:222-266`) - for every part, the part's own figure (adjusted for wounds, load and skill) is added; when a limb type is asked
/// for, **only the parts of that type count, and only by their weight in it** (`* id.first->limbtypes.at( bp )`). The total is floored at zero (`:266`).
#[test]
fn a_creature_s_score_is_its_parts_added_up() {
    use cdda_sim::limbs::{limb_score_total, part_limb_score};

    let arm = vec![
        ("manip".to_owned(), 0.1, 0.2),
        ("lift".to_owned(), 0.5, 0.5),
    ];
    assert!((part_limb_score(&arm, "lift") - 0.5).abs() < 1e-6, "an arm lifts half");
    assert!((part_limb_score(&arm, "grip") - 0.0).abs() < 1e-6, "**a score it does not state is nothing**, not a guess");

    // Two arms at a weight of one each, and a torso that lifts nothing.
    assert!((limb_score_total(&[(0.5, 1.0), (0.5, 1.0), (0.0, 1.0)]) - 1.0).abs() < 1e-6, "two arms lift one");
    // **A part that is only half of a limb type counts half**, which is what the weight is for.
    assert!((limb_score_total(&[(0.5, 0.5)]) - 0.25).abs() < 1e-6, "half an arm counts half");
    // And the floor is zero: a wound cannot make a score negative.
    assert!((limb_score_total(&[(0.5, -3.0)]) - 0.0).abs() < 1e-6, "a negative total is nothing");
}

/// The dumped body's scores add up the way the reference's own sum would.
///
/// Reference: `Character::get_limb_score` over every part of a body (`character_modifier.cpp:222-266`), whose input is the data the dump carries (`corpus/anatomy.json`). **The test reads the
/// dump rather than a fixture**, so a part whose figures changed under the port shows up here.
#[test]
fn the_dumped_body_adds_up() {
    use cdda_sim::limbs::{limb_score_total, part_limb_score};

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/anatomy.json");
    let bytes = std::fs::read(&path).expect("the anatomy dump is in the corpus");
    let value = cdda_json::parse(&bytes).expect("the anatomy dump parses");
    let cdda_json::Value::Object(root) = &value else {
        panic!("the anatomy dump is an object");
    };
    let Some(cdda_json::Value::Array(entries)) = root.get("parts") else {
        panic!("the anatomy dump has parts");
    };
    let parts = cdda_content::body_parts::load_objects(entries).expect("the anatomy dump loads");

    // **A human body is a list of parts, not the dump's whole list**: the dump states every part type the game has, so the test names the ones a human has.
    let human = [
        "torso", "head", "eyes", "mouth", "arm_l", "arm_r", "hand_l", "hand_r", "leg_l", "leg_r", "foot_l", "foot_r",
    ];
    let figures: Vec<(f32, f32)> = human
        .iter()
        .map(|id| {
            let part = parts
                .iter()
                .find(|part| part.id == *id)
                .unwrap_or_else(|| panic!("{id} is in the dump"));
            (part_limb_score(&part.limb_scores, "balance"), 1.0)
        })
        .collect();
    let whole = limb_score_total(&figures);
    assert!(
        (whole - 1.0).abs() < 1e-6,
        "**a human body's balance is one**: the torso's 0.7 and an arm's 0.15 each - {whole}"
    );
    // The arms carry theirs and the legs carry none, which is a fact the data states rather than one the port assumes.
    let legs = limbs_balance(&parts, "leg_l");
    assert!(legs.abs() < 1e-6, "a leg states no balance: {legs}");
    let torso = parts.iter().find(|part| part.id == "torso").expect("a torso");
    assert!(
        (part_limb_score(&torso.limb_scores, "balance") - 0.7).abs() < 1e-6,
        "and the torso carries most of it"
    );
    // The arms' own figure, through the same rule.
    assert!((limbs_balance(&parts, "arm_l") - 0.15).abs() < 1e-6);
}

/// The balance one dumped part states, through the port's lookup.
fn limbs_balance(parts: &[cdda_content::body_parts::BodyPartType], id: &str) -> f32 {
    let part = parts.iter().find(|part| part.id == id).expect("the part is in the dump");
    cdda_sim::limbs::part_limb_score(&part.limb_scores, "balance")
}

/// A whole human reacts at one, and a hurt head slows the reaction down.
///
/// Reference: `Character::get_dodge` multiplies by `get_limb_score( limb_score_reaction )` (`melee.cpp:1229-1231`), and the score is the sum of the parts' `reaction` figures after the wound
/// adjustment (`bodypart.cpp:967-976, 998-1018`). **A whole human's reaction is exactly one** - a head at 0.3 and eyes at 0.7 - so the rule does not move a healthy survivor's dodge at all,
/// and a hurt head is the only thing that does.
#[test]
fn a_hurt_head_slows_the_reaction_down() {
    use cdda_sim::limbs::reaction_score;

    // ( score, base health, current health )
    let whole = [(0.3, 100, 100), (0.7, 100, 100)];
    assert!(
        (reaction_score(&whole) - 1.0).abs() < 1e-6,
        "**a whole body reacts at one**"
    );
    // A head at half its health is scaled by half over three quarters, which is two thirds of 0.3.
    let hurt = [(0.3, 100, 50), (0.7, 100, 100)];
    assert!(
        (reaction_score(&hurt) - (0.2 + 0.7)).abs() < 1e-6,
        "**a head at half health leaves nine tenths of the reaction**: {}",
        reaction_score(&hurt)
    );
    assert!(
        (reaction_score(&[(0.3, 100, 0), (0.7, 100, 0)]) - 0.0).abs() < 1e-6,
        "and a ruined body reacts not at all"
    );
}

/// A hurt leg or foot multiplies the dodge down, and a whole body multiplies it by one.
///
/// Reference: `Character::get_dodge` multiplies by `get_modifier( character_modifier_limb_dodge_mod )` (`melee.cpp:1233-1235`), whose data is
/// `{ "limb_score": [["balance", 1.5], "move_speed", "footing"], "limb_score_op": "x", "min": 0.0 }` (`data/json/character_modifiers.json`) - so the three scores are **multiplied**, the
/// balance is raised to **1.5**, and the product cannot go below zero.
///
/// **A whole human multiplies by exactly one**: balance 1.0, move speed 1.0 and footing 1.0 - the torso's 0.7 and an arm's 0.15 each, a leg's 0.5 each and a foot's 0.5 each - so the rule
/// only moves a body that has lost something.
#[test]
fn a_hurt_leg_multiplies_the_dodge_down() {
    use cdda_sim::limbs::limb_dodge_modifier;

    assert!((limb_dodge_modifier(1.0, 1.0, 1.0) - 1.0).abs() < 1e-6, "**a whole body is one**");
    assert!(
        (limb_dodge_modifier(1.0, 0.5, 1.0) - 0.5).abs() < 1e-6,
        "**one leg out of two halves the dodge**"
    );
    assert!(
        (limb_dodge_modifier(1.0, 1.0, 0.5) - 0.5).abs() < 1e-6,
        "and so does one foot"
    );
    assert!(
        (limb_dodge_modifier(0.5, 1.0, 1.0) - 0.5f64.powf(1.5)).abs() < 1e-6,
        "**and the balance is raised to three halves**, so losing half of it costs more than half"
    );
    assert!((limb_dodge_modifier(1.0, 0.0, 1.0) - 0.0).abs() < 1e-6, "no legs, no dodge");
    assert!(
        (limb_dodge_modifier(-1.0, 1.0, 1.0) - 0.0).abs() < 1e-6,
        "and the product is floored at zero, which is what the data's min says"
    );
}
