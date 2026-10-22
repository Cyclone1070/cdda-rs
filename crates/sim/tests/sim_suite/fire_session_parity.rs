//! One turn of the reference's own session, replayed by the port.
//!
//! `corpus/fields/fire-floor-offset.json` is a five-turn recording of a fire on `t_floor`, and `fire-floor-offset.rolls.txt` is the dice the reference rolled while it
//! happened, labelled by phase. This test takes the first *processed* turn of that session - the newborn's first turn is spared its processors - hands the
//! port the whole terrain grid, the fire's state before the turn, and the reference's dice, and asks for the reference's state after it.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::rng::{Recording, Rolls};
use cdda_json::{Value, parse};
use cdda_sim::TerrainFacts;
use std::collections::BTreeMap;

fn fixture() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/fields/fire-floor-offset.json");
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

/// Every die the reference rolled **inside a fire processor**, in order, for the whole session.
///
/// The marks bound each processor: `fire.begin` opens one and `fire.done` closes it, so the draws in between belong to the fire's own rules rather than to
/// the sweep's other squares. Nothing is left out - and that took a second fixture to achieve. The first one put the fire on the player's own square,
/// where every `add_field` the hot-air rule makes lands under them and the reference spends a hundred draws rolling their body parts, which is the creature
/// layer and not the fire's. The fire now starts one square east, and the session's dice are the fields' own from beginning to end.
fn fire_processor_dice() -> Recording {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/fields/fire-floor-offset.rolls.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the recording at {}: {error}", path.display()));
    let mut lines = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if let Some(label) = line.strip_prefix("mark ") {
            match label {
                "fire.begin" => inside = true,
                "fire.done" => inside = false,
                _ => {}
            }
            continue;
        }
        if inside {
            lines.push(line.to_string());
        }
    }
    Recording::parse(&lines.join("\n")).expect("the processors' dice parse")
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
fn the_fire_lives_the_same_five_turns_the_recording_gives_it() {
    let fixture = fixture();
    let flags = terrain_flags();
    let grid = array(object(&fixture).get("grid").expect("a grid"));
    let mut map = cdda_sim::MapFacts::new();
    for square in grid {
        let square = object(square);
        let id = string(square.get("ter").expect("terrain"));
        let x = i32::try_from(integer(square.get("x").expect("x"))).expect("x fits");
        let y = i32::try_from(integer(square.get("y").expect("y"))).expect("y fits");
        let z = i32::try_from(integer(square.get("z").expect("z"))).expect("z fits");
        map.set_terrain((x, y, z), facts_for(id, &flags));
    }
    let turns = array(object(&fixture).get("turns").expect("turns"));
    let fires_at = |index: usize| -> Vec<(i32, i32, i32, i64)> {
        let mut fires: Vec<(i32, i32, i32, i64)> =
            array(object(&turns[index]).get("fields").expect("fields"))
                .iter()
                .filter_map(|field| {
                    let field = object(field);
                    if string(field.get("type").expect("type")) == "fd_fire" {
                        Some((
                            i32::try_from(integer(field.get("x").expect("x"))).expect("x fits"),
                            i32::try_from(integer(field.get("y").expect("y"))).expect("y fits"),
                            i32::try_from(integer(field.get("intensity").expect("intensity")))
                                .expect("intensity fits"),
                            integer(field.get("age").expect("age")),
                        ))
                    } else {
                        None
                    }
                })
                .collect();
        fires.sort_unstable();
        fires
    };

    let first = fires_at(1);
    assert_eq!(
        first.len(),
        1,
        "the session starts with one fire: {first:?}"
    );
    let (x, y, intensity, age) = first[0];
    let mut world = bevy_ecs::prelude::World::new();
    let mut entry = cdda_core_types::field::FieldEntry::new(3, 1_000_000, false, true);
    entry.set_age(age);
    entry.set_intensity(intensity);
    world.spawn((
        cdda_sim::Field {
            id: cdda_sim::FIRE,
            entry,
            priority: 0,
        },
        cdda_sim::Position { x, y, z: 0 },
    ));
    map.note_fire((x, y, 0), intensity, age);

    // One replay for the **whole session**: the recording is the reference's processor dice, in the order the sweep spent them, and every turn draws
    // from wherever the last one stopped. Splitting it per turn would hide the thing worth checking - that the port and the reference agree about how
    // many dice each turn costs.
    let mut rolls = Rolls::replaying(fire_processor_dice());
    for turn in 2..turns.len() {
        // The fields the sweep **found** when it started: only those it decays afterwards. A fire created during the pass - a newborn from a spread, or one
        // of the hot air squares - is not revisited, which is why the reference's turn-five newborn still has the two minutes it was born with.
        let present: Vec<bevy_ecs::prelude::Entity> = {
            let mut query = world.query::<(bevy_ecs::prelude::Entity, &cdda_sim::Field)>();
            query
                .iter(&world)
                .filter(|(_, field)| field.id == cdda_sim::FIRE)
                .map(|(entity, _)| entity)
                .collect()
        };
        cdda_sim::process_fires(&mut world, &map, 0, &mut rolls);
        // The sweep's second half: processors first, then each field's own decay. `process_fires` leaves the decay to the sweep that owns it.
        for entity in present {
            if let Some(mut field) = world.get_mut::<cdda_sim::Field>(entity) {
                field.entry.do_decay(1, &mut Rolls::new(7));
            }
        }
        let mut query = world.query::<(&cdda_sim::Field, &cdda_sim::Position)>();
        let mut held: Vec<(i32, i32, i32, i64)> = query
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
        held.sort_unstable();
        assert_eq!(
            held,
            fires_at(turn),
            "the fires the reference has at turn {turn}"
        );
        // The map the next turn reads is the world the last one left.
        for (x, y, intensity, age) in &held {
            map.note_fire((*x, *y, 0), *intensity, *age);
        }
    }
    assert_eq!(
        rolls.recording_remaining(),
        Some(0),
        "and the session spent exactly the dice the reference spent"
    );
}
