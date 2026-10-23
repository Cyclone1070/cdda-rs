//! What the player can tell about a square, given how lit it is and what they are seeing it through.
//!
//! Reference: `map::get_visibility( lit_level, visibility_variables )` (`src/map.cpp:6808-6849`), and the two enums it joins: `lit_level` (`src/lightmap.h:55-63`,
//! seven values) and `visibility_type` (`src/enums.h:128-135`, six).
//!
//! The port's `LitLevel` used to have four of the seven, on the stated grounds that `MEMORIZED` and `BLANK` "are not light levels but states of the map overlay". That
//! was wrong in a way that matters: `map::apparent_light_at` **returns** `BLANK` and `BRIGHT_ONLY` as answers, so they are values the light code produces and the
//! visibility rule has to read.

#![allow(clippy::expect_used)]

use cdda_core_types::light::{LitLevel, VisibilityType, visibility};

#[test]
fn a_clear_square_is_clear_at_every_level_of_light_that_can_be_seen_by() {
    // The three middles all read the same and are not distinguished: low light, normal light and a light source are all "you can see this".
    for level in [LitLevel::Low, LitLevel::Lit, LitLevel::Bright] {
        assert_eq!(visibility(level, false), VisibilityType::Clear, "{level:?}");
    }
}

#[test]
fn darkness_is_dark_and_a_boomer_makes_it_worse() {
    assert_eq!(visibility(LitLevel::Dark, false), VisibilityType::Dark);
    assert_eq!(visibility(LitLevel::Dark, true), VisibilityType::BoomerDark);
}

#[test]
fn a_square_that_is_bright_but_not_visible_is_lit_rather_than_clear() {
    // `BRIGHT_ONLY` is what `apparent_light_at` answers when the light gets through but the detail does not: bright, and not something you can look at.
    assert_eq!(visibility(LitLevel::BrightOnly, false), VisibilityType::Lit);
    assert_eq!(
        visibility(LitLevel::BrightOnly, true),
        VisibilityType::Boomer,
        "a boomer's bile turns it into a smear of colour instead"
    );
}

#[test]
fn blank_and_memorised_are_hidden_and_a_boomer_cannot_tell() {
    // The reference groups these two with the binary's own default: nothing is drawn. What is memorised is drawn by the memory code, not by this rule.
    for level in [LitLevel::Blank, LitLevel::Memorized] {
        assert_eq!(
            visibility(level, false),
            VisibilityType::Hidden,
            "{level:?}"
        );
        assert_eq!(
            visibility(level, true),
            VisibilityType::BoomerDark,
            "{level:?} through bile"
        );
    }
}
