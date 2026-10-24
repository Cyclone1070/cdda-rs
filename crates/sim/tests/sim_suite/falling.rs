//! What falls when the ground goes out from under it, against the reference's own recording.
//!
//! Reference: `map::process_falling` (`src/map.cpp:3018-3035`), `map::drop_everything` (`src/map.cpp:2693-2705`), `map::drop_items` (`src/map.cpp:2853-2960`) and
//! `map::drop_fields` (`src/map.cpp:2977-2994`). `corpus/falling/fall.json` is five columns of open air with something on top, every one of which lost its support, and
//! where everything ended up afterwards.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use bevy_ecs::prelude::*;
use cdda_content::mapdata::{self, MapData};
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_json::{parse, Value};
use cdda_sim::falling::{drop_everything, FieldRules, Floors};
use cdda_sim::{Field, GroundItem, ItemTypeId, Position};

fn read(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus").join(name);
    let text = std::fs::read_to_string(path).expect("the corpus file exists");
    parse(text.as_bytes()).expect("the corpus file is valid JSON")
}

fn member<'a>(value: &'a Value, key: &str) -> &'a Value {
    match value {
        Value::Object(object) => object.get(key).unwrap_or_else(|| panic!("{key} is a member")),
        other => panic!("an object: {other:?}"),
    }
}

fn array(value: &Value) -> &[Value] {
    match value {
        Value::Array(entries) => entries,
        other => panic!("a list: {other:?}"),
    }
}

fn text(value: &Value) -> &str {
    match value {
        Value::Str(text) => text,
        other => panic!("a string: {other:?}"),
    }
}

fn int(value: &Value) -> i32 {
    match value {
        Value::Int(number) => i32::try_from(*number).expect("small"),
        other => panic!("an integer: {other:?}"),
    }
}

/// The prototypes the reference's own data defines, which is what the recorded terrain ids name.
fn terrain() -> BTreeMap<String, MapData> {
    let reference = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    mapdata::load_paths(&[&reference.join("core"), &reference.join("json")])
        .expect("the map data loads")
        .into_iter()
        .map(|prototype| (prototype.id.clone(), prototype))
        .collect()
}

/// The field prototypes, in the loader's order, which is the order the port's ids are in.
fn field_prototypes() -> Vec<cdda_content::fields::FieldType> {
    cdda_content::fields::from_dump(&read("prototypes/fields.json")).expect("the field dump loads")
}

fn field_id(name: &str) -> FieldId {
    let index = field_prototypes()
        .iter()
        .position(|field| field.id == name)
        .unwrap_or_else(|| panic!("{name} is a field"));
    FieldId(u32::try_from(index).expect("small"))
}

/// Which fields fall, and which of them may land on a hole, from the data.
fn rules() -> FieldRules {
    let prototypes = field_prototypes();
    let mut falls = BTreeSet::new();
    let mut gas = BTreeSet::new();
    for (index, field) in prototypes.iter().enumerate() {
        let id = FieldId(u32::try_from(index).expect("small"));
        if field.accelerated_decay {
            falls.insert(id);
        }
        if field.phase == "gas" {
            gas.insert(id);
        }
    }
    FieldRules { falls, gas }
}

struct Column {
    dx: i32,
    before: String,
    levels: Vec<Level>,
}

struct Level {
    z: i32,
    ter: String,
    furn: String,
    items: Vec<String>,
    fields: Vec<(String, i32)>,
}

fn columns() -> Vec<Column> {
    let value = read("falling/fall.json");
    let before = array(member(&value, "before"));
    array(member(&value, "columns"))
        .iter()
        .enumerate()
        .map(|(index, column)| Column {
            dx: int(member(column, "dx")),
            before: text(&before[index]).to_owned(),
            levels: array(member(column, "levels"))
                .iter()
                .map(|level| Level {
                    z: int(member(level, "z")),
                    ter: text(member(level, "ter")).to_owned(),
                    furn: text(member(level, "furn")).to_owned(),
                    items: array(member(level, "items"))
                        .iter()
                        .map(|item| text(item).to_owned())
                        .collect(),
                    fields: array(member(level, "fields"))
                        .iter()
                        .map(|field| {
                            let parts = array(field);
                            (text(&parts[0]).to_owned(), int(&parts[1]))
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

impl Column {
    fn at(&self, z: i32) -> &Level {
        self.levels
            .iter()
            .find(|level| level.z == z)
            .unwrap_or_else(|| panic!("dx {} has a level {z}", self.dx))
    }

    /// The items the `before` line says were on the top square, in the order it says them.
    fn payload(&self) -> Vec<&str> {
        self.before
            .split(';')
            .filter_map(|part| part.strip_prefix("i:"))
            .collect()
    }

    /// The fields the `before` line says were on the top square: name and intensity.
    fn starting_fields(&self) -> Vec<(&str, i32)> {
        self.before
            .split(';')
            .filter_map(|part| part.strip_prefix("f:"))
            .map(|part| {
                let mut halves = part.split(':');
                let name = halves.next().expect("a name");
                let intensity = halves.next().expect("an intensity").parse().expect("a number");
                (name, intensity)
            })
            .collect()
    }
}

/// The record's own terrain, as the two questions a fall asks.
struct Recorded {
    data: BTreeMap<String, MapData>,
    tiles: BTreeMap<(i32, i32), String>,
}

impl Floors for Recorded {
    fn no_floor(&self, at: (i32, i32, i32)) -> bool {
        self.tiles.get(&(at.0, at.2)).is_some_and(|id| {
            self.data
                .get(id)
                .is_some_and(|prototype| prototype.flags.contains("NO_FLOOR"))
        })
    }

    fn in_bounds(&self, at: (i32, i32, i32)) -> bool {
        self.tiles.contains_key(&(at.0, at.2))
    }
}

fn recorded(column: &Column, data: &BTreeMap<String, MapData>) -> Recorded {
    Recorded {
        data: data.clone(),
        tiles: column
            .levels
            .iter()
            .map(|level| ((column.dx, level.z), level.ter.clone()))
            .collect(),
    }
}

/// Puts the column's fixture down: its items in order, its fields at their intensities, all on the top square.
fn furnish(world: &mut World, column: &Column) {
    for (order, _) in column.payload().iter().enumerate() {
        world.spawn((
            Position { x: column.dx, y: 0, z: 0 },
            GroundItem {
                kind: ItemTypeId(u32::try_from(order).expect("small")),
                charges: 1,
                age: 0,
                burnt: 0,
                damage: 0,
                burn: None,
            },
        ));
    }
    for (name, intensity) in column.starting_fields() {
        let mut entry = FieldEntry::new(3, 100, true, false);
        entry.set_intensity(intensity);
        world.spawn((
            Position { x: column.dx, y: 0, z: 0 },
            Field {
                id: field_id(name),
                entry,
                priority: 0,
            },
        ));
    }
}

/// Where the items on a column ended up, by level, in stack order, named the way the recording names them.
fn landed_items(world: &mut World, column: &Column) -> BTreeMap<i32, Vec<String>> {
    let payload: Vec<String> = column.payload().iter().map(|name| (*name).to_owned()).collect();
    let mut query = world.query::<(Entity, &Position, &GroundItem)>();
    let mut found: Vec<(i32, u32, i32)> = query
        .iter(world)
        .filter(|(_, at, _)| at.x == column.dx && at.y == 0)
        .enumerate()
        .map(|(order, (_, at, item))| (at.z, item.kind.0, i32::try_from(order).expect("small")))
        .collect();
    found.sort_by_key(|(_, _, order)| *order);
    let mut by_level: BTreeMap<i32, Vec<String>> = BTreeMap::new();
    for (z, kind, _) in found {
        by_level
            .entry(z)
            .or_default()
            .push(payload[usize::try_from(kind).expect("small")].clone());
    }
    by_level
}

/// Which squares the recording says an item landed on.
fn recorded_landings(column: &Column) -> BTreeMap<i32, Vec<String>> {
    column
        .levels
        .iter()
        .filter(|level| !level.items.is_empty())
        .map(|level| (level.z, level.items.clone()))
        .collect()
}

#[test]
fn a_square_with_a_floor_of_its_own_keeps_everything_on_it() {
    let data = terrain();
    let all = columns();
    let column = all.iter().find(|column| column.dx == 10).expect("the floor column");
    assert_eq!(column.before, "i:rock;", "it holds a rock before the turn");
    assert!(
        !data
            .get(&column.at(0).ter)
            .expect("its terrain is loaded")
            .flags
            .contains("NO_FLOOR"),
        "and its own square has a floor, so `drop_everything` does nothing at all"
    );
    let mut world = World::new();
    furnish(&mut world, column);
    let facts = recorded(column, &data);
    let (items, fields) = drop_everything(&mut world, &facts, &rules(), (column.dx, 0, 0));
    assert_eq!((items, fields), (0, 0), "nothing moved");
    let landed = landed_items(&mut world, column);
    assert_eq!(
        landed,
        BTreeMap::from([(0, vec!["rock".to_owned()])]),
        "the rock is where it was"
    );
}

#[test]
fn an_item_falls_to_the_lowest_floor_under_it() {
    let data = terrain();
    let all = columns();
    let mut wrong = Vec::new();
    for column in all.iter().filter(|column| matches!(column.dx, -6 | -2)) {
        let mut world = World::new();
        furnish(&mut world, column);
        let facts = recorded(column, &data);
        let payload_count = column.payload().len();
        let (items, _) = drop_everything(&mut world, &facts, &rules(), (column.dx, 0, 0));
        if items != payload_count {
            wrong.push(format!("dx {}: {items} of {payload_count} items moved", column.dx));
        }
        let mine = landed_items(&mut world, column);
        let theirs = recorded_landings(column);
        if mine != theirs {
            wrong.push(format!("dx {}: landed at {mine:?}, recorded {theirs:?}", column.dx));
        }
        // **The whole point of the loop.** One column has its floor one level down and the other three, so a rule that subtracted one would pass one column and fail the other.
        let deepest = theirs.keys().min().copied().expect("a landing");
        if deepest == 0 {
            wrong.push(format!("dx {}: the fixture has no fall at all", column.dx));
        }
    }
    assert!(wrong.is_empty(), "{} wrong:\n{}", wrong.len(), wrong.join("\n"));
    // Non-vacuity: the two columns land at different depths, which is what makes the loop a rule rather than a constant.
    let depths: Vec<i32> = all
        .iter()
        .filter(|column| matches!(column.dx, -6 | -2))
        .map(|column| *recorded_landings(column).keys().min().expect("a landing"))
        .collect();
    assert_eq!(depths, vec![-1, -3], "one level down and three");
}

#[test]
fn a_cosmetic_field_falls_exactly_one_level() {
    let data = terrain();
    let all = columns();
    let column = all.iter().find(|column| column.dx == 2).expect("the field column");
    assert_eq!(
        column.starting_fields(),
        vec![("fd_smoke", 3)],
        "the fixture holds thick smoke on its top square"
    );
    let mut world = World::new();
    furnish(&mut world, column);
    let facts = recorded(column, &data);
    let (_, moved) = drop_everything(&mut world, &facts, &rules(), (column.dx, 0, 0));
    assert_eq!(moved, 1, "one field moved");
    let mine: Vec<(i32, i32)> = world
        .query::<(&Position, &Field)>()
        .iter(&world)
        .map(|(at, field)| (at.z, field.entry.intensity()))
        .collect();
    assert_eq!(mine, vec![(-1, 3)], "one level down, at the same intensity");
    let theirs = column
        .levels
        .iter()
        .find(|level| !level.fields.is_empty())
        .expect("the recording has a landing");
    assert_eq!((theirs.z, theirs.fields.clone()), (-1, vec![("fd_smoke".to_owned(), 3)]));
}

/// The furniture column is **recorded and not ported**: a bench falls and bashes the floor it lands on, and bashing a floor kicks up dust.
///
/// Reference: `map::drop_furniture` (`src/map.cpp:2707-2790`). The dust is the bash's, not the fall's, which is why it is declared here rather than modelled: the port's
/// bash does not create fields yet. The recording is asserted so the gap is a number rather than a note.
#[test]
fn a_falling_bench_bashes_where_it_lands_which_is_not_portable_yet() {
    let all = columns();
    let column = all.iter().find(|column| column.dx == 6).expect("the furniture column");
    assert_eq!(column.before, "u:f_bench;", "the fixture holds a bench on its top square");
    let with_furniture: Vec<(i32, &str)> = column
        .levels
        .iter()
        .filter(|level| level.furn != "f_null")
        .map(|level| (level.z, level.furn.as_str()))
        .collect();
    assert_eq!(with_furniture, vec![(-2, "f_bench")], "the bench fell two levels");
    let dust: Vec<(i32, &[(String, i32)])> = column
        .levels
        .iter()
        .filter(|level| !level.fields.is_empty())
        .map(|level| (level.z, level.fields.as_slice()))
        .collect();
    assert_eq!(
        dust,
        vec![(-2, [("fd_dust".to_owned(), 2)].as_slice())],
        "and the bash it made on landing raised dust"
    );
}
