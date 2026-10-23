//! What the character can see: the map's `seen_cache` in miniature.
//!
//! Reference: `map::seen` over the cache `map::build_seen_cache` fills (`src/lightmap.cpp:1039-1100`).

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::level::Level;

const OPEN: u32 = 1;
const WALL: u32 = 2;

fn level() -> Level {
    let mut level = Level::new();
    level.ensure(0, 0, OPEN);
    level
}

fn blocks(terrain: u32) -> bool {
    terrain == WALL
}

#[test]
fn an_open_square_next_to_the_origin_is_seen() {
    let mut level = level();
    assert!(level.seen_at((5, 5), 5, blocks, (5, 6)));
    assert!(level.seen_at((5, 5), 5, blocks, (6, 6)), "diagonals too");
}

#[test]
fn a_wall_hides_what_is_behind_it() {
    let mut level = level();
    for y in 0..12 {
        level.set_terrain(9, y, WALL, OPEN);
    }
    assert!(
        level.seen_at((5, 5), 5, blocks, (8, 5)),
        "the wall itself is seen"
    );
    assert!(
        !level.seen_at((5, 5), 5, blocks, (10, 5)),
        "the square behind it is not"
    );
}

#[test]
fn nothing_beyond_the_sight_range_is_seen() {
    let mut level = level();
    assert!(
        !level.seen_at((5, 5), 5, blocks, (11, 5)),
        "six squares out of five"
    );
    assert!(
        !level.seen_at((5, 5), 5, blocks, (5, 40)),
        "and far away is definitely not"
    );
}

#[test]
fn the_square_the_character_stands_on_is_always_seen() {
    // `build_seen_cache` writes `VISIBILITY_FULL` to the origin before the cast runs (`lightmap.cpp:1057-1059`), so the character's own square is seen even inside a sealed
    // room. Without it the square under the player's feet is drawn blank, which is the kind of bug that only shows on a screen.
    let mut level = level();
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, WALL, OPEN);
        }
    }
    assert!(level.seen_at((5, 5), 5, blocks, (5, 5)));
}

#[test]
fn a_wall_you_look_at_is_seen_clearly_and_a_square_behind_fog_is_not() {
    // `cast_seen_light` (`lightmap.cpp:1039-1100`) starts every ray at `VISIBILITY_FULL` and attenuates it by the squares it crosses, so the answer is a fraction: a wall three
    // squares away is about nine tenths seen, and a square behind a fog that absorbs much more is a tenth. `obstructed` is that fraction at or below a tenth
    // (`lightmap.cpp:733`), which is the difference the draw path draws between a wall and a glow.
    use cdda_core_types::light::TRANSPARENCY_OPEN_AIR;

    let mut level = level();
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, OPEN, OPEN);
        }
    }
    let open_air = |_code: u32| TRANSPARENCY_OPEN_AIR;
    let wall = level.seen_through((5, 5), 10, open_air, (8, 5));
    assert!(
        wall > 0.5,
        "open ground three squares away is seen clearly: {wall}"
    );

    // Fog, as the weather data gives it: a sight penalty that multiplies every square's transparency by 1.7.
    let fogged = |_code: u32| TRANSPARENCY_OPEN_AIR * 1.7;
    let behind_fog = level.seen_through((5, 5), 10, fogged, (8, 5));
    assert!(
        behind_fog < wall,
        "fog dims what is seen: {behind_fog} against {wall}"
    );
}

#[test]
fn one_cast_answers_many_squares() {
    // `build_seen_cache` builds the level's sight cache **once** and every query reads it (`lightmap.cpp:1039-1100`); the reference's draw loop asks about 15,000 squares a frame.
    // The port rebuilt the cast per question, which is the same answer at a hundred times the work. The receipt is the level's own count of casts.
    use cdda_core_types::light::TRANSPARENCY_OPEN_AIR;

    let mut level = level();
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, OPEN, OPEN);
        }
    }
    let before = level.sight_casts();
    let grid = level.seen_grid((5, 5), 6, |_code| TRANSPARENCY_OPEN_AIR);
    let mut answered = 0;
    for y in 0..12 {
        for x in 0..12 {
            let _ = grid.at((x, y));
            answered += 1;
        }
    }
    assert_eq!(answered, 144);
    assert_eq!(
        level.sight_casts() - before,
        1,
        "one cast, not one per square"
    );
}

#[test]
fn an_open_corridor_is_seen_by_the_attenuation_it_crosses() {
    // The reference's own arithmetic, worked out here rather than asked of the port: `sight_calc` is `numerator / e^( transparency * distance )` with a numerator of one
    // (`shadowcasting.h:96-99`), and along a straight line of open air the accumulated average is exactly the open-air value. So a square `d` along an open corridor is seen
    // at `exp( -OPEN_AIR * d )`, and any disagreement is a real one.
    use cdda_core_types::light::TRANSPARENCY_OPEN_AIR;

    let mut level = level();
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, OPEN, OPEN);
        }
    }
    let grid = level.seen_grid((5, 5), 6, |_code| TRANSPARENCY_OPEN_AIR);
    for distance in 1..=6 {
        let along = f32::from(i16::try_from(distance).unwrap_or(0));
        let expected = (-TRANSPARENCY_OPEN_AIR * along).exp();
        let answer = grid.at((5 + distance, 5));
        assert!(
            (answer - expected).abs() < 0.02,
            "{distance} squares along: {answer} against {expected}"
        );
    }
}

#[test]
fn a_wall_ends_the_cast_and_the_squares_beyond_it_are_not_seen() {
    use cdda_core_types::light::TRANSPARENCY_OPEN_AIR;

    let mut level = level();
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, OPEN, OPEN);
        }
    }
    for y in 0..12 {
        level.set_terrain(8, y, WALL, OPEN);
    }
    let grid = level.seen_grid(
        (5, 5),
        6,
        |code| {
            if code == WALL {
                0.0
            } else {
                TRANSPARENCY_OPEN_AIR
            }
        },
    );
    assert!(grid.at((8, 5)) > 0.0, "the wall itself is seen");
    assert!(grid.at((9, 5)) == 0.0, "nothing behind it");
    assert!(grid.at((11, 5)) == 0.0, "nor further along");
}
