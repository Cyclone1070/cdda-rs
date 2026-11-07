//! Which martial arts buffs are in play, and what they need in hand.
//!
//! Reference: `ma_buff::is_valid_character` (`martialarts.cpp:713-760`). A static buff states its requirement **first** and its numbers second, and the requirement decides whether the buff
//! applies at all: an unarmed-only buff needs an empty hand, a melee-only buff needs a weapon the style allows, and a buff that names neither applies to **neither** - which is why the reference
//! loads both flags as `false` rather than `true` (`martialarts.cpp:250-251`).

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp, clippy::indexing_slicing)]

use cdda_sim::martial_arts::{Hands, Requirement, applies};

fn requirement(unarmed: bool, melee: bool) -> Requirement {
    Requirement {
        unarmed_allowed: unarmed,
        melee_allowed: melee,
        strictly_unarmed: false,
        wall_adjacent: false,
        weapon_categories: Vec::new(),
    }
}

/// **A buff that names neither hand applies to neither**, which is the reference's own default.
#[test]
fn a_buff_that_names_no_hand_applies_to_none() {
    let neither = requirement(false, false);
    assert!(!applies(&neither, Hands::Empty, &[]), "an empty hand earns nothing from a buff that does not ask for one");
    assert!(!applies(&neither, Hands::Weapon("a knife"), &["knife"]), "and neither does a weapon");
}

/// An unarmed-only buff is worth something with an empty hand and nothing with a weapon.
#[test]
fn an_unarmed_buff_needs_an_empty_hand() {
    let unarmed = requirement(true, false);
    assert!(applies(&unarmed, Hands::Empty, &[]), "**an empty hand is what it asks for**");
    assert!(
        !applies(&unarmed, Hands::Weapon("a knife"), &["knife"]),
        "and a knife is not, even one the style would allow"
    );
}

/// A melee-only buff needs a weapon **the style allows**, which is the reference's `is_valid_weapon` and the category list beside it.
#[test]
fn a_melee_buff_needs_a_weapon_of_a_category_it_names() {
    let mut melee = requirement(false, true);
    assert!(!applies(&melee, Hands::Empty, &[]), "a melee buff does nothing in an empty hand");
    assert!(
        applies(&melee, Hands::Weapon("a knife"), &["knife"]),
        "**a weapon with no category stated is allowed by any melee buff**"
    );
    melee.weapon_categories = vec!["KNIFE".to_owned()];
    assert!(applies(&melee, Hands::Weapon("a knife"), &["KNIFE"]), "and a stated category needs a weapon of it");
    assert!(
        !applies(&melee, Hands::Weapon("a bat"), &["BLUNT"]),
        "**a bat is not a knife, so the buff is not in play**"
    );
}

/// A buff that allows both is worth something either way.
#[test]
fn a_buff_that_allows_both_is_worth_something_either_way() {
    let both = requirement(true, true);
    assert!(applies(&both, Hands::Empty, &[]), "an empty hand is allowed");
    assert!(applies(&both, Hands::Weapon("a knife"), &["knife"]), "and so is a weapon");
}

/// A bonus's scale is multiplied by whatever it scales with, and a bonus that scales with nothing is its scale alone.
///
/// Reference: `effect_scaling::get` (`bonuses.cpp:318-372`) and `stat_bonus`'s own constructor (`bonuses.cpp:120-160`). The `stat` is the reference's `scaling_stat`: a **character stat**
/// (`str`, `dex`, `int`, `per`), a **skill** by name (`melee`, `dodge`, `unarmed`), or `NONE` - which is not zero but **the scale itself**, and that is the case a port gets wrong by defaulting
/// to nothing.
#[test]
fn a_bonuss_scale_is_multiplied_by_what_it_scales_with() {
    use cdda_sim::martial_arts::{Scaling, scaled};

    let dex_two = Scaling::Stat { name: "dex".to_owned(), value: 12.0 };
    assert_eq!(scaled(1.5, &dex_two), 18.0, "**a scale of one and a half with a dexterity of twelve is eighteen**");
    let melee_three = Scaling::Skill { name: "melee".to_owned(), level: 3.0 };
    assert_eq!(scaled(0.5, &melee_three), 1.5, "and half of a melee skill of three is one and a half");
    assert_eq!(
        scaled(2.0, &Scaling::None),
        2.0,
        "**a bonus that scales with nothing is its scale, not zero**"
    );
    assert_eq!(scaled(2.0, &Scaling::Unknown("speed".to_owned())), 0.0, "a stat the port does not read answers nothing");
}

/// Flat bonuses add, and multiplied bonuses multiply - and a multiplier that comes out negative is **clamped to nothing**.
///
/// Reference: `bonus_container::get_flat` (`bonuses.cpp:221-240`) sums and answers zero when the stat is absent; `get_mult` (`:242-258`) starts at **one**, multiplies, and answers
/// `max( 0.0f, ret )` with the reference's own comment that "all relevant effects require non-negative values".
#[test]
fn flat_bonuses_add_and_multipliers_multiply_then_clamp() {
    use cdda_sim::martial_arts::{Bonus, flat_of, mult_of};
    use std::collections::BTreeMap;

    let mut flat = BTreeMap::new();
    flat.insert(
        "hit".to_owned(),
        vec![
            Bonus { scale: 1.0, scaling: cdda_sim::martial_arts::Scaling::None },
            Bonus { scale: 0.5, scaling: cdda_sim::martial_arts::Scaling::Skill { name: "melee".to_owned(), level: 4.0 } },
        ],
    );
    assert_eq!(flat_of(&flat, "hit"), 3.0, "**flat bonuses add**: one plus half of a melee skill of four");
    assert_eq!(flat_of(&flat, "dodge"), 0.0, "a stat with no bonuses is worth nothing at all");

    let mut mult = BTreeMap::new();
    mult.insert(
        "block".to_owned(),
        vec![
            Bonus { scale: 2.0, scaling: cdda_sim::martial_arts::Scaling::None },
            Bonus { scale: 0.5, scaling: cdda_sim::martial_arts::Scaling::Skill { name: "dex".to_owned(), level: 4.0 } },
        ],
    );
    assert_eq!(mult_of(&mult, "block"), 4.0, "**multipliers multiply**: two times half of four");
    assert_eq!(mult_of(&mult, "speed"), 1.0, "a stat with no multipliers is worth one, not zero");

    // **Two negatives multiply to a positive**, which is why the clamp needs an odd count to show: one bonus of minus two is a product of minus two, and that is what the reference floors.
    let mut negative = BTreeMap::new();
    negative.insert(
        "speed".to_owned(),
        vec![Bonus { scale: -2.0, scaling: cdda_sim::martial_arts::Scaling::None }],
    );
    assert_eq!(mult_of(&negative, "speed"), 0.0, "**and a product below zero is clamped to zero**");
}
