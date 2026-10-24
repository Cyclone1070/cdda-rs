//! The light a fire throws, turn by turn, on four levels.
//!
//! The M2 exit gate asks for **light changes across z-levels**, and until this fixture existed no session carried light at all: the dumps said what the fields were
//! doing and nothing about what that made visible. `light-session.json` is a fire on one level, dumped every turn with `map::light_at` (`src/lightmap.cpp:674-697`)
//! and the square's own light source beside it, on the two levels under the fire and the two above.
//!
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]

//! This file holds the **fixture's** gate - the numbers a replay will have to reproduce - because the replay itself is the next slice. Pinning them first is what makes
//! the replay a port of a measured behaviour rather than a comparison of a port with itself.

use std::path::Path;

/// One square's light on one turn.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Lit {
    x: i32,
    y: i32,
    z: i32,
    /// `map::light_at`: 0 dark, 1 low, 2 bright-only, 3 lit, 4 bright.
    level: i32,
    /// The square's own light source, which is the field's contribution and not the sky's.
    source: i32,
    /// The ambient light the square receives, which the sky decides.
    ambient: f64,
}

fn sessions() -> Vec<Vec<Lit>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/fields/light-session.json");
    let text = std::fs::read_to_string(&path).expect("the recorded light session");
    let cdda_json::Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(turns)) = root.get("turns") else {
        panic!("turns")
    };
    turns
        .iter()
        .map(|turn| {
            let cdda_json::Value::Object(turn) = turn else {
                panic!("a turn")
            };
            let Some(cdda_json::Value::Array(light)) = turn.get("light") else {
                panic!("every turn carries its light")
            };
            light
                .iter()
                .map(|entry| {
                    let cdda_json::Value::Object(entry) = entry else {
                        panic!("a square")
                    };
                    let whole = |key: &str| match entry.get(key) {
                        Some(cdda_json::Value::Int(value)) => *value,
                        other => panic!("{key} is a whole number: {other:?}"),
                    };
                    let ambient = match entry.get("ambient") {
                        Some(cdda_json::Value::Float(value)) => *value,
                        other => panic!("ambient is a number: {other:?}"),
                    };
                    Lit {
                        x: i32::try_from(whole("x")).expect("a square's x"),
                        y: i32::try_from(whole("y")).expect("a square's y"),
                        z: i32::try_from(whole("z")).expect("a square's z"),
                        level: i32::try_from(whole("lit")).expect("a lit level"),
                        source: i32::try_from(whole("source")).expect("a source"),
                        ambient,
                    }
                })
                .collect()
        })
        .collect()
}

fn at(turn: &[Lit], x: i32, y: i32, z: i32) -> Lit {
    *turn
        .iter()
        .find(|square| (square.x, square.y, square.z) == (x, y, z))
        .unwrap_or_else(|| panic!("({x}, {y}, {z}) is in the window"))
}

#[test]
fn the_session_carries_light_on_four_levels_every_turn() {
    let sessions = sessions();
    assert_eq!(sessions.len(), 7, "seven turns: the state before the first sweep and six after");
    for turn in &sessions {
        assert_eq!(turn.len(), 100, "twenty-five squares on each of four levels");
        let levels: std::collections::BTreeSet<i32> = turn.iter().map(|square| square.z).collect();
        assert_eq!(
            levels,
            [0, 1, 2, -1].into_iter().collect::<std::collections::BTreeSet<i32>>(),
            "the fire's level, the two above it and the one below"
        );
    }
}

#[test]
fn a_fire_is_bright_on_its_own_square_and_never_reaches_the_level_below() {
    // **The same rule the sight cast found, seen from the light's side**: a floor above a square below the fire is a ceiling from underneath, so the fire lights its own
    // level and the level above it through the openings, and the level below stays at LOW with a source of zero. A port that lit every square within reach on every
    // level would pass a shape test and fail this one.
    let sessions = sessions();
    let turn = &sessions[1];
    let fire = at(turn, 63, 71, 0);
    assert_eq!(fire.level, 4, "the fire's own square is BRIGHT");
    assert_eq!(fire.source, 160, "and its source is intensity three's 160");

    let below = at(turn, 63, 71, -1);
    assert_eq!(below.source, 0, "nothing below the floor emits light");
    assert_eq!(below.level, 1, "and the square reads LOW, which is the ambient of a cellar");
    assert!(
        below.ambient < 4.0,
        "the cellar's ambient ({}) is the sky's, not the fire's",
        below.ambient
    );

    let beside = at(turn, 64, 71, 0);
    assert_eq!(beside.source, 0, "a neighbour has no source of its own");
    assert!(
        beside.level >= 1,
        "but the fire's rays reach it: {} is not dark",
        beside.level
    );
    assert!(beside.level < fire.level, "and it is dimmer than the fire itself");
}

#[test]
fn the_fire_grows_and_the_light_grows_with_it() {
    // A session whose light never changes would pass the shape gate and cover nothing. The fire spreads on the third turn; the number of lights with a source of their
    // own goes up with it, which is the change the replay has to reproduce.
    let sessions = sessions();
    let first: usize = sessions[0].iter().filter(|square| square.source > 0).count();
    let last: usize = sessions[6].iter().filter(|square| square.source > 0).count();
    assert!(
        last > first,
        "the fire spread: {first} lit squares at the start, {last} at the end"
    );
}
