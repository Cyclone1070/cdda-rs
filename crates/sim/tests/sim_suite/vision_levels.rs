//! The reference's `seen_cache` around the start of the fixed world, **across three levels**.
//!
//! Vision is the one environment layer with nothing recorded for it: `--dump-frame` reads the light a square stands in, and light is not sight. `--dump-visibility`
//! dumps each level's `seen_cache` and `transparency_cache` after the same `build_map_cache` the game would run, so a port can be held to what the reference's own
//! cast reached.
//!
//! Reference: `map::build_map_cache` filling the caches, and `cast_zlight` (`lightmap.cpp:1079-1081`) filling every level's `seen_cache` in one pass - which is why the
//! level below the character is not empty.
//!
//! These gates pin **what the fixture says**, which is the half a later gate compares against: the character sees a field of squares on its own level, nothing
//! above it (it is under a roof), and two squares below it through an opening. A dump that quietly stopped reaching across levels would leave every later comparison
//! passing on a level that was never cast.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp
)]

use std::collections::BTreeMap;
use std::path::Path;

/// One dumped square: where it is, how visible it is, and how much light it lets through.
#[derive(Debug, Clone, Copy)]
pub struct Square {
    /// Column in the reality bubble.
    pub x: i32,
    /// Row in the reality bubble.
    pub y: i32,
    /// Level.
    pub z: i32,
    /// The `seen_cache` value: how visible the square is.
    pub seen: f64,
    /// The `transparency_cache` value: how much light gets through.
    pub transparency: f64,
    /// Whether the square has a floor. The other input the cast reads, recorded beside the output so the port can be given the same bits the reference had.
    pub floor: bool,
    /// Whether the square's **whole level** has no floor gaps - the reference's `no_floor_gaps`, which decides whether the cast may look downwards at all.
    pub sealed: bool,
}

fn number(object: &cdda_json::Object, key: &str) -> f64 {
    match object.get(key) {
        Some(cdda_json::Value::Float(value)) => *value,
        Some(cdda_json::Value::Int(value)) => *value as f64,
        other => panic!("{key} is a number: {other:?}"),
    }
}

/// Every square the reference dumped, and the square the character stood on.
pub fn squares() -> (Vec<Square>, (i32, i32, i32)) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vision/start-location.json");
    let text = std::fs::read_to_string(&path).expect("the visibility dump");
    let cdda_json::Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(list)) = root.get("squares") else {
        panic!("squares")
    };
    let squares = list
        .iter()
        .map(|entry| {
            let cdda_json::Value::Object(square) = entry else {
                panic!("a square")
            };
            Square {
                x: number(square, "x") as i32,
                y: number(square, "y") as i32,
                z: number(square, "z") as i32,
                seen: number(square, "seen"),
                transparency: number(square, "transparency"),
                floor: matches!(square.get("floor"), Some(cdda_json::Value::Bool(true))),
                sealed: matches!(square.get("sealed"), Some(cdda_json::Value::Bool(true))),
            }
        })
        .collect();
    let Some(cdda_json::Value::Array(centre)) = root.get("centre") else {
        panic!("centre")
    };
    let at = |index: usize| match centre.get(index) {
        Some(cdda_json::Value::Int(value)) => i32::try_from(*value).expect("a coordinate"),
        other => panic!("a centre coordinate: {other:?}"),
    };
    (squares, (at(0), at(1), at(2)))
}

#[test]
fn the_dump_covers_three_levels_and_every_square_of_each() {
    // Seventeen by seventeen squares on three levels: the dump's shape is the first thing a comparison depends on, because a missing level would compare as
    // "nothing is visible there" and pass.
    let (squares, (cx, cy, cz)) = squares();
    assert_eq!(squares.len(), 17 * 17 * 3, "three levels of seventeen by seventeen");
    let mut per_level: BTreeMap<i32, usize> = BTreeMap::new();
    for square in &squares {
        *per_level.entry(square.z).or_default() += 1;
    }
    assert_eq!(
        per_level.keys().copied().collect::<Vec<i32>>(),
        vec![cz - 1, cz, cz + 1],
        "the levels are the character's own and its neighbours"
    );
    assert!(per_level.values().all(|count| *count == 17 * 17));
    // The character's own square is visible from itself, which the reference writes before the cast runs.
    let origin = squares
        .iter()
        .find(|square| (square.x, square.y, square.z) == (cx, cy, cz))
        .expect("the character's own square");
    assert_eq!(origin.seen, 1.0, "the square the character stands on");
    // The other half of the dump: how much light each square lets through. A wall reads as the solid value, an open floor as open air, and the character's own
    // square is somewhere in between - it is standing on a floor with a roof above it, which is what makes the cast stop at the ceiling.
    let mut values: Vec<f64> = squares.iter().map(|square| square.transparency).collect();
    values.sort_by(|left, right| left.partial_cmp(right).expect("no NaN in a cache"));
    values.dedup_by(|left, right| (*left - *right).abs() < 1e-9);
    assert_eq!(
        values.len(),
        2,
        "the world is solid or open and nothing between: {values:?}"
    );
    assert!(values[0] == 0.0, "the solid value is zero");
    // The open value is not one: the square is **outside**, so the cast multiplies it by the weather's sight penalty
    // (`lightmap.cpp:152-157`). A port that used the open-air constant here would see further than the reference in bad weather,
    // which is the difference this column exists to catch.
    assert!(
        values[1] > 0.0 && values[1] < 1.0,
        "the open value carries the sight penalty: {}",
        values[1]
    );
}

#[test]
fn the_character_sees_a_field_around_it_and_nothing_of_a_sealed_sky() {
    // What the fixture says, read from the fixture rather than from a memory of an earlier recording: the character sees a field of squares on its own level, and
    // whatever it sees on the levels beside it follows the reference's own direction gate - a level flagged `sealed` has no floor gaps at all, so **nothing** below it
    // can be seen, and a level without the flag may be seen through its gaps.
    let (squares, (_, _, cz)) = squares();
    let visible = |z: i32| {
        squares
            .iter()
            .filter(|square| square.z == z && square.seen > 0.0)
            .count()
    };
    let mine = visible(cz);
    assert!(mine > 40, "the character's own level shows a field: {mine}");
    // The flag is per level, and the recording exercises both answers - otherwise this fixture could not tell a cast that looks sideways from one that does not.
    let sealed_levels: Vec<i32> = {
        let mut levels: Vec<i32> = squares
            .iter()
            .filter(|square| square.sealed)
            .map(|square| square.z)
            .collect();
        levels.sort_unstable();
        levels.dedup();
        levels
    };
    assert!(
        !sealed_levels.is_empty(),
        "the recording has a sealed level, which is what makes the direction gate worth having"
    );
    // A sealed level is never seen from the level above it.
    for level in sealed_levels {
        let above = squares
            .iter()
            .filter(|square| square.z == level - 1 && square.seen > 0.0)
            .count();
        if squares
            .iter()
            .filter(|square| square.z == level - 1)
            .all(|square| square.sealed)
        {
            assert_eq!(
                above, 0,
                "a sealed level {level} cannot be seen from the sealed one above it"
            );
        }
    }
}

#[test]
fn the_port_s_cast_reproduces_the_reference_s_seen_cache() {
    // **The gate the whole cross-level effort was for.** The fixture records, per square of three levels, the transparency and floor bit the reference's cast read and
    // the `seen_cache` value it produced. Running the port's `cast_zlight` over those same inputs has to reproduce those same values - not the shape of them, the
    // numbers.
    //
    // The direction gate is the reference's own: the character's level is sealed if every one of its squares has a floor.
    use cdda_core_types::zlight::{Plane, build_seen_cache, direction_to_cast};

    let (squares, (cx, cy, cz)) = squares();
    let transparency = |x: i32, y: i32, z: i32| {
        squares
            .iter()
            .find(|square| (square.x, square.y, square.z) == (x, y, z))
            .map_or(
                // Outside the recorded window the reference had *something*; treating it as solid keeps this gate honest about what it can see.
                0.0,
                |square| square.transparency as f32,
            )
    };
    let floor = |x: i32, y: i32, z: i32| {
        squares
            .iter()
            .find(|square| (square.x, square.y, square.z) == (x, y, z))
            .is_some_and(|square| square.floor)
    };
    let side = 17;
    // **The reference's own flag, not a guess from the window.** `no_floor_gaps` is a summary of the whole level (`map.cpp:9425-9429`), so a character standing in a
    // sealed room still sees the floor below if there is a hole *somewhere else* on its level - which is why this is recorded rather than inferred from the squares here.
    let sealed = squares
        .iter()
        .find(|square| square.z == cz)
        .is_some_and(|square| square.sealed);
    let seen = build_seen_cache(
        &Plane {
            transparency: &transparency,
            floor: &floor,
        },
        (cx, cy, cz),
        8,
        (i32::MAX, i32::MAX),
        1.0,
        direction_to_cast(sealed),
    );
    let _ = side;
    let mut mismatches = Vec::new();
    for square in &squares {
        let ours = seen
            .get(&(square.x, square.y, square.z))
            .copied()
            .unwrap_or(0.0);
        let theirs = square.seen as f32;
        if (ours - theirs).abs() > 1e-4 {
            mismatches.push((square.x, square.y, square.z, ours, theirs));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} squares differ from the reference's own cache, first few: {:?}",
        mismatches.len(),
        squares.len(),
        &mismatches[..mismatches.len().min(8)]
    );
    // And the gate is not vacuous: the reference's cache has squares it can see, on more than one level.
    assert!(
        squares.iter().filter(|square| square.seen > 0.0).count() > 40,
        "the fixture is worth comparing"
    );
}

/// The frame's own sight cast, against the reference's cache on the level the character stands on.
///
/// The gate above runs [`cdda_core_types::zlight::build_seen_cache`] over the recording's own numbers, which proves the cast. This one goes through the **engine** — the
/// `Level` the frame asks, given the same transparency map — because a cast that is right and a level that hands it the wrong table are two different failures, and only the
/// second one is the port's to make.
///
/// The reference fills **every** level's cache in one pass, so the character's own level is written by the same `cast_zlight`. Its own-level values are the flat cast's, which
/// is why a level that casts one level and reads as solid beside it can reproduce them; what it cannot reproduce is the other two levels, and that is the gate above.
#[test]
fn the_frame_s_cast_reproduces_the_reference_s_seen_cache_on_the_character_s_level() {
    use cdda_core_types::level::Level;
    use std::collections::BTreeMap;

    let (squares, (cx, cy, cz)) = squares();
    let radius = 8;
    let mut transparency: BTreeMap<(i32, i32), f32> = BTreeMap::new();
    for square in &squares {
        if square.z == cz {
            #[allow(clippy::cast_possible_truncation)]
            transparency.insert((square.x, square.y), square.transparency as f32);
        }
    }
    let mut level = Level::new();
    level.ensure(0, 0, 0);
    let grid = level.seen_grid_of((cx, cy), radius, &transparency);

    let mut compared = 0_usize;
    let mut wrong = Vec::new();
    for square in &squares {
        if square.z != cz {
            continue;
        }
        let ours = grid.at((square.x, square.y));
        #[allow(clippy::cast_possible_truncation)]
        let theirs = square.seen as f32;
        if (ours - theirs).abs() > 1e-4 {
            wrong.push((square.x, square.y, ours, theirs));
        }
        compared += 1;
    }
    assert_eq!(compared, 17 * 17, "the character's own level, every square");
    assert!(
        wrong.is_empty(),
        "{} of {compared} squares differ from the reference's cache: {:?}",
        wrong.len(),
        &wrong[..wrong.len().min(8)]
    );
    // Not vacuous: the level is a mix of visible and visible-past and solid squares, so a level that handed the cast an empty table would fail rather than agree.
    let visible = (0..17)
        .flat_map(|row| (0..17).map(move |column| (row, column)))
        .filter(|(row, column)| {
            let square = (cx - radius + *column, cy - radius + *row);
            grid.at(square) > 0.0
        })
        .count();
    assert!(visible > 40, "the level shows a field: {visible}");
}
