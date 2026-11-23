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
    /// How big the chunk is, in squares (`mapgen.cpp:5105-5110` reads `mapgensize`, whose default is the overmap tile's 24).
    ///
    /// A **nested** chunk declares its own, and a small one is the common case: a 4x4 sewer blockage or a 1x1 piece of furniture. The size is what the reference's
    /// `check_inbounds` compares a placement's coordinates against (`:5130-5140`), so an entry naming a square outside the chunk is deferred rather than drawn.
    pub mapgensize: (i32, i32),
    /// How likely this one is when several build the same terrain.
    pub weight: i32,
    /// The toilets a symbol places (`jmapgen_toilet`, `mapgen.cpp:2824-2845`): furniture with water in it, and the **amount** is charges in the water.
    pub toilets: BTreeMap<char, (i32, i32)>,
    /// The liquids a symbol places (`jmapgen_liquid_item`, `mapgen.cpp:2900-2944`): a bottle of water on a counter is this, not an item group.
    pub liquids: BTreeMap<char, LiquidPlacement>,
    /// The item groups scattered over a rectangle (`jmapgen_item_group`, `mapgen.cpp:2983-3016`): how a room's drawers and counters get their contents, 428 times in the data.
    pub place_items: Vec<AreaItemPlacement>,
    /// The single items and groups dropped on one random square of a rectangle (`jmapgen_loot`, `mapgen.cpp:3019-3072`), which is the mapgen's most used member: 669 times.
    pub place_loot: Vec<LootPlacement>,
    /// Furniture with something **inside** it (`jmapgen_sealed_item`, `mapgen.cpp:3913-4040`): a locker and its contents, 77 times in the data.
    pub sealed_items: BTreeMap<char, SealedPlacement>,
    /// Vending machines (`jmapgen_vending_machine`, `mapgen.cpp:2784-2819`): 118 times in the data, and each one is furniture **plus a stock**.
    pub vending_machines: BTreeMap<char, VendingPlacement>,
    /// Signs and graffiti (`jmapgen_sign`, `mapgen.cpp:2683-2724`): furniture that carries **words**.
    pub signs: BTreeMap<char, SignPlacement>,
    /// Gas pumps (`jmapgen_gaspump`, `mapgen.cpp:2850-2895`): a **terrain** and the fuel in it, which is what a station's forecourt is made of.
    pub gas_pumps: BTreeMap<char, GasPumpPlacement>,
    /// Nested mapgens (`jmapgen_nested`, `mapgen.cpp:4206-4500`): a whole chunk, drawn shifted, 335 times in the data - the way a room's interior is chosen.
    pub nested: BTreeMap<char, NestedPlacement>,
    /// Terrain scattered over a rectangle (`jmapgen_terrain` placed through `place_terrain`): a **random square per repeat**, not the whole box.
    pub place_terrain: Vec<AreaSet>,
    /// Furniture scattered over a rectangle (`place_furniture`): the same shape as terrain, a different layer.
    pub place_furniture: Vec<AreaSet>,
    /// Traps scattered over a rectangle (`place_traps`): a minefield is this, not a symbol map.
    pub place_traps: Vec<AreaSet>,
    /// The single items a symbol places (`jmapgen_spawn_item` under `item`): the port read the group form and not this one, and the data uses this one most.
    pub items_single: BTreeMap<char, SingleItemPlacement>,
    /// The field patches a symbol places (`jmapgen_field` under `fields`): smoke, fire and blood as **symbols**, not rectangles.
    pub field_symbols: BTreeMap<char, FieldPatch>,
    /// Computer terminals (`jmapgen_computer`, `mapgen.cpp:3841-3912`): 85 times in the data, and each one is a **record** - a name, a security level and the actions it offers.
    pub computers: BTreeMap<char, ComputerPlacement>,
    /// Strips a rectangle down to its terrain before the rest is drawn (`jmapgen_remove_all`, `mapgen.cpp:4169-4205`): the **first phase** of a mapgen.
    pub remove_all: Vec<RemoveArea>,
    /// The monsters a symbol places (`jmapgen_monster` under `monster`): 1,748 uses in the data, the most of any member, and a symbol is how a mapgen says "a zombie stands
    /// here" without naming a rectangle.
    pub monster_symbols: BTreeMap<char, SymbolMonster>,
    /// How far the whole chunk is turned, as a range of quarter turns (`mapgen.cpp:5172-5173`, `6072`): **2,198 uses**, and a mapgen without it is not turned at all.
    pub rotation: (i32, i32),
    /// The `set` list's **point** entries (`jmapgen_setmap`, `mapgen.cpp:5432-5480`): 1,129 of them, and the oldest way the data says "this square is that".
    pub set_points: Vec<SetPoint>,
    /// The `set` list's radiation entries (`mapgen.cpp:1712-1713`, `5676-5679`): 26 of them, and the only `set` op whose value is a **number**, not an id.
    pub set_radiation: Vec<SetRadiation>,
    /// The `set` list's **variable** entries (`mapgen.cpp:1736-1737`, `5735-5737`): 230 of them, each naming a square the rest of the game will read by name.
    pub set_variables: Vec<SetVariable>,
    /// Rubble a symbol places (`jmapgen_make_rubble` under `rubble`, `mapgen.cpp:3796-3837`): furniture, a floor, and the debris that goes with it.
    pub rubble: BTreeMap<char, RubblePlacement>,
    /// Rubble placed over a rectangle (`place_rubble`, `:5409`): the same piece with the square written out.
    pub place_rubble: Vec<AreaRubble>,
    /// Nested mapgens placed over a rectangle instead of at a symbol (`place_nested`): the same piece as `nested`, 1,382 times, with the dice choosing the marker.
    pub place_nested: Vec<AreaNested>,
    /// Single items placed over a rectangle (`place_item`, 165 uses): the `item` symbol map with the square written out.
    pub place_item: Vec<AreaSingleItem>,
    /// Signs placed over a rectangle (`place_signs`, 127 uses): the `signs` symbol map with the square written out.
    pub place_signs: Vec<AreaSign>,
    /// Liquids placed over a rectangle (`place_liquids`, `mapgen.cpp:5398`): the `liquids` symbol map with the square written out.
    pub place_liquids: Vec<AreaLiquid>,
    /// The fields the mapgen places, which is how an update fills a room with smoke rather than writing terrain.
    pub fields: Vec<Placement>,
    /// The terrain the whole chunk is filled with before the rows are drawn on it (`mapgen.cpp:5168-5172`), which is why a map may write six squares and leave a
    /// twenty-four-square chunk at one terrain rather than blank.
    pub fill_ter: Option<String>,
    /// The terrain whose mapgen is drawn **first**, under this one's rows (`mapgen.cpp:5176-5180` and `:6047-6050`): how a roof keeps the walls of the house below it.
    pub predecessor_mapgen: Option<String>,
    /// The mapgen drawn below when the **place** has no predecessor of its own (`mapgen.cpp:5182`, `:6055-6062`).
    ///
    /// Its presence is what the reference's `expects_predecessor` answers (`:6101-6104`): a mapgen that expects a predecessor falls back to this one when the mapgendata
    /// has none to offer. 57 of the data's mapgens declare one.
    pub fallback_predecessor_mapgen: Option<String>,
}

/// The chunk side a mapgen has when it declares none: the overmap tile's, in squares (`mapgen.cpp:5105-5110`).
pub const DEFAULT_SIDE: i32 = 24;

impl MapgenObject {
    /// The chunk's side, which is what a placement outside of is deferred by (`check_inbounds`, `mapgen.cpp:5130-5140`).
    #[must_use]
    pub fn size(&self) -> (i32, i32) {
        self.mapgensize
    }
}

/// Reads a mapgen's declared size, or the overmap tile's when it declares none.
fn read_size(value: Option<&cdda_json::Value>) -> (i32, i32) {
    let Some(cdda_json::Value::Array(pair)) = value else {
        return (DEFAULT_SIDE, DEFAULT_SIDE);
    };
    let one = match pair.first() {
        Some(cdda_json::Value::Int(value)) => Some(*value),
        _ => None,
    };
    let other = match pair.get(1) {
        Some(cdda_json::Value::Int(value)) => Some(*value),
        _ => None,
    };
    match (one, other) {
        (Some(x), Some(y)) => (
            i32::try_from(x).unwrap_or(DEFAULT_SIDE),
            i32::try_from(y).unwrap_or(DEFAULT_SIDE),
        ),
        _ => (DEFAULT_SIDE, DEFAULT_SIDE),
    }
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

/// Reads a mapgen's `mapping` object into the three symbol tables it may fill.
///
/// Reference: the palette reader's `mapping` member, whose entries are **bundles**: `{ "L": { "furniture": "f_locker", "items": [ ... ] } }`. A member already given by the flat
/// tables keeps its value, because the flat table is the more specific statement and the mapping is the palette's fallback - which is the order the reference's own palette merge
/// produces.
///
/// The terrain and furniture members are read; the item members are a **named gap** (see below), because a bundle lists them and the symbol table holds one per symbol.
fn read_mapping(
    mapping: Option<&cdda_json::Value>,
    terrain: &mut BTreeMap<char, String>,
    furniture: &mut BTreeMap<char, String>,
) {
    let Some(cdda_json::Value::Object(members)) = mapping else {
        return;
    };
    for (symbol, value) in members.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        let cdda_json::Value::Object(bundle) = value else {
            // A bare string is a terrain id, which is the shortest form the member has.
            if let cdda_json::Value::Str(id) = value {
                terrain.entry(symbol).or_insert_with(|| id.clone());
            }
            continue;
        };
        if let Some(cdda_json::Value::Str(id)) = bundle.get("terrain") {
            terrain.entry(symbol).or_insert_with(|| id.clone());
        }
        if let Some(cdda_json::Value::Str(id)) = bundle.get("furniture") {
            furniture.entry(symbol).or_insert_with(|| id.clone());
        }
        // **Named gap**: a bundle's `items` and `item` members are not read here. A bundle states its items as a *list* of placements
        // (`"items": [ { "item": "bed", "chance": 60 }, ... ]`), and the port's symbol table holds **one** placement per symbol - so reading them needs the table to become a
        // list first, which is a change to the model and the writer rather than to this reader. Reading only the first would drop the rest silently, and that is the failure this
        // project refuses.
    }
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
        // **`mapping` is the third way the data names a symbol** (`mapgen.cpp`, the palette reader): one object per symbol holding a bundle of members - a terrain, a
        // furniture, an item group, a single item - where `terrain`, `furniture` and `items` are one member each per symbol. Seven mapgens use it, and a loader that reads
        // only the three flat tables draws their symbols as nothing at all.
        let mut mapped_terrain = symbols("terrain");
        let mut mapped_furniture = symbols("furniture");
        let mapped_items = read_items(inner);
        read_mapping(
            inner.get("mapping"),
            &mut mapped_terrain,
            &mut mapped_furniture,
        );
        for om_terrain in &om_terrains {
            out.push(MapgenObject {
                om_terrain: om_terrain.clone(),
                rows: rows.clone(),
                terrain: mapped_terrain.clone(),
                furniture: mapped_furniture.clone(),
                traps: symbols("traps"),
                items: mapped_items.clone(),
                monsters: read_monsters(inner),
                toilets: read_toilets(inner.get("toilets")),
                liquids: read_liquids(inner.get("liquids")),
                place_items: read_area_items(inner.get("place_items")),
                place_loot: read_loot(inner.get("place_loot")),
                sealed_items: read_sealed(inner.get("sealed_item")),
                vending_machines: read_vending(inner.get("vendingmachines")),
                signs: read_signs(inner.get("signs")),
                gas_pumps: read_gas_pumps(inner.get("gaspumps")),
                nested: read_nested(inner.get("nested")),
                place_terrain: read_area_sets(inner.get("place_terrain"), "ter"),
                place_furniture: read_area_sets(inner.get("place_furniture"), "furn"),
                place_traps: read_area_sets(inner.get("place_traps"), "trap"),
                items_single: read_single_items(inner.get("item")),
                field_symbols: read_field_symbols(inner.get("fields")),
                computers: read_computers(inner.get("computers")),
                remove_all: read_remove_areas(inner.get("remove_all")),
                monster_symbols: {
                    // **Two members name the monster a glyph stands for**, and the data uses both: `monster`, whose values are ids, and `monsters`, whose values are
                    // objects (`{ "Z": { "monster": "GROUP_VANILLA_NO_FERAL" } }`). 68 entries use the second form, and reading only the first left their glyphs empty.
                    // The flat member is read last so it wins a symbol both name, which is what "the more specific statement" means here.
                    let mut monsters = read_monster_symbols_of(inner.get("monsters"), true);
                    monsters.extend(read_monster_symbols(inner.get("monster")));
                    monsters
                },
                rotation: span(inner.get("rotation")),
                set_points: read_set_points(inner.get("set")),
                set_radiation: read_set_radiation(inner.get("set")),
                set_variables: read_set_variables(inner.get("set")),
                rubble: read_rubble(inner.get("rubble")),
                place_rubble: read_area_rubble(inner.get("place_rubble")),
                place_nested: read_area_nested(inner.get("place_nested")),
                place_item: read_area_single_items(inner.get("place_item")),
                place_signs: read_area_signs(inner.get("place_signs")),
                place_liquids: read_area_liquids(inner.get("place_liquids")),
                fields: read_placements(inner),
                fill_ter: fill_ter.clone(),
                predecessor_mapgen: match inner.get("predecessor_mapgen") {
                    Some(cdda_json::Value::Str(pre)) if pre != "null" => Some(pre.clone()),
                    _ => None,
                },
                fallback_predecessor_mapgen: match inner.get("fallback_predecessor_mapgen") {
                    Some(cdda_json::Value::Str(pre)) if pre != "null" => Some(pre.clone()),
                    _ => None,
                },
                mapgensize: read_size(inner.get("mapgensize")),
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
/// One `set` variable entry: a name given to one square (`mapgen.cpp:1736-1737`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetVariable {
    /// The name, which the rest of the game reads the square under.
    pub name: String,
    /// The columns it may land in.
    pub x: (i32, i32),
    /// The rows it may land in.
    pub y: (i32, i32),
}

/// Reads the `variable` entries of a mapgen's `set` list.
fn read_set_variables(list: Option<&cdda_json::Value>) -> Vec<SetVariable> {
    let Some(cdda_json::Value::Array(entries)) = list else {
        return Vec::new();
    };
    let mut placed = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(one) = entry else {
            continue;
        };
        let is_variable = ["point", "set"]
            .iter()
            .any(|key| matches!(one.get(key), Some(cdda_json::Value::Str(op)) if op == "variable"));
        if !is_variable {
            continue;
        }
        let Some(cdda_json::Value::Str(name)) = one.get("id") else {
            continue;
        };
        placed.push(SetVariable {
            name: name.clone(),
            x: span(one.get("x")),
            y: span(one.get("y")),
        });
    }
    placed
}

/// One `rubble` entry: what a rubble square is made of (`jmapgen_make_rubble`, `mapgen.cpp:3796-3837`).
///
/// The defaults are the reference's: `f_rubble` furniture, `t_dirt` floor, no items, no overwrite (`:3799-3811`). **Named gap**: the non-overwrite branch first destroys bashable
/// furniture and terrain (`:3246-3257`), and the port's terrain and furniture carry no bash results yet, so only the impassable-check part is missed - the furniture is set and the
/// floor is left alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RubblePlacement {
    /// The furniture the debris stands as.
    pub rubble_type: String,
    /// The floor under it, used when `overwrite` is set.
    pub floor_type: String,
    /// Whether the debris drops items.
    pub items: bool,
    /// Whether the floor is forced - the reference's `overwrite` (`:3242-3244`).
    pub overwrite: bool,
}

/// One `place_rubble` entry: rubble whose square the entry writes out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaRubble {
    /// What the rubble is made of.
    pub rubble: RubblePlacement,
    /// The columns it may land in.
    pub x: (i32, i32),
    /// The rows it may land in.
    pub y: (i32, i32),
}

/// Reads one rubble specification, from either form.
fn read_one_rubble(spec: &cdda_json::Value) -> Option<RubblePlacement> {
    let cdda_json::Value::Object(one) = spec else {
        return None;
    };
    Some(RubblePlacement {
        rubble_type: match one.get("rubble_type") {
            Some(cdda_json::Value::Str(id)) => id.clone(),
            _ => "f_rubble".to_owned(),
        },
        floor_type: match one.get("floor_type") {
            Some(cdda_json::Value::Str(id)) => id.clone(),
            _ => "t_dirt".to_owned(),
        },
        items: matches!(one.get("items"), Some(cdda_json::Value::Bool(true))),
        overwrite: matches!(one.get("overwrite"), Some(cdda_json::Value::Bool(true))),
    })
}

/// Reads a mapgen's `rubble` object, the symbol map.
fn read_rubble(
    object: Option<&cdda_json::Value>,
) -> std::collections::BTreeMap<char, RubblePlacement> {
    let mut placed = std::collections::BTreeMap::new();
    let Some(cdda_json::Value::Object(members)) = object else {
        return placed;
    };
    for (symbol, value) in members.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        if let Some(rubble) = read_one_rubble(value) {
            placed.insert(symbol, rubble);
        }
    }
    placed
}

/// Reads a mapgen's `place_rubble` list.
fn read_area_rubble(list: Option<&cdda_json::Value>) -> Vec<AreaRubble> {
    let Some(cdda_json::Value::Array(entries)) = list else {
        return Vec::new();
    };
    let mut placed = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(one) = entry else {
            continue;
        };
        if let Some(rubble) = read_one_rubble(entry) {
            placed.push(AreaRubble {
                rubble,
                x: span(one.get("x")),
                y: span(one.get("y")),
            });
        }
    }
    placed
}

/// One `set` radiation entry: an absolute dose on one square or a range of squares (`mapgen.cpp:1712-1713`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetRadiation {
    /// The dose, as a range the dice answer between.
    pub amount: (i32, i32),
    /// The columns it may land in.
    pub x: (i32, i32),
    /// The rows it may land in.
    pub y: (i32, i32),
    /// How many squares it writes, as a range.
    pub repeat: (i32, i32),
}

/// Reads the `radiation` entries of a mapgen's `set` list.
fn read_set_radiation(list: Option<&cdda_json::Value>) -> Vec<SetRadiation> {
    let Some(cdda_json::Value::Array(entries)) = list else {
        return Vec::new();
    };
    let mut placed = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(one) = entry else {
            continue;
        };
        let is_radiation = ["point", "set"].iter().any(
            |key| matches!(one.get(key), Some(cdda_json::Value::Str(op)) if op == "radiation"),
        );
        if !is_radiation {
            continue;
        }
        placed.push(SetRadiation {
            amount: span(one.get("amount")),
            x: span(one.get("x")),
            y: span(one.get("y")),
            repeat: span(one.get("repeat")),
        });
    }
    placed
}

/// Which layer a `set` point writes, or what it takes away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetKind {
    /// Terrain.
    Terrain,
    /// Furniture.
    Furniture,
    /// A trap.
    Trap,
    /// A trap's removal (`mapgen.cpp:5589-5592`, `trap_remove`).
    TrapRemove,
    /// The items on the square (`item_remove`).
    ItemRemove,
    /// The fields on the square (`field_remove`).
    FieldRemove,
    /// The creature standing there (`creature_remove`).
    CreatureRemove,
}

/// The shape a `set` entry draws (`jmapgen_setmap`, `mapgen.cpp:5432-5480`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetShape {
    /// One square, drawn from the entry's ranges.
    Point,
    /// A Bresenham line from `( x, y )` to `( x2, y2 )` - **which does not include its own start** (`line_to` returns the cells after `p1`, `line.cpp:224-238`).
    Line,
    /// A filled rectangle between the two corners, both included (`draw_square`, `drawing_primitives.cpp:22-36`).
    Square,
}

/// One `set` entry that writes terrain, furniture or a trap (`jmapgen_setmap`, `mapgen.cpp:5432-5480`).
///
/// **Named gaps**: `variable`, `bash`, `burn`, `radiation` and the three `*_remove` ops are not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPoint {
    /// Which layer it writes.
    pub kind: SetKind,
    /// What shape it draws.
    pub shape: SetShape,
    /// The id it writes there.
    pub id: String,
    /// The columns its first corner may land in.
    pub x: (i32, i32),
    /// The rows its first corner may land in.
    pub y: (i32, i32),
    /// The columns its second corner may land in, for a line or a square.
    pub x2: (i32, i32),
    /// The rows its second corner may land in, for a line or a square.
    pub y2: (i32, i32),
    /// How many times it draws, as a range: a point entry is drawn `repeat` times like any other piece, and a line or a square once per repeat.
    pub repeat: (i32, i32),
}

/// Reads a mapgen's `set` list, keeping the point entries that name terrain, furniture or a trap.
fn read_set_points(list: Option<&cdda_json::Value>) -> Vec<SetPoint> {
    let Some(cdda_json::Value::Array(entries)) = list else {
        return Vec::new();
    };
    let mut placed = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(one) = entry else {
            continue;
        };
        let named = ["point", "set", "line", "square"]
            .iter()
            .find_map(|key| match one.get(key) {
                Some(cdda_json::Value::Str(op)) => Some((*key, op.as_str())),
                _ => None,
            });
        let Some((form, op)) = named else {
            continue;
        };
        let kind = match op {
            "terrain" => SetKind::Terrain,
            "furniture" => SetKind::Furniture,
            "trap" => SetKind::Trap,
            "trap_remove" => SetKind::TrapRemove,
            "item_remove" => SetKind::ItemRemove,
            "field_remove" => SetKind::FieldRemove,
            "creature_remove" => SetKind::CreatureRemove,
            _ => continue,
        };
        // A removal names no id: that is what makes it a removal rather than a write.
        let id = match kind {
            SetKind::TrapRemove
            | SetKind::ItemRemove
            | SetKind::FieldRemove
            | SetKind::CreatureRemove => String::new(),
            _ => match one.get("id") {
                Some(cdda_json::Value::Str(id)) => id.clone(),
                _ => continue,
            },
        };
        let shape = match form {
            "line" => SetShape::Line,
            "square" => SetShape::Square,
            _ => SetShape::Point,
        };
        placed.push(SetPoint {
            kind,
            shape,
            id,
            x: span(one.get("x")),
            y: span(one.get("y")),
            x2: span(one.get("x2")),
            y2: span(one.get("y2")),
            repeat: span(one.get("repeat")),
        });
    }
    placed
}

/// One `place_liquids` entry: a liquid whose square the entry writes out (`jmapgen_liquid_item` under `place_liquids`, `mapgen.cpp:5398`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaLiquid {
    /// The liquid's item id.
    pub liquid: String,
    /// The charges, before the hundred the piece multiplies in.
    pub amount: (i32, i32),
    /// The one-in-**n** chance the piece uses, which is the opposite of a percentage.
    pub chance: i32,
    /// The columns it may land in.
    pub x: (i32, i32),
    /// The rows it may land in.
    pub y: (i32, i32),
}

/// Reads a mapgen's `place_liquids` list.
fn read_area_liquids(list: Option<&cdda_json::Value>) -> Vec<AreaLiquid> {
    let Some(cdda_json::Value::Array(entries)) = list else {
        return Vec::new();
    };
    let mut placed = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(one) = entry else {
            continue;
        };
        let Some(cdda_json::Value::Str(liquid)) = one.get("liquid") else {
            continue;
        };
        placed.push(AreaLiquid {
            liquid: liquid.clone(),
            amount: span(one.get("amount")),
            chance: match one.get("chance") {
                Some(cdda_json::Value::Int(value)) => i32::try_from(*value).unwrap_or(1).max(1),
                _ => 1,
            },
            x: span(one.get("x")),
            y: span(one.get("y")),
        });
    }
    placed
}

/// One `place_item` entry: a single item whose square the entry writes out (`jmapgen_spawn_item` under `place_item`, `mapgen.cpp:5390`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaSingleItem {
    /// The item id.
    pub item: String,
    /// How many each spawn drops, as a range.
    pub amount: (i32, i32),
    /// A percentage of one spawn.
    pub chance: i32,
    /// The columns it may land in.
    pub x: (i32, i32),
    /// The rows it may land in.
    pub y: (i32, i32),
}

/// Reads a mapgen's `place_item` list.
fn read_area_single_items(list: Option<&cdda_json::Value>) -> Vec<AreaSingleItem> {
    let Some(cdda_json::Value::Array(entries)) = list else {
        return Vec::new();
    };
    let mut placed = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(one) = entry else {
            continue;
        };
        let Some(cdda_json::Value::Str(item)) = one.get("item") else {
            continue;
        };
        placed.push(AreaSingleItem {
            item: item.clone(),
            amount: span(one.get("amount")),
            chance: match one.get("chance") {
                Some(cdda_json::Value::Int(value)) => {
                    i32::try_from(*value).unwrap_or(100).clamp(0, 100)
                }
                _ => 100,
            },
            x: span(one.get("x")),
            y: span(one.get("y")),
        });
    }
    placed
}

/// One `place_signs` entry: a sign whose square the entry writes out (`jmapgen_sign` under `place_signs`, `mapgen.cpp:5393`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaSign {
    /// The furniture it stands as.
    pub furniture: String,
    /// What it says.
    pub text: String,
    /// The columns it may land in.
    pub x: (i32, i32),
    /// The rows it may land in.
    pub y: (i32, i32),
}

/// Reads a mapgen's `place_signs` list.
fn read_area_signs(list: Option<&cdda_json::Value>) -> Vec<AreaSign> {
    let Some(cdda_json::Value::Array(entries)) = list else {
        return Vec::new();
    };
    let mut placed = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(one) = entry else {
            continue;
        };
        placed.push(AreaSign {
            furniture: match one.get("furniture") {
                Some(cdda_json::Value::Str(id)) => id.clone(),
                _ => "f_sign".to_owned(),
            },
            text: match one.get("signage") {
                Some(cdda_json::Value::Str(text)) => text.clone(),
                _ => String::new(),
            },
            x: span(one.get("x")),
            y: span(one.get("y")),
        });
    }
    placed
}

/// One `place_nested` entry: a nested chunk whose marker the dice pick from a rectangle (`jmapgen_nested`, `mapgen.cpp:4490-4505`).
///
/// The chunks are the same weighted list the symbol form carries; only the square's source differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaNested {
    /// The columns the marker may land in.
    pub x: (i32, i32),
    /// The rows the marker may land in.
    pub y: (i32, i32),
    /// The weighted chunks it may draw.
    pub chunks: Vec<(String, i32)>,
}

/// Reads a mapgen's `place_nested` list.
fn read_area_nested(list: Option<&cdda_json::Value>) -> Vec<AreaNested> {
    let Some(cdda_json::Value::Array(entries)) = list else {
        return Vec::new();
    };
    let mut placed = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(one) = entry else {
            continue;
        };
        placed.push(AreaNested {
            x: span(one.get("x")),
            y: span(one.get("y")),
            chunks: read_chunks(one.get("chunks")),
        });
    }
    placed
}

/// One `monster` entry: a monster a **symbol** places (`jmapgen_monster`, `mapgen.cpp:3095-3280`).
///
/// The piece is the same one `place_monster` uses: it may name a monster or a group, it has a chance out of a hundred, a pack size and a friendly flag. The difference is
/// where the square comes from - here the row's symbol, there the entry's rectangle - so the type carries no square at all and the writer supplies one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolMonster {
    /// The monster id, when the entry names one.
    pub monster: Option<String>,
    /// The monster group, when the entry names one instead.
    pub group: Option<String>,
    /// How likely the spawn is at all, out of a hundred.
    pub chance: i32,
    /// How many, as a range.
    pub pack_size: (i32, i32),
    /// Whether the spawn is on the player's side.
    pub friendly: bool,
}

/// Reads a mapgen's `monster` object, the symbol map of monsters.
fn read_monster_symbols(
    object: Option<&cdda_json::Value>,
) -> std::collections::BTreeMap<char, SymbolMonster> {
    read_monster_symbols_of(object, false)
}

/// Reads a monster symbol map, and how its `monster` key is meant.
///
/// The two members disagree about that key, and the data is the reason: `monster`'s values name a **monster**, while `monsters`' values name a **group** under the same key
/// (`jmapgen_monster_group`'s `id( jsi.get_member( "monster" ) )` is a `mongroup_id`, `mapgen.cpp:3088`). One reader with a flag keeps the shapes in one place instead of two
/// nearly identical walkers.
fn read_monster_symbols_of(
    object: Option<&cdda_json::Value>,
    monster_means_group: bool,
) -> std::collections::BTreeMap<char, SymbolMonster> {
    let mut placed = std::collections::BTreeMap::new();
    let Some(cdda_json::Value::Object(members)) = object else {
        return placed;
    };
    for (symbol, value) in members.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        let cdda_json::Value::Object(spec) = value else {
            continue;
        };
        let named = |name: &str| match spec.get(name) {
            Some(cdda_json::Value::Str(text)) => Some(text.clone()),
            _ => None,
        };
        placed.insert(
            symbol,
            SymbolMonster {
                monster: if monster_means_group {
                    None
                } else {
                    named("monster")
                },
                group: if monster_means_group {
                    named("monster")
                } else {
                    named("group")
                },
                chance: match spec.get("chance") {
                    Some(cdda_json::Value::Int(value)) => {
                        i32::try_from(*value).unwrap_or(100).clamp(0, 100)
                    }
                    _ => 100,
                },
                pack_size: span(spec.get("pack_size")),
                friendly: matches!(spec.get("friendly"), Some(cdda_json::Value::Bool(true))),
            },
        );
    }
    placed
}

/// One `remove_all` entry: a rectangle stripped of furniture, items, traps and fields (`jmapgen_remove_all`, `mapgen.cpp:4169-4205`).
///
/// Unlike every other rectangle in a mapgen this one is **filled**: `apply` walks `tripoint_range( start, end )` (`:4183`) and clears each square, so the rectangle is the
/// area itself and there is no `repeat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveArea {
    /// The columns.
    pub x: (i32, i32),
    /// The rows.
    pub y: (i32, i32),
}

/// Reads a mapgen's `remove_all` list.
fn read_remove_areas(list: Option<&cdda_json::Value>) -> Vec<RemoveArea> {
    let Some(cdda_json::Value::Array(entries)) = list else {
        return Vec::new();
    };
    let mut placed = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(one) = entry else {
            continue;
        };
        placed.push(RemoveArea {
            x: span(one.get("x")),
            y: span(one.get("y")),
        });
    }
    placed
}

/// One `computers` entry: a terminal's record (`jmapgen_computer`, `mapgen.cpp:3841-3912`).
///
/// The record is what a hacking pass will read: a name, a refusal message, a security level, and the actions the terminal offers. **Named gaps**: an option's own
/// parameters (`ter_set`, `furn_set`, an elevator's target), the `failures` list, `target` and `chat_topics`' content are not carried; the port keeps the names and actions
/// the data writes so the record is the reference's record and not a summary of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputerPlacement {
    /// What the terminal calls itself.
    pub name: String,
    /// What it says when access is refused.
    pub access_denied: String,
    /// The security level a hack has to beat.
    pub security: i32,
    /// The actions it offers, as name-and-action pairs.
    pub options: Vec<(String, String)>,
    /// The effect-on-conditions the data attaches.
    pub eocs: Vec<String>,
    /// The chat topics it can start.
    pub chat_topics: Vec<String>,
}

/// Reads a mapgen's `computers` object.
fn read_computers(
    object: Option<&cdda_json::Value>,
) -> std::collections::BTreeMap<char, ComputerPlacement> {
    let mut placed = std::collections::BTreeMap::new();
    let Some(cdda_json::Value::Object(members)) = object else {
        return placed;
    };
    for (symbol, value) in members.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        let cdda_json::Value::Object(spec) = value else {
            continue;
        };
        let text = |name: &str| match spec.get(name) {
            Some(cdda_json::Value::Str(text)) => text.clone(),
            _ => String::new(),
        };
        let list = |name: &str| -> Vec<String> {
            match spec.get(name) {
                Some(cdda_json::Value::Array(entries)) => entries
                    .iter()
                    .filter_map(|entry| match entry {
                        cdda_json::Value::Str(text) => Some(text.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            }
        };
        let options = match spec.get("options") {
            Some(cdda_json::Value::Array(entries)) => entries
                .iter()
                .filter_map(|entry| match entry {
                    cdda_json::Value::Object(option) => Some((
                        match option.get("name") {
                            Some(cdda_json::Value::Str(name)) => name.clone(),
                            _ => String::new(),
                        },
                        match option.get("action") {
                            Some(cdda_json::Value::Str(action)) => action.clone(),
                            _ => String::new(),
                        },
                    )),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        placed.insert(
            symbol,
            ComputerPlacement {
                name: text("name"),
                access_denied: text("access_denied"),
                security: match spec.get("security") {
                    Some(cdda_json::Value::Int(value)) => i32::try_from(*value).unwrap_or(0),
                    _ => 0,
                },
                options,
                eocs: list("eocs"),
                chat_topics: list("chat_topics"),
            },
        );
    }
    placed
}

/// One `item` entry: a **single item** a symbol places (`jmapgen_spawn_item`, `mapgen.cpp:3477-3525`).
///
/// The chance is the reference's percentage-of-a-spawn: a hundred is **one** spawn, and anything less is `roll_remainder( chance / 100 )` spawns, each of which drops
/// `amount` items (`:3512-3519`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingleItemPlacement {
    /// The item id.
    pub item: String,
    /// How many each spawn drops, as a range.
    pub amount: (i32, i32),
    /// A percentage of one spawn.
    pub chance: i32,
}

/// Reads a mapgen's `item` object, the single-item symbol map.
fn read_single_items(
    object: Option<&cdda_json::Value>,
) -> std::collections::BTreeMap<char, SingleItemPlacement> {
    let mut placed = std::collections::BTreeMap::new();
    let Some(cdda_json::Value::Object(members)) = object else {
        return placed;
    };
    for (symbol, value) in members.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        let cdda_json::Value::Object(spec) = value else {
            continue;
        };
        let Some(cdda_json::Value::Str(item)) = spec.get("item") else {
            continue;
        };
        placed.insert(
            symbol,
            SingleItemPlacement {
                item: item.clone(),
                amount: span(spec.get("amount")),
                chance: match spec.get("chance") {
                    Some(cdda_json::Value::Int(value)) => {
                        i32::try_from(*value).unwrap_or(100).clamp(0, 100)
                    }
                    _ => 100,
                },
            },
        );
    }
    placed
}

/// One `fields` entry: a field patch a symbol places (`jmapgen_field`, `mapgen.cpp:2542-2584`).
///
/// The chance is a percentage (`x_in_y( chance, 100 )`, `:2575`), the intensity is drawn from a **list** of allowed strengths, and `remove` takes the field away instead of
/// adding it. **Named gap**: `remove` and `age` are not read - the port's fields carry no age yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldPatch {
    /// The field's id.
    pub field: String,
    /// The strengths it may take, any one of which the game picks.
    pub intensity: (i32, i32),
    /// A percentage chance of the patch being placed.
    pub chance: i32,
}

/// Reads a mapgen's `fields` object, the symbol map of field patches.
fn read_field_symbols(
    object: Option<&cdda_json::Value>,
) -> std::collections::BTreeMap<char, FieldPatch> {
    let mut placed = std::collections::BTreeMap::new();
    let Some(cdda_json::Value::Object(members)) = object else {
        return placed;
    };
    for (symbol, value) in members.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        let cdda_json::Value::Object(spec) = value else {
            continue;
        };
        let Some(cdda_json::Value::Str(field)) = spec.get("field") else {
            continue;
        };
        placed.insert(
            symbol,
            FieldPatch {
                field: field.clone(),
                intensity: span(spec.get("intensity")),
                chance: match spec.get("chance") {
                    Some(cdda_json::Value::Int(value)) => {
                        i32::try_from(*value).unwrap_or(100).clamp(0, 100)
                    }
                    _ => 100,
                },
            },
        );
    }
    placed
}

/// One `place_terrain`, `place_furniture` or `place_traps` entry: **a random square per repeat** in a rectangle.
///
/// Reference: `jmapgen_objects::apply` (`mapgen.cpp:6161-6164`) applies the piece `repeat` times at `where.x.get(), where.y.get()` - and a `jmapgen_int`'s `get` draws
/// **one** value from its range, so a rectangle is a field of candidate squares, not a box that is filled. The data relies on this: a minefield is `repeat: 200` over a
/// 24×24 rectangle, not a wall of traps on every square.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaSet {
    /// The id placed: a terrain, a furniture or a trap, according to which list it came from.
    pub id: String,
    /// The columns it may land on.
    pub x: (i32, i32),
    /// The rows it may land on.
    pub y: (i32, i32),
    /// How many squares get it.
    pub repeat: (i32, i32),
}

/// Reads one of the three `place_*` lists, each entry naming its id under `member`.
fn read_area_sets(list: Option<&cdda_json::Value>, member: &str) -> Vec<AreaSet> {
    let Some(cdda_json::Value::Array(entries)) = list else {
        return Vec::new();
    };
    let mut placed = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(one) = entry else {
            continue;
        };
        let Some(cdda_json::Value::Str(id)) = one.get(member) else {
            continue;
        };
        placed.push(AreaSet {
            id: id.clone(),
            x: span(one.get("x")),
            y: span(one.get("y")),
            repeat: span(one.get("repeat")),
        });
    }
    placed
}

/// One `nested` entry: the weighted list of nested mapgens a symbol may draw (`jmapgen_nested`, `mapgen.cpp:4206-4500`).
///
/// The reference keeps **two** lists - `chunks` and `else_chunks` - and chooses between them by the neighbours' terrain, joins, flags, predecessors and z level
/// (`:4488-4496`). **Named gap**: the port reads `chunks` only; a mapgen whose `else_chunks` the data wrote gets the `chunks` list, and the neighbour conditions are not
/// evaluated. That is a deliberate first slice: the weighted list and the anchoring are the part every nested mapgen uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NestedPlacement {
    /// The nested mapgen ids and their weights, as the data writes them under `chunks`.
    pub chunks: Vec<(String, i32)>,
}

/// Reads a mapgen's `nested` object.
fn read_nested(
    object: Option<&cdda_json::Value>,
) -> std::collections::BTreeMap<char, NestedPlacement> {
    let mut placed = std::collections::BTreeMap::new();
    let Some(cdda_json::Value::Object(members)) = object else {
        return placed;
    };
    for (symbol, value) in members.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        let cdda_json::Value::Object(spec) = value else {
            continue;
        };
        let entries = read_chunks(spec.get("chunks"));
        if !entries.is_empty() {
            placed.insert(symbol, NestedPlacement { chunks: entries });
        }
    }
    placed
}

/// Reads a nested chunk list: the data writes a chunk as a pair, `[ "id", weight ]`, and sometimes as a bare id.
fn read_chunks(list: Option<&cdda_json::Value>) -> Vec<(String, i32)> {
    let Some(cdda_json::Value::Array(chunks)) = list else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for chunk in chunks {
        match chunk {
            cdda_json::Value::Array(pair) => {
                if let Some(cdda_json::Value::Str(id)) = pair.first() {
                    let weight = match pair.get(1) {
                        Some(cdda_json::Value::Int(value)) => {
                            i32::try_from(*value).unwrap_or(100).max(0)
                        }
                        _ => 100,
                    };
                    entries.push((id.clone(), weight));
                }
            }
            cdda_json::Value::Str(id) => entries.push((id.clone(), 100)),
            _ => {}
        }
    }
    entries
}

/// One `gaspumps` entry: a pump's fuel capacity and, when the data says, which fuel (`jmapgen_gaspump`, `mapgen.cpp:2850-2895`).
///
/// The `amount` is in **tens of litres of fuel**: the reference multiplies it by a hundred (`:2877`), so an amount of one is a hundred charges. No amount means a
/// randomly filled pump - ten thousand to fifty thousand charges (`:2879-2881`) - and no `fuel` means diesel one time in four and gasoline otherwise (`mapgen.cpp:7294`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GasPumpPlacement {
    /// The amount's range, before the hundred.
    pub amount: (i32, i32),
    /// Which fuel, when the data names one.
    pub fuel: Option<String>,
}

/// Reads a mapgen's `gaspumps` object.
fn read_gas_pumps(
    object: Option<&cdda_json::Value>,
) -> std::collections::BTreeMap<char, GasPumpPlacement> {
    let mut placed = std::collections::BTreeMap::new();
    let Some(cdda_json::Value::Object(members)) = object else {
        return placed;
    };
    for (symbol, value) in members.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        let cdda_json::Value::Object(spec) = value else {
            continue;
        };
        placed.insert(
            symbol,
            GasPumpPlacement {
                amount: span(spec.get("amount")),
                fuel: match spec.get("fuel") {
                    Some(cdda_json::Value::Str(id)) => Some(id.clone()),
                    _ => None,
                },
            },
        );
    }
    placed
}

/// One `signs` entry: furniture that carries words (`jmapgen_sign`, `mapgen.cpp:2683-2724`).
///
/// The furniture defaults to `f_sign` and must carry the reference's `SIGN` flag; the text comes from `signage` (a written string) or a `snippet` category the game rolls
/// from. **Named gap**: there is no snippet registry in the port, so a `snippet` entry places the furniture and **no text** - the reference would roll one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignPlacement {
    /// The furniture the words sit on.
    pub furniture: String,
    /// What it says, when the data writes it out.
    pub text: String,
}

/// Reads a mapgen's `signs` object.
fn read_signs(
    object: Option<&cdda_json::Value>,
) -> std::collections::BTreeMap<char, SignPlacement> {
    let mut placed = std::collections::BTreeMap::new();
    let Some(cdda_json::Value::Object(members)) = object else {
        return placed;
    };
    for (symbol, value) in members.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        let cdda_json::Value::Object(spec) = value else {
            continue;
        };
        placed.insert(
            symbol,
            SignPlacement {
                furniture: match spec.get("furniture") {
                    Some(cdda_json::Value::Str(id)) => id.clone(),
                    _ => "f_sign".to_owned(),
                },
                text: match spec.get("signage") {
                    Some(cdda_json::Value::Str(text)) => text.clone(),
                    _ => String::new(),
                },
            },
        );
    }
    placed
}

/// One `vendingmachines` entry: furniture **plus a stock** (`jmapgen_vending_machine`, `mapgen.cpp:2784-2819`).
///
/// Four flags choose the furniture - `place_vending` (`mapgen.cpp:7306-7336`) picks among six ids by `powered`, `reinforced` and `networked` - and the `item_group` is the
/// machine's stock, defaulting to `default_vending_machine` when the data names none (`:2797-2801`).
#[expect(
    clippy::struct_excessive_bools,
    reason = "the reference's three furniture flags are independent booleans (`powered`, `reinforced`, `networked`) that choose among six ids, and `lootable` is a fourth, different axis; an enum would have to spell out a cross product the data writes directly"
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendingPlacement {
    /// The item group the machine is stocked with.
    pub group: String,
    /// Whether the machine is armoured, which chooses the `_reinforced` furniture.
    pub reinforced: bool,
    /// Whether it stands connected to a network, which chooses the `_networked` furniture.
    pub networked: bool,
    /// Whether it has power, which chooses the lit furniture over the `_off` one.
    pub powered: bool,
    /// Whether it may be found ransacked. **Named gap**: the port stocks every machine; the reference's ransack branch needs the day count and a bash.
    pub lootable: bool,
}

/// Reads a mapgen's `vendingmachines` object.
fn read_vending(
    object: Option<&cdda_json::Value>,
) -> std::collections::BTreeMap<char, VendingPlacement> {
    let mut placed = std::collections::BTreeMap::new();
    let Some(cdda_json::Value::Object(members)) = object else {
        return placed;
    };
    for (symbol, value) in members.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        let cdda_json::Value::Object(spec) = value else {
            continue;
        };
        let group = match spec.get("item_group") {
            Some(cdda_json::Value::Str(id)) => id.clone(),
            _ => "default_vending_machine".to_owned(),
        };
        let flag = |name: &str| matches!(spec.get(name), Some(cdda_json::Value::Bool(true)));
        placed.insert(
            symbol,
            VendingPlacement {
                group,
                reinforced: flag("reinforced"),
                networked: flag("networked"),
                powered: flag("powered"),
                lootable: flag("lootable"),
            },
        );
    }
    placed
}

/// One `sealed_item` entry: furniture placed **over** what it holds (`jmapgen_sealed_item`, `mapgen.cpp:3913-4040`).
///
/// The artist is explicit: the furniture is cleared, the item or group is placed, and the furniture is set back **on top** (`:4049-4060`) - so the contents are inside it,
/// not beside it, which is what makes a locker's contents stay put until it is opened or broken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedPlacement {
    /// The furniture that seals it.
    pub furniture: String,
    /// A percentage chance of the entry being placed at all; a hundred is always.
    pub chance: i32,
    /// The single item inside, when the entry names one.
    pub item: Option<SealedItem>,
    /// The item group inside, when it names one instead. The reference wants at least one of the two (`:3937-3941`).
    pub group: Option<SealedGroup>,
}

/// The single item a sealed square holds: `jmapgen_spawn_item`'s three numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedItem {
    /// The item id.
    pub id: String,
    /// How many, as a range.
    pub amount: (i32, i32),
    /// A percentage chance of it being there at all.
    pub chance: i32,
}

/// The item group a sealed square holds: `jmapgen_item_group`'s group and chance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedGroup {
    /// The group id - the member is called `item` and holds a group, as in `place_items`.
    pub group: String,
    /// A percentage chance of the group being drawn at all.
    pub chance: i32,
}

/// Reads a mapgen's `sealed_item` object.
fn read_sealed(
    object: Option<&cdda_json::Value>,
) -> std::collections::BTreeMap<char, SealedPlacement> {
    let mut placed = std::collections::BTreeMap::new();
    let Some(cdda_json::Value::Object(members)) = object else {
        return placed;
    };
    for (symbol, value) in members.members() {
        let Some(symbol) = symbol.chars().next() else {
            continue;
        };
        let cdda_json::Value::Object(spec) = value else {
            continue;
        };
        let Some(cdda_json::Value::Str(furniture)) = spec.get("furniture") else {
            continue;
        };
        let item = match spec.get("item") {
            Some(cdda_json::Value::Object(inner)) => match inner.get("item") {
                Some(cdda_json::Value::Str(id)) => Some(SealedItem {
                    id: id.clone(),
                    amount: span(inner.get("amount")),
                    chance: match inner.get("chance") {
                        Some(cdda_json::Value::Int(value)) => {
                            i32::try_from(*value).unwrap_or(100).clamp(0, 100)
                        }
                        _ => 100,
                    },
                }),
                _ => None,
            },
            _ => None,
        };
        let group = match spec.get("items") {
            Some(cdda_json::Value::Object(inner)) => match inner.get("item") {
                Some(cdda_json::Value::Str(id)) => Some(SealedGroup {
                    group: id.clone(),
                    chance: match inner.get("chance") {
                        Some(cdda_json::Value::Int(value)) => {
                            i32::try_from(*value).unwrap_or(100).clamp(0, 100)
                        }
                        _ => 100,
                    },
                }),
                _ => None,
            },
            _ => None,
        };
        if item.is_none() && group.is_none() {
            continue;
        }
        placed.insert(
            symbol,
            SealedPlacement {
                furniture: furniture.clone(),
                chance: match spec.get("chance") {
                    Some(cdda_json::Value::Int(value)) => {
                        i32::try_from(*value).unwrap_or(100).clamp(0, 100)
                    }
                    _ => 100,
                },
                item,
                group,
            },
        );
    }
    placed
}

/// One `place_loot` entry: **one** item or group, on one random square of a rectangle (`jmapgen_loot`, `mapgen.cpp:3019-3072`).
///
/// Two members make it different from `place_items` even though both name a rectangle: the chance is a percentage of **being placed at all** (`rng( 0, 99 ) < chance`,
/// `:3058`), and there is **no `repeat`** - the class parses none, so one entry is one roll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootPlacement {
    /// The single item id, when the entry names one.
    pub item: Option<String>,
    /// The item group id, when it names one instead. The reference requires exactly one of the two (`:3032-3034`).
    pub group: Option<String>,
    /// The percentage chance of placing anything at all.
    pub chance: i32,
    /// The columns.
    pub x: (i32, i32),
    /// The rows.
    pub y: (i32, i32),
}

/// Reads a mapgen's `place_loot` list.
fn read_loot(list: Option<&cdda_json::Value>) -> Vec<LootPlacement> {
    let Some(cdda_json::Value::Array(entries)) = list else {
        return Vec::new();
    };
    let mut placed = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(one) = entry else {
            continue;
        };
        let item = match one.get("item") {
            Some(cdda_json::Value::Str(id)) => Some(id.clone()),
            _ => None,
        };
        let group = match one.get("group") {
            Some(cdda_json::Value::Str(id)) => Some(id.clone()),
            _ => None,
        };
        // The reference throws on both or neither (`:3032`); a reader that silently took one would place loot the data did not ask for.
        if item.is_some() == group.is_some() {
            continue;
        }
        placed.push(LootPlacement {
            item,
            group,
            chance: match one.get("chance") {
                Some(cdda_json::Value::Int(value)) => {
                    i32::try_from(*value).unwrap_or(100).clamp(0, 100)
                }
                _ => 100,
            },
            x: span(one.get("x")),
            y: span(one.get("y")),
        });
    }
    placed
}

/// One `place_items` entry: an item **group** scattered over a rectangle (`jmapgen_item_group`, `mapgen.cpp:2983-3016`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaItemPlacement {
    /// The group id - the member is called `item` and holds a **group**, which is the reference's own spelling.
    pub group: String,
    /// A **percentage**, unlike the symbol maps' one-in-n: `chance( jsi, "chance", 100, 100 )` (`mapgen.cpp:2990`).
    pub chance: i32,
    /// How many times the entry is placed, as a range.
    pub repeat: (i32, i32),
    /// The columns.
    pub x: (i32, i32),
    /// The rows.
    pub y: (i32, i32),
}

/// Reads a mapgen's `place_items` list.
fn read_area_items(list: Option<&cdda_json::Value>) -> Vec<AreaItemPlacement> {
    let Some(cdda_json::Value::Array(entries)) = list else {
        return Vec::new();
    };
    let mut placed = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(one) = entry else {
            continue;
        };
        // A plain group name; an inline array of entries is a group the data writes on the spot, which this reader does not take.
        let Some(cdda_json::Value::Str(group)) = one.get("item") else {
            continue;
        };
        placed.push(AreaItemPlacement {
            group: group.clone(),
            chance: match one.get("chance") {
                Some(cdda_json::Value::Int(value)) => {
                    i32::try_from(*value).unwrap_or(100).clamp(0, 100)
                }
                _ => 100,
            },
            repeat: span(one.get("repeat")),
            x: span(one.get("x")),
            y: span(one.get("y")),
        });
    }
    placed
}

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
