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
    /// The wind at **this square**, which is what the spread rule's chance ceiling and its blockers read (`map_field.cpp:279-284`). A gas on a roof and one in the
    /// cellar beside it stand in different winds, so the fixture records the number per square rather than per turn.
    pub windpower: i32,
    /// Whether this square is under a roof. A sheltered gas never feels the wind, however hard it blows.
    pub sheltered: bool,
    /// Whether the reference still counts this entry as a field.
    ///
    /// `field::remove_field` marks an entry dead and leaves it in the square's list until the sweep next tidies up, and the dump prints what is there - so a
    /// corpse appears in the record with its intensity clamped to one, which reads exactly like a living field of intensity one unless this flag is read. The
    /// port despawns a field the moment it dies, so a comparison that kept corpses would report every death twice: once as a missing field and once as a mismatch.
    pub alive: bool,
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
    /// The dice of each turn separately, when the recording labels its turns (`gas.turn.<n>`); empty for a flat recording.
    ///
    /// A processor's own label says what it rolled but not which turn rolled it, so a flat slice cannot tell a turn whose gas could not spread from a turn whose
    /// processor never ran - and after the first such turn every later die is one position out.
    pub dice_by_turn: Vec<Recording>,
    /// What the calendar said at each turn, as `calendar::turn` in turns.
    ///
    /// A field's decay is arithmetic on the clock - the half-life draws a **deadline**, and the field loses a level when the clock passes it - so a replay that aged a
    /// field with a clock of zero would keep every field alive forever, or kill it at once, depending on which side of zero its deadline fell.
    pub now: Vec<i64>,
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
    ///
    /// # Panics
    ///
    /// Panics when the fixture or its recording is missing or malformed. A corpus file that cannot be
    /// read is a broken repository, not a case to carry on past: the session is the specification.
    #[must_use]
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
        let now: Vec<i64> = array(object(&fixture).get("turns").expect("turns"))
            .iter()
            .map(|turn| object(turn).get("now").map_or(0, integer))
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
        let dice_by_turn = dice_per_turn(&recording, &turns_of(&fixture));
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
                            windpower: i32::try_from(field.get("windpower").map_or(0, integer))
                                .expect("windpower fits"),
                            sheltered: field
                                .get("sheltered")
                                .is_some_and(|value| matches!(value, Value::Bool(true))),
                            alive: field
                                .get("alive")
                                .is_none_or(|value| matches!(value, Value::Bool(true))),
                        }
                    })
                    .collect()
            })
            .collect();
        Session {
            grid,
            dice_by_turn,
            turns,
            now,
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

/// How one replay compared with the reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    /// The fixture's name.
    pub name: String,
    /// How many turns the port and the reference both took.
    pub turns: usize,
    /// The first turn that disagreed, written out, if any did.
    pub first_difference: Option<String>,
    /// Dice the port did not spend.
    pub dice_left: usize,
}

impl Comparison {
    /// Whether the replay agreed on every turn and spent every die.
    #[must_use]
    pub fn agrees(&self) -> bool {
        self.first_difference.is_none() && self.dice_left == 0
    }
}

/// The fixtures under `corpus/fields/replayable.txt`, in the order the list gives them.
#[must_use]
pub fn replayable() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/fields/replayable.txt");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Replays a fixture and says how it compared with the reference.
#[must_use]
pub fn compare(name: &str) -> Comparison {
    let session = Session::load(name);
    let replay = session.replay_fires();
    let mut turns = 0;
    let mut first_difference = None;
    for (index, played) in replay.turns.iter().enumerate() {
        turns += 1;
        let recorded = session.fires_at(index + 1);
        if *played != recorded && first_difference.is_none() {
            first_difference = Some(format!(
                "turn {}: the reference held {recorded:?}, the port held {played:?}",
                index + 1
            ));
        }
    }
    if first_difference.is_none() && replay.dice_left != 0 {
        first_difference = Some(format!(
            "{} dice left unspent, so the replay only approached the session",
            replay.dice_left
        ));
    }
    Comparison {
        name: name.to_string(),
        turns,
        first_difference,
        dice_left: replay.dice_left,
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
///
/// # Panics
///
/// Panics when the dumped prototype file is missing or malformed, which is a broken corpus.
#[must_use]
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
#[must_use]
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

/// The watched square: the one the fixture's first field stands on. A gas trace marks its draws with the square, and the bubble holds other gases.
fn turns_of(fixture: &Value) -> Vec<Value> {
    array(object(fixture).get("turns").expect("turns")).to_vec()
}

/// Splits a trace into one recording per turn, when the trace labels its turns.
///
/// Returns nothing for a recording without `gas.turn.<n>` labels: those are the fire fixtures, whose dice are one flat run and whose replay wants exactly that.
fn dice_per_turn(text: &str, turns: &[Value]) -> Vec<Recording> {
    if !text.contains("gas.turn.") {
        return Vec::new();
    }
    // Every square the fixture's own gas ever stands on: a gas that falls or rises marks its draws under the square it is **leaving**, so a set built from the first
    // turn alone would drop the draws of every square the session later walks onto. Any other gas in the bubble is somebody else's business.
    let mut watched: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for turn in turns {
        for field in array(object(turn).get("fields").expect("fields")) {
            let field = object(field);
            watched.insert(format!(
                "{}.{}.{}",
                integer(field.get("x").expect("x")),
                integer(field.get("y").expect("y")),
                integer(field.get("z").expect("z"))
            ));
        }
    }
    let mut per_turn: Vec<Vec<String>> = vec![Vec::new()];
    let mut inside = false;
    // A turn's **decay** draws happen after its processors have finished, so they are outside `.begin`/`.done`: the turn is the span between two
    // `gas.turn.<n>` marks, and a `drawn` line anywhere in it belongs to it.
    let mut in_turn = false;
    for line in text.lines() {
        if let Some(label) = line.strip_prefix("mark ") {
            if let Some(number) = label.strip_prefix("gas.turn.") {
                let index: usize = number.parse().expect("a turn number");
                per_turn.resize(index + 1, Vec::new());
                inside = false;
                in_turn = true;
            } else if let Some(square) = label.strip_prefix("gas.begin.") {
                inside = watched.is_empty() || watched.contains(square);
            } else if label.contains(".done") {
                inside = false;
            }
            continue;
        }
        if inside || (in_turn && line.starts_with("drawn ")) {
            per_turn.last_mut().expect("a turn").push(line.to_string());
        }
    }
    per_turn
        .into_iter()
        .map(|lines| Recording::parse(&lines.join("\n")).expect("a turn's dice parse"))
        .collect()
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
    // **`drawn` lines come along.** They are not dice, but a turn's decay draws its own and a slice without them hands the port a turn whose
    // exponential it has to invent - which is how a risen field stayed alive in a replay that the reference had already buried.
    Recording::parse(&lines.join("\n")).expect("the processors' dice parse")
}

/// The two prototype dumps a fire needs in order to eat something: what an item is made of, and how each material burns.
pub struct Catalogue {
    materials: BTreeMap<String, Vec<cdda_core_types::fire::MaterialBurn>>,
    items: BTreeMap<String, (f64, Vec<(String, i32)>)>,
}

impl Catalogue {
    /// Reads `corpus/prototypes/{materials,items}.json`.
    ///
    /// # Panics
    ///
    /// Panics when either prototype dump is missing or malformed, which is a broken corpus.
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
