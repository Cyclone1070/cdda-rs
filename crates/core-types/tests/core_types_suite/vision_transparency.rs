//! What blocks *vision* rather than light, and the three exceptions the reference makes for the player.
//!
//! Reference: `map::build_vision_transparency_cache( zlev )` (`src/lightmap.cpp:207-286`). The vision cache starts as a copy of the light's transparency cache and is then
//! changed in three ways, each of which is a way seeing is not the same as lighting:
//!
//! 1. **translucent terrain blocks vision** - frosted glass lets light through and not detail, so a `TFLAG_TRANSLUCENT` tile is solid to the eye while staying lit;
//! 2. **crouching behind something blocks vision** - if the player is crouching, prone or running on four legs, every square next to them with `coverage >= 30` becomes
//!    opaque, which is how hiding behind a low wall works. The reference's own TODO admits this changes vision for *everyone* on the level, monsters included;
//! 3. **the player's own square is always visible** - forced back to open air last, so standing on frosted glass does not blind you to your own feet.

#![allow(clippy::expect_used)]
#![allow(clippy::indexing_slicing)]

use cdda_core_types::light::TRANSPARENCY_OPEN_AIR;

/// Floats: "the same" means "as close as floats get".
#[track_caller]
fn assert_close(passed: f32, expected: f32, what: &str) {
    assert!(
        (passed - expected).abs() < 1e-6,
        "{what}: got {passed}, expected {expected}"
    );
}
use cdda_core_types::transparency::{COVERAGE_TO_HIDE_BEHIND, VisionFacts, vision_transparency_of};

const SIDE: usize = 5;

/// A level of plain air, with a function deciding which squares are translucent.
fn level(translucent: impl Fn(usize, usize) -> bool) -> VisionFacts {
    let mut facts = VisionFacts {
        side: SIDE,
        transparency: vec![TRANSPARENCY_OPEN_AIR; SIDE * SIDE],
        translucent: vec![false; SIDE * SIDE],
        coverage: vec![0; SIDE * SIDE],
        player: None,
        hiding: false,
    };
    for x in 0..SIDE {
        for y in 0..SIDE {
            facts.translucent[x * SIDE + y] = translucent(x, y);
        }
    }
    facts
}

#[test]
fn a_translucent_tile_can_be_lit_but_cannot_be_seen_through() {
    // The first rule. Lighting and seeing read the same cache except here: frosted glass passes light and not detail.
    let facts = level(|x, y| x == 2 && y == 2);
    let vision = vision_transparency_of(&facts);
    assert_close(vision[2 * SIDE + 2], 0.0, "solid to the eye");
    assert_close(
        facts.transparency[2 * SIDE + 2],
        TRANSPARENCY_OPEN_AIR,
        "and clear to the light",
    );
    assert_close(vision[0], TRANSPARENCY_OPEN_AIR, "the rest is untouched");
}

#[test]
fn the_square_the_player_stands_on_is_always_visible() {
    // The third rule, applied last (`lightmap.cpp:280-283`): a player standing on frosted glass sees the square they are on.
    let mut facts = level(|x, y| x == 2 && y == 2);
    facts.player = Some((2, 2));
    let vision = vision_transparency_of(&facts);
    assert_close(
        vision[2 * SIDE + 2],
        TRANSPARENCY_OPEN_AIR,
        "forced back to open air",
    );
}

#[test]
fn crouching_behind_a_high_obstacle_blocks_the_view_past_it() {
    // The second rule: `coverage( loc ) >= 30`, and the threshold is inclusive. Only the eight neighbours of the player are considered - the loop is `points_in_radius( p, 1 )`
    // - so this is a check for a low wall, not a sight line.
    let mut facts = level(|_, _| false);
    facts.player = Some((2, 2));
    facts.hiding = true;
    facts.coverage[2 * SIDE + 1] = COVERAGE_TO_HIDE_BEHIND;
    let vision = vision_transparency_of(&facts);
    assert_close(
        vision[2 * SIDE + 1],
        0.0,
        "the square behind cover is opaque",
    );
    assert_close(
        vision[2 * SIDE],
        TRANSPARENCY_OPEN_AIR,
        "and the square further along that row, which is not adjacent, is not",
    );
}

#[test]
fn a_square_with_just_under_the_coverage_is_not_cover() {
    // Twenty-nine is not thirty. The boundary is the interesting part of a threshold, so it is pinned.
    let mut facts = level(|_, _| false);
    facts.player = Some((2, 2));
    facts.hiding = true;
    facts.coverage[2 * SIDE + 1] = COVERAGE_TO_HIDE_BEHIND - 1;
    assert_close(
        vision_transparency_of(&facts)[2 * SIDE + 1],
        TRANSPARENCY_OPEN_AIR,
        "not quite tall enough to hide behind",
    );
}

#[test]
fn standing_up_or_being_on_another_level_leaves_the_view_alone() {
    // The crouch rule needs both the player's level and `hiding`. A tall obstacle next to a standing player is cover for nothing.
    let mut standing = level(|_, _| false);
    standing.player = Some((2, 2));
    standing.coverage[2 * SIDE + 1] = 100;
    assert_close(
        vision_transparency_of(&standing)[2 * SIDE + 1],
        TRANSPARENCY_OPEN_AIR,
        "standing, so no cover",
    );

    let mut elsewhere = level(|_, _| false);
    elsewhere.player = None;
    elsewhere.hiding = true;
    elsewhere.coverage[2 * SIDE + 1] = 100;
    assert_close(
        vision_transparency_of(&elsewhere)[2 * SIDE + 1],
        TRANSPARENCY_OPEN_AIR,
        "and the player is not on this level",
    );
}

#[test]
fn the_player_square_is_exempt_even_when_they_are_hiding_in_the_open() {
    // The order of the exceptions matters: the crouch rule runs first and covers the eight neighbours, and the player's own square is restored afterwards. A player crouching
    // on top of something is still able to see the square they are on.
    let mut facts = level(|_, _| false);
    facts.player = Some((2, 2));
    facts.hiding = true;
    facts.coverage[2 * SIDE + 2] = 100;
    assert_close(
        vision_transparency_of(&facts)[2 * SIDE + 2],
        TRANSPARENCY_OPEN_AIR,
        "always visible where you stand",
    );
}
