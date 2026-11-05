//! How fast aiming improves, and how far point shooting goes before sights matter.
//!
//! Reference: `modified_sight_speed` and `point_shooting_limit` (`character.cpp:955-987`). Both are arithmetic over the recoil round 1152 accumulates and the skill a gun names, so the gate
//! works them by hand and pins the two ends of each curve.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

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

/// Sight error is the shooter's perception added to the sights' own figure, and a zoomed sight takes a quarter of it.
///
/// Reference: `get_character_parallax` and `effective_dispersion` (`character.cpp:941-953`). The rule exists so that **the same sights are worth different numbers to different shooters** - the
/// thing a port flattens by storing one "sight dispersion" per gun - and the zoom quarter is the only place the two differ.
#[test]
fn the_same_sights_are_worse_in_weak_eyes_and_a_zoom_takes_a_quarter_off() {
    use cdda_sim::aim::{character_parallax, effective_dispersion};

    // Ten perception is twelve units of parallax (the 1.2 a point from `ranged_per_mod`), and eight is fourteen.
    assert_eq!(character_parallax(10, false), 12, "ten perception costs twelve");
    assert_eq!(character_parallax(8, false), 14, "and eight costs fourteen");
    assert_eq!(character_parallax(20, false), 0, "twenty is free");
    assert_eq!(character_parallax(24, false), 0, "and above twenty it stays free - the clamp at zero is after rounding");

    assert_eq!(character_parallax(10, true), 3, "a zoomed sight takes a quarter of the twelve");
    assert_eq!(character_parallax(8, true), 4, "fourteen quarters to three and a half, which rounds to four");
    assert_eq!(character_parallax(2, true), 5, "twenty-one and a half quarters to five");

    // The sights themselves: a hundred-dispersion sight is 112 in ten-perception hands and 100 in perfect ones.
    assert_eq!(effective_dispersion(100, 10, false), 112, "the sights plus the parallax");
    assert_eq!(effective_dispersion(100, 20, false), 100, "and the sights alone in good eyes");
    assert_eq!(effective_dispersion(100, 10, true), 103, "while a scope takes all but three of it");
    assert!(
        effective_dispersion(100, 8, false) > effective_dispersion(100, 10, false),
        "worse eyes, worse sights: 114 against 112"
    );
}

/// A gun's sights, in a shooter's eyes, against the recoil that shooter is carrying: the whole aiming chain in one call.
///
/// Reference: `item::sight_dispersion` → `Character::effective_dispersion` → `modified_sight_speed` (`item.cpp:11134-11160`, `character.cpp:941-975`). The gate uses the corpus's own
/// Mini-14, whose sights are **40**, and shows the two things a port gets wrong: **the same sights are worse in worse eyes**, and **the speed bonus is nothing until the recoil is past them.**
#[test]
fn a_gun_s_sights_are_worse_in_poor_eyes_and_worth_nothing_while_the_shooter_is_shaking() {
    use cdda_sim::aim::sight_report;

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/guns.json");
    let bytes = std::fs::read(path).expect("the corpus file exists");
    let value = cdda_json::parse(&bytes).expect("valid JSON");
    let guns = cdda_content::guns::load_dump(&value).expect("the guns load");
    let gun = guns.iter().find(|gun| gun.id == "ruger_mini").expect("the Mini-14").clone();
    assert_eq!(gun.sight_dispersion, 40, "the corpus says its sights are forty");

    let (good_eyes, _) = sight_report(&gun, 20, 1_000, false);
    let (poor_eyes, _) = sight_report(&gun, 8, 1_000, false);
    assert_eq!(good_eyes, 40, "in perfect eyes the sights are the sights");
    assert_eq!(poor_eyes, 54, "and in eight-perception eyes they are 40 + 14");
    assert!(poor_eyes > good_eyes, "worse eyes, worse sights");

    // The speed bonus: nothing while the recoil is inside the sights, and the full ten once it is past three times them plus one.
    let (sights, calm) = sight_report(&gun, 20, 20, false);
    let (_, shaking) = sight_report(&gun, 20, 500, false);
    assert!(calm.abs() < f64::EPSILON, "no bonus while the aim is inside the sights: {calm}");
    assert!((shaking - 10.0).abs() < 0.000_1, "and the full ten when it is far outside: {shaking}");
    assert_eq!(sights, 40, "the sights do not change with the recoil");
}

/// One move of aiming buys a little aim, more when the shooter is shaking and nothing once the sights are reached.
///
/// Reference: `Character::aim_per_move` (`character.cpp:1146-1210`). The gate works four cases by hand, and each is one of the rule's four moving parts: **skill divides**, **recoil fades the
/// speed in**, **the sight limit caps it**, and a shooter with nothing left to recover gains nothing at all.
#[test]
fn one_move_of_aiming_buys_aim_and_stops_at_the_sights() {
    use cdda_sim::aim::{MIN_RECOIL_IMPROVEMENT, aim_per_move};

    // Skill zero, at the recoil cap, no sight speed: 10 / 2.5 = 4, the fade is 1, the cap is 15, and 2.4 scales it to 9.6.
    let untrained = aim_per_move(3000.0, 0.0, 0, 0.0);
    assert!((untrained - 9.6).abs() < 0.000_1, "an unskilled shooter at the cap gains 9.6: {untrained}");

    // Ten skill divides by 1 instead of 2.5, so the same case gains 24 - two and a half times as fast.
    let master = aim_per_move(3000.0, 0.0, 10, 0.0);
    assert!((master - 24.0).abs() < 0.000_1, "and at mastery, 24: {master}");
    assert!(master > untrained, "skill makes aiming faster");

    // At rest the fade is near zero, so almost nothing is gained however skilled the shooter; the floor keeps it above nothing.
    let at_rest = aim_per_move(0.0, 0.0, 10, 0.0);
    assert!(at_rest <= MIN_RECOIL_IMPROVEMENT, "at rest there is nothing to aim away: {at_rest}");

    // The sight limit is the last word: with the recoil already inside it, the gain is zero, and it clips a gain that would overshoot.
    assert!(aim_per_move(20.0, 20.0, 10, 0.0).abs() < f64::EPSILON, "inside the sights, nothing");
    // Below the sights the reference returns a **negative** improvement - `min( aim_speed, recoil - limit )` with `recoil < limit` - and every caller rejects it with
    // `if( adj <= MIN_RECOIL_IMPROVEMENT ) break;`. The port keeps the negative rather than clamping it, because the sign is the reference's way of saying "stop aiming", and a port that
    // clamped it to zero would look identical here and differ at the caller.
    let below = aim_per_move(5.0, 20.0, 10, 0.0);
    assert!(below < MIN_RECOIL_IMPROVEMENT, "below the sights the gain is below the floor, which is how a caller knows to stop: {below}");
    assert!(below < 0.0, "and in fact negative, which is the reference's own answer: {below}");
    let clipped = aim_per_move(100.0, 90.0, 10, 0.0);
    assert!(clipped <= 10.0, "and a gain never takes the aim past what the sights allow: {clipped}");

    // The sights' own speed adds to the base ten before anything divides, so a slow sight is worth less than the ten itself.
    assert!(
        aim_per_move(3000.0, 0.0, 0, 5.0) > aim_per_move(3000.0, 0.0, 0, 0.0),
        "faster sights, faster aim"
    );
}

/// Aiming spends moves until the floor stops it, and it stops rather than grinding.
///
/// Reference: the `AIM` activity's loop over `aim_per_move` (`activity_actor.cpp`), which breaks on `adj <= MIN_RECOIL_IMPROVEMENT`. The point of the gate is the **stopping condition**: a
/// loop that ran for every move asked for would lower the recoil past the sights' limit and silently make the shooter better than the rule allows.
#[test]
fn aiming_spends_moves_until_the_floor_stops_it() {
    use cdda_sim::aim::spend_aim;

    // Ten moves at the cap, unskilled, no sight speed. The **first** move buys 9.6, and each later one buys slightly less, because the recoil fade is part of the same rule - so the ten
    // together take off **95.78**, not 96. A port that multiplied the first gain by ten would be off by a fifth of a point and, more importantly, would have dropped the fade from the loop.
    let (after, spent) = spend_aim(3000.0, 0.0, 0, 0.0, 10);
    assert_eq!(spent, 10, "every move was spent");
    let fell = 3000.0 - after;
    assert!((fell - 95.778_484_341_478_82).abs() < 0.000_001, "and the recoil fell by the sum of ten shrinking gains: {fell}");
    assert!(fell < 96.0, "which is less than ten times the first: {fell}");

    // With a limit of 90 and a recoil of 95, one move spends the last five and the next finds nothing to do.
    let (clamped, moves) = spend_aim(95.0, 90.0, 10, 0.0, 10);
    assert!(clamped >= 90.0, "aiming never goes past what the sights allow: {clamped}");
    assert!(moves < 10, "and the loop stopped early rather than spending all ten: {moves}");

    // Inside the sights already: nothing to spend, no move spent.
    assert_eq!(spend_aim(50.0, 90.0, 10, 0.0, 10).1, 0, "no moves spent when there is nothing to gain");
    // And a shooter who asks for no moves gets none.
    assert_eq!(spend_aim(3000.0, 0.0, 10, 0.0, 0), (3000.0, 0), "zero moves is zero change");
}

/// How many moves of aiming a shot needs, and the count is exactly the one that reaches the target.
///
/// Reference: `Character::gun_engagement_moves` (`ranged.cpp:675-691`). The gate checks the **edge** rather than a magic number: the moves the function reports take the recoil to the target, and
/// **one fewer does not** - which is what "how long will this take" has to mean, and what a port that rounded the count either way would get wrong.
#[test]
fn the_moves_a_shot_needs_are_exactly_enough_and_not_one_more() {
    use cdda_sim::aim::{moves_to_reach, spend_aim};

    let (recoil, target, skill) = (615.0, 200.0, 0);
    let moves = moves_to_reach(recoil, target, skill, 0.0);
    assert!(moves > 0, "a shot from 615 down to 200 takes some aiming: {moves}");
    let (reached, _) = spend_aim(recoil, 0.0, skill, 0.0, moves);
    assert!(reached <= target, "and that many moves reaches the target: {reached} against {target}");
    let (short, _) = spend_aim(recoil, 0.0, skill, 0.0, moves - 1);
    assert!(short > target, "while one fewer does not: {short} against {target}");

    // A shooter already inside the target needs nothing, and a target at zero needs everything.
    assert_eq!(moves_to_reach(150.0, 200.0, 0, 0.0), 0, "already inside it: no moves");
    assert!(moves_to_reach(3000.0, 0.0, 10, 0.0) > moves_to_reach(615.0, 0.0, 10, 0.0), "and further to go, more moves");
    // The floor stops the count rather than the loop running forever: at rest nothing improves, so the count is finite whatever the target.
    assert_eq!(moves_to_reach(0.5, 0.0, 10, 0.0), 0, "a recoil the floor will not improve costs no moves");
}
