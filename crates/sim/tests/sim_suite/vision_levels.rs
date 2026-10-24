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
    ///
    /// Read by the comparison a later round makes, and pinned here so the dump's other half cannot be dropped without a gate noticing.
    pub transparency: f64,
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
fn the_character_sees_a_field_around_it_and_two_squares_below_it() {
    // **The fact the fixture exists for.** A character standing in the fixed world's start location sees its own level, sees **nothing above** - it is under a
    // roof - and sees two squares of the level below through an opening. A cast that never left its own level would report the first and miss the other two,
    // which is exactly the gap the port's per-level cast has today.
    let (squares, (_, _, cz)) = squares();
    let visible = |z: i32| {
        squares
            .iter()
            .filter(|square| square.z == z && square.seen > 0.0)
            .count()
    };
    let mine = visible(cz);
    assert!(mine > 150, "the character's own level is mostly visible: {mine}");
    assert_eq!(visible(cz + 1), 0, "a roofed level sees nothing of the sky above");
    let below: Vec<(i32, i32)> = squares
        .iter()
        .filter(|square| square.z == cz - 1 && square.seen > 0.0)
        .map(|square| (square.x, square.y))
        .collect();
    assert_eq!(
        below.len(),
        2,
        "two squares of the level below are visible through the opening: {below:?}"
    );
    // And they are seen *dimly*, because the cast crosses two levels rather than one: the reference's `sight_calc` divides by the distance, so a square below is
    // never as bright as one beside.
    let dimmest_beside = squares
        .iter()
        .filter(|square| square.z == cz && square.seen > 0.0)
        .map(|square| square.seen)
        .fold(1.0_f64, f64::min);
    let brightest_below = squares
        .iter()
        .filter(|square| square.z == cz - 1)
        .map(|square| square.seen)
        .fold(0.0_f64, f64::max);
    assert!(
        brightest_below < dimmest_beside.max(1.0),
        "the squares below are dimmer than a square beside: {brightest_below} against {dimmest_beside}"
    );
}
