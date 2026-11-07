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
