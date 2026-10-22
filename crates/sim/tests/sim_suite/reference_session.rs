//! The machinery that turns a reference recording into a replay.
//!
//! A fixture under `corpus/fields` is a dump the instrumented reference wrote while it ran a scene: the terrain of every square around the action, the fields
//! it held after each turn, and - in the sibling `*.rolls.txt` - every die it rolled, labelled by phase. This module loads one, builds the port's world from
//! it, and plays the turns, so a test only has to say what to compare.
//!
//! The rules it encodes about *how* to replay, each of which cost a round to learn:
//!
//! * a fixture's dice are sliced from `fire.begin` to `fire.done`, because that is one processor's worth; the draws in between belong to the sweep's other
//!   squares and the port has no business spending them;
//! * the sweep decays only the fields it **found** (`map::process_fields` lists its squares before it visits them), so a newborn keeps the age it was born
//!   with;
//! * the map the next turn reads is the world the last one left - a fire's neighbours are fires this turn, not last turn.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::{Entity, World};
use cdda_core_types::rng::{Recording, Rolls};
use cdda_json::{Value, parse};
use cdda_sim::{FIRE, Field, Position, TerrainFacts};
use std::collections::BTreeMap;

/// One field the reference dumped: where it was, what it was, and how strong and old.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    /// The square's column.
    pub x: i32,
    /// The square's row.
    pub y: i32,
    /// The square's floor.
    pub z: i32,
    /// The field's id, as the reference spells it (`fd_fire`).
    pub kind: String,
    /// How strong it was.
    pub intensity: i32,
    /// How long it had burned, in turns.
    pub age: i64,
}

impl Placed {
    /// What a test compares: the square, the strength and the age, without the id.
    #[must_use]
    pub fn fire(&self) -> (i32, i32, i32, i64) {
        (self.x, self.y, self.intensity, self.age)
    }
}

/// A recorded scene: its terrain, its turns, and its dice.
pub struct Session {
    /// Every square in the radius, with the terrain and the furniture it held.
    pub grid: Vec<(i32, i32, i32, String, String)>,
    /// The fields after each turn, starting with the state before anything ran.
    pub turns: Vec<Vec<Placed>>,
    /// The dice the reference rolled inside its field processors, in order.
    pub dice: Recording,
    /// Where the fixture came from, so a failed assertion can say which one.
    pub name: String,
}

impl Session {
    /// Loads `corpus/fields/{name}.json` and its sibling recording.
    pub fn load(name: &str) -> Session {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/fields");
        let fixture = value(&root.join(format!("{name}.json")));
        let recording = std::fs::read_to_string(root.join(format!("{name}.rolls.txt")))
            .unwrap_or_else(|error| panic!("the recording for {name}: {error}"));
        let grid = array(object(&fixture).get("grid").expect("a grid"))
            .iter()
            .map(|square| {
                let square = object(square);
                (
                    i32::try_from(integer(square.get("x").expect("x"))).expect("x fits"),
                    i32::try_from(integer(square.get("y").expect("y"))).expect("y fits"),
                    i32::try_from(integer(square.get("z").expect("z"))).expect("z fits"),
                    string(square.get("ter").expect("terrain")).to_string(),
                    square.get("furn").map_or("f_null", string).to_string(),
                )
            })
            .collect();
        let turns = array(object(&fixture).get("turns").expect("turns"))
            .iter()
            .map(|turn| {
                array(object(turn).get("fields").expect("fields"))
                    .iter()
                    .map(|field| {
                        let field = object(field);
                        Placed {
                            x: i32::try_from(integer(field.get("x").expect("x"))).expect("x fits"),
                            y: i32::try_from(integer(field.get("y").expect("y"))).expect("y fits"),
                            z: i32::try_from(integer(field.get("z").expect("z"))).expect("z fits"),
                            kind: string(field.get("type").expect("type")).to_string(),
                            intensity: i32::try_from(integer(
                                field.get("intensity").expect("intensity"),
                            ))
                            .expect("intensity fits"),
                            age: integer(field.get("age").expect("age")),
                        }
                    })
                    .collect()
            })
            .collect();
        Session {
            grid,
            turns,
            dice: processor_dice(&recording),
            name: name.to_string(),
        }
    }

    /// The map the port must read: the grid's terrain, translated through the loaded prototypes' flags.
    #[must_use]
    pub fn map(&self) -> cdda_sim::MapFacts {
        let flags = terrain_flags();
        let mut map = cdda_sim::MapFacts::new();
        for (x, y, z, terrain, furniture) in &self.grid {
            // `ter_furn_has_flag` reads both, so the square's answers are its terrain's and its furniture's together - a bench on a tiled floor burns.
            map.set_terrain((*x, *y, *z), facts_for(terrain, &flags));
            map.set_furniture((*x, *y, *z), facts_for(furniture, &flags));
        }
        map
    }

    /// The fires the reference held after `turn`, as `(x, y, intensity, age)` in a stable order.
    #[must_use]
    pub fn fires_at(&self, turn: usize) -> Vec<(i32, i32, i32, i64)> {
        let mut fires: Vec<(i32, i32, i32, i64)> = self
            .turns
            .get(turn)
            .map(|fields| {
                fields
                    .iter()
                    .filter(|field| field.kind == "fd_fire")
                    .map(Placed::fire)
                    .collect()
            })
            .unwrap_or_default();
        fires.sort_unstable();
        fires
    }

    /// Plays the session in the port.
    #[must_use]
    pub fn replay_fires(&self) -> Replay {
        let mut map = self.map();
        let mut world = World::new();
        for field in self.turns.first().map(Vec::as_slice).unwrap_or_default() {
            if field.kind != "fd_fire" {
                continue;
            }
            spawn_fire(&mut world, field);
            map.note_fire((field.x, field.y, field.z), field.intensity, field.age);
        }
        let mut rolls = Rolls::replaying(self.dice.clone());
        let mut answers = Vec::new();
        for _ in 1..self.turns.len() {
            let present: Vec<(Entity, i32, i32)> = {
                let mut query = world.query::<(Entity, &Field, &Position)>();
                query
                    .iter(&world)
                    .map(|(entity, _, position)| (entity, position.x, position.y))
                    .collect()
            };
            cdda_sim::process_fires(&mut world, &map, 0, &mut rolls);
            cdda_sim::decay_the_fields_the_sweep_found(
                &mut world,
                &present,
                0,
                &mut Rolls::new(7),
                |_field| None,
                |_, _, _| {},
            );
            let mut fires: Vec<(i32, i32, i32, i64)> = {
                let mut query = world.query::<(&Field, &Position)>();
                query
                    .iter(&world)
                    .filter(|(field, _)| field.id == FIRE)
                    .map(|(field, position)| {
                        (
                            position.x,
                            position.y,
                            field.entry.intensity(),
                            field.entry.age(),
                        )
                    })
                    .collect()
            };
            fires.sort_unstable();
            for (x, y, intensity, age) in &fires {
                map.note_fire((*x, *y, 0), *intensity, *age);
            }
            answers.push(fires);
        }
        Replay {
            turns: answers,
            dice_left: rolls.recording_remaining().unwrap_or(0),
        }
    }
}

/// What the port made of a session: its answer per turn, and how much of the recording it left unspent.
pub struct Replay {
    /// The fires the port held after each turn, in the same shape as [`Session::fires_at`].
    pub turns: Vec<Vec<(i32, i32, i32, i64)>>,
    /// Dice the port did not spend. A session that ends with dice left over has not been replayed, only approached.
    pub dice_left: usize,
}

fn spawn_fire(world: &mut World, field: &Placed) {
    let mut entry = cdda_core_types::field::FieldEntry::new(3, 1_000_000, false, true);
    entry.set_age(field.age);
    entry.set_intensity(field.intensity);
    world.spawn((
        Field {
            id: FIRE,
            entry,
            priority: 0,
        },
        Position {
            x: field.x,
            y: field.y,
            z: field.z,
        },
    ));
}

fn value(path: &std::path::Path) -> Value {
    let text = std::fs::read_to_string(path)
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

/// The prototype flags, by terrain id.
pub fn terrain_flags() -> BTreeMap<String, Vec<String>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/prototypes/terrain.json");
    let value = value(&path);
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

/// One terrain's answers, read from its prototype flags rather than written by hand.
pub fn facts_for(id: &str, flags: &BTreeMap<String, Vec<String>>) -> TerrainFacts {
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

/// The dice the reference rolled inside its field processors: `fire.begin` opens one and `fire.done` closes it.
fn processor_dice(text: &str) -> Recording {
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
