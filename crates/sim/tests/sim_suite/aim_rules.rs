//! How fast aiming improves, and how far point shooting goes before sights matter.
//!
//! Reference: `modified_sight_speed` and `point_shooting_limit` (`character.cpp:955-987`). Both are arithmetic over the recoil round 1152 accumulates and the skill a gun names, so the gate
//! works them by hand and pins the two ends of each curve.

#![allow(clippy::panic)]

/// The sight-speed bonus fades to nothing as the recoil approaches the sights' own precision, and is full once it is well past.
///
/// Reference: `modified_sight_speed` (`character.cpp:955-975`). The three cases worth pinning are the **zero** inside the sights' precision, the **near-zero** just past it, and the **full**
/// `10 + modifier` at three times it plus one - the shape is a logistic, so the middle is where a port can be quietly wrong.
#[test]
fn the_sight_speed_bonus_fades_as_the_recoil_comes_down_to_the_sights() {
    use cdda_sim::aim::sight_speed_with_attenuation;

    assert!(sight_speed_with_attenuation(0.0, 100.0, 90.0).abs() < f64::EPSILON, "inside the sights' precision nothing improves");
    assert!(sight_speed_with_attenuation(0.0, 100.0, 100.0).abs() < f64::EPSILON, "and at exactly it, nothing either");
    assert!(sight_speed_with_attenuation(5.0, -1.0, 500.0).abs() < f64::EPSILON, "a negative sight dispersion is the reference's guard");
    assert!(
        (sight_speed_with_attenuation(0.0, 100.0, 301.0) - 10.0).abs() < 0.000_1,
        "at three times the precision plus one the bonus is the full ten: {}",
        sight_speed_with_attenuation(0.0, 100.0, 301.0)
    );
    let near = sight_speed_with_attenuation(0.0, 100.0, 101.0);
    assert!(near > 0.0 && near < 1.0, "and just past the precision it is nearly nothing: {near}");
    assert!(
        sight_speed_with_attenuation(0.0, 100.0, 200.0) > near,
        "the bonus grows with the recoil: it is a fade, not a step"
    );
    assert!(
        (sight_speed_with_attenuation(5.0, 100.0, 301.0) - 15.0).abs() < 0.000_1,
        "and the modifier adds to the ten rather than scaling it"
    );
}

/// Point shooting's limit is the gun skill's, and archery's branch divides where every other one subtracts.
///
/// Reference: `point_shooting_limit` (`character.cpp:977-987`). A rifle's limit falls **ten a level from 200**; a bow's is `30 + 220 / (1 + skill)`, so it starts at **250**, is 66 at skill
/// five and settles at 50 by skill ten. **The two curves cross between nothing and mastery** - a bow starts higher and falls faster - which is why the port keeps the branch rather than one formula.
#[test]
fn point_shooting_goes_further_with_a_rifle_than_with_a_bow_until_mastery() {
    use cdda_sim::aim::point_shooting_limit;

    assert_eq!(point_shooting_limit(0, false), 200, "an unskilled rifle shooter point-shoots from two hundred");
    assert_eq!(point_shooting_limit(5, false), 150, "ten a level off it");
    assert_eq!(point_shooting_limit(10, false), 100, "and a hundred at mastery");

    assert_eq!(point_shooting_limit(0, true), 250, "a bow starts at 30 + 220 / 1, which is two hundred and fifty");
    assert_eq!(point_shooting_limit(5, true), 66, "and at skill five it is 30 + 36");
    assert_eq!(point_shooting_limit(10, true), 50, "settling at fifty by mastery");

    // Skill above ten is clamped, which is what `std::min( ..., MAX_SKILL )` says in both branches.
    assert_eq!(point_shooting_limit(15, false), 100, "skill past ten is ten");
    assert_eq!(point_shooting_limit(15, true), 50, "in both branches");
    // And the curves **cross**: a bow point-shoots further at nothing (250 against 200) and a rifle further at mastery (100 against 50), so the branch is not cosmetic - one formula for both
    // would be wrong at one end or the other.
    assert!(point_shooting_limit(0, true) > point_shooting_limit(0, false), "a bow point-shoots further at nothing: 250 against 200");
    assert!(point_shooting_limit(10, false) > point_shooting_limit(10, true), "and a rifle further at mastery: 100 against 50");
}
