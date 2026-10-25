//! `Character::fun_for`, transcribed from the reference's own cases.
//!
//! Reference: `src/consumption.cpp:483`, and the upstream suite `tests/food_fun_for_test.cpp`, which this file follows case for case - the same foods, the same
//! character states, the same assertions. The upstream cases are inequalities (`actual_fun.first < toastem_fun / 2`) because the reference has no recording of the
//! exact numbers; the port will have one, but the inequalities are still the right first gate: they say what the rule is *for*.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_sim::fun_for::{ConsumptionEvent, Eater, Food, fun_for};
use std::collections::BTreeSet;

fn flags(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn food(fun: i32, food_flags: &BTreeSet<String>) -> Food<'_> {
    Food {
        type_id: "toastem",
        fun,
        monotony_penalty: 2,
        flags: food_flags,
        relative_rot: 0.0,
        component_hash: 0,
        is_food: true,
    }
}

fn eater<'a>(
    traits: &'a BTreeSet<String>,
    effects: &'a BTreeSet<String>,
    history: &'a [ConsumptionEvent],
) -> Eater<'a> {
    static NO_FLAGS: std::sync::OnceLock<BTreeSet<String>> = std::sync::OnceLock::new();
    static NO_BIONICS: std::sync::OnceLock<BTreeSet<String>> = std::sync::OnceLock::new();
    Eater {
        effects,
        traits,
        flags: NO_FLAGS.get_or_init(BTreeSet::new),
        active_bionics: NO_BIONICS.get_or_init(BTreeSet::new),
        bionics: NO_BIONICS.get_or_init(BTreeSet::new),
        power_kj: 0,
        history,
        now: 0,
    }
}

#[test]
fn a_non_food_has_no_fun_value() {
    // Upstream: `fun_for_non-food`. The rule only ever runs for something with a comestible slot; the port's caller passes `None` for the fun instead, so what is
    // worth pinning here is that a food with **no** fun of its own still returns the zero the reference returns for a non-food.
    let none = flags(&[]);
    let traits = flags(&[]);
    let effects = flags(&[]);
    assert_eq!(
        fun_for(&food(0, &none), &eater(&traits, &effects, &[])),
        (0, 0),
        "a food worth nothing is worth nothing"
    );
}

#[test]
fn food_tastes_much_worse_when_the_eater_is_sick() {
    // Upstream: `fun_for_food_eaten_while_sick`. The reference divides by three when the eater has a cold or the flu, and only when the fun was positive.
    let none = flags(&[]);
    let traits = flags(&[]);
    let toastem = food(10, &none);
    for sickness in ["common_cold", "flu"] {
        let effects = flags(&[sickness]);
        let (fun, _) = fun_for(&toastem, &eater(&traits, &effects, &[]));
        assert!(fun < 10 / 2, "a cold makes {sickness} much less fun: {fun}");
    }
    let effects = flags(&[]);
    assert_eq!(
        fun_for(&toastem, &eater(&traits, &effects, &[])).0,
        10,
        "and being well leaves it alone"
    );
}

#[test]
fn rotten_food_is_disgusting_unless_the_eater_eats_rot() {
    // Upstream: `fun_for_rotten_food`. Pine nuts at `relative_rot` 1.5 are rotten; a saprophage or a saprovore likes them that way.
    let none = flags(&[]);
    let effects = flags(&[]);
    let mut rotten = food(1, &none);
    rotten.relative_rot = 1.5;
    let ordinary = flags(&[]);
    let (fun, _) = fun_for(&rotten, &eater(&ordinary, &effects, &[]));
    assert!(fun < 0, "nobody else likes rotten food: {fun}");
    for rot_eater in ["SAPROPHAGE", "SAPROVORE"] {
        let traits = flags(&[rot_eater]);
        let (fun, _) = fun_for(&rotten, &eater(&traits, &effects, &[]));
        assert!(fun > 0, "a {rot_eater} does: {fun}");
    }
}

#[test]
fn cold_food_that_tastes_good_is_twice_the_fun() {
    // Upstream: `fun_for_cold_food`. `EATEN_COLD` food is doubled **only** while it is `COLD`; food that does not ask to be eaten cold ignores the temperature, and
    // cold food that tastes bad is rescued rather than doubled.
    let effects = flags(&[]);
    let traits = flags(&[]);
    let wants_cold = flags(&["EATEN_COLD"]);
    let cold = flags(&["EATEN_COLD", "COLD"]);
    let plain = flags(&[]);
    assert_eq!(
        fun_for(&food(10, &wants_cold), &eater(&traits, &effects, &[])).0,
        10,
        "warm cola is just cola"
    );
    assert_eq!(
        fun_for(&food(10, &cold), &eater(&traits, &effects, &[])).0,
        20,
        "cold cola is twice as good"
    );
    assert_eq!(
        fun_for(&food(10, &plain), &eater(&traits, &effects, &[])).0,
        10,
        "and coffee does not care"
    );
    let (fun, fun_max) = fun_for(&food(-5, &cold), &eater(&traits, &effects, &[]));
    assert_eq!((fun, fun_max), (1, 5), "but cold, bad food is made tolerable instead");
}

#[test]
fn melted_food_is_half_as_fun_and_bad_food_tastes_worse_once_melted() {
    // Upstream: `fun_for_melted_food`. Ice cream that is no longer frozen is half as good; the reference's comment says a melted freezable food that tasted *bad*
    // should taste 25% worse, and the code does exactly that.
    let effects = flags(&[]);
    let traits = flags(&[]);
    let melted = flags(&["MELTS"]);
    let frozen = flags(&["MELTS", "FROZEN"]);
    assert_eq!(
        fun_for(&food(10, &frozen), &eater(&traits, &effects, &[])).0,
        10,
        "frozen ice cream is ice cream"
    );
    assert_eq!(
        fun_for(&food(10, &melted), &eater(&traits, &effects, &[])).0,
        5,
        "melted, it is half as fun"
    );
    assert_eq!(
        fun_for(&food(-4, &melted), &eater(&traits, &effects, &[])).0,
        -5,
        "and a melted food that tasted bad tastes 25% worse"
    );
}

#[test]
fn a_feline_enjoys_cat_food_and_a_lupine_dog_food() {
    // Upstream: `fun_for_cat_food` and `fun_for_dog_food`. Both foods taste foul to everyone else and are flipped, then halved, for the threshold that matches
    // them: `fun = -fun`, then `fun /= 2`. A food exactly as fun as it is unpleasant - zero - becomes 2 instead.
    let effects = flags(&[]);
    let ordinary = flags(&[]);
    for (threshold, food_flag) in [("THRESH_FELINE", "FELINE"), ("THRESH_LUPINE", "LUPINE")] {
        let foul = flags(&[food_flag]);
        let (fun, _) = fun_for(&food(-4, &foul), &eater(&ordinary, &effects, &[]));
        assert!(fun < 0, "everyone else dislikes {food_flag} food: {fun}");
        let traits = flags(&[threshold]);
        let (fun, _) = fun_for(&food(-4, &foul), &eater(&traits, &effects, &[]));
        assert_eq!(fun, 2, "a {threshold} eater likes it as much as they disliked it, halved");
    }
    let feline = flags(&["THRESH_FELINE"]);
    let neutral = flags(&["FELINE"]);
    assert_eq!(
        fun_for(&food(0, &neutral), &eater(&feline, &effects, &[])).0,
        2,
        "and a food worth nothing becomes worth something to the right species"
    );
}

#[test]
fn a_gourmand_tastes_food_more_strongly_in_both_directions() {
    // Upstream: `fun_for_gourmand`. Good food becomes half again as fun with three times the ceiling; bad food - anything below -1, so a -1 is not bad enough to
    // count - is half as bad but keeps the ceiling it had.
    let effects = flags(&[]);
    let ordinary = flags(&[]);
    let gourmand = flags(&["GOURMAND"]);
    let none = flags(&[]);
    assert_eq!(fun_for(&food(10, &none), &eater(&ordinary, &effects, &[])), (10, 30));
    assert_eq!(fun_for(&food(10, &none), &eater(&gourmand, &effects, &[])), (15, 90));
    assert_eq!(fun_for(&food(-8, &none), &eater(&ordinary, &effects, &[])), (-8, -48));
    assert_eq!(fun_for(&food(-8, &none), &eater(&gourmand, &effects, &[])), (-4, -8));
    assert_eq!(
        fun_for(&food(-1, &none), &eater(&gourmand, &effects, &[])),
        (-1, -6),
        "and a -1 is untouched: the trait only starts below it"
    );
}

#[test]
fn food_eaten_too_often_costs_its_monotony_penalty_each_time() {
    // Upstream: `fun_for_food_eaten_too_often`. Two toast-ems in a row cost 2 each off the next one's fun, and the drop stops at zero: the penalty can never make
    // food taste bad, unless the food says it may (`NEGATIVE_MONOTONY_OK`).
    let effects = flags(&[]);
    let traits = flags(&[]);
    let none = flags(&[]);
    let toastem = food(10, &none);
    let once = [ConsumptionEvent { when: 0, type_id: "toastem".to_owned(), component_hash: 0 }];
    let twice = [
        ConsumptionEvent { when: 0, type_id: "toastem".to_owned(), component_hash: 0 },
        ConsumptionEvent { when: 0, type_id: "toastem".to_owned(), component_hash: 0 },
    ];
    assert_eq!(fun_for(&toastem, &eater(&traits, &effects, &[])).0, 10);
    assert_eq!(fun_for(&toastem, &eater(&traits, &effects, &once)).0, 8);
    assert_eq!(fun_for(&toastem, &eater(&traits, &effects, &twice)).0, 6);

    // A different meal of the same food, a meal eaten three days ago, and a meal of something else all count for nothing.
    let other_jam =
        [ConsumptionEvent { when: 0, type_id: "toastem".to_owned(), component_hash: 7 }];
    let stale = [ConsumptionEvent { when: -3 * 24 * 60 * 60, type_id: "toastem".to_owned(), component_hash: 0 }];
    let other_food = [ConsumptionEvent { when: 0, type_id: "bread".to_owned(), component_hash: 0 }];
    for history in [&other_jam[..], &stale[..], &other_food[..]] {
        assert_eq!(
            fun_for(&toastem, &eater(&traits, &effects, history)).0,
            10,
            "only the same food, the same way, within two days"
        );
    }

    // Four plates of a 10-fun food would be 2 fun, five would be 0, and it stops there rather than going negative.
    let many: Vec<ConsumptionEvent> = (0..5)
        .map(|_| ConsumptionEvent { when: 0, type_id: "toastem".to_owned(), component_hash: 0 })
        .collect();
    assert_eq!(fun_for(&toastem, &eater(&traits, &effects, &many)).0, 0);

    // Unless it is sour, which does not get less sour.
    let sour = flags(&["NEGATIVE_MONOTONY_OK"]);
    let sour_food = food(1, &sour);
    assert_eq!(
        fun_for(&sour_food, &eater(&traits, &effects, &many)).0,
        -9,
        "5 meals x 2 penalty off a fun of 1, with no floor"
    );
}

#[test]
fn a_taste_blocker_bionic_needs_enough_power_to_nullify_bad_taste() {
    // Upstream: `fun_for_bionic_bio_taste_blocker`. One kilojoule of power per unit of **the food's stated** bad taste - not per unit of what the eater would have
    // tasted - hides it entirely. With not enough power nothing happens at all; there is no partial hiding.
    let effects = flags(&[]);
    let traits = flags(&[]);
    let none = flags(&[]);
    let garlic = food(-15, &none);
    let active = flags(&["bio_taste_blocker"]);
    let mut blocker = eater(&traits, &effects, &[]);
    blocker.active_bionics = &active;
    blocker.power_kj = 10;
    assert_eq!(fun_for(&garlic, &blocker).0, -15, "10 kJ cannot hide a -15");

    blocker.power_kj = 20;
    assert_eq!(fun_for(&garlic, &blocker).0, 0, "20 kJ can");

    blocker.power_kj = 15;
    assert_eq!(
        fun_for(&garlic, &blocker).0,
        -15,
        "and exactly enough is not enough: the reference asks for strictly more than the bad taste"
    );

    // The rotten, the melted and the monotonous all count: the power is compared against the **stated** fun, so a garlic that would have tasted even worse still
    // only needs the 15.
    let mut rotten = garlic;
    rotten.relative_rot = 1.5;
    let _ = &rotten;
    blocker.power_kj = 16;
    assert_eq!(fun_for(&rotten, &blocker).0, 0, "16 kJ hides a rotten -15 too");

    // A bionic that is not active does nothing, and neither does power without the bionic.
    let mut inactive = eater(&traits, &effects, &[]);
    inactive.power_kj = 100;
    assert_eq!(fun_for(&garlic, &inactive).0, -15, "no bionic, no hiding");
}

#[test]
fn blood_eaters_like_blood_and_a_faulty_gross_food_bionic_makes_food_worse() {
    // Not an upstream case: `HEMOVORE_FUN`, `BLOODFEEDER` and `bio_faulty_grossfood` are the three branches `tests/food_fun_for_test.cpp` does not reach, and they
    // are in the reference (`src/consumption.cpp:560-583`, `:594-597`). Transcribed from the code rather than from a case, and marked as such.
    let effects = flags(&[]);
    let traits = flags(&[]);
    let blood = flags(&["HEMOVORE_FUN"]);
    let ordinary_food = flags(&[]);

    // Eating blood is fine but it is better raw: a bloodfeeder gains 25 on anything that was not worth something, and 20% on what was.
    let bloodfeeder = flags(&["BLOODFEEDER"]);
    let mut eater_bloodfeeder = eater(&traits, &effects, &[]);
    eater_bloodfeeder.flags = &bloodfeeder;
    assert_eq!(fun_for(&food(-3, &blood), &eater_bloodfeeder).0, 22);
    assert_eq!(fun_for(&food(10, &blood), &eater_bloodfeeder).0, 12);

    // A plain hemovore gets 13 and 10%.
    let hemovore = flags(&["HEMOVORE"]);
    let mut eater_hemovore = eater(&traits, &effects, &[]);
    eater_hemovore.flags = &hemovore;
    assert_eq!(fun_for(&food(-3, &blood), &eater_hemovore).0, 10);
    assert_eq!(fun_for(&food(10, &blood), &eater_hemovore).0, 11);

    // And a bloodfeeder finds ordinary good food only half as interesting: "this cherry soda's just not the same".
    assert_eq!(fun_for(&food(10, &ordinary_food), &eater_bloodfeeder).0, 5);

    // A faulty gross-food bionic takes 13 off food, but not off a drink.
    let faulty = flags(&["bio_faulty_grossfood"]);
    let mut eater_faulty = eater(&traits, &effects, &[]);
    eater_faulty.bionics = &faulty;
    assert_eq!(fun_for(&food(10, &ordinary_food), &eater_faulty).0, -3);
    let mut drink = food(10, &ordinary_food);
    drink.is_food = false;
    assert_eq!(fun_for(&drink, &eater_faulty).0, 10, "drinks are untouched");
}

#[test]
fn the_foods_the_upstream_cases_name_still_have_the_properties_those_cases_assume() {
    // The cases above stand on what the *content* says about six foods: that garlic is worse than -1, that cola asks to be eaten cold, that ice cream melts, that cat
    // and dog food carry the species flags, and that toast-em is worth eating at all. If the data moves, those cases would keep passing while testing nothing, so
    // the assumptions are checked against the corpus rather than against numbers transcribed here.
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [data.join("core"), data.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let items: std::collections::BTreeMap<String, cdda_content::items::ItemType> =
        cdda_content::items::load_paths(&paths)
            .expect("the items load")
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect();

    let fun = |id: &str| {
        items
            .get(id)
            .unwrap_or_else(|| panic!("{id} is in the corpus"))
            .comestible
            .as_ref()
            .unwrap_or_else(|| panic!("{id} is a comestible"))
            .fun
    };
    let has = |id: &str, flag: &str| {
        assert!(
            items
                .get(id)
                .unwrap_or_else(|| panic!("{id} is in the corpus"))
                .flags
                .contains(flag),
            "{id} carries {flag}"
        );
    };

    assert!(fun("toastem") > 4, "toast-em is worth eating: {}", fun("toastem"));
    assert!(fun("garlic") < -1, "garlic is worse than -1: {}", fun("garlic"));
    has("cola", "EATEN_COLD");
    has("icecream", "MELTS");
    has("catfood", "FELINE");
    has("dogfood", "LUPINE");
    assert!(fun("catfood") < 0, "cat food tastes foul to the rest of us: {}", fun("catfood"));
    assert!(fun("dogfood") < 0, "and so does dog food: {}", fun("dogfood"));
    assert!(fun("pine_nuts") > 0, "pine nuts are worth eating: {}", fun("pine_nuts"));
}
