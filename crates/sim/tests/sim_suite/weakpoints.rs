//! What chance an attack has of hitting a monster's weak point.
//!
//! Source: `corpus/weakpoints/weakpoint-chance.json` is `--dump-weakpoint-chance`, which writes the reference's own `weakpoint::hit_chance` over a grid of skill, difficulty and coverage, for
//! both halves of its branch. A test that wrote its own bell curve would prove the arithmetic and say nothing about whether the port's `erf` is the reference's.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

/// The port's normal tail is the reference's, row for row.
///
/// Reference: `weakpoint::hit_chance` (`weakpoint.cpp:536-578`). **All 240 rows must match to a ten-thousandth of a percent**, which is what a `float` return can carry: the reference's own answer
/// is a `float` multiplied by `float` coverage, so a difference in the fifth decimal is the reference's precision rather than the port's error. The tolerance is still a hundred times tighter
/// than a `float` at a hundred, which is what makes this a gate on the *formula*: an `erf` that was a percent out, a difficulty that was not shifted by ten for the bad branch, or a floor that was
/// missed would move whole columns of the grid.
#[test]
fn the_ported_weakpoint_chance_matches_the_reference() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/weakpoints/weakpoint-chance.json");
    let text = std::fs::read_to_string(&path).expect("the weakpoint dump is in the corpus");
    let value = cdda_json::parse(text.as_bytes()).expect("the dump parses");
    let cdda_json::Value::Object(root) = value else {
        panic!("the dump's root is an object");
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("rows") else {
        panic!("the dump has rows");
    };
    let number = |row: &cdda_json::Value, key: &str| -> f64 {
        let cdda_json::Value::Object(object) = row else {
            panic!("a row is an object");
        };
        match object.get(key) {
            Some(cdda_json::Value::Float(found)) => *found,
            Some(cdda_json::Value::Int(found)) => *found as f64,
            _ => panic!("a row states {key}"),
        }
    };
    let mut checked = 0;
    for row in rows {
        let cdda_json::Value::Object(object) = row else {
            panic!("a row is an object");
        };
        let helps = matches!(object.get("good"), Some(cdda_json::Value::Bool(true)));
        let mine = cdda_sim::weakpoints::hit_chance(
            number(row, "coverage"),
            number(row, "difficulty"),
            number(row, "skill"),
            helps,
            1.0,
        );
        let reference = number(row, "chance");
        assert!(
            (mine - reference).abs() < 1e-4,
            "coverage {} difficulty {} skill {} good {helps}: port {mine}, reference {reference}",
            number(row, "coverage"),
            number(row, "difficulty"),
            number(row, "skill")
        );
        checked += 1;
    }
    assert_eq!(checked, 240, "**every row of the reference's grid was checked**");
}

/// A skilful attacker gets several draws at a weak point, and the reweighing favours the hard ones.
///
/// Reference: `select_weakpoint`'s `rolls = std::max( 1.0f, 1.0f + attack.wp_skill / 2.5f )` and `reweigh( base, rolls ) = 1 - ( 1 - base )^rolls` (`weakpoint.cpp:583-596`). **The reference's
/// own comment is the gate**: "a 1% base combined with 5 rolls has a 4.9% weight... a 75% chance of not hitting a weak point becomes a 23.7% chance with 5 rolls".
#[test]
fn a_skilful_attacker_reweighs_the_hard_weak_points_up() {
    use cdda_sim::weakpoints::{reweigh, rolls_for};

    // The reference's own two worked examples, to a thousandth.
    assert!((rolls_for(10.0) - 5.0).abs() < 1e-9, "**ten points of skill is five rolls**");
    assert!((rolls_for(0.0) - 1.0).abs() < 1e-9, "and a beginner gets one");
    assert!((rolls_for(-3.0) - 1.0).abs() < 1e-9, "and never fewer than one");
    assert!((reweigh(0.01, 5.0) - 0.049).abs() < 1e-3, "one percent with five rolls is 4.9 percent");
    // The reference's second example is written the other way round: **a seventy-five percent chance of *not* hitting becomes 23.7** - that is `1 - reweigh( 0.25, 5 )`, not `reweigh( 0.75, 5 )`.
    assert!((1.0 - reweigh(0.25, 5.0) - 0.237).abs() < 1e-3, "**and a quarter's chance of missing falls to 23.7 percent**");
    // A single roll changes nothing, which is why a beginner's distribution is the plain coverage.
    assert!((reweigh(0.42, 1.0) - 0.42).abs() < 1e-12, "one roll is no reweighing at all");
}
