//! What a worn thing takes off a blow.
//!
//! Reference: `item::_resist` (`item.cpp:9199-9231`). The rule has two parts that a tidier version would merge and
//! get wrong: a material only counts when the hit's roll beats its coverage, and what it contributes is its
//! resistance **times its thickness in millimetres** - not its resistance alone.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use cdda_content::armour::{Layer, layers_on, resist_of};

use crate::corpus;

#[test]
fn a_layer_only_counts_when_the_hit_meets_it() {
    let layer = Layer {
        resist: 6.0,
        thickness: 0.5,
        cover: 60,
    };
    assert_eq!(
        resist_of(&[layer], 59),
        3.0,
        "a roll of fifty-nine beats a coverage of sixty, so steel at six per millimetre contributes three"
    );
    assert_eq!(
        resist_of(&[layer], 60),
        0.0,
        "and sixty does not: the test is `<`, so the boundary is the hit's to lose"
    );
}

#[test]
fn layers_add_and_thickness_multiplies() {
    let thin = Layer {
        resist: 6.0,
        thickness: 0.25,
        cover: 100,
    };
    let thick = Layer {
        resist: 1.0,
        thickness: 2.0,
        cover: 100,
    };
    assert_eq!(resist_of(&[thin], 0), 1.5, "a quarter of a millimetre of steel");
    assert_eq!(resist_of(&[thick], 0), 2.0, "two millimetres of flesh");
    assert_eq!(resist_of(&[thin, thick], 0), 3.5, "and both together");
    assert_eq!(resist_of(&[], 0), 0.0, "nothing in the way protects nothing");
}

#[test]
fn a_real_garment_is_worth_what_its_materials_and_thickness_say() {
    let materials = corpus::materials_loaded();
    let items = corpus::items_loaded();
    // Every garment in the data that covers a torso with a material the loader describes.
    let mut checked = 0;
    for item in &items {
        let layers = layers_on(item, &materials, "torso", false);
        if layers.is_empty() {
            continue;
        }
        checked += 1;
        let total = resist_of(&layers, 0);
        assert!(
            total >= 0.0,
            "{} protects {total}, which cannot be negative",
            item.id
        );
        // The sum must be exactly what the layers say, layer by layer - a garment that reported a different
        // number here would be one whose protection and whose data disagree.
        let expected: f64 = layers
            .iter()
            .map(|layer| layer.resist * f64::from(layer.thickness))
            .sum();
        assert_eq!(total, expected, "{} does not add up", item.id);
    }
    assert!(
        checked >= 50,
        "only {checked} garments cover a torso, which is too few to prove anything"
    );
}

/// Whether a blow ruins the garment that stopped it.
///
/// Reference: `item::damage_armor_durability` (`item.cpp:9481-9537`). Its comment states three numbers plainly -
/// at most 80%, 40% when the blow is the garment's own resistance times 1.333, and a floor of ten points for
/// anything not fragile - so those are what this pins. The constants matter: the floor of ten is why a t-shirt
/// survives a punch, and dropping it would turn every fistfight into a naked one.
#[test]
fn a_blow_damages_a_garment_by_a_logistic_that_never_reaches_eighty_per_cent() {
    let chance = cdda_content::armour::damage_chance;

    // Not zero: the logistic never reaches zero, so a blow of nothing still has a sliver of a chance - about three
    // in a hundred thousand. That is the reference's own behaviour (`x_in_y( damage_chance, 1.0 )`), and rounding it
    // away here would be tidier and wrong.
    let nothing = chance(10.0, 0.0, 1, false, false);
    assert!(
        nothing > 0.0 && nothing < 1e-4,
        "a blow of nothing is not quite nothing: {nothing}"
    );
    let overwhelming = chance(10.0, 1_000.0, 1, false, false);
    assert!(
        overwhelming <= 0.8 && overwhelming > 0.79,
        "a blow beyond any resistance approaches eighty per cent and never passes it: {overwhelming}"
    );

    // The floor of ten: a cotton shirt resists almost nothing, so its threshold is ten, not its own resistance.
    let shirt = chance(0.5, 10.0, 1, false, false);
    let shirt_without_floor = chance(0.5, 10.0, 1, true, false);
    assert!(
        shirt < shirt_without_floor,
        "the floor of ten is why ordinary clothes survive a punch: {shirt} against {shirt_without_floor}"
    );
    assert!(shirt < 0.45, "and a punch is a coin toss at worst: {shirt}");

    // At the threshold the logistic is at its midpoint: 0.8 / (1 + e^0) = 0.4.
    let at_threshold = chance(0.0, 10.0, 1, false, false);
    assert!(
        (at_threshold - 0.4).abs() < 1e-9,
        "a blow that exactly meets the floor is forty per cent: {at_threshold}"
    );

    // More of the garment in the way means less chance of ruining it.
    let coat = chance(10.0, 20.0, 4, false, false);
    let glove = chance(10.0, 20.0, 1, false, false);
    assert!(coat * 4.0 - glove < 1e-9, "a garment covering four parts is a quarter as likely");
}

#[test]
fn a_sturdy_garment_shrugs_off_a_blow_below_its_threshold() {
    let chance = cdda_content::armour::damage_chance;
    assert_eq!(
        chance(20.0, 20.0, 1, false, true),
        0.0,
        "sturdy and under its own resistance is no damage at all"
    );
    assert!(
        chance(20.0, 40.0, 1, false, true) > 0.5,
        "but a blow past it is more likely to ruin the thing than not"
    );
    assert!(
        chance(20.0, 20.0, 1, false, false) > 0.0,
        "and the same garment without the flag is damaged by the same blow"
    );
}

/// A garment wears out in four steps and is ruined by the fifth.
///
/// Reference: `itype::damage_scale` 1000 and `itype::damage_max_` 4000 (`itype.h:1568-1588`), with
/// `item::mod_damage` (`item.cpp:9441-9452`) reporting a destroy when `damage_ + qty > max_damage()` - **passing**,
/// not reaching. So an untouched garment takes four steps and breaks on the fifth; a `>=` would break it on the
/// fourth, and this test is the only thing standing between the two.
#[test]
fn a_garment_takes_four_steps_of_damage_and_breaks_on_the_fifth() {
    use cdda_content::armour::{DAMAGE_MAX, DAMAGE_SCALE, damage_level};

    let mut damage = 0;
    for step in 1..=4 {
        damage += DAMAGE_SCALE;
        let ruined = damage > DAMAGE_MAX;
        assert!(!ruined, "step {step} is damage {damage}, which does not pass {DAMAGE_MAX}");
    }
    assert_eq!(damage, DAMAGE_MAX, "four steps is exactly the maximum");
    assert_eq!(
        damage_level(damage, DAMAGE_MAX),
        5,
        "and the reference's scale calls the maximum five - the top of its five steps"
    );

    damage += DAMAGE_SCALE;
    assert!(
        damage > DAMAGE_MAX,
        "the fifth step passes the maximum, which is what ruins the garment"
    );

    assert_eq!(damage_level(0, DAMAGE_MAX), 0, "an untouched garment is level zero");
    assert_eq!(damage_level(1, 0), 0, "and a thing with no maximum cannot be damaged");
}

/// The three flags that decide how hard a garment is to ruin exist in the data, and in sensible numbers.
///
/// Reference: `FRAGILE`, `STURDY` and `UNBREAKABLE` in `item::damage_armor_durability` (`item.cpp:9508-9532`). This
/// gate is about **non-vacuity**: if the data carried none of them, every rule about them would be tested against
/// nothing, and the armour model would look complete while being one branch short.
#[test]
fn the_data_carries_the_flags_that_decide_how_a_garment_wears_out() {
    let items = corpus::items_loaded();
    let count = |flag: &str| {
        items
            .iter()
            .filter(|item| item.flags.contains(flag))
            .count()
    };
    let fragile = count("FRAGILE");
    let sturdy = count("STURDY");
    let unbreakable = count("UNBREAKABLE");
    assert!(
        fragile > 5,
        "only {fragile} things in the data are fragile, which is too few to be the game's"
    );
    assert!(sturdy > 5, "only {sturdy} things are sturdy");
    assert!(
        unbreakable > 0,
        "nothing in the data is unbreakable, so that branch is tested against an empty set"
    );
}
