//! One turn of the reference's own session, replayed by the port.
//!
//! `corpus/fields/fire-floor.json` is a five-turn recording of a fire on `t_floor`, and `fire-floor.rolls.txt` is the dice the reference rolled while it
//! happened, labelled by phase. This test takes the first *processed* turn of that session - the newborn's first turn is spared its processors - hands the
//! port the whole terrain grid, the fire's state before the turn, and the reference's dice, and asks for the reference's state after it.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::fire::Ground;
use cdda_core_types::rng::{Recording, Rolls};
use cdda_json::{Value, parse};
use cdda_sim::{FireMap, MapFacts, TerrainFacts};
use std::collections::BTreeMap;

fn fixture() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/fields/fire-floor.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture at {}: {error}", path.display()));
    parse(text.as_bytes()).expect("the fixture parses")
}

fn array(value: &Value) -> &[Value] {
    match value {
        Value::Array(items) => items,
        other => panic!("expected an array, found {other:?}"),
    }
}

fn integer(value: &Value) -> i64 {
    match value {
        Value::Int(number) => *number,
        other => panic!("expected an integer, found {other:?}"),
    }
}

fn string(value: &Value) -> &str {
    match value {
        Value::Str(text) => text,
        other => panic!("expected a string, found {other:?}"),
    }
}

fn object(value: &Value) -> &cdda_json::Object {
    match value {
        Value::Object(members) => members,
        other => panic!("expected an object, found {other:?}"),
    }
}

fn terrain_flags() -> BTreeMap<String, Vec<String>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/prototypes/terrain.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the prototypes at {}: {error}", path.display()));
    let value = parse(text.as_bytes()).expect("the prototypes parse");
    let mut flags = BTreeMap::new();
    for entry in array(
        object(&value)
            .get("prototypes")
            .expect("a list of prototypes"),
    ) {
        let Some(id) = object(entry).get("id").map(string) else {
            continue;
        };
        let set: Vec<String> = object(entry)
            .get("flags")
            .map(array)
            .map(|flags| flags.iter().map(|flag| string(flag).to_string()).collect())
            .unwrap_or_default();
        flags.insert(id.to_string(), set);
    }
    flags
}

fn facts_for(id: &str, flags: &BTreeMap<String, Vec<String>>) -> TerrainFacts {
    let has = |flag: &str| {
        flags
            .get(id)
            .is_some_and(|set| set.iter().any(|name| name == flag))
    };
    TerrainFacts {
        flammable: has("FLAMMABLE"),
        flammable_ash: has("FLAMMABLE_ASH"),
        flammable_hard: has("FLAMMABLE_HARD"),
        fire_container: has("FIRE_CONTAINER"),
        no_floor: has("NO_FLOOR"),
        suppress_smoke: has("SUPPRESS_SMOKE"),
    }
}

/// The dice the reference rolled during the first processed turn, from `fire.begin` to the next `fire.begin`.
fn first_turn_dice() -> Recording {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/fields/fire-floor.rolls.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the recording at {}: {error}", path.display()));
    let mut lines = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if let Some(label) = line.strip_prefix("mark ") {
            if label == "fire.begin" {
                if inside {
                    break;
                }
                inside = true;
            }
            continue;
        }
        if inside {
            lines.push(line.to_string());
        }
    }
    Recording::parse(&lines.join("\n")).expect("the turn's dice parse")
}

#[test]
fn the_terrain_the_recording_names_is_the_terrain_the_port_reads() {
    // A guard on the fixture and the loader rather than on the fire: if the grid or the prototype flags ever disagree, every parity claim below is void.
    let fixture = fixture();
    let flags = terrain_flags();
    let grid = array(object(&fixture).get("grid").expect("a grid"));
    assert_eq!(grid.len(), 49, "seven by seven around the player");
    let ground = facts_for("t_floor", &flags);
    assert!(ground.flammable_hard, "t_floor is FLAMMABLE_HARD");
    let wall = facts_for("t_wall_w", &flags);
    assert!(
        wall.flammable && !wall.flammable_hard,
        "a wired wall burns, and it is not hard ground"
    );
}

#[test]
#[ignore = "blocked on the flashpoint rule: the reference draws one_in( 3 ) before the spread loop and grows a fire only when that die fails, so the \
            whole turn is a die ahead of the port from the second draw onwards. See docs/parity.md, round 324."]
fn one_whole_turn_matches_the_reference_given_the_references_own_dice() {
    let fixture = fixture();
    let flags = terrain_flags();
    let grid = array(object(&fixture).get("grid").expect("a grid"));
    let mut map = MapFacts::new();
    for square in grid {
        let x = i32::try_from(integer(object(square).get("x").expect("x"))).expect("a small x");
        let y = i32::try_from(integer(object(square).get("y").expect("y"))).expect("a small y");
        let z = i32::try_from(integer(object(square).get("z").expect("z"))).expect("a small z");
        let ter = string(object(square).get("ter").expect("terrain"));
        map.set_terrain((x, y, z), facts_for(ter, &flags));
    }
    let turns = array(object(&fixture).get("turns").expect("turns"));
    let fire_at = |turn: usize, kind: &str| -> Vec<(i32, i32, i32, i32)> {
        array(object(&turns[turn]).get("fields").expect("fields"))
            .iter()
            .filter(|field| object(field).get("type").map(string) == Some(kind))
            .map(|field| {
                (
                    i32::try_from(integer(object(field).get("x").expect("x"))).expect("x"),
                    i32::try_from(integer(object(field).get("y").expect("y"))).expect("y"),
                    i32::try_from(integer(object(field).get("intensity").expect("intensity")))
                        .expect("i"),
                    i32::try_from(integer(object(field).get("age").expect("age"))).expect("age"),
                )
            })
            .collect()
    };
    let before = fire_at(1, "fd_fire");
    let after = fire_at(2, "fd_fire");
    assert_eq!(before.len(), 1, "one fire before the turn: {before:?}");
    assert_eq!(after.len(), 1, "and one after it: {after:?}");

    // The ground class is the map's answer, and the fixture's own terrain says which branch the reference ran.
    let (x, y, _, _) = before[0];
    assert_eq!(
        map.ground((x, y, 0)),
        Ground::FlammableHard,
        "t_floor, as the prototype says"
    );

    // What the port must reproduce: the same fire, one turn older, having eaten or not eaten what the dice said.
    let mut rolls = Rolls::replaying(first_turn_dice());
    let mut world = bevy_ecs::prelude::World::new();
    let mut entry = cdda_core_types::field::FieldEntry::new(3, 1_000_000, false, true);
    entry.set_age(i64::from(before[0].3));
    entry.set_intensity(before[0].2);
    world.spawn((
        cdda_sim::Field {
            id: cdda_sim::FIRE,
            entry,
            priority: 0,
        },
        cdda_sim::Position { x, y, z: 0 },
    ));
    cdda_sim::process_fires(&mut world, &map, 0, &mut rolls);
    let mut query = world.query::<(&cdda_sim::Field, &cdda_sim::Position)>();
    let held: Vec<(i32, i32, i32, i64)> = query
        .iter(&world)
        .filter(|(field, _)| field.id == cdda_sim::FIRE)
        .map(|(field, position)| {
            (
                position.x,
                position.y,
                field.entry.intensity(),
                field.entry.age(),
            )
        })
        .collect();
    assert_eq!(
        held,
        vec![(after[0].0, after[0].1, after[0].2, i64::from(after[0].3))],
        "the port's fire against the reference's, one turn later"
    );
}
