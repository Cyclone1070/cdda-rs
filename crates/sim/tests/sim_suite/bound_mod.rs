//! How far a per-turn modifier may move a value: the last piece of `process_one_effect`'s arithmetic.
//!
//! `corpus/bound-mod.json` is `--dump-bound-mod`: 875 cases over five starting values, seven movements and five ceilings and floors each, with the reference's answer.
//! It is `bound_mod_to_vals` (`src/cata_utility.cpp:199-208`), the rule every "the effect moves your hunger by three, but not past its stated ceiling" goes through.
//!
//! Two of its rules read backwards from a clamp written from instinct: a ceiling or a floor of **zero means no limit at all**, not "clamp to zero"; and the answer is
//! the **movement** rather than the new value, so the caller adds it back.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use cdda_json::Value;

fn integer(object: &cdda_json::Object, key: &str) -> i32 {
    match object.get(key) {
        Some(Value::Int(value)) => i32::try_from(*value).expect("an int"),
        other => panic!("{key}: {other:?}"),
    }
}

#[test]
fn every_bounding_matches_the_reference() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/bound-mod.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(cases)) = root.get("cases") else {
        panic!("cases")
    };
    assert_eq!(cases.len(), 875, "every recorded case");

    let mut differing: Vec<String> = Vec::new();
    let mut clamped_high = 0;
    let mut clamped_low = 0;
    let mut untouched = 0;
    let mut zero_limit = 0;
    for case in cases {
        let Value::Object(case) = case else { panic!("a case") };
        let value = integer(case, "val");
        let modulation = integer(case, "mod");
        let maximum = integer(case, "max");
        let minimum = integer(case, "min");
        let bounded = integer(case, "bounded");
        if maximum == 0 || minimum == 0 {
            zero_limit += 1;
        }
        let ours = cdda_sim::effects::bound_mod_to_vals(value, modulation, maximum, minimum);
        if ours != bounded {
            differing.push(format!(
                "value {value}, movement {modulation}, ceiling {maximum}, floor {minimum}: the port says {ours}, the reference says {bounded}"
            ));
            continue;
        }
        match ours.cmp(&modulation) {
            std::cmp::Ordering::Equal => untouched += 1,
            std::cmp::Ordering::Greater => clamped_low += 1,
            std::cmp::Ordering::Less => clamped_high += 1,
        }
    }
    // **Non-vacuity**: the grid has to reach all three outcomes and the zero-limit cases, or it is only testing the identity.
    assert!(untouched > 200, "movements left alone: {untouched}");
    assert!(clamped_high > 50, "movements cut by the ceiling: {clamped_high}");
    assert!(clamped_low > 50, "movements raised by the floor: {clamped_low}");
    assert!(zero_limit > 200, "cases with a zero limit: {zero_limit}");
    assert!(
        differing.is_empty(),
        "{} disagreements over 875 cases; the first twelve are {:#?}",
        differing.len(),
        differing.get(..12).unwrap_or(&differing)
    );
}
