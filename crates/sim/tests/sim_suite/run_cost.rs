//! What a step costs, layer by layer: the limb scores turned into multipliers, and the blend the step is charged by.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_json::Value;

/// **The limb multipliers and their blend, replayed against the reference.** `corpus/run_cost.json` is `--dump-run-cost`: a survivor's `move_speed` and `footing` scores, the
/// multiplier each becomes, the blend of the two, and - one layer further - the charge itself.
///
/// The charge is **not** compared: the blend reproduces the multipliers exactly and then falls short of the charge by a factor of about 0.85 that the port has not found. Comparing
/// only what is understood is the point; `run_cost.rs` names the rest.
#[test]
fn a_limb_score_becomes_a_multiplier_and_the_multipliers_blend() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/run_cost.json");
    let bytes = std::fs::read(path).expect("the run-cost recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the recording is an object");
    };
    let Some(Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let number = |case: &Value, field: &str| -> f64 {
        let Value::Object(row) = case else {
            panic!("a case is an object");
        };
        match row.get(field) {
            Some(Value::Float(value)) => *value,
            Some(Value::Int(value)) => f64::from(i32::try_from(*value).expect("an int")),
            other => panic!("{field}: {other:?}"),
        }
    };

    let mut seen = 0;
    let mut charged = 0;
    for case in cases {
        let name = {
            let Value::Object(row) = case else {
                panic!("a case is an object");
            };
            match row.get("case") {
                Some(Value::Str(text)) => text.clone(),
                _ => panic!("a name"),
            }
        };
        let move_speed = number(case, "move_speed");
        let footing = number(case, "footing");
        let recorded_speed_mod = number(case, "move_speed_mod");
        let recorded_footing_mod = number(case, "footing_mod");
        let recorded_blend = number(case, "run_cost_mod");

        // A score is single precision in the reference too, so the narrowing is the same one the game makes, and `f32::from` cannot hide a truncation.
        let as_score = |value: f64| -> f32 {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a limb score is single precision in the reference too"
            )]
            let narrowed = value as f32;
            narrowed
        };
        let ours_speed = cdda_sim::run_cost::limb_cost_modifier(as_score(move_speed));
        let ours_footing = cdda_sim::run_cost::limb_cost_modifier(as_score(footing));
        let close = |a: f32, b: f64| (f64::from(a) - b).abs() < 1e-4;
        assert!(
            close(ours_speed, recorded_speed_mod),
            "{name}: **a score becomes its reciprocal** - {ours_speed} against {recorded_speed_mod}"
        );
        assert!(
            close(ours_footing, recorded_footing_mod),
            "{name}: footing's multiplier - {ours_footing} against {recorded_footing_mod}"
        );
        let blend = cdda_sim::run_cost::limb_run_cost_modifier(ours_footing, ours_speed);
        assert!(
            close(blend, recorded_blend),
            "{name}: **footing once and speed twice, over three** - {blend} against {recorded_blend}"
        );
        // **And the charge itself**, from the two layers the reference's own chain names: the limb blend and the flat-ground multiplier.
        let mut limb_modifier = 1.0_f32;
        let mut flat_ground = 1.0_f32;
        let chain = match case {
            Value::Object(row) => row.get("effects"),
            _ => None,
        };
        if let Some(Value::Array(effects)) = chain {
            for effect in effects {
                let Value::Object(entry) = effect else {
                    continue;
                };
                let what = match entry.get("what") {
                    Some(Value::Str(text)) => text.as_str(),
                    _ => "",
                };
                let times = match entry.get("times") {
                    Some(Value::Float(value)) => {
                        #[expect(
                            clippy::cast_possible_truncation,
                            reason = "a multiplier is single precision in the reference too"
                        )]
                        let narrowed = *value as f32;
                        narrowed
                    }
                    _ => 1.0,
                };
                if what.contains("Encum") {
                    limb_modifier = times;
                } else if what.contains("Flat Ground") {
                    flat_ground = times;
                }
            }
        }
        let ours = cdda_sim::run_cost::run_cost(100.0, limb_modifier, flat_ground);
        let recorded_charge = match case {
            Value::Object(row) => match row.get("run_cost_100") {
                Some(Value::Int(value)) => i32::try_from(*value).expect("a charge"),
                _ => panic!("{name}: a charge"),
            },
            _ => panic!("a case"),
        };
        assert_eq!(
            ours, recorded_charge,
            "{name}: **the charge**, from the limb blend {limb_modifier} and flat ground {flat_ground}"
        );
        charged += 1;

        seen += 1;
    }
    // **`FLEET`'s enchantment is `multiply: -0.15`**, which the cache answers as 1 + that, and it is the only contributor on a bare survivor
    // (`data/json/mutations/mutations.json:331`).
    assert!(
        (cdda_sim::run_cost::flat_ground_multiplier(-0.15) - 0.85).abs() < 1e-6,
        "**a -0.15 enchantment is a 0.85 multiplier**"
    );
    assert_eq!(seen, 2, "both recorded loads were replayed");
    assert_eq!(charged, 2, "and both charges were computed");
}
