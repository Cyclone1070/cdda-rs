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
        seen += 1;
    }
    assert_eq!(seen, 2, "both recorded loads were replayed");
}
