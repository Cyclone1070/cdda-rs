//! What one square can see of another on the same level.
//!
//! Source: `map::sees` (`src/map.cpp:7329-7362`), the same-level branch. The rule is a
//! Bresenham walk that stops **before** checking the square it is looking at, so a wall is
//! still visible as a wall - it just does not let anything behind it be seen.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::vision::{View, lit_level_at, sees_on_one_level};

const SIDE: i32 = 12;

/// A room of the given side with the listed squares opaque.
fn room(walls: &[(i32, i32)]) -> impl Fn(i32, i32) -> bool + '_ {
    move |x, y| !walls.contains(&(x, y))
}

fn in_bounds(x: i32, y: i32) -> bool {
    (0..SIDE).contains(&x) && (0..SIDE).contains(&y)
}

#[test]
fn an_open_room_can_be_seen_across() {
    let transparent = room(&[]);
    assert!(sees_on_one_level(
        (1, 1),
        (9, 9),
        60,
        transparent,
        in_bounds
    ));
}

#[test]
fn a_wall_in_the_way_stops_the_sight() {
    let transparent = room(&[(5, 5)]);
    assert!(!sees_on_one_level(
        (1, 1),
        (9, 9),
        60,
        transparent,
        in_bounds
    ));
}

#[test]
fn the_square_being_looked_at_is_seen_even_when_it_is_the_wall() {
    // The reference exits the walk before testing the last square, and says so in a comment: "Exit before checking the last square, it's still visible even if
    // opaque" (`map.cpp:7345-7347`). So a wall is seen as a wall; only what is **behind** it is not.
    let transparent = room(&[(9, 9)]);
    assert!(sees_on_one_level(
        (1, 1),
        (9, 9),
        60,
        transparent,
        in_bounds
    ));
}

#[test]
fn nothing_is_seen_past_the_range() {
    let transparent = room(&[]);
    assert!(!sees_on_one_level(
        (1, 1),
        (9, 9),
        5,
        &transparent,
        in_bounds
    ));
    assert!(
        sees_on_one_level((1, 1), (5, 5), 5, &transparent, in_bounds),
        "and exactly at the range it is still seen"
    );
}

#[test]
fn nothing_off_the_level_is_seen() {
    let transparent = room(&[]);
    assert!(!sees_on_one_level(
        (1, 1),
        (20, 5),
        60,
        transparent,
        in_bounds
    ));
}

#[test]
fn a_diagonal_through_a_corner_squeezes_between_the_two_walls() {
    // The walk steps through one of the two cells that touch the corner, and which one is the Bresenham tie-break. This pins the answer rather than the intent: the
    // reference's line from (0,0) to (2,2) takes (1,1) first, so a wall pair at (1,0) and (0,1) leaves the diagonal open.
    let transparent = room(&[(1, 0), (0, 1)]);
    assert!(
        sees_on_one_level((0, 0), (2, 2), 60, transparent, in_bounds),
        "the diagonal passes through (1,1), which is open"
    );

    // And a wall on the diagonal itself closes it.
    let blocked = room(&[(1, 1)]);
    assert!(!sees_on_one_level((0, 0), (2, 2), 60, blocked, in_bounds));
}

#[test]
fn a_square_seen_through_fog_is_bright_without_detail() {
    // `apparent_light_at` (`lightmap.cpp:826-833`): a square whose own `seen_cache` value is below solid - so the light got through only as haze - is `BRIGHT_ONLY` when the
    // light behind it is bright, and `BLANK` otherwise. The distinction exists so the draw path can show a glow without showing what is in it.
    use cdda_core_types::light::LitLevel;

    assert_eq!(
        lit_level_at(20.0, true, 12.0, 4.0, false),
        LitLevel::BrightOnly
    );
    assert_eq!(lit_level_at(5.0, true, 12.0, 4.0, false), LitLevel::Blank);
}

#[test]
fn the_unobstructed_chain_is_bright_then_lit_then_low_then_nothing() {
    // The remaining branches, in the reference's order (`lightmap.cpp:836-850`): bright above `LIGHT_SOURCE_BRIGHT`, then `LIT`, then `LOW` at or above the vision threshold,
    // then `BLANK`.
    use cdda_core_types::light::LitLevel;

    assert_eq!(lit_level_at(50.0, false, 12.0, 4.0, false), LitLevel::Bright);
    assert_eq!(
        lit_level_at(10.0, false, 12.0, 4.0, true),
        LitLevel::Bright,
        "a square with items on it is bright to the player who can see them"
    );
    assert_eq!(lit_level_at(10.0, false, 12.0, 4.0, false), LitLevel::Low);
    assert_eq!(lit_level_at(4.0, false, 12.0, 4.0, false), LitLevel::Low);
    assert_eq!(lit_level_at(3.9, false, 12.0, 4.0, false), LitLevel::Blank);
}

#[test]
fn the_lit_branch_is_unreachable_because_two_thresholds_are_the_same_number() {
    // `LIGHT_SOURCE_BRIGHT` and `LIGHT_AMBIENT_LIT` are both ten, so the `> LIGHT_SOURCE_BRIGHT` test above has already claimed every square the `> LIGHT_AMBIENT_LIT` test
    // could find. `LIT` is dead code in the reference rather than a level the port forgot, and pinning it keeps the next reader from "fixing" the chain into disagreement.
    use cdda_core_types::light::{AMBIENT_LIT, LitLevel, SOURCE_BRIGHT};

    assert!((AMBIENT_LIT - SOURCE_BRIGHT).abs() < f32::EPSILON);
    assert_ne!(lit_level_at(15.0, false, 12.0, 4.0, false), LitLevel::Lit);
}

/// A level of one submap where every square is open ground.
fn open_level() -> cdda_core_types::level::Level {
    let mut level = cdda_core_types::level::Level::new();
    level.ensure(0, 0, 1);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, 1, 1);
        }
    }
    level
}

/// The sight rule the walls in these tests obey: code 2 stops the sight and absorbs all of it, everything else is open air.
fn walls_are_opaque(view: &mut View) {
    view.blocks_light = |code| code == 2;
    view.transparency_of = |code| {
        if code == 2 {
            0.0
        } else {
            cdda_core_types::light::TRANSPARENCY_OPEN_AIR
        }
    };
}

/// A character standing in the open at (5, 5), with a threshold of three.
fn view() -> View {
    View {
        origin: (5, 5),
        radius: 10,
        blocks_light: |_code| false,
        transparency_of: |_code| cdda_core_types::light::TRANSPARENCY_OPEN_AIR,
        ground_light_level: 10.0,
        vision_threshold: 3.0,
    }
}

/// Light of the given strength on one square.
fn lit_square(square: (i32, i32), value: f32) -> std::collections::BTreeMap<(i32, i32), cdda_core_types::shadowcasting::FourQuadrants> {
    let mut light = std::collections::BTreeMap::new();
    light.insert(
        square,
        cdda_core_types::shadowcasting::FourQuadrants::uniform(value),
    );
    light
}

#[test]
fn a_square_in_sight_and_above_the_threshold_is_shown() {
    // `pl_sees` (`map.cpp:854-871`) joins the cast with the light through the character's own vision threshold: in sight, and bright enough to see by. The old port asked
    // only "is there any light at all", which showed a corridor the character could not actually make out.
    use cdda_core_types::vision::shown_at;

    let mut level = open_level();
    let light = lit_square((5, 5), 20.0);
    assert!(shown_at(&mut level, &view(), &light, (5, 5)));
}

#[test]
fn a_square_in_sight_but_below_the_threshold_is_not_shown() {
    // The rule the port was missing. A dim glow of 0.5 is light, and it is not enough: below the character's threshold the square is `BLANK` and nothing is drawn there.
    use cdda_core_types::vision::shown_at;

    let mut level = open_level();
    let light = lit_square((5, 5), 0.5);
    assert!(
        !shown_at(&mut level, &view(), &light, (5, 5)),
        "half a unit of light is not something to see by"
    );
}

#[test]
fn a_square_out_of_sight_is_not_shown_however_bright_it_is() {
    // The other half of the conjunction: the light behind a wall is not a thing the character sees.
    use cdda_core_types::vision::shown_at;

    let mut level = open_level();
    for y in 0..12 {
        level.set_terrain(6, y, 2, 1);
    }
    let light = lit_square((8, 5), 40.0);
    let mut blocked = view();
    walls_are_opaque(&mut blocked);
    assert!(!shown_at(&mut level, &blocked, &light, (8, 5)));
}

/// A block of opaque squares, with the player standing east of it.
fn blocked_room() -> cdda_core_types::level::Level {
    let mut level = open_level();
    for square in [(4, 4), (4, 5), (4, 6), (5, 4), (5, 5), (5, 6)] {
        level.set_terrain(square.0, square.1, 2, 1);
    }
    level
}

fn four_way(north_east: f32, south_east: f32, south_west: f32, north_west: f32) -> cdda_core_types::shadowcasting::FourQuadrants {
    use cdda_core_types::shadowcasting::{FourQuadrants, Quadrant};
    let mut quarters = FourQuadrants::uniform(0.0);
    quarters.set(Quadrant::NorthEast, north_east);
    quarters.set(Quadrant::SouthEast, south_east);
    quarters.set(Quadrant::SouthWest, south_west);
    quarters.set(Quadrant::NorthWest, north_west);
    quarters
}

#[test]
fn the_face_of_a_wall_only_counts_the_quarters_the_player_can_see_it_from() {
    // `apparent_light_helper`'s complicated branch (`lightmap.cpp:751-797`): for an **opaque** square the reference counts only the quarters that face a visible, non-opaque
    // neighbour, so light arriving from behind the wall does not brighten the face the player is looking at. The port took the brightest quarter of all four and would read the
    // hundred here; the reference reads the five.
    use cdda_core_types::vision::apparent_light_at_seen;

    let mut level = blocked_room();
    let mut light = std::collections::BTreeMap::new();
    // Bright from the north-west, dim from the two quarters the player can see from the east.
    light.insert((5, 5), four_way(5.0, 1.0, 1.0, 100.0));
    let mut east = view();
    east.origin = (8, 5);
    walls_are_opaque(&mut east);

    // Five, dimmed a little by the sight cast: the quarter in front of the player. Counting the north-west quarter as well would answer about ninety-six, and the test's band is
    // narrow enough that no arithmetic slip can hide inside it.
    let apparent = apparent_light_at_seen(&mut level, &east, &light, (5, 5));
    assert!(
        apparent > 4.0 && apparent < 5.5,
        "the wall's face, not its back: {apparent}"
    );
}

#[test]
fn a_floor_square_counts_the_brightest_quarter_whichever_way_it_points() {
    // The simple branch (`lightmap.cpp:792-796`): for a non-opaque square light from all directions is equivalent, so the brightest quarter is the answer. The wall rule above must
    // not leak into open ground, or a room would be lit only where the nearest lamp happens to face.
    use cdda_core_types::vision::apparent_light_at_seen;

    let mut level = open_level();
    let mut light = std::collections::BTreeMap::new();
    light.insert((3, 5), four_way(1.0, 1.0, 1.0, 100.0));
    let mut east = view();
    east.origin = (8, 5);

    // The brightest quarter, and then the sight cast's own fraction of it: five squares of open air is not the same as standing on it. A quarter rule that kept only the quarters
    // facing the player would answer one instead, which is what the wall test beside this one pins.
    let apparent = apparent_light_at_seen(&mut level, &east, &light, (3, 5));
    assert!(
        apparent > 50.0 && apparent < 100.0,
        "the brightest quarter, dimmed by the distance: {apparent}"
    );
}

#[test]
fn a_wall_the_character_sees_is_drawn_bright_rather_than_hazy() {
    // The reference distinguishes "seen" from "seen through haze" with the seen cache's own **number**: `obstructed = vis <= LIGHT_TRANSPARENCY_SOLID + 0.1`
    // (`lightmap.cpp:733`). A wall the character is looking at is seen, so its visibility is high, so it is not obstructed and is drawn `Bright`.
    //
    // The port's visibility is a bool, so `BrightOnly` is **unreachable through the world** until the seen cache carries a fraction - the `bright_without_detail` unit
    // test above pins the branch, and this one pins the boundary the port actually sits on. Declared as a gap rather than papered over: the frame will be wrong on fogged
    // squares until the seen cache is a float, and nothing else on the frame depends on it.
    use cdda_core_types::light::LitLevel;
    use cdda_core_types::vision::lit_level_at_square;

    let mut level = blocked_room();
    let mut light = std::collections::BTreeMap::new();
    light.insert((5, 5), four_way(200.0, 200.0, 200.0, 200.0));
    let mut east = view();
    east.origin = (8, 5);
    walls_are_opaque(&mut east);

    assert_eq!(
        lit_level_at_square(&mut level, &east, &light, (5, 5)),
        LitLevel::Bright,
        "a wall in plain sight is a wall, not a glow"
    );
}

#[test]
fn the_two_questions_a_frame_asks_agree() {
    // `shown_at` is `lit_level_at_square != Blank` and nothing else, so the draw loop's "skip blank squares" and its "how bright is this" can never disagree.
    use cdda_core_types::light::LitLevel;
    use cdda_core_types::vision::{lit_level_at_square, shown_at};

    let mut level = open_level();
    let mut light = std::collections::BTreeMap::new();
    light.insert((6, 5), four_way(40.0, 40.0, 40.0, 40.0));
    light.insert((3, 5), four_way(0.2, 0.2, 0.2, 0.2));
    let east = view();
    for square in [(6, 5), (3, 5), (9, 9)] {
        let level_at = lit_level_at_square(&mut level, &east, &light, square);
        assert_eq!(
            shown_at(&mut level, &east, &light, square),
            level_at != LitLevel::Blank,
            "at {square:?}: {level_at:?}"
        );
    }
}

#[test]
fn a_square_seen_through_smoke_glows_without_showing_what_is_on_it() {
    // The hazy branch, reached through the world at last. `cast_seen_light` attenuates by the transparency of the squares a ray crosses, and smoke's is enormous (ten at its
    // second intensity, against open air's two hundredths), so a square behind it is seen at a fraction below the tenth that `obstructed` asks for
    // (`lightmap.cpp:733`). What the player sees is a glow: `BRIGHT_ONLY`.
    use cdda_core_types::light::{LitLevel, TRANSPARENCY_OPEN_AIR};
    use cdda_core_types::vision::lit_level_at_square;

    const SMOKY: u32 = 3;
    let mut level = open_level();
    // A skin of smoke between the character and the square behind it, one square thick.
    for y in 0..12 {
        level.set_terrain(6, y, SMOKY, 1);
    }
    let mut light = std::collections::BTreeMap::new();
    light.insert((7, 5), four_way(200.0, 200.0, 200.0, 200.0));

    let mut smoking = view();
    smoking.transparency_of = |code| {
        if code == SMOKY {
            10.0
        } else {
            TRANSPARENCY_OPEN_AIR
        }
    };

    // The sight cast measures 0.078 there - below the tenth - while the square *past* the smoke recovers to 0.43, because the reference's accumulator averages the transparency
    // across a whole octant row: one column of smoke dims its neighbour most and is diluted by the open air beside it. A quirk of the reference's arithmetic, reproduced rather
    // than corrected, and the reason this test looks at the square immediately behind the smoke.
    assert_eq!(
        lit_level_at_square(&mut level, &smoking, &light, (7, 5)),
        LitLevel::BrightOnly,
        "a glow in the smoke: the light got there, the detail did not"
    );
}
