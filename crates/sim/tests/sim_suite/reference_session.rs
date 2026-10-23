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

/// One square of the dump: where it is, what it is made of, and what is lying on it.
#[derive(Debug, Clone)]
pub struct Square {
    /// Where.
    pub x: i32,
    /// Where.
    pub y: i32,
    /// Which floor.
    pub z: i32,
    /// The terrain's id.
    pub terrain: String,
    /// The furniture's id.
    pub furniture: String,
    /// Whether anything on it would burn.
    pub burns: bool,
    /// The ids of the items lying on it, in the order they were dropped.
    pub items: Vec<String>,
}

/// A recorded scene: its terrain, its turns, and its dice.
pub struct Session {
    /// Every square in the radius, as the dump describes it.
    pub grid: Vec<Square>,
    /// The fields after each turn, starting with the state before anything ran.
    pub turns: Vec<Vec<Placed>>,
    /// The dice the reference rolled inside its field processors, in order.
    pub dice: Recording,
    /// The wind on each turn, which a gas needs: its spread chance is `rng( 1, 100 - windpower )` and its destination is resisted when it blows into that wind.
    pub wind: Vec<Wind>,
    /// Where the fixture came from, so a failed assertion can say which one.
    pub name: String,
}

/// The wind a turn ran under: the sky's speed and direction, and the local power at the square the dump watched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wind {
    /// The weather's own wind speed.
    pub speed: i32,
    /// Which way it blows, in degrees.
    pub direction: i32,
    /// The power `get_local_windpower` worked out for the watched square, which is what the dice ceiling uses.
    pub power: i32,
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
                Square {
                    x: i32::try_from(integer(square.get("x").expect("x"))).expect("x fits"),
                    y: i32::try_from(integer(square.get("y").expect("y"))).expect("y fits"),
                    z: i32::try_from(integer(square.get("z").expect("z"))).expect("z fits"),
                    terrain: string(square.get("ter").expect("terrain")).to_string(),
                    furniture: square.get("furn").map_or("f_null", string).to_string(),
                    burns: integer(square.get("items_burn").expect("items_burn")) != 0,
                    items: match square.get("items_at") {
                        Some(Value::Array(list)) => {
                            list.iter().map(string).map(str::to_string).collect()
                        }
                        _ => Vec::new(),
                    },
                }
            })
            .collect();
        let wind = array(object(&fixture).get("turns").expect("turns"))
            .iter()
            .map(|turn| {
                let turn = object(turn);
                // Fixtures recorded before the wind was beside the fields have none, and a fire does not read it; a gas fixture that wanted it would be a fixture
                // recorded without the instrument and would have to be re-recorded, not guessed at.
                let of = |key: &str| match turn.get(key) {
                    Some(value) => i32::try_from(integer(value)).expect("a wind number"),
                    None => 0,
                };
                Wind {
                    speed: of("windspeed"),
                    direction: of("winddirection"),
                    power: of("windpower"),
                }
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
            wind,
            dice: processor_dice(&recording),
            name: name.to_string(),
        }
    }

    /// The map the port must read: the grid's terrain, translated through the loaded prototypes' flags.
    #[must_use]
    pub fn map(&self) -> cdda_sim::MapFacts {
        let flags = terrain_flags();
        let mut map = cdda_sim::MapFacts::new();
        for square in &self.grid {
            // `ter_furn_has_flag` reads both, so the square's answers are its terrain's and its furniture's together - a bench on a tiled floor burns.
            let at = (square.x, square.y, square.z);
            map.set_terrain(at, facts_for(&square.terrain, &flags));
            map.set_furniture(at, facts_for(&square.furniture, &flags));
            map.set_items(at, square.burns);
        }
        map
    }

    /// Puts every item the dump names on the ground, resolved against the prototype dumps.
    ///
    /// A fire eats items a fraction at a time, and the fraction depends on the item's own volume and its materials' burn tables - so an item on the ground is
    /// an entity with state, and a replay that did not spawn them would not draw the reals the reference drew.
    pub fn spawn_items(&self, world: &mut bevy_ecs::prelude::World) {
        let catalogue = Catalogue::load();
        for square in &self.grid {
            for (index, id) in square.items.iter().enumerate() {
                let Some(burn) = catalogue.burning_item(id) else {
                    continue;
                };
                world.spawn((
                    cdda_sim::Position {
                        x: square.x,
                        y: square.y,
                        z: square.z,
                    },
                    cdda_sim::GroundItem {
                        kind: cdda_sim::ItemTypeId(u32::try_from(index).unwrap_or(0)),
                        charges: 1,
                        age: 0,
                        burnt: 0,
                        damage: 0,
                        burn: Some(burn),
                    },
                ));
            }
        }
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
        self.spawn_items(&mut world);
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

/// The dice the reference rolled inside its field processors: `<kind>.begin` opens one and `<kind>.done` closes it.
///
/// Any kind, not just `fire`: the gas processor marks its own draws the same way, and a slicer that only knew `fire` silently produced an **empty** recording -
/// which reads as "the port asked for a die the reference never rolled" when in fact the fixture had simply been cut out of the file.
fn processor_dice(text: &str) -> Recording {
    let mut lines = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if let Some(label) = line.strip_prefix("mark ") {
            // The kind of processor, not the exact label: a gas marks `gas.begin.<x>.<y>.<z>`, because the bubble can hold other gases and a slice that could not
            // tell whose draw was whose would replay the bubble instead of the fixture.
            if label.contains(".begin") {
                inside = true;
            } else if label.contains(".done") {
                inside = false;
            }
            continue;
        }
        if inside {
            lines.push(line.to_string());
        }
    }
    Recording::parse(&lines.join("\n")).expect("the processors' dice parse")
}

/// The two prototype dumps a fire needs in order to eat something: what an item is made of, and how each material burns.
pub struct Catalogue {
    materials: BTreeMap<String, Vec<cdda_core_types::fire::MaterialBurn>>,
    items: BTreeMap<String, (f64, Vec<(String, i32)>)>,
}

impl Catalogue {
    /// Reads `corpus/prototypes/{materials,items}.json`.
    #[must_use]
    pub fn load() -> Self {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes");
        let materials_value = value(&root.join("materials.json"));
        let items_value = value(&root.join("items.json"));
        let mut materials = BTreeMap::new();
        for entry in array(
            object(&materials_value)
                .get("prototypes")
                .expect("materials"),
        ) {
            let material = object(entry);
            let Some(id) = material.get("id").map(string) else {
                continue;
            };
            let mut levels = Vec::new();
            for level in array(material.get("burn").expect("burn data")) {
                let row = object(level);
                levels.push(cdda_core_types::fire::MaterialBurn {
                    immune: match row.get("immune") {
                        Some(Value::Bool(flag)) => *flag,
                        _ => false,
                    },
                    volume_per_turn_ml: match row.get("volume_per_turn") {
                        Some(Value::Float(volume)) => *volume,
                        Some(Value::Int(volume)) => f64::from(i32::try_from(*volume).unwrap_or(0)),
                        _ => 0.0,
                    },
                    fuel: number(row.get("fuel")),
                    smoke: number(row.get("smoke")),
                    burn: number(row.get("burn")),
                });
            }
            materials.insert(id.to_string(), levels);
        }
        let mut items = BTreeMap::new();
        for entry in array(object(&items_value).get("prototypes").expect("items")) {
            let item = object(entry);
            let Some(id) = item.get("id").map(string) else {
                continue;
            };
            let volume = number(item.get("volume"));
            let mut portions = Vec::new();
            if let Some(Value::Array(list)) = item.get("materials") {
                for pair in list {
                    if let Value::Array(pair) = pair
                        && let (Some(material), Some(count)) = (pair.first(), pair.get(1))
                    {
                        portions.push((
                            string(material).to_string(),
                            i32::try_from(integer(count)).unwrap_or(1),
                        ));
                    }
                }
            }
            items.insert(id.to_string(), (volume, portions));
        }
        Self { materials, items }
    }

    /// What a fire would make of one of these, or `None` when the item or one of its materials is unknown.
    #[must_use]
    pub fn burning_item(&self, id: &str) -> Option<cdda_core_types::fire::BurningItem> {
        let (volume_ml, portions) = self.items.get(id)?;
        let mut materials = Vec::new();
        for (material, count) in portions {
            let levels = self.materials.get(material)?;
            materials.push((levels.clone(), *count));
        }
        if materials.is_empty() {
            return None;
        }
        Some(cdda_core_types::fire::BurningItem {
            volume_ml: *volume_ml,
            materials,
            burnt: 0.0,
        })
    }
}

/// A number that the dump may hold as an integer or a real.
fn number(value: Option<&Value>) -> f64 {
    match value {
        Some(Value::Int(number)) => f64::from(i32::try_from(*number).unwrap_or(0)),
        Some(Value::Float(number)) => *number,
        _ => 0.0,
    }
}
