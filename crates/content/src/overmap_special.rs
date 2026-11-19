//! Overmap specials: the buildings and landmarks that are placed into the world as a whole.
//!
//! Reference: `overmap_special.h` / `overmap_special.cpp` and the loader in `overmap_specials.cpp`. A special is a
//! **shape**: a list of overmap terrain tiles at offsets, plus the rules that decide whether it may be placed - where
//! it can sit, how far from a city, how rare it is.
//!
//! This module reads the data and nothing else. Placement is a separate job, and it is the job the world layer has
//! been waiting for: the port can grow a town and put streets and woodland down, and until now could not say what a
//! building *is*.

use cdda_json::Value;

/// One terrain tile of a special, at an offset from the special's own origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialTerrain {
    /// The offset, in overmap tiles, including the z level.
    pub offset: (i32, i32, i32),
    /// The terrain placed there.
    pub terrain: String,
}

/// A connection a special makes to a road or a subway, which the world has to join up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    /// Where the connection's road tile sits.
    pub offset: (i32, i32, i32),
    /// The terrain kind it connects as, e.g. `road`.
    pub terrain: String,
    /// Which connection it plugs into.
    pub connection: String,
    /// Where the road it joins comes from.
    pub from: (i32, i32, i32),
}

/// One special, as loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OvermapSpecial {
    /// The id the JSON gives it.
    pub id: String,
    /// The tiles it occupies.
    pub overmaps: Vec<SpecialTerrain>,
    /// How many times it may appear, as a percentage range of the overmap's capacity.
    pub occurrences: (i32, i32),
    /// Flags such as `OVERMAP_UNIQUE` and `GLOBALLY_UNIQUE`.
    pub flags: Vec<String>,
    /// The kinds of ground it may be placed on.
    pub locations: Vec<String>,
    /// How far from a city it may be, in overmap tiles.
    pub city_distance: (i32, i32),
    /// The range of city sizes it belongs to.
    pub city_sizes: (i32, i32),
    /// Whether the placement may rotate it.
    pub rotate: bool,
    /// Larger priorities are placed first.
    pub priority: i32,
    /// The roads and subways it connects to.
    pub connections: Vec<Connection>,
    /// `mutable` specials are edited by the world, which the port does not do yet.
    pub subtype: Option<String>,
}

#[allow(clippy::cast_possible_truncation)]
fn number(value: Option<&Value>, fallback: i32) -> i32 {
    match value {
        Some(Value::Int(n)) => i32::try_from(*n).unwrap_or(fallback),
        // A count written as `75.0` is a count; truncating it is the reference's own reading of a float in an
        // integer field.
        Some(Value::Float(n)) => *n as i32,
        _ => fallback,
    }
}

fn pair(value: Option<&Value>, fallback: (i32, i32)) -> (i32, i32) {
    let Some(Value::Array(items)) = value else {
        return fallback;
    };
    if items.len() != 2 {
        return fallback;
    }
    (
        number(items.first(), fallback.0),
        number(items.get(1), fallback.1),
    )
}

fn point(value: Option<&Value>) -> (i32, i32, i32) {
    match value {
        Some(Value::Array(items)) if items.len() == 3 => (
            number(items.first(), 0),
            number(items.get(1), 0),
            number(items.get(2), 0),
        ),
        _ => (0, 0, 0),
    }
}

fn text(value: Option<&Value>, fallback: &str) -> String {
    match value {
        Some(Value::Str(text)) => text.clone(),
        _ => fallback.to_string(),
    }
}

fn list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Reads one special, or nothing when the entry is not one.
#[must_use]
pub fn read_special(value: &Value) -> Option<OvermapSpecial> {
    let Value::Object(fields) = value else {
        return None;
    };
    if fields.get("type") != Some(&Value::Str("overmap_special".to_string())) {
        return None;
    }
    // Two specials describe their shape as a **graph of named nodes** for the mutable generator rather than as a list
    // of tiles. They are still specials and are still counted; their tiles arrive when the mutable generator does.
    let overmaps = match fields.get("overmaps") {
        Some(Value::Array(tiles)) => tiles,
        _ => &Vec::new(),
    };
    let overmaps = overmaps
        .iter()
        .filter_map(|tile| {
            let Value::Object(tile) = tile else {
                return None;
            };
            Some(SpecialTerrain {
                offset: point(tile.get("point")),
                terrain: text(tile.get("overmap"), ""),
            })
        })
        .collect();
    let connections = match fields.get("connections") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                let Value::Object(entry) = item else {
                    return None;
                };
                Some(Connection {
                    offset: point(entry.get("point")),
                    terrain: text(entry.get("terrain"), ""),
                    connection: text(entry.get("connection"), ""),
                    from: point(entry.get("from")),
                })
            })
            .collect(),
        _ => Vec::new(),
    };
    Some(OvermapSpecial {
        id: text(fields.get("id"), ""),
        overmaps,
        occurrences: pair(fields.get("occurrences"), (0, 100)),
        flags: list(fields.get("flags")),
        locations: list(fields.get("locations")),
        city_distance: pair(fields.get("city_distance"), (-1, 1000)),
        city_sizes: pair(fields.get("city_sizes"), (0, 1000)),
        // The reference's default is true: a special rotates unless it says otherwise.
        rotate: !matches!(fields.get("rotate"), Some(Value::Bool(false))),
        priority: number(fields.get("priority"), 0),
        connections,
        subtype: match fields.get("subtype") {
            Some(Value::Str(text)) => Some(text.clone()),
            _ => None,
        },
    })
}

/// Reads every special in a list of parsed JSON documents.
#[must_use]
pub fn load_specials(documents: &[Value]) -> Vec<OvermapSpecial> {
    let mut specials = Vec::new();
    for document in documents {
        let entries: &[Value] = match document {
            Value::Array(entries) => entries,
            other => std::slice::from_ref(other),
        };
        for entry in entries {
            if let Some(special) = read_special(entry) {
                specials.push(special);
            }
        }
    }
    specials
}

/// A named kind of ground a special may be placed on.
///
/// Reference: `overmap_location` in `special_locations.json` - just a name and the terrains that count as it. The
/// indirection matters: a special says it goes on `forest`, and what counts as forest is stated once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OvermapLocation {
    /// The id a special refers to.
    pub id: String,
    /// The terrains that count as this location.
    pub terrains: Vec<String>,
    /// Terrain flags that also count, for locations written against flags rather than ids.
    pub flags: Vec<String>,
}

impl OvermapLocation {
    /// Whether a terrain counts as this location.
    ///
    /// Reference: `overmap_location::test`, which matches the terrain's id against the list and then its flags.
    #[must_use]
    pub fn test(&self, terrain: &str, terrain_flags: &[&str]) -> bool {
        self.terrains.iter().any(|known| known == terrain)
            || self
                .flags
                .iter()
                .any(|flag| terrain_flags.contains(&flag.as_str()))
    }
}

/// Reads one location, or nothing when the entry is not one.
#[must_use]
pub fn read_location(value: &Value) -> Option<OvermapLocation> {
    let Value::Object(fields) = value else {
        return None;
    };
    if fields.get("type") != Some(&Value::Str("overmap_location".to_string())) {
        return None;
    }
    Some(OvermapLocation {
        id: text(fields.get("id"), ""),
        terrains: list(fields.get("terrains")),
        flags: list(fields.get("flags")),
    })
}

/// Reads every location in a list of parsed JSON documents.
#[must_use]
pub fn load_locations(documents: &[Value]) -> Vec<OvermapLocation> {
    let mut locations = Vec::new();
    for document in documents {
        let entries: &[Value] = match document {
            Value::Array(entries) => entries,
            other => std::slice::from_ref(other),
        };
        for entry in entries {
            if let Some(location) = read_location(entry) {
                locations.push(location);
            }
        }
    }
    locations
}

/// The city's lists of buildings, and how far from the middle each kind belongs.
///
/// Reference: `city_settings` in `regional_settings.h` and the `city` block of `regional_map_settings.json`. Three
/// bins - houses, shops, parks - each a weighted list, plus the two radii and their spreads that decide which bin a
/// spot in the city draws from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitySettings {
    /// How far from the city's middle shops are normal.
    pub shop_radius: i32,
    /// How far that spreads.
    pub shop_sigma: i32,
    /// How far from the middle parks are normal.
    pub park_radius: i32,
    /// How far that spreads.
    pub park_sigma: i32,
    /// The houses, by weight.
    pub houses: Vec<(String, i32)>,
    /// The shops, by weight.
    pub shops: Vec<(String, i32)>,
    /// The parks, by weight.
    pub parks: Vec<(String, i32)>,
}

fn weighted_bin(value: Option<&Value>) -> Vec<(String, i32)> {
    let Some(Value::Object(entries)) = value else {
        return Vec::new();
    };
    entries
        .members()
        .iter()
        .filter(|(name, weight)| !name.starts_with("//") && matches!(weight, Value::Int(_)))
        .map(|(name, weight)| (name.clone(), number(Some(weight), 0)))
        .collect()
}

/// Reads the `city` block of a region's settings.
#[must_use]
pub fn read_city_settings(value: &Value) -> Option<CitySettings> {
    // The region settings file is a **list** of regions, and the first one carrying a `city` block is the one that
    // describes a town. Accepting both shapes keeps the caller from having to know which it was handed.
    let entry = match value {
        Value::Array(entries) => entries.iter().find(|entry| match entry {
            Value::Object(fields) => matches!(fields.get("city"), Some(Value::Object(_))),
            _ => false,
        })?,
        other => other,
    };
    let Value::Object(fields) = entry else {
        return None;
    };
    let city = match fields.get("city") {
        Some(Value::Object(city)) => city,
        _ => fields,
    };
    Some(CitySettings {
        shop_radius: number(city.get("shop_radius"), 30),
        shop_sigma: number(city.get("shop_sigma"), 20),
        park_radius: number(city.get("park_radius"), 30),
        park_sigma: number(city.get("park_sigma"), 70),
        houses: weighted_bin(city.get("houses")),
        shops: weighted_bin(city.get("shops")),
        parks: weighted_bin(city.get("parks")),
    })
}

/// Reads the `overmap_forest` block of a region's settings.
///
/// Reference: `overmap_forest_settings` in `regional_settings.cpp`. The file's values **override** the header's
/// defaults, and they are not the same: the JSON asks for `0.2` and `0.25` where the header says `0.25` and `0.3`.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub fn read_forest_settings(value: &Value) -> Option<cdda_core_types::overmap::ForestSettings> {
    let entry = match value {
        Value::Array(entries) => entries.iter().find(|entry| match entry {
            Value::Object(fields) => {
                matches!(
                    fields.get("overmap_forest_settings"),
                    Some(Value::Object(_))
                )
            }
            _ => false,
        })?,
        other => other,
    };
    let Value::Object(fields) = entry else {
        return None;
    };
    // The block is named `overmap_forest_settings` in the file, while the type it fills is `overmap_forest_settings`
    // too - the shorter name inside the C++ code is a struct field, not a JSON key.
    let Some(Value::Object(forest)) = fields.get("overmap_forest_settings") else {
        return None;
    };
    let threshold = match forest.get("noise_threshold_forest") {
        Some(Value::Float(n)) => *n as f32,
        Some(Value::Int(n)) => *n as f32,
        _ => 0.25,
    };
    let threshold_thick = match forest.get("noise_threshold_forest_thick") {
        Some(Value::Float(n)) => *n as f32,
        Some(Value::Int(n)) => *n as f32,
        _ => 0.3,
    };
    Some(cdda_core_types::overmap::ForestSettings {
        threshold,
        threshold_thick,
    })
}

/// One layer of a forest's generation: how often it appears, and what it may put down.
///
/// Reference: `forest_mapgen_settings::component` in `regional_settings.h`. `sequence` is the order the layers run
/// in, `chance` is a denominator - **one square in `chance`** gets something from this layer - and the types are a
/// weighted list of what that something may be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForestComponent {
    /// The layer's name, e.g. `trees`.
    pub id: String,
    /// The order the layers run in.
    pub sequence: i32,
    /// One square in this many gets something from the layer.
    pub chance: i32,
    /// The weighted list of what may be placed.
    pub types: Vec<(String, i32)>,
}

/// How the reference generates woodland on the map level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForestMapgen {
    /// The overmap terrains this applies to.
    pub terrains: Vec<String>,
    /// How much open ground beside a forest square makes the next one sparse.
    pub sparseness_adjacency_factor: i32,
    /// The layer put down first.
    pub groundcover: Vec<(String, i32)>,
    /// The layers after it, in the order their `sequence` gives.
    pub components: Vec<ForestComponent>,
    /// The item group scattered through it.
    pub item_group: String,
    /// A percentage chance per iteration of placing from that group.
    pub item_group_chance: i32,
    /// How many times to place items.
    pub item_spawn_iterations: i32,
    /// **Furniture that follows the ground** - the reference's `terrain_dependent_furniture`, keyed by the **terrain** it appears on. The JSON calls the field
    /// `terrain_furniture`, and it is **nested**: each terrain names its own chance and its own weighted furniture list. Reading it as a flat weighted list, which is what
    /// the port used to do, turns the keys `chance` and `clear_furniture` into furniture ids.
    pub terrain_furniture: std::collections::BTreeMap<String, TerrainDependentFurniture>,
}

/// **Furniture that appears on one terrain, rolled per square** - the reference's `forest_biome_terrain_dependent_furniture` (`regional_settings.h:124-132`), loaded from one
/// member of a biome's `terrain_furniture` object (`regional_settings.cpp:67-92`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerrainDependentFurniture {
    /// One roll in `chance` places a piece - `read_and_set_or_throw<int>( jo, "chance", ... )`. The roll happens **only once the furniture has weight**, which is the
    /// reference's own early return in `set_terrain_dependent_furniture` (`mapgen_functions.cpp:1094-1116`).
    pub chance: i32,
    /// Whether this definition **replaces** an earlier one's furniture rather than adding to it - `clear_furniture`.
    pub clear_furniture: bool,
    /// The furniture and its weights, sorted by id.
    pub furniture: Vec<(String, i32)>,
}

/// Reads a biome's `terrain_furniture`: a map from a **terrain id** to that terrain's own chance and furniture.
///
/// Reference: `load_forest_biome` (`regional_settings.cpp:144-160`) walks the object and hands each member to
/// `load_forest_biome_terrain_dependent_furniture` (`:67-92`), which reads `chance`, `clear_furniture` and a `furniture` weighted list. The nesting is the whole point: a biome
/// may place cattails on shallow water and nothing anywhere else, which a flat list cannot say.
#[must_use]
pub fn terrain_dependent_furniture(
    value: Option<&Value>,
) -> std::collections::BTreeMap<String, TerrainDependentFurniture> {
    let Some(Value::Object(all)) = value else {
        return std::collections::BTreeMap::new();
    };
    all.members()
        .iter()
        .filter_map(|(terrain, spec)| {
            let Value::Object(fields) = spec else {
                return None;
            };
            Some((
                terrain.clone(),
                TerrainDependentFurniture {
                    chance: number(fields.get("chance"), 0),
                    clear_furniture: matches!(
                        fields.get("clear_furniture"),
                        Some(Value::Bool(true))
                    ),
                    furniture: ordered_weighted(fields.get("furniture")),
                },
            ))
        })
        .collect()
}

fn ordered_weighted(value: Option<&Value>) -> Vec<(String, i32)> {
    let mut entries = weighted_bin(value);
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

/// Reads one forest definition, folding in the `components` that share a name.
///
/// Reference: the loader walks `forest_mapgen_settings` and calls `load_forest_component` for each named layer; when
/// two files define layers of the same forest, `clear_types` decides whether the later one replaces the earlier
/// layer's types or adds to them. The port keeps the additive case and records the replacing one as a cleared list.
#[must_use]
pub fn read_forest_mapgen(value: &Value, terrain: &str) -> Option<ForestMapgen> {
    let entry = match value {
        Value::Array(entries) => entries.iter().find(|entry| match entry {
            Value::Object(fields) => {
                matches!(fields.get("forest_mapgen_settings"), Some(Value::Object(_)))
            }
            _ => false,
        })?,
        other => other,
    };
    let Value::Object(fields) = entry else {
        return None;
    };
    let Some(Value::Object(all)) = fields.get("forest_mapgen_settings") else {
        return None;
    };
    let Some(Value::Object(forest)) = all.get(terrain) else {
        return None;
    };
    let components = match forest.get("components") {
        Some(Value::Object(layers)) => layers
            .members()
            .iter()
            .filter_map(|(id, layer)| {
                let Value::Object(layer) = layer else {
                    return None;
                };
                Some(ForestComponent {
                    id: id.clone(),
                    sequence: number(layer.get("sequence"), 0),
                    chance: number(layer.get("chance"), 1),
                    types: ordered_weighted(layer.get("types")),
                })
            })
            .collect(),
        _ => Vec::new(),
    };
    let mut components: Vec<ForestComponent> = components;
    components.sort_by_key(|component| component.sequence);
    Some(ForestMapgen {
        terrains: list(forest.get("terrains")),
        sparseness_adjacency_factor: number(forest.get("sparseness_adjacency_factor"), 0),
        groundcover: ordered_weighted(forest.get("groundcover")),
        components,
        item_group: text(forest.get("item_group"), ""),
        item_group_chance: number(forest.get("item_group_chance"), 0),
        item_spawn_iterations: number(forest.get("item_spawn_iterations"), 0),
        terrain_furniture: terrain_dependent_furniture(forest.get("terrain_furniture")),
    })
}

/// Reads a region's groundcover palette by the id a mapgen names.
///
/// Reference: `region_terrain_and_furniture.terrain` in `regional_map_settings.json`, where a name like
/// `t_region_groundcover_forest` stands for a weighted list of terrains. The weights are **large on purpose** - the
/// file says so in a comment - so that a mod can add a rare terrain without rebalancing the base ones.
#[must_use]
pub fn read_groundcover(value: &Value, palette: &str) -> Option<Vec<(String, i32)>> {
    let entry = match value {
        Value::Array(entries) => entries.first()?,
        other => other,
    };
    let Value::Object(fields) = entry else {
        return None;
    };
    let Some(Value::Object(region)) = fields.get("region_terrain_and_furniture") else {
        return None;
    };
    let Some(Value::Object(terrains)) = region.get("terrain") else {
        return None;
    };
    let Some(Value::Object(members)) = terrains.get(palette) else {
        return None;
    };
    Some(weighted_bin(Some(&Value::Object(members.clone()))))
}

/// Every named palette in a region, so a feature that names one can be resolved to real terrain.
///
/// Reference: `region_terrain_and_furniture` in `regional_settings.json`. A forest's tree layer names
/// `t_region_tree_forest`, which is **not a terrain** - it is a name for a weighted list of terrains, and the reference
/// expands it while loading. A port that skips the expansion produces ids like `t_region_tree_forest` that appear
/// nowhere on a real map.
#[must_use]
pub fn read_region_palettes(
    value: &Value,
) -> std::collections::BTreeMap<String, Vec<(String, i32)>> {
    let entry = match value {
        Value::Array(entries) => match entries.first() {
            Some(entry) => entry,
            None => return std::collections::BTreeMap::new(),
        },
        other => other,
    };
    let Value::Object(fields) = entry else {
        return std::collections::BTreeMap::new();
    };
    let Some(Value::Object(region)) = fields.get("region_terrain_and_furniture") else {
        return std::collections::BTreeMap::new();
    };
    let Some(Value::Object(terrains)) = region.get("terrain") else {
        return std::collections::BTreeMap::new();
    };
    terrains
        .members()
        .iter()
        .map(|(name, members)| (name.clone(), weighted_bin(Some(members))))
        .collect()
}

/// Resolves a name to the terrain it stands for, drawing from its weight list when it is a palette.
#[must_use]
pub fn resolve_region_terrain(
    palettes: &std::collections::BTreeMap<String, Vec<(String, i32)>>,
    name: &str,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> String {
    // Palettes name other palettes - `t_region_tree_forest` lists `t_region_tree_forest_dense`, which lists real
    // trees - so resolving once leaves placeholder ids on the map. The reference expands the whole chain at load time;
    // the port walks it here, with a bound so a cycle in the data cannot hang the generator.
    let mut current = name.to_string();
    for _ in 0..16 {
        match palettes.get(&current) {
            Some(entries) if !entries.is_empty() => {
                let list =
                    cdda_core_types::weighted::WeightedList::from_entries(entries.iter().cloned());
                match list.pick(rolls).cloned() {
                    Some(next) => current = next,
                    None => break,
                }
            }
            _ => break,
        }
    }
    current
}

/// The densest forest in the region, which is the ceiling the feathering arithmetic compares against.
///
/// Reference: the `max_factor` loop at the top of `mapgen_forest`. It is the **largest** `sparseness_adjacency_factor`
/// over every forest biome, not the value of the one being generated - dense forest is 4 while ordinary forest is 3 -
/// and using the square's own value instead tilts the whole chunk towards woodland.
#[must_use]
pub fn max_sparseness(value: &Value) -> i32 {
    let entry = match value {
        Value::Array(entries) => match entries.first() {
            Some(entry) => entry,
            None => return 0,
        },
        other => other,
    };
    let Value::Object(fields) = entry else {
        return 0;
    };
    let Some(Value::Object(all)) = fields.get("forest_mapgen_settings") else {
        return 0;
    };
    all.members()
        .iter()
        .map(|(_, forest)| match forest {
            Value::Object(forest) => number(forest.get("sparseness_adjacency_factor"), 0),
            _ => 0,
        })
        .max()
        .unwrap_or(0)
}

/// Every forest biome the region defines, keyed by the terrain it generates.
///
/// Reference: `forest_composition.biomes`, filled from `forest_mapgen_settings` and keyed by **terrain type id**. The
/// neighbouring squares of a forest are looked up here: a neighbour that is woodland owns a biome, which gives it a
/// density and a palette of its own, and a neighbour that is not - a road, a field - owns none.
#[must_use]
pub fn load_forest_mapgens(value: &Value) -> std::collections::BTreeMap<String, ForestMapgen> {
    let entry = match value {
        Value::Array(entries) => match entries.first() {
            Some(entry) => entry,
            None => return std::collections::BTreeMap::new(),
        },
        other => other,
    };
    let Value::Object(fields) = entry else {
        return std::collections::BTreeMap::new();
    };
    let Some(Value::Object(all)) = fields.get("forest_mapgen_settings") else {
        return std::collections::BTreeMap::new();
    };
    all.members()
        .iter()
        .filter_map(|(terrain, _)| {
            read_forest_mapgen(value, terrain).map(|forest| (terrain.clone(), forest))
        })
        .collect()
}

/// **Whether a square on this terrain rolls for furniture, and what one roll in `chance` places** - `set_terrain_dependent_furniture` (`mapgen_functions.cpp:1094-1116`).
///
/// The reference's order matters for the dice: it returns **before** rolling when the terrain has no entry, and again before rolling when the furniture list has no weight.
/// Only a terrain that has both an entry and weight consumes a draw, which is what makes the rule observable in a replay at all.
#[must_use]
pub fn terrain_dependent_furniture_roll(
    entry: &TerrainDependentFurniture,
) -> Option<(i32, &[(String, i32)])> {
    let total: i32 = entry.furniture.iter().map(|(_, weight)| *weight).sum();
    if total <= 0 {
        return None;
    }
    Some((entry.chance, &entry.furniture))
}
