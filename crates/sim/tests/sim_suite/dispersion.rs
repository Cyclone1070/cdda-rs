//! A shot's spread is a **sum of sources**, with the total capped and the spread added outside the multipliers.
//!
//! Reference: `dispersion_sources` (`dispersion.cpp:8-42`, `dispersion.h:16-36`). Three kinds of source - linear ones that are each drawn from zero, normal ones that are
//! drawn around zero, and multipliers that scale everything before them - and the data's `spread` is added **after** the multipliers in the total but **not at all** in a
//! roll. That asymmetry is the rule, and it is what this test pins.

use cdda_core_types::rng::Rolls;

/// The three sums, and what they come to.
fn sources() -> cdda_sim::dispersion::Dispersion {
    cdda_sim::dispersion::Dispersion {
        linear: vec![2.0, 3.0],
        normal: vec![1.0],
        multipliers: vec![2.0],
        spread: 500.0,
    }
}

#[test]
fn the_total_is_the_sources_then_the_multipliers_then_the_spread() {
    let shot = sources();
    // (2 + 3 + 1) * 2, then the spread is added: 12 + 500.
    assert!(
        (shot.max() - 512.0).abs() < f64::EPSILON,
        "the most a shot can spread: {}",
        shot.max()
    );
    assert!(
        (shot.avg() - 256.0).abs() < f64::EPSILON,
        "and its average is half of that: {}",
        shot.avg()
    );
}

#[test]
fn a_roll_leaves_the_spread_out_and_stops_at_a_full_circle() {
    // One enormous linear source: every draw is a real number below it, so the sum is over the cap and the cap is what comes back.
    let huge = cdda_sim::dispersion::Dispersion {
        linear: vec![10_000.0],
        normal: Vec::new(),
        multipliers: vec![1.0],
        spread: 500.0,
    };
    let mut dice = Rolls::new(7);
    assert!(
        (huge.roll(&mut dice) - 3600.0).abs() < f64::EPSILON,
        "a shot cannot spread past a full circle"
    );
    assert!(
        (huge.max() - 10_500.0).abs() < f64::EPSILON,
        "while its total still counts the spread: {}",
        huge.max()
    );

    // A spread with no dispersion at all rolls zero: the spread is not a source a roll draws from.
    let steady = cdda_sim::dispersion::Dispersion {
        linear: Vec::new(),
        normal: Vec::new(),
        multipliers: Vec::new(),
        spread: 500.0,
    };
    assert!(
        (steady.roll(&mut dice)).abs() < f64::EPSILON,
        "a perfect aim rolls no spread at all"
    );
    assert!(
        (steady.max() - 500.0).abs() < f64::EPSILON,
        "and its total is only what the data gave it: {}",
        steady.max()
    );
}

#[test]
fn a_doubling_multiplier_doubles_the_draw() {
    // The same dice, spent twice: once on a plain source and once on the same source behind a multiplier of two.
    let plain = cdda_sim::dispersion::Dispersion {
        linear: vec![100.0],
        normal: Vec::new(),
        multipliers: Vec::new(),
        spread: 0.0,
    };
    let doubled = cdda_sim::dispersion::Dispersion {
        multipliers: vec![2.0],
        ..plain.clone()
    };
    let mut first = Rolls::new(11);
    let mut second = Rolls::new(11);
    let one = plain.roll(&mut first);
    let two = doubled.roll(&mut second);
    assert!(one > 0.0, "the die is real: {one}");
    assert!(
        (two - 2.0 * one).abs() < f64::EPSILON,
        "and a multiplier of two doubles it: {one} against {two}"
    );
}

/// A shooter's skill is a source like any other, and the reference's curve is not linear.
///
/// Reference: `dispersion_from_skill` (`ranged.cpp:2477-2497`). Below skill five the shortfall costs `10` a level **and** scales the weapon's own dispersion by
/// `1.25 + shortfall * 2`; at or above five it costs `10` a level plus the weapon's dispersion times the remaining shortfall over five. At the maximum skill it is zero.
#[test]
fn skill_becomes_dispersion_on_the_reference_s_curve() {
    use cdda_sim::dispersion::from_skill;

    assert!((from_skill(10.0, 300.0)).abs() < f64::EPSILON, "a master has no penalty");
    // Skill 0: 10 * 10 = 100, plus 300 * (1.25 + 5 * 10 / 5) = 300 * 11.25.
    let unskilled = from_skill(0.0, 300.0);
    assert!(
        (unskilled - 3_475.0).abs() < 1e-9,
        "an unskilled shooter is off by thousands: {unskilled}"
    );
    // Skill 5: 10 * 5 = 50, plus 300 * 5 * 1.25 / 5 = 375.
    let middling = from_skill(5.0, 300.0);
    assert!(
        (middling - 425.0).abs() < 1e-9,
        "and half-way up the curve: {middling}"
    );
    assert!(
        unskilled > middling && middling > from_skill(9.0, 300.0),
        "and it falls as the skill climbs"
    );
    // The weapon's own dispersion matters more to a beginner than to an expert, which is the comment the reference writes beside the branch.
    assert!(
        from_skill(9.0, 600.0) - from_skill(9.0, 300.0) < from_skill(1.0, 600.0) - from_skill(1.0, 300.0),
        "a worse weapon hurts a poor shot more"
    );
}
