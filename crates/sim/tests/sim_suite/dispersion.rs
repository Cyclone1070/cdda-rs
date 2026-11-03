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

/// A thrown object's spread is worked out from what is thrown, how strong the arm is, and who is dodging - and the reference's **own comments disagree with its code**.
///
/// Reference: `Character::throwing_dispersion` (`ranged.cpp:1342-1384`) and `throw_dispersion_per_dodge` (`ranged.cpp:1329-1340`). The comment above
/// `throwing_dispersion` says "perfect situation gives us 1000 dispersion at lvl 0, down linearly to 250 at lvl 10"; the code gives **2500** and **119** for the same
/// conditions, because it divides by `8 * skill + 4`. The comment above `throw_dispersion_per_dodge` says "+200 per dodge point at 0 dexterity"; the code gives **50**. The
/// port transcribes the **code** - a transcribed comment would be a port that is wrong in every number and right in its shape.
#[test]
fn a_throw_s_spread_follows_the_reference_s_code_and_not_its_comments() {
    use cdda_sim::dispersion::{Throw, throwing_dispersion};

    let rock = Throw {
        weight_g: 500,
        volume_ml: 1_000,
        charges: 1,
        arm_strength: 8,
        skill: 0.0,
        perception: 10,
        eye_encumbrance: 0,
        blinded: false,
        target: None,
    };
    assert_eq!(
        throwing_dispersion(&rock),
        2_500,
        "difficulty 1000 over (8 * 0 + 4): the comment says 1000, the code says 2500"
    );
    assert_eq!(
        throwing_dispersion(&Throw { skill: 10.0, ..rock.clone() }),
        119,
        "and at full skill: 10000 / 84, truncated - the comment says 250, the code says 119"
    );

    // Every gram above `strength * 100` is a point of difficulty, and every millilitre over one litre is another.
    let heavy = Throw { weight_g: 1_000, ..rock.clone() };
    assert_eq!(throwing_dispersion(&heavy), 3_000, "1200 of difficulty");
    let bulky = Throw { volume_ml: 2_000, ..rock.clone() };
    assert_eq!(throwing_dispersion(&bulky), 5_000, "2000 of difficulty");
    // Charges are split out first, so a handful of pebbles throws like one pebble.
    let handful = Throw { weight_g: 2_500, volume_ml: 5_000, charges: 5, ..rock.clone() };
    assert_eq!(throwing_dispersion(&handful), 2_500, "one 500 g, one litre pebble's worth");

    // A dodging target adds its own term: the per-dodge cost times a dodge that counts **ten times** at point blank and once from two squares out.
    let dodge_two = Some(cdda_sim::dispersion::Dodging {
        dodge: 2.0,
        distance: 0,
        dexterity: 8,
        dex_modifier: 1.0,
    });
    assert_eq!(
        throwing_dispersion(&Throw { target: dodge_two.clone(), ..rock.clone() }),
        3_000,
        "20 points of dodge at 25 a point"
    );
    assert_eq!(
        throwing_dispersion(&Throw {
            target: Some(cdda_sim::dispersion::Dodging { distance: 2, ..dodge_far() }),
            ..rock.clone()
        }),
        2_550,
        "and two squares out the dodge counts once: 2 * 25"
    );

    // Eyes full of water cost ten points of perception each, and being blind quadruples the whole thing.
    assert_eq!(
        throwing_dispersion(&Throw { eye_encumbrance: 20, ..rock.clone() }),
        2_600,
        "ten points over perception, ten each"
    );
    assert_eq!(
        throwing_dispersion(&Throw { blinded: true, ..rock.clone() }),
        10_000,
        "a blind throw is four times as wide"
    );
}

/// The per-dodge cost falls as dexterity climbs, and the reference's comment about it is stale too.
///
/// Reference: `throw_dispersion_per_dodge` (`ranged.cpp:1329-1340`): `100 / max( 1, 2 + dex / 4 )`, truncated to a whole number. The comment says 200 at zero dexterity and
/// 100 at eight; the code gives **50** and **25**.
#[test]
fn the_per_dodge_cost_is_a_hundred_over_two_plus_a_quarter_of_dexterity() {
    use cdda_sim::dispersion::per_dodge;
    assert_eq!(per_dodge(0, 1.0), 50, "the comment says 200");
    assert_eq!(per_dodge(8, 1.0), 25, "the comment says 100");
    assert_eq!(per_dodge(12, 1.0), 20);
    assert_eq!(per_dodge(24, 1.0), 12, "truncated from 12.5");
    // A modifier above one makes dexterity worth more, and never lets the cost fall below zero.
    assert_eq!(per_dodge(8, 2.0), 12, "twice the dexterity of eight is sixteen");
    assert_eq!(per_dodge(0, 10.0), 5, "and one is the floor of the divisor");
}

/// A target that dodges two points, one square away: the same dodge, a different distance.
fn dodge_far() -> cdda_sim::dispersion::Dodging {
    cdda_sim::dispersion::Dodging {
        dodge: 2.0,
        distance: 2,
        dexterity: 8,
        dex_modifier: 1.0,
    }
}
