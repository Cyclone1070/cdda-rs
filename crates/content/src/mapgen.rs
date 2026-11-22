//! Mapgen keys: which overmap terrain ids have a map to generate.
//!
//! Reference: `mapgen::load_mapgen` and `mapgen_factory` (`src/mapgen.cpp:1138-1450`). A mapgen is
//! registered under the ids in its `om_terrain` field, which may be a string, a list, or a grid of
//! lists; nested and update mapgens register elsewhere and are kept apart here.
//!
//! The key the game looks up is the terrain's **mapgen id**, not its own id: 24,191 of 24,218
//! terrains have a map under that key while only 325 match the terrain id. Getting that wrong is
//! what made this family look blocked for several rounds, so it is written down here.

use cdda_json::{Object, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The keys one mapgen object registers, split by the registry they belong to.
#[derive(Debug, Default, Clone)]
pub struct MapgenKeys {
    /// Overmap terrain ids (`om_terrain`), the ones `mapgen_factory` holds.
    pub terrains: Vec<String>,
    /// Nested mapgen ids.
    pub nested: Vec<String>,
    /// Update mapgen ids, which are applied to an existing map.
    pub update: Vec<String>,
}

/// Every mapgen key in a set of data directories.
#[derive(Debug, Default, Clone)]
pub struct MapgenIndex {
    /// Overmap terrain mapgen keys, deduplicated and sorted.
    pub terrains: BTreeSet<String>,
    /// Nested mapgen ids.
    pub nested: BTreeSet<String>,
    /// Update mapgen ids.
    pub update: BTreeSet<String>,
}

/// Why a mapgen index could not be read.
#[derive(Debug)]
pub struct LoadError {
    /// The problem.
    pub message: String,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LoadError {}

/// Reads every mapgen object under a set of data directories.
///
/// # Errors
/// Returns an error when a directory cannot be walked.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<MapgenIndex, LoadError> {
    let mut index = MapgenIndex::default();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            let Value::Array(objects) = value.as_ref() else {
                continue;
            };
            for object in objects {
                let Value::Object(object) = object else {
                    continue;
                };
                let keys = keys_of(object);
                index.terrains.extend(keys.terrains);
                index.nested.extend(keys.nested);
                index.update.extend(keys.update);
            }
        }
    }
    Ok(index)
}

/// The keys one JSON object registers, when it is a mapgen at all.
#[must_use]
pub fn keys_of(object: &Object) -> MapgenKeys {
    let mut keys = MapgenKeys::default();
    let is_mapgen = matches!(object.get("type"), Some(Value::Str(kind)) if kind == "mapgen");
    if !is_mapgen {
        return keys;
    }
    if let Some(Value::Str(id)) = object.get("om_terrain") {
        keys.terrains.push(id.clone());
    } else if let Some(Value::Array(rows)) = object.get("om_terrain") {
        for row in rows {
            match row {
                // A list of ids for one map.
                Value::Str(id) => keys.terrains.push(id.clone()),
                // A grid row: several ids, one per column.
                Value::Array(columns) => {
                    for column in columns {
                        if let Value::Str(id) = column {
                            keys.terrains.push(id.clone());
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(Value::Str(id)) = object.get("nested_mapgen_id") {
        keys.nested.push(id.clone());
    }
    if let Some(Value::Str(id)) = object.get("update_mapgen_id") {
        keys.update.push(id.clone());
    }
    keys
}

/// One mapgen object as the JSON states it: the rows of glyphs and what each glyph means.
///
/// Reference: `mapgen::load` for `method: "json"` (`src/mapgen.cpp`), which reads `rows` and resolves each
/// character through `terrain` or `furniture`. The port keeps the raw rows and the symbol table, because
/// resolving a symbol to an id is the terrain loader's job and doing it twice is how two loaders drift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapgenObject {
    /// The overmap terrain this builds, or the nested id it is filed under.
    pub om_terrain: String,
    /// The rows, top to bottom.
    pub rows: Vec<String>,
    /// What each character means, as terrain ids.
    pub terrain: BTreeMap<char, String>,
    /// What each character means, as furniture ids.
    pub furniture: BTreeMap<char, String>,
    /// The traps a symbol sets (`mapgen.cpp:5470-5480`), placed after the furniture: a minefield is drawn as characters, not placed piece by piece.
    pub traps: BTreeMap<char, String>,
    /// The items a symbol places (`jmapgen_item_group`), the last layer a character can carry.
    pub items: BTreeMap<char, ItemPlacement>,
    /// The monsters the mapgen places (`jmapgen_monster`), which are named by coordinate rather than by symbol.
    pub monsters: Vec<MonsterPlacement>,
    /// How likely this one is when several build the same terrain.
    pub weight: i32,
    /// The toilets a symbol places (`jmapgen_toilet`, `mapgen.cpp:2824-2845`): furniture with water in it, and the **amount** is charges in the water.
    pub toilets: BTreeMap<char, (i32, i32)>,
    /// The liquids a symbol places (`jmapgen_liquid_item`, `mapgen.cpp:2900-2944`): a bottle of water on a counter is this, not an item group.
    pub liquids: BTreeMap<char, LiquidPlacement>,
    /// The fields the mapgen places, which is how an update fills a room with smoke rather than writing terrain.
    pub fields: Vec<Placement>,
    /// The terrain the whole chunk is filled with before the rows are drawn on it (`mapgen.cpp:5168-5172`), which is why a map may write six squares and leave a
    /// twenty-four-square chunk at one terrain rather than blank.
    pub fill_ter: Option<String>,
    /// The terrain whose mapgen is drawn **first**, under this one's rows (`mapgen.cpp:5176-5180` and `:6047-6050`): how a roof keeps the walls of the house below it.
    pub predecessor_mapgen: Option<String>,
}

/// One `items` entry: an item a symbol places (`jmapgen_item_group`, `mapgen.cpp:5320-5340`).
///
/// The reference's item placement carries several shapes - a single `item`, a `group`, a `chance`, an `amount` range, a `repeat` - and the port reads the two that matter to a
/// square: **which item**, and **how often it is there**. A group would need the item-group machinery the loader already has for loot, which is the next layer rather than this one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemPlacement {
    /// The item id the symbol places.
    pub item: Option<String>,
    /// The item **group** the symbol draws from, when it names one instead of an item (`jmapgen_item_group::apply`, `mapgen.cpp:5360-5400`).
    pub group: Option<String>,
    /// How likely it is to be there at all, out of a hundred: one hundred is always.
    pub chance: i32,
    /// How many, as a range the dice answer between.
    pub amount: (i32, i32),
}

/// The `items` map: which item each symbol places, and how many.
///
/// Reference: `jmapgen_item_group::load` (`mapgen.cpp:5320-5340`). A symbol's value is either a string - an item **group** id, which the port does not read yet - or an object
/// naming an `item`, a `group`, a `chance` out of a hundred (defaulting to one hundred) and an `amount` that is a number or a range. The two shapes are told apart here so the
/// writer never has to guess which it was given.
fn read_items(inner: &cdda_json::Object) -> BTreeMap<char, ItemPlacement> {
    let mut items = BTreeMap::new();
    let Some(cdda_json::Value::Object(map)) = inner.get("items") else {
        return items;
    };
    for (symbol, value) in map.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        let placement = match value {
            // An object is a placement; anything else is a group id, which this layer does not read.
            cdda_json::Value::Object(entry) => ItemPlacement {
                item: match entry.get("item") {
                    Some(cdda_json::Value::Str(id)) => Some(id.clone()),
                    _ => None,
                },
                group: match entry.get("group") {
                    Some(cdda_json::Value::Str(id)) => Some(id.clone()),
                    _ => None,
                },
                chance: match entry.get("chance") {
                    Some(cdda_json::Value::Int(chance)) => i32::try_from(*chance).unwrap_or(100),
                    _ => 100,
                },
                amount: read_amount(entry.get("amount")),
            },
            // A bare string is a group id, which is now read rather than skipped: `"items": { "c": "livingroom" }`.
            cdda_json::Value::Str(group) => ItemPlacement {
                item: None,
                group: Some(group.clone()),
                chance: 100,
                amount: (1, 1),
            },
            _ => continue,
        };
        items.insert(symbol, placement);
    }
    items
}

/// The `place_monster` array: which monsters the mapgen places, where, and how many (`jmapgen_monster::load`, `mapgen.cpp:5560-5610`).
fn read_monsters(inner: &cdda_json::Object) -> Vec<MonsterPlacement> {
    let Some(cdda_json::Value::Array(entries)) = inner.get("place_monster") else {
        return Vec::new();
    };
    let mut placements = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(entry) = entry else {
            continue;
        };
        placements.push(MonsterPlacement {
            monster: match entry.get("monster") {
                Some(cdda_json::Value::Str(id)) => Some(id.clone()),
                _ => None,
            },
            group: match entry.get("group") {
                Some(cdda_json::Value::Str(id)) => Some(id.clone()),
                _ => None,
            },
            chance: match entry.get("chance") {
                Some(cdda_json::Value::Int(chance)) => i32::try_from(*chance).unwrap_or(100),
                _ => 100,
            },
            pack_size: read_amount(entry.get("pack_size")),
            x: read_amount(entry.get("x")),
            y: read_amount(entry.get("y")),
            friendly: matches!(entry.get("friendly"), Some(cdda_json::Value::Bool(true))),
        });
    }
    placements
}

/// An `amount` that is a number or a two-number range, defaulting to one.
fn read_amount(value: Option<&cdda_json::Value>) -> (i32, i32) {
    match value {
        Some(cdda_json::Value::Int(one)) => {
            let one = i32::try_from(*one).unwrap_or(1);
            (one, one)
        }
        Some(cdda_json::Value::Array(numbers)) => {
            let low = numbers
                .first()
                .and_then(|one| match one {
                    cdda_json::Value::Int(value) => i32::try_from(*value).ok(),
                    _ => None,
                })
                .unwrap_or(1);
            let high = numbers
                .get(1)
                .and_then(|one| match one {
                    cdda_json::Value::Int(value) => i32::try_from(*value).ok(),
                    _ => None,
                })
                .unwrap_or(low);
            (low, high)
        }
        _ => (1, 1),
    }
}

/// One `place_monster` entry: a monster (or a group of them) placed at a square or a range of squares (`jmapgen_monster`, `mapgen.cpp:5560-5610`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonsterPlacement {
    /// The monster id the entry names.
    pub monster: Option<String>,
    /// The monster **group** the entry names instead, which the port does not draw from yet.
    pub group: Option<String>,
    /// How likely the entry is to spawn at all, out of a hundred: one hundred is always.
    pub chance: i32,
    /// How many, as a range the dice answer between.
    pub pack_size: (i32, i32),
    /// The columns it may stand in, as a range.
    pub x: (i32, i32),
    /// The rows it may stand in, as a range.
    pub y: (i32, i32),
    /// Whether the spawn is on the player's side.
    ///
    /// Reference: `jmapgen_monster`'s `friendly` (`mapgen.cpp:3124`), which `add_spawn` passes to the monster as `friendly = -1` - friendly for good, not for a count.
    pub friendly: bool,
}

/// One `place_fields` entry: a field scattered over a rectangle.
///
/// Reference: `mapgen::place_fields` (`mapgen.cpp`): `x`, `y` and `intensity` are each a number or a `[low, high]` pair rolled between, and
/// `repeat` says how many squares get a patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    /// The field's id, as the data writes it.
    pub field: String,
    /// The columns it may land on.
    pub x: (i32, i32),
    /// The rows it may land on.
    pub y: (i32, i32),
    /// The strength each patch starts with.
    pub intensity: (i32, i32),
    /// How many patches.
    pub repeat: i32,
}

impl MapgenObject {
    /// How wide the rows are, which is the map's own width when the rows agree.
    #[must_use]
    pub fn width(&self) -> usize {
        // Characters, not bytes: a map may use a non-ASCII glyph, and counting bytes would call every
        // row of such a map ragged.
        self.rows.first().map_or(0, |row| row.chars().count())
    }

    /// How many rows there are.
    #[must_use]
    pub fn height(&self) -> usize {
        self.rows.len()
    }

    /// Whether every row is the same width, which the reference requires.
    #[must_use]
    pub fn rows_agree(&self) -> bool {
        let width = self.width();
        self.rows.iter().all(|row| row.chars().count() == width)
    }

    /// The terrain id at a square, if the rows cover it.
    #[must_use]
    pub fn terrain_at(&self, x: usize, y: usize) -> Option<&str> {
        let symbol = self.rows.get(y)?.chars().nth(x)?;
        self.terrain.get(&symbol).map(String::as_str)
    }
}

/// The fields a mapgen places, as the data writes them.
fn read_placements(inner: &Object) -> Vec<Placement> {
    let Some(cdda_json::Value::Array(entries)) = inner.get("place_fields") else {
        return Vec::new();
    };
    let mut placed = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(one) = entry else {
            continue;
        };
        let Some(cdda_json::Value::Str(field)) = one.get("field") else {
            continue;
        };
        placed.push(Placement {
            field: field.clone(),
            x: span(one.get("x")),
            y: span(one.get("y")),
            intensity: span(one.get("intensity")),
            repeat: whole(one.get("repeat")).unwrap_or(1),
        });
    }
    placed
}

/// A number or a `[low, high]` pair, as a span. A missing one is a single square at zero, which is what the reference's default is.
fn span(value: Option<&cdda_json::Value>) -> (i32, i32) {
    match value {
        Some(cdda_json::Value::Array(ends)) => {
            let low = ends.first().and_then(|end| whole(Some(end))).unwrap_or(0);
            let high = ends.get(1).and_then(|end| whole(Some(end))).unwrap_or(low);
            (low.min(high), low.max(high))
        }
        other => {
            let only = whole(other).unwrap_or(0);
            (only, only)
        }
    }
}

/// A whole number from the data, refusing a fractional one rather than rounding a value nobody meant to be fractional.
fn whole(value: Option<&cdda_json::Value>) -> Option<i32> {
    match value {
        Some(cdda_json::Value::Int(number)) => i32::try_from(*number).ok(),
        Some(cdda_json::Value::Float(number)) => as_whole(*number),
        Some(cdda_json::Value::Str(text)) => text.parse::<i32>().ok(),
        _ => None,
    }
}

/// A fractional number as a whole one, or nothing when it is out of range.
#[allow(clippy::cast_possible_truncation)]
fn as_whole(number: f64) -> Option<i32> {
    let rounded = number.round();
    (rounded.abs() < f64::from(i32::MAX)).then_some(rounded as i32)
}

/// Reads every JSON mapgen in load order.
#[must_use]
pub fn load_objects(value: &cdda_json::Value) -> Vec<MapgenObject> {
    let mut out = Vec::new();
    // A file may hold one object or a list of them; the reference accepts both.
    let single = [value.clone()];
    let entries: &[cdda_json::Value] = match value {
        cdda_json::Value::Array(entries) => entries,
        cdda_json::Value::Object(_) => &single,
        _ => &[],
    };
    for entry in entries {
        let cdda_json::Value::Object(object) = entry else {
            continue;
        };
        // Only the JSON method builds from rows; the others are code.
        // Only the JSON method builds from rows; the others are code (`src/mapgen.cpp`).
        if let Some(cdda_json::Value::Str(method)) = object.get("method")
            && method != "json"
        {
            continue;
        }
        // **One map may build several terrains**, and the reference files it under every id it names: `om_terrain` is a string, a list of ids, or a grid of ids (one per
        // column). Reading only the string form dropped every list-shaped mapgen - `nether_glass_deep`, `mansion_+4d` and `barry_mi-go_scout_tower_1` are all written that
        // way - which is why a search for those tiles found none.
        let Some(named) = object
            .get("om_terrain")
            .or_else(|| object.get("update_mapgen_id"))
            .or_else(|| object.get("nested_mapgen_id"))
        else {
            continue;
        };
        let om_terrains = terrain_ids(named);
        if om_terrains.is_empty() {
            continue;
        }
        // The rows live under `object`, not beside `om_terrain`: a mapgen entry looks like
        // {"type":"mapgen","method":"json","om_terrain":"shelter","object":{"rows":[...]}}.
        // Reading the top level instead finds nothing and looks like a broken loader.
        let Some(cdda_json::Value::Object(inner)) = object.get("object") else {
            continue;
        };
        // An **update** mapgen may place fields and no terrain at all, which is why the rows are optional here: the reference allows an update
        // that only fills a room with smoke, and refusing it at the loader is what made eight programs refuse at run time.
        let rows = match inner.get("rows") {
            Some(cdda_json::Value::Array(rows)) => rows,
            _ => &Vec::new(),
        };
        let rows: Vec<String> = rows
            .iter()
            .filter_map(|row| match row {
                cdda_json::Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect();
        let symbols = |key: &str| {
            let mut table = BTreeMap::new();
            if let Some(cdda_json::Value::Object(map)) = inner.get(key) {
                for (symbol, id) in map.members() {
                    if let (Some(character), cdda_json::Value::Str(id)) =
                        (symbol.chars().next(), id)
                    {
                        table.insert(character, id.clone());
                    }
                }
            }
            table
        };
        // Reference: `if( jo.has_member( "fill_ter" ) ) fill_ter = cata::make_value<mapgen_value<ter_id>>(...)` (`mapgen.cpp:5168-5172`). The member may be a palette
        // reference or a weighted list, which the port does not read yet; a plain terrain id is what the data overwhelmingly writes, and the rest is refused here rather
        // than guessed at.
        let fill_ter = match inner.get("fill_ter") {
            Some(cdda_json::Value::Str(id)) => Some(id.clone()),
            _ => None,
        };
        for om_terrain in &om_terrains {
            out.push(MapgenObject {
                om_terrain: om_terrain.clone(),
                rows: rows.clone(),
                terrain: symbols("terrain"),
                furniture: symbols("furniture"),
                traps: symbols("traps"),
                items: read_items(inner),
                monsters: read_monsters(inner),
                toilets: read_toilets(inner.get("toilets")),
                liquids: read_liquids(inner.get("liquids")),
                fields: read_placements(inner),
                fill_ter: fill_ter.clone(),
                predecessor_mapgen: match inner.get("predecessor_mapgen") {
                    Some(cdda_json::Value::Str(pre)) if pre != "null" => Some(pre.clone()),
                    _ => None,
                },
                weight: match object.get("weight") {
                    Some(cdda_json::Value::Int(value)) => i32::try_from(*value).unwrap_or(1000),
                    _ => 1000,
                },
            });
        }
    }
    out
}

/// **Every overmap terrain one mapgen builds**, in the data's order.
///
/// Reference: `mapgen_factory`'s registration, which a JSON mapgen reaches through `mapgen::load` - the member may be one id, a list of ids, or a grid of ids with one per
/// column (`mapgen.cpp`, and the `om_terrain` shapes the data actually writes).
#[must_use]
pub fn terrain_ids(named: &cdda_json::Value) -> Vec<String> {
    match named {
        cdda_json::Value::Str(one) => vec![one.clone()],
        cdda_json::Value::Array(rows) => {
            let mut found = Vec::new();
            for row in rows {
                match row {
                    cdda_json::Value::Str(id) => found.push(id.clone()),
                    cdda_json::Value::Array(columns) => {
                        for column in columns {
                            if let cdda_json::Value::Str(id) = column {
                                found.push(id.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
            found
        }
        _ => Vec::new(),
    }
}

/// What placing a map produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Placed {
    /// How many squares were filled with a terrain.
    pub squares: usize,
    /// The symbols that had no meaning, which the reference treats as an error in the data.
    pub unknown_symbols: Vec<char>,
    /// The furniture standing on each square, by position, as the map declares it.
    pub furniture: Vec<(i32, i32, String)>,
}

impl MapgenObject {
    /// Writes this map into a level at an origin, resolving each glyph through its own symbol table.
    ///
    /// Reference: `mapgen::load` for the JSON method (`src/mapgen.cpp`), which walks the rows and resolves
    /// every symbol through `terrain` (then `furniture` for what stands on it). A symbol the map does not
    /// explain is a data error in the reference rather than a blank square, so it is collected here and
    /// left as the level's blank rather than silently becoming floor.
    ///
    /// `terrain_id_of` turns an id the content uses into the number the level stores; it is the terrain
    /// loader's job, not this one's.
    pub fn place(
        &self,
        level: &mut cdda_core_types::level::Level,
        origin: (i32, i32),
        blank: u32,
        terrain_id_of: impl Fn(&str) -> Option<u32>,
    ) -> Placed {
        let mut placed = Placed::default();
        for (row_index, row) in self.rows.iter().enumerate() {
            for (column, symbol) in row.chars().enumerate() {
                let Ok(dx) = i32::try_from(column) else {
                    continue;
                };
                let Ok(dy) = i32::try_from(row_index) else {
                    continue;
                };
                let (x, y) = (origin.0 + dx, origin.1 + dy);
                let terrain = match self.terrain.get(&symbol) {
                    Some(id) => {
                        let resolved = terrain_id_of(id);
                        if resolved.is_none() {
                            placed.unknown_symbols.push(symbol);
                        }
                        resolved.unwrap_or(blank)
                    }
                    None => {
                        // Furniture-only squares have no terrain of their own; they are floor under it.
                        if self.furniture.contains_key(&symbol) {
                            blank
                        } else {
                            placed.unknown_symbols.push(symbol);
                            blank
                        }
                    }
                };
                level.set_terrain(x, y, terrain, blank);
                placed.squares += 1;
                if let Some(furniture) = self.furniture.get(&symbol) {
                    placed.furniture.push((x, y, furniture.clone()));
                }
            }
        }
        placed
    }
}

/// The generator keys the game registers in **code** rather than in the data.
///
/// Reference: `mapgen.cpp`'s built-in registrations - the forests, the rivers with their curve and mouth variants, the lake and ocean shores, the ravine edges and the subway pieces.
/// A registry built by scanning the data files comes up short by exactly these twenty-six, which is why the port carries them explicitly and a test names every one.
pub const CODE_REGISTERED: [&str; 26] = [
    "forest",
    "forest_thick",
    "forest_water",
    "lake_shore",
    "ocean_shore",
    "ravine_edge",
    "ravine_floor_edge",
    "river",
    "river_c_not_ne",
    "river_c_not_nw",
    "river_c_not_se",
    "river_c_not_sw",
    "river_ne",
    "river_nw",
    "river_se",
    "river_sw",
    "lab_subway_curved",
    "lab_subway_end",
    "lab_subway_four_way",
    "lab_subway_straight",
    "lab_subway_tee",
    "subway_curved",
    "subway_end",
    "subway_four_way",
    "subway_straight",
    "subway_tee",
];

/// The keys the data writes that are **not** chunks: the lab's floorplan selectors and the plaza's placeholder.
///
/// They are `om_terrain` ids, so a scan of the data finds them, but the reference never registers a generator under them. Offering them would be offering chunks the game cannot
/// generate.
pub const NOT_CHUNKS: [&str; 4] = ["lab_1side", "lab_4side", "lab_finale_1level", "null"];

/// Resolves a chunk's `rows` into terrain ids through its own `terrain` map.
///
/// Reference: `mapgen.cpp`'s symbol lookup - the `terrain` object maps a **symbol to the id it draws**, as in `".": "t_floor"`, and every character in `rows` has to be found in it.
/// A space draws nothing and is not a terrain, which is why the answer is `Option`: a chunk's grid may be mostly empty.
///
/// # Errors
///
/// Returns the character that has no id in this mapgen's terrain, because a silent default would draw the wrong chunk rather than say the data is unreadable.
pub fn resolve_rows(rows: &[String], terrain: &Object) -> Result<Vec<Vec<Option<String>>>, String> {
    let mut by_symbol = std::collections::BTreeMap::new();
    read_terrain_map(terrain, &mut by_symbol);
    let mut resolved = Vec::with_capacity(rows.len());
    for row in rows {
        let mut line = Vec::with_capacity(row.len());
        for symbol in row.chars() {
            if symbol == ' ' {
                line.push(None);
                continue;
            }
            match by_symbol.get(&symbol) {
                Some(id) => line.push(Some(id.clone())),
                None => return Err(format!("'{symbol}' has no terrain in this mapgen")),
            }
        }
        resolved.push(line);
    }
    Ok(resolved)
}

/// Reads a terrain map into symbol, then id - the direction **every** terrain map in this format is written in.
///
/// This is the port's longest-standing misreading and the whole of it: `terrain` is symbol-first (`".": "t_floor"`), and the port read it id-first for many rounds. Each symptom
/// looked like a different bug - a chunk that needed a fill, a palette that resolved to single letters, stairs that came out as plain floor - and all of them were this one direction.
///
/// A symbol may carry several ids, or an id with a weight. The first id is what a draw needs: choosing between them is a roll the caller owns.
/// One `liquids` entry: a liquid on the floor, in charges (`jmapgen_liquid_item`, `mapgen.cpp:2900-2944`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidPlacement {
    /// The liquid's item id.
    pub liquid: String,
    /// How many charges, as a range the dice answer between. **`-1` means "whatever the item type's own count is"**, which is the reference's default.
    pub amount: (i32, i32),
    /// One chance in this many of being there at all: the reference's `chance( jsi, "chance", 1, 1 )` is a **1-in-n** draw, not a percentage.
    pub chance: i32,
}

/// Reads a mapgen's `liquids` object, each symbol's liquid, amount and chance.
fn read_liquids(
    object: Option<&cdda_json::Value>,
) -> std::collections::BTreeMap<char, LiquidPlacement> {
    let mut into = std::collections::BTreeMap::new();
    let Some(cdda_json::Value::Object(members)) = object else {
        return into;
    };
    for (symbol, value) in members.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        let cdda_json::Value::Object(spec) = value else {
            continue;
        };
        let liquid = match spec.get("liquid") {
            Some(cdda_json::Value::Str(id)) => id.clone(),
            _ => continue,
        };
        into.insert(
            symbol,
            LiquidPlacement {
                liquid,
                amount: read_amount(spec.get("amount")),
                chance: match spec.get("chance") {
                    Some(cdda_json::Value::Int(value)) => i32::try_from(*value).unwrap_or(1).max(1),
                    _ => 1,
                },
            },
        );
    }
    into
}

/// Reads a mapgen's `toilets` object: each symbol's **amount**, the charges of water in it.
///
/// Reference: `jmapgen_toilet` (`mapgen.cpp:2824-2845`) reads `amount` as a `jmapgen_int` defaulting to zero, and a zero means `place_toilet`'s own default of 24 charges
/// (`map.h:1679`). The port keeps the zero rather than substituting here, because the roll happens when the square is drawn.
fn read_toilets(object: Option<&cdda_json::Value>) -> std::collections::BTreeMap<char, (i32, i32)> {
    let mut into = std::collections::BTreeMap::new();
    let Some(cdda_json::Value::Object(members)) = object else {
        return into;
    };
    for (symbol, value) in members.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        let amount = match value {
            cdda_json::Value::Object(spec) => read_amount(spec.get("amount")),
            _ => read_amount(None),
        };
        into.insert(symbol, amount);
    }
    into
}

fn read_terrain_map(terrain: &Object, into: &mut std::collections::BTreeMap<char, String>) {
    for (symbol, value) in terrain.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        match value {
            Value::Str(id) => {
                into.insert(symbol, id.clone());
            }
            Value::Array(items) => {
                for item in items {
                    if let Value::Str(id) = item {
                        into.insert(symbol, id.clone());
                        break;
                    }
                }
            }
            // **A parameter with a fallback**: `{"param": "exterior_wall_type", "fallback": "t_brick_wall"}`. The format says what to
            // draw when nobody supplies the parameter - the fallback - so a reader with no parameter source still draws the chunk the
            // game draws by default. The shelter's walls are this shape, and their concrete variant comes from the overmap special that
            // placed the shelter, not from the mapgen.
            Value::Object(spec) => {
                if let Some(Value::Str(fallback)) = spec.get("fallback") {
                    into.insert(symbol, fallback.clone());
                }
            }
            _ => {}
        }
    }
}

/// One entry of a chunk's `palettes` list, in the shape the data writes it.
///
/// The data uses three: a plain name, a weighted `distribution` the game rolls between, and a `param` whose value the overmap special supplies. The port keeps them apart instead of
/// flattening the list to strings, because two of the three cannot be resolved without a decision the caller owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteRef {
    /// `"shelter"`.
    Named(String),
    /// `{"distribution": [["cabin_palette", 1], ["cabin_palette_abandoned", 1]]}` - the game rolls.
    Distribution(Vec<(String, i32)>),
    /// `{"param": "food_type"}` - the overmap special decides.
    Param(String),
}

impl PaletteRef {
    /// The name to gather, or `None` when the choice belongs to a roll or to a parameter the port does not have yet.
    ///
    /// A distribution takes its first candidate, which is the same bargain the furniture picker strikes: the data's weights stay in the
    /// structure for when the caller can supply a roll, and the first candidate is what a reader with no dice can draw.
    #[must_use]
    pub fn resolved(&self) -> Option<&str> {
        match self {
            PaletteRef::Named(name) => Some(name),
            PaletteRef::Distribution(candidates) => {
                candidates.first().map(|(name, _)| name.as_str())
            }
            PaletteRef::Param(_) => None,
        }
    }
}

/// Reads a chunk's `palettes` list into references that remember which shape they came in.
#[must_use]
pub fn palette_refs(names: &[String]) -> Vec<PaletteRef> {
    names
        .iter()
        .map(|name| PaletteRef::Named(name.clone()))
        .collect()
}

/// The symbols a chunk can draw with, gathered through its palettes and their palettes.
///
/// Reference: `mapgen.cpp`'s palette resolution - a mapgen names palettes, a palette may name more, and the union of their `terrain` maps is what the chunk's rows are drawn from.
/// Later names win, which is why the walk runs in reverse order before recursing, and a name already on the path is skipped: the data is acyclic, but a resolver that hangs on a cycle
/// costs an afternoon, and the guard is three lines.
#[must_use]
pub fn palette_symbols(
    refs: &[PaletteRef],
    lookup: &dyn Fn(&str) -> Option<Object>,
) -> std::collections::BTreeMap<char, String> {
    fn gather(
        refs: &[PaletteRef],
        lookup: &dyn Fn(&str) -> Option<Object>,
        visited: &mut std::collections::BTreeSet<String>,
        into: &mut std::collections::BTreeMap<char, String>,
    ) {
        for reference in refs.iter().rev() {
            let Some(name) = reference.resolved() else {
                continue;
            };
            if !visited.insert(name.to_string()) {
                continue;
            }
            let Some(palette) = lookup(name) else {
                continue;
            };
            if let Some(Value::Object(terrain)) = palette.get("terrain") {
                // A palette's map is written symbol first, exactly like a mapgen's own.
                read_terrain_map(terrain, into);
            }
            if let Some(Value::Array(more)) = palette.get("palettes") {
                let more: Vec<PaletteRef> = more.iter().filter_map(read_palette_ref).collect();
                gather(&more, lookup, visited, into);
            }
        }
    }

    let mut into = std::collections::BTreeMap::new();
    let mut visited = std::collections::BTreeSet::new();
    gather(refs, lookup, &mut visited, &mut into);
    into
}

/// Reads one entry of a `palettes` list, in any of the three shapes the data uses.
#[must_use]
pub fn read_palette_ref(value: &Value) -> Option<PaletteRef> {
    match value {
        Value::Str(name) => Some(PaletteRef::Named(name.clone())),
        Value::Object(spec) => {
            if let Some(Value::Str(param)) = spec.get("param") {
                return Some(PaletteRef::Param(param.clone()));
            }
            let Some(Value::Array(list)) = spec.get("distribution") else {
                return None;
            };
            let candidates: Vec<(String, i32)> = list
                .iter()
                .filter_map(|candidate| {
                    let Value::Array(pair) = candidate else {
                        return None;
                    };
                    let (Some(Value::Str(name)), weight) = (pair.first(), pair.get(1)) else {
                        return None;
                    };
                    let weight = match weight {
                        Some(Value::Int(weight)) => i32::try_from(*weight).ok()?,
                        _ => 1,
                    };
                    Some((name.clone(), weight))
                })
                .collect();
            (!candidates.is_empty()).then_some(PaletteRef::Distribution(candidates))
        }
        _ => None,
    }
}

/// Draws one chunk: its rows become terrain ids through its palettes and its own terrain map.
///
/// Reference: `mapgen.cpp`'s draw step, which resolves a symbol through the mapgen's own `terrain` first and its palettes afterwards - the entry's map is what a chunk author writes
/// last, so it wins. A space draws nothing, as in [`resolve_rows`].
///
/// # Errors
///
/// Returns the character that no palette and no inline map supplies, because a silent default would draw a chunk the game never draws.
pub fn draw_chunk(
    rows: &[String],
    terrain: Option<&Object>,
    palettes: &[PaletteRef],
    lookup: &dyn Fn(&str) -> Option<Object>,
    fill: Option<&str>,
) -> Result<Vec<Vec<Option<String>>>, String> {
    let mut symbols = palette_symbols(palettes, lookup);
    if let Some(terrain) = terrain {
        // The entry's own map wins over its palettes, and it is written the same way they are: symbol first.
        read_terrain_map(terrain, &mut symbols);
    }
    let mut drawn = Vec::with_capacity(rows.len());
    for row in rows {
        let mut line = Vec::with_capacity(row.len());
        for symbol in row.chars() {
            if symbol == ' ' {
                line.push(None);
                continue;
            }
            // **A symbol the terrain maps do not know is not an error**: `furniture`, `items` and `traps` have their own symbol
            // maps, and a square that carries one of those keeps the chunk's `fill_ter`. Only a symbol no layer claims would be a
            // mistake, and the port cannot tell that apart from here - so it leaves the fill, which is what the reference draws.
            match symbols.get(&symbol) {
                Some(id) => line.push(Some(id.clone())),
                None => line.push(fill.map(ToString::to_string)),
            }
        }
        drawn.push(line);
    }
    Ok(drawn)
}

/// The furniture a symbol can place: one or more candidate ids, each with a weight.
///
/// Reference: `mapgen.cpp`'s furniture map, where a symbol maps either to a list of ids - each weighted one - or to a list of `[id, weight]` pairs. A symbol with several candidates is
/// decided by a roll, which the caller supplies, in keeping with this port's rule that dice belong to the caller.
#[must_use]
pub fn furniture_choices(
    furniture: &Object,
) -> std::collections::BTreeMap<char, Vec<(String, i32)>> {
    let mut choices: std::collections::BTreeMap<char, Vec<(String, i32)>> =
        std::collections::BTreeMap::new();
    for (symbol, value) in furniture.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        let list = match value {
            Value::Array(items) => items,
            Value::Str(id) => {
                choices.entry(symbol).or_default().push((id.clone(), 1));
                continue;
            }
            _ => continue,
        };
        for item in list {
            match item {
                Value::Str(id) => choices.entry(symbol).or_default().push((id.clone(), 1)),
                Value::Array(pair) => {
                    let (Some(Value::Str(id)), Some(Value::Int(weight))) =
                        (pair.first(), pair.get(1))
                    else {
                        continue;
                    };
                    #[allow(clippy::cast_possible_truncation)]
                    choices
                        .entry(symbol)
                        .or_default()
                        .push((id.clone(), *weight as i32));
                }
                _ => {}
            }
        }
    }
    choices
}

/// One candidate, given the roll that chooses it.
///
/// Reference: `random_entry` over weighted candidates - the roll runs from zero to the total weight minus one, and the walk takes the first candidate whose running weight exceeds it. So
/// weights `[25, 20, 5, 5]` give the first candidate rolls 0 to 24, the second 25 to 44, and so on.
#[must_use]
pub fn pick_furniture(choices: &[(String, i32)], roll: i32) -> Option<&str> {
    let mut remaining = roll;
    for (id, weight) in choices {
        if remaining < *weight {
            return Some(id.as_str());
        }
        remaining -= *weight;
    }
    choices.last().map(|(id, _)| id.as_str())
}
