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

/// Which weak point a roll lands on, walked the way the reference walks its list.
///
/// Reference: `weakpoints::select_weakpoint` (`weakpoint.cpp:588-631`). Each entry's **raw** chance is added to a running base, the base is **reweighed** through `1 - (1-base)^rolls`, and the roll is
/// compared against the **difference** between consecutive reweighed figures. **The comparison is `idx < chance`, strictly**, so a roll exactly on a boundary belongs to the next entry - and the
/// answer is the *index*, because a caller wants to spend the hit, not hold a reference the store may have moved.
///
/// **A coverage of fifty is not a chance of fifty.** `hit_chance` multiplies by the normal tail, which is one half when the attacker's skill equals the weak point's difficulty - so two entries of
/// fifty coverage at difficulty zero are worth twenty-five each, and the test's boundaries are twenty-five and a hundred rather than fifty and a hundred.
#[test]
fn a_roll_walks_the_weak_points_in_reweighed_order() {
    use cdda_sim::weakpoints::{Weakpoint, reweigh, select_weakpoint};

    let list = [
        Weakpoint { id: "eye".to_owned(), coverage: 50.0, difficulty: 0.0, helps_attacker: true },
        Weakpoint { id: "knee".to_owned(), coverage: 50.0, difficulty: 0.0, helps_attacker: true },
    ];
    // A beginner gets one roll, so each entry's share is its own raw chance: twenty-five, then twenty-five.
    assert_eq!(select_weakpoint(&list, 0.0, 0.0), Some(0), "a low roll takes the first");
    assert_eq!(select_weakpoint(&list, 0.0, 24.999), Some(0), "**and twenty-five is the boundary**: below it is the first");
    assert_eq!(select_weakpoint(&list, 0.0, 25.0), Some(1), "at exactly twenty-five the first has had its share");
    assert_eq!(select_weakpoint(&list, 0.0, 49.999), Some(1), "and the second absorbs the rest");
    assert_eq!(
        select_weakpoint(&list, 0.0, 100.0),
        None,
        "**a roll past every share is the default weak point**, which is the monster's body rather than a failure"
    );

    // With five rolls the distribution is reweighed: the pair's fifty percent becomes 96.875, and the first entry takes most of it.
    assert!((100.0 * reweigh(0.5, 5.0) - 96.875).abs() < 1e-9, "fifty percent through five rolls is 96.875");
    // **An expert nearly always finds the first weak point**: at ten points of skill against a difficulty of zero the tail is almost one, so the pair's raw chance is nearly its whole coverage
    // and the reweighed first share is 96.875 of a hundred.
    assert_eq!(select_weakpoint(&list, 10.0, 50.0), Some(0), "**an expert's fifty is still the first weak point**");
    assert_eq!(select_weakpoint(&list, 10.0, 89.0), Some(0), "and so is eighty-nine");
    assert_eq!(select_weakpoint(&list, 10.0, 96.9), Some(1), "only past 96.875 does the second one open");
    assert_eq!(select_weakpoint(&list, 10.0, 99.99), Some(1), "which absorbs what is left of a hundred");
    assert_eq!(select_weakpoint(&list, 10.0, 100.0), None, "and the roll's own top is the body");
}

/// Hitting a weak point changes the armour it meets and the damage it deals.
///
/// Reference: `weakpoint::apply_to( resistances & )` and `apply_to( damage_instance &, bool is_crit )` (`weakpoint.cpp:496-526`). Two rules, and the second has a **substitution** in it:
///
/// ```text
/// armour:  value *= armor_mult      then   value -= armor_penalty
/// damage:  if is_crit  -> *= crit_mult       (and damage_mult is NOT applied)
///          else        -> *= damage_mult
/// ```
///
/// **A critical hit replaces the weak point's ordinary multiplier rather than stacking with it**, which is the reference's `else if`: a weak point worth twice the damage normally is not worth
/// six times on a crit unless its own `crit_mult` says so. That is the whole reason a critical hit is worth hunting - and the reason a port that multiplied both would quietly double it.
#[test]
fn a_weak_point_changes_the_armour_and_the_damage() {
    use cdda_sim::weakpoints::{WeakpointFactors, apply_to_armour, apply_to_damage};

    let factors = WeakpointFactors {
        armour_multiplier: [("bash".to_owned(), 0.5)].into_iter().collect(),
        armour_penalty: [("bash".to_owned(), 2.0)].into_iter().collect(),
        damage_multiplier: [("cut".to_owned(), 2.0)].into_iter().collect(),
        crit_multiplier: [("cut".to_owned(), 3.0)].into_iter().collect(),
    };

    // Armour: ten bash becomes five by the multiplier and three by the penalty after it - **in that order**, so a penalty is worth more against heavy armour.
    let mut armour = [("bash".to_owned(), 10.0), ("cut".to_owned(), 4.0)].into_iter().collect();
    apply_to_armour(&factors, &mut armour);
    assert!((armour["bash"] - 3.0).abs() < 1e-9, "**half of ten, less two, is three**: {}", armour["bash"]);
    assert!((armour["cut"] - 4.0).abs() < 1e-9, "and a type the weak point says nothing about is untouched");

    // Damage: a cut of ten is twenty normally, and **thirty on a crit rather than sixty**.
    let mut damage = [("cut".to_owned(), 10.0), ("bash".to_owned(), 10.0)];
    let ordinary = apply_to_damage(&factors, &mut damage, false);
    assert!((ordinary[0].1 - 20.0).abs() < 1e-9, "the cut is doubled: {}", ordinary[0].1);
    assert!((ordinary[1].1 - 10.0).abs() < 1e-9, "and the bash is untouched");

    let mut damage = [("cut".to_owned(), 10.0), ("bash".to_owned(), 10.0)];
    let critical = apply_to_damage(&factors, &mut damage, true);
    assert!(
        (critical[0].1 - 30.0).abs() < 1e-9,
        "**a crit triples, and does not also double**: {}",
        critical[0].1
    );
    assert!((critical[1].1 - 10.0).abs() < 1e-9, "and the bash is still untouched");
}

/// How good an attacker is at finding a weak point.
///
/// Reference: `Character::generic_weakpoint_skill` (`weakpoint.cpp:82-90`):
///
/// ```text
/// skill = ( level( skill_1 ) + level( skill_2 ) ) / 2
/// stat  = ( dex - 8 ) / 8 + ( per - 8 ) / 8
/// mul   = ( limb score 1 + limb score 2 ) / 2
/// return ( skill + stat ) * mul
/// ```
///
/// **It is a product, not a sum**: an attacker with no limbs to see or react with finds nothing however skilled it is, which is why a hurt head costs more than a point of skill. A monster's
/// answer is simply its melee skill (`monster.cpp:76-79`).
#[test]
fn a_weak_point_needs_a_skilled_eye_to_find() {
    use cdda_sim::weakpoints::attacker_skill;

    // A survivor with three levels of melee and none of the weapon's skill, an ordinary body's dexterity and perception, and whole eyes and head.
    let ordinary = attacker_skill(3.0, 0.0, 10, 10, 1.0, 1.0);
    assert!((ordinary - 2.0).abs() < 1e-9, "**one and a half of skill plus half of a point of stat**: {ordinary}");
    // **Eight is the centre**: a survivor of exactly eight and eight has no stat bonus at all, and no skill means no chance of finding anything.
    assert!((attacker_skill(0.0, 0.0, 8, 8, 1.0, 1.0) - 0.0).abs() < 1e-9, "a beginner of ordinary build");
    // **The limbs multiply the whole thing**: half the reaction halves the skill.
    let hurt = attacker_skill(3.0, 0.0, 10, 10, 1.0, 0.5);
    assert!((hurt - 1.5).abs() < 1e-9, "**half of the head halves the skill**: {hurt}");
    assert!(
        (attacker_skill(3.0, 0.0, 10, 10, 0.0, 0.0) - 0.0).abs() < 1e-9,
        "and no eyes and no reaction finds nothing at all"
    );
    // A better weapon skill is averaged with melee, not added to it.
    assert!(
        (attacker_skill(4.0, 2.0, 8, 8, 1.0, 1.0) - 3.0).abs() < 1e-9,
        "four and two average to three"
    );
}

/// A blow that lands on a weak point is changed by it.
///
/// Reference: `Creature::deal_damage` asks `mon->type->weakpoints.select_weakpoint( attack )` (`creature.cpp:1146`) and then spends the answer three ways - the **armour** it meets
/// (`apply_to( resistances )`), the **damage** it deals (`apply_to( damage_instance, is_crit )`), and whether the blow can critical at all (`crit_roll &= ret.wp->is_good &&
/// !ret.wp->id.empty()`).
///
/// **A weak point that hurts the monster cancels the attacker's crit**: a blow that found a bad place is not a clean one, however well it was struck. The gate below is the doubled-damage
/// half: the same seed, the same swing, two worlds - one body with the weak point and one without - and **the weak point's body takes more from the same blow**.
#[test]
fn a_blow_that_lands_on_a_weak_point_is_changed_by_it() {
    use bevy_ecs::prelude::*;
    use cdda_sim::item_registry::{Kind, set_item_types};
    use cdda_sim::weakpoints::{WeakPoint, WeakPoints, Weakpoint, WeakpointFactors};

    /// One world, one swing: a survivor with a rock in hand against a body of nine hundred hit points, with or without a weak point that doubles the bash.
    fn blow(weak: bool) -> i32 {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(9));
        set_item_types(
            &mut world,
            &[Kind {
                id: "rock",
                volume_ml: 50,
                weight_g: 657,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 20.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: 0,
            }],
        );
        let rock = cdda_sim::item_registry::id_of(&mut world, "rock").expect("the rock");
        let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (1, 0, 0));
        world.entity_mut(zombie).insert((
            cdda_sim::creature::Health { hp: 9_000, max_hp: 9_000 },
            // **An unarmoured body**, because the fixture is about the weak point rather than about which layer the blow stopped in.
            cdda_sim::creature::Armour { bash: 0, cut: 0, stab: 0 },
        ));
        world.entity_mut(survivor).insert((
            cdda_sim::creature::Dexterity { base: 8 },
            cdda_sim::creature::Wielding(rock),
        ));
        if weak {
            let mut factors = WeakpointFactors::default();
            factors.damage_multiplier.insert("bash".to_owned(), 2.0);
            let doubled = WeakPoint {
                weakpoint: Weakpoint {
                    id: "ribs".to_owned(),
                    // **A hundred percent coverage at a difficulty of minus a hundred**: it cannot be missed, so the comparison is about the multiplier rather than a roll.
                    coverage: 100.0,
                    difficulty: -100.0,
                    helps_attacker: true,
                },
                factors,
            };
            let plain = WeakPoint {
                weakpoint: Weakpoint {
                    id: String::new(),
                    coverage: 100.0,
                    difficulty: -100.0,
                    helps_attacker: true,
                },
                factors: WeakpointFactors::default(),
            };
            world.entity_mut(zombie).insert(WeakPoints {
                entries: vec![doubled],
                default: plain,
            });
        }
        let dealt = cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie).unwrap_or(-1);
        let lost = 9_000 - cdda_sim::creature::health_of(&world, zombie).unwrap_or(9_000);
        assert_eq!(dealt, lost, "what the swing says it dealt is what the body lost");
        dealt
    }

    let plain = blow(false);
    let weak = blow(true);
    assert!(plain > 0, "the rock hurts a body with no weak points: {plain}");
    assert_eq!(weak, plain * 2, "**and the same blow into a doubled weak point is worth twice as much**: {weak} against {plain}");
}
