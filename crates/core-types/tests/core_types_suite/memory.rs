//! Map memory: what stays on the character's map after the light goes out.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::level::Level;
use cdda_core_types::memory::{MapMemory, RememberedLight, remember_seen};
use cdda_core_types::shadowcasting::FourQuadrants;
use std::collections::BTreeMap;

const OPEN: u32 = 1;
const WALL: u32 = 2;
const GRASS: u32 = 3;

fn level() -> Level {
    let mut level = Level::new();
    level.ensure(0, 0, OPEN);
    level
}

// A square the world has made, with one patch of grass to tell remembered terrain apart. The Option
// is what the memory API asks for: a square with no submap has no terrain to remember.
#[allow(clippy::unnecessary_wraps)]
fn terrain(x: i32, y: i32) -> Option<u32> {
    Some(if (x, y) == (5, 6) { GRASS } else { OPEN })
}

fn blocks(terrain: u32) -> bool {
    terrain == WALL
}

fn light(squares: &[(i32, i32)], luminance: f32) -> BTreeMap<(i32, i32), FourQuadrants> {
    squares
        .iter()
        .map(|square| (*square, FourQuadrants::uniform(luminance)))
        .collect()
}

#[test]
fn what_is_seen_is_remembered_with_its_terrain() {
    let mut level = level();
    let mut memory = MapMemory::new();
    let lit = light(&[(5, 5), (5, 6)], 30.0);
    let written = remember_seen(&mut memory, &mut level, (5, 5), 5, blocks, &lit, terrain);
    assert!(written > 0);
    // The origin is not lit by its own cast - the reference's shadowcasting does not light the
    // square it starts from - so the neighbour is the square to check.
    let here = memory.at((5, 6)).expect("the neighbour is remembered");
    assert_eq!(here.terrain, GRASS, "terrain travels with the memory");
    assert_eq!(here.light, RememberedLight::Lit);
}

#[test]
fn the_memory_survives_the_light_going_out_and_stays_dim() {
    let mut level = level();
    let mut memory = MapMemory::new();
    let lit = light(&[(5, 6)], 30.0);
    remember_seen(&mut memory, &mut level, (5, 5), 5, blocks, &lit, terrain);
    assert_eq!(
        memory.at((5, 6)).map(|r| r.light),
        Some(RememberedLight::Lit)
    );

    // Next turn there is no light at all: the square is still in sight, so it is remembered, but dim.
    let dark = BTreeMap::new();
    remember_seen(&mut memory, &mut level, (5, 5), 5, blocks, &dark, terrain);
    assert_eq!(
        memory.at((5, 6)).map(|r| r.light),
        Some(RememberedLight::Dim),
        "the square is remembered, and dark now"
    );
    assert_eq!(
        memory.at((5, 6)).map(|r| r.terrain),
        Some(GRASS),
        "with its terrain intact"
    );
}

#[test]
fn what_is_behind_a_wall_never_enters_the_memory() {
    let mut level = level();
    for y in 0..12 {
        level.set_terrain(9, y, WALL, OPEN);
    }
    let mut memory = MapMemory::new();
    let lit = light(&[(10, 5)], 30.0);
    remember_seen(&mut memory, &mut level, (5, 5), 5, blocks, &lit, terrain);
    assert!(memory.at((10, 5)).is_none(), "not seen, not remembered");
    assert!(memory.at((9, 5)).is_some(), "the wall itself is on the map");
}

#[test]
fn an_empty_memory_is_empty() {
    let memory = MapMemory::new();
    assert!(memory.is_empty());
    assert_eq!(memory.len(), 0);
    assert!(memory.at((0, 0)).is_none());
}

/// The tile the character keeps, against the reference's own recording of a scripted set of writes.
///
/// Reference: `map_memory::set_tile_terrain`, `set_tile_decoration`, `set_tile_symbol` and `clear_tile_decoration` (`src/map_memory.cpp:218-276`), and
/// `corpus/memory/tiles.json` for what the reference answered. The interesting column is the decoration: the clear takes a **prefix**, so clearing `vp_` leaves
/// `graffiti_hello` alone, and clearing with the empty prefix matches everything.
#[test]
#[allow(clippy::too_many_lines)]
fn a_remembered_tile_carries_its_terrain_its_decoration_and_its_symbol() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/memory/tiles.json");
    let text = std::fs::read_to_string(path).expect("the memory dump exists");
    let value = cdda_json::parse(text.as_bytes()).expect("the memory dump is valid JSON");
    let cdda_json::Value::Object(root) = value else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(squares)) = root.get("squares") else {
        panic!("squares")
    };
    let whole = |entry: &cdda_json::Value, key: &str| -> i64 {
        match entry {
            cdda_json::Value::Object(object) => match object.get(key) {
                Some(cdda_json::Value::Int(number)) => *number,
                other => panic!("{key} is a whole number: {other:?}"),
            },
            other => panic!("a square: {other:?}"),
        }
    };
    let text_of = |entry: &cdda_json::Value, key: &str| -> String {
        match entry {
            cdda_json::Value::Object(object) => match object.get(key) {
                Some(cdda_json::Value::Str(text)) => text.clone(),
                other => panic!("{key} is a string: {other:?}"),
            },
            other => panic!("a square: {other:?}"),
        }
    };
    // The same scripted sequence the dump runs, in the same order. A gate is only worth what it shares with the recording, so the writes are transcribed one for one
    // rather than generated from the expected answers.
    let mut memory = MapMemory::new();
    memory.set_terrain((0, 0), 1, "t_floor", 2, 1);
    memory.set_terrain((1, 0), 2, "t_pavement", 0, 0);
    memory.set_decoration((1, 0), "vp_frame", 3, 2);
    memory.set_symbol((1, 0), u32::from(b'X'));
    memory.clear_decoration((1, 0), "vp_");
    memory.set_terrain((2, 0), 3, "t_dirt", 1, 3);
    memory.set_decoration((2, 0), "graffiti_hello", 0, 0);
    memory.set_symbol((2, 0), u32::from(b'Y'));
    memory.clear_decoration((2, 0), "vp_");
    memory.set_decoration((3, 0), "f_bench", 1, 1);
    memory.set_symbol((3, 0), u32::from(b'Z'));
    memory.clear_decoration((3, 0), "");
    memory.set_terrain((4, 0), 4, "t_floor", 0, 0);
    memory.set_decoration((4, 0), "f_chair", 0, 0);
    memory.set_terrain((4, 0), 5, "t_grass", 2, 1);

    // **Every branch of the clear, pinned before the comparison.** A gate that compared an empty store with an empty recording would pass and prove nothing.
    assert_eq!(memory.len(), 5, "five squares were written");
    assert_eq!(
        memory.at((1, 0)).map(|tile| (tile.decoration.clone(), tile.symbol)),
        Some((String::new(), 0)),
        "a decoration cleared by its own prefix takes its symbol with it"
    );
    assert_eq!(
        memory.at((2, 0)).map(|tile| (tile.decoration.clone(), tile.symbol)),
        Some(("graffiti_hello".to_owned(), u32::from(b'Y'))),
        "a decoration a prefix does not match keeps everything"
    );
    assert_eq!(
        memory.at((3, 0)).map(|tile| tile.decoration.clone()),
        Some(String::new()),
        "the empty prefix matches every decoration"
    );
    assert_eq!(
        memory.at((4, 0)).map(|tile| (tile.terrain_id.clone(), tile.decoration.clone())),
        Some(("t_grass".to_owned(), "f_chair".to_owned())),
        "a second terrain write keeps the decoration"
    );
    let mut compared = 0_usize;
    let mut wrong = Vec::new();
    for square in squares {
        let dx = i32::try_from(whole(square, "dx")).expect("small");
        let dy = i32::try_from(whole(square, "dy")).expect("small");
        let Some(mine) = memory.at((dx, dy)) else {
            // A square that was never written answers the default tile, which is what the reference records for it.
            let unwritten = text_of(square, "ter").is_empty()
                && text_of(square, "dec").is_empty()
                && whole(square, "symbol") == 0;
            if !unwritten {
                wrong.push(format!("({dx}, {dy}) is not remembered, and the reference has a tile there"));
            }
            continue;
        };
        let theirs = (
            text_of(square, "ter"),
            whole(square, "ter_subtile"),
            whole(square, "ter_rotation"),
            text_of(square, "dec"),
            whole(square, "dec_subtile"),
            whole(square, "dec_rotation"),
            whole(square, "symbol"),
        );
        let port = (
            mine.terrain_id.clone(),
            i64::from(mine.terrain_subtile),
            i64::from(mine.terrain_rotation),
            mine.decoration.clone(),
            i64::from(mine.decoration_subtile),
            i64::from(mine.decoration_rotation),
            i64::from(mine.symbol),
        );
        if port != theirs {
            wrong.push(format!("({dx}, {dy}): ours {port:?}, theirs {theirs:?}"));
        }
        compared += 1;
    }
    assert_eq!(compared, 5, "five remembered squares are compared with the reference");
    assert!(wrong.is_empty(), "{} wrong:\n{}", wrong.len(), wrong.join("\n"));
}
