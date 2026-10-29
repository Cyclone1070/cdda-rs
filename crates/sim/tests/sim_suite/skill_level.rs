//! One character's standing in one skill: the level, the experience behind it, and what training does to both.
//!
//! Reference: `SkillLevel` (`src/skill.h:130-253`, `src/skill.cpp:285-480`). The rules that read wrongly from instinct:
//!
//! * experience is stored **raw** and reported **scaled**: `exercise()` divides by `100 * ( level + 1 )^2`, so the same raw number is worth less at every level;
//! * levelling **spends** the experience only when multi-level training is allowed — otherwise it lands exactly on zero on every crossing, so a character who gains four
//!   levels' worth lands on one with nothing left over;
//! * the level jumps **inside a loop** whose threshold is `10000 * ( level + 1 )^2`, so the first level costs ten thousand, the second forty, the third ninety;
//! * the knowledge level follows the skill level up and resets the knowledge experience as it goes.
//!
//! Maximum skill is ten (`game_constants.h:87`). `level()` reports the **capped** level while the arithmetic runs on the uncapped one, which is what lets rust take a
//! character back down from a level the cap was hiding.

#![allow(clippy::expect_used, clippy::panic, clippy::float_cmp)]

use cdda_sim::skills::SkillLevel;

#[test]
fn exercise_is_raw_points_reported_against_the_level() {
    // A hundred points at level zero is one exercised level, and four hundred at level one is also one: the divisor is `100 * ( level + 1 )^2`.
    // `set_exercise` in report units multiplies by the level's divisor, so one point of report at level zero is a hundred raw points - and a hundred raw points is
    // **exactly** the first level's threshold, which is why the report form starts at one rather than at a hundred.
    let mut level = SkillLevel::new();
    level.set_exercise(1, false);
    assert_eq!(level.level(), 0, "a hundred points is not a level");
    assert_eq!(level.exercise(false), 1, "but it is one exercised point");
    assert_eq!(
        level.exercised_level(),
        1,
        "a level is a hundred squared, so the report is level squared times a hundred plus the points"
    );

    level.set_exercise(1, true);
    assert_eq!(level.exercise(true), 1, "raw reads back what was stored");
    assert_eq!(
        level.exercise(false),
        0,
        "and one raw point is no report at all, because the report truncates"
    );
    level.level_to(1);
    level.set_exercise(400, true);
    assert_eq!(
        level.exercise(false),
        1,
        "and four hundred points at level one is one point of report, because the divisor is four hundred"
    );
}

#[test]
fn a_level_is_spent_by_default_and_crossed_with_multi_level_training_on() {
    // Ten thousand points is exactly a level from zero (`100 * 100 * ( 0 + 1 )^2`). The reference **spends** the excess only when told it may cross more than one level;
    // otherwise the loop sets the experience to zero on the crossing, so forty thousand points land on one level with nothing left over.
    // The spending path is `train`'s: with `allow_multilevel` off, the crossing drops the experience to zero instead of carrying the remainder.
    let mut spent = SkillLevel::new();
    spent.train(40_000, 1.0, 1.0, 1.0, false);
    assert_eq!(spent.level(), 1, "one level, because the excess is spent");
    assert_eq!(spent.exercise(true), 0, "and nothing is kept");

    let mut carried = SkillLevel::new();
    carried.train(40_000, 1.0, 1.0, 1.0, true);
    assert_eq!(carried.level(), 1, "and with the crossing allowed the level is the same");
    assert_eq!(carried.exercise(true), 30_000, "but the thirty thousand beyond the threshold are kept");
    assert_eq!(
        spent.knowledge_level(),
        1,
        "the knowledge level follows the skill level up"
    );

    let mut kept = SkillLevel::new();
    kept.set_exercise(40_000, true);
    assert_eq!(kept.level(), 1, "forty thousand crosses only the first threshold, ten thousand");
    assert_eq!(kept.exercise(true), 30_000, "and thirty thousand are left over");

    let mut crossing = SkillLevel::new();
    crossing.set_exercise(140_000, true);
    assert_eq!(
        crossing.level(),
        3,
        "ten, forty and ninety thousand are the first three thresholds, so a hundred and forty reaches level three exactly"
    );
    assert_eq!(crossing.exercise(true), 0, "with nothing left over");

    let mut nearly = SkillLevel::new();
    nearly.set_exercise(159_999, true);
    assert_eq!(nearly.level(), 3, "one point short of the fourth threshold");
    assert_eq!(nearly.exercise(true), 19_999, "and that point is the difference");
}

#[test]
fn every_recorded_experience_change_matches_the_reference() {
    // `corpus/skill-level.json` is `--dump-skill-level`: one fresh skill level per raw value, from zero to a hundred million, with the reporting divisor crossing at every
    // level. The recorded `knowledge_raw` is the same as the exercise until a level is crossed, which is the reference's own bookkeeping (`_knowledgeExperience = _exercise`).
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/skill-level.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let cdda_json::Value::Array(cases) = root.get("cases").expect("cases") else {
        panic!("cases")
    };
    assert_eq!(cases.len(), 17, "every recorded experience change");
    let mut levelled = 0;
    let mut capped_at_zero = 0;
    for case in cases {
        let cdda_json::Value::Object(case) = case else {
            panic!("a case")
        };
        let raw = match case.get("raw") {
            Some(cdda_json::Value::Int(raw)) => i32::try_from(*raw).expect("a raw value"),
            other => panic!("raw: {other:?}"),
        };
        let expected_level = match case.get("level") {
            Some(cdda_json::Value::Int(level)) => i32::try_from(*level).expect("a level"),
            other => panic!("level: {other:?}"),
        };
        let expected_raw = match case.get("exercise_raw") {
            Some(cdda_json::Value::Int(value)) => i32::try_from(*value).expect("a raw exercise"),
            other => panic!("exercise_raw: {other:?}"),
        };
        let expected_reported = match case.get("exercise") {
            Some(cdda_json::Value::Int(value)) => i32::try_from(*value).expect("an exercise"),
            other => panic!("exercise: {other:?}"),
        };
        let expected_knowledge = match case.get("knowledge_level") {
            Some(cdda_json::Value::Int(value)) => i32::try_from(*value).expect("a knowledge level"),
            other => panic!("knowledge_level: {other:?}"),
        };
        let expected_exercised = match case.get("exercised_level") {
            Some(cdda_json::Value::Int(value)) => i32::try_from(*value).expect("an exercised level"),
            other => panic!("exercised_level: {other:?}"),
        };
        let mut level = SkillLevel::new();
        level.set_exercise(raw, true);
        assert_eq!(
            level.exercise(true),
            expected_raw,
            "raw {raw}: the experience left after the level it bought was spent"
        );
        assert_eq!(level.level(), expected_level, "raw {raw}: the level");
        assert_eq!(level.exercise(false), expected_reported, "raw {raw}: the reported exercise");
        assert_eq!(level.knowledge_level(), expected_knowledge, "raw {raw}: the knowledge level");
        assert_eq!(level.exercised_level(), expected_exercised, "raw {raw}: the exercised level");
        if expected_level > 0 {
            levelled += 1;
        } else {
            capped_at_zero += 1;
        }
    }
    assert!(
        levelled > 0 && capped_at_zero > 0,
        "both a crossed level and an uncrossed one: {levelled} levelled, {capped_at_zero} not"
    );
}

#[test]
fn negative_experience_takes_a_level_and_the_report_stops_at_zero() {
    // The reference's rust path sets the experience to one point below the top of the level below and decrements the level (`src/skill.cpp:452-455`). At level zero that
    // leaves the level at **minus one** and `exercise()` divides by `100 * ( level + 1 )^2` = zero, which kills the reference with SIGFPE - measured, not guessed: the
    // first recording crashed exactly there. The port answers zero instead of crashing, and pins both halves of that behaviour here.
    let mut level = SkillLevel::new();
    level.level_to(2);
    level.set_exercise(-1, true);
    assert_eq!(level.level(), 1, "a point of negative experience drops the level by one");
    assert_eq!(
        level.exercise(true),
        39_999,
        "and parks one point below the top of the level below: 100 * 100 * 2^2 - 1"
    );

    let mut bottom = SkillLevel::new();
    bottom.set_exercise(-1, true);
    assert_eq!(bottom.level(), -1, "at level zero the level goes below zero, as the reference's does");
    assert_eq!(
        bottom.exercise(false),
        0,
        "and the report answers zero where the reference divides by zero"
    );
}

#[test]
fn every_recorded_training_matches_the_reference() {
    // The training path is where the excess is **spent**, where knowledge is capped at ninety percent of the catch-up, and where the real additions truncate into whole
    // points. Each case starts from a fresh skill level, repeats the same call as many times as the recording says, and compares six numbers.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/skill-level.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let cdda_json::Value::Array(trains) = root.get("trains").expect("trains") else {
        panic!("trains")
    };
    assert_eq!(trains.len(), 19, "every recorded training case");

    let mut differing: Vec<String> = Vec::new();
    let mut spent = 0;
    let mut kept = 0;
    for case in trains {
        let cdda_json::Value::Object(case) = case else {
            panic!("a training case")
        };
        let whole = |key: &str| match case.get(key) {
            Some(cdda_json::Value::Int(value)) => i32::try_from(*value).expect("a whole number"),
            other => panic!("{key}: {other:?}"),
        };
        #[allow(clippy::cast_precision_loss)]
        let real = |key: &str| match case.get(key) {
            Some(cdda_json::Value::Float(value)) => *value,
            Some(cdda_json::Value::Int(value)) => *value as f64,
            other => panic!("{key}: {other:?}"),
        };
        let amount = whole("amount");
        let repeats = whole("repeats");
        let multilevel = whole("multilevel") != 0;
        #[allow(clippy::cast_possible_truncation)]
        let (catchup, knowledge) = (real("catchup") as f32, real("knowledge") as f32);
        let knowledge_first = whole("knowledge_first");
        let mut level = SkillLevel::new();
        if knowledge_first > 0 {
            level.knowledge_train(knowledge_first, 0, 1.0);
        }
        for _ in 0..repeats {
            level.train(amount, catchup, knowledge, 1.0, multilevel);
        }
        let expected = (
            whole("level"),
            whole("exercise_raw"),
            whole("knowledge_level"),
            whole("knowledge_raw"),
            whole("rust"),
            whole("exercised_level"),
        );
        let actual = (
            level.level(),
            level.exercise(true),
            level.knowledge_level(),
            level.knowledge_experience(true),
            level.rust_accumulator(),
            level.exercised_level(),
        );
        if expected.1 == 0 && expected.0 > 0 {
            spent += 1;
        }
        if expected.1 > 0 {
            kept += 1;
        }
        if actual != expected {
            differing.push(format!(
                "amount {amount} x{repeats} at {catchup}/{knowledge} multilevel {multilevel}: \
                 the port says {actual:?}, the reference {expected:?}"
            ));
        }
    }
    assert!(spent > 0, "at least one case spends its excess: {spent}");
    assert!(kept > 0, "and at least one keeps a remainder: {kept}");
    assert!(differing.is_empty(), "{} differences:\n{}", differing.len(), differing.join("\n"));
}
