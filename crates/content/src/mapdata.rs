//! Terrain and furniture: the first content family with a field-level oracle.
//!
//! The reference loads both types through one shared loader, `map_data_common_t`
//! (`mapdata.cpp:509-...`), and each adds a handful of fields of its own
//! (`ter_t::load` at `:1137`, `furn_t::load` at `:1336`). The rules worth
//! reproducing exactly:
//!
//! * `symbol` and `color` are four-season arrays; one string fills all four,
//!   four strings are one per season, and any other length is an error
//!   (`mapdata.cpp:1516-1544`).
//! * A child that names `flags` replaces the whole set, while `extend.flags`
//!   adds to it and `delete.flags` removes from it (`mapdata.cpp:620-650`).
//! * Everything else is inherited when the child does not mention it, because
//!   the reference copies the base object and then loads the child over it
//!   (`generic_factory.h:295-313`).

use cdda_core_types::colors::{NcColor, bgcolor_from_string, color_from_string, string_from_color};
use cdda_json::{Object, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Which of the two lists a prototype belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MapDataKind {
    /// `terrain` in JSON, the ground itself.
    Terrain,
    /// `furniture` in JSON, what stands on the ground.
    Furniture,
}

/// One resolved terrain or furniture prototype.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapData {
    /// Which list the prototype belongs to.
    pub kind: MapDataKind,
    /// Its id.
    pub id: String,
    /// The name a player sees, untranslated.
    pub name: String,
    /// The glyph per season.
    pub symbol: [i32; 4],
    /// The colour per season, as a value rather than a name: two names can
    /// print the same and one name can print as another (`i_red` prints as
    /// `c_red_red`), so the value is what has to match.
    pub color: [NcColor; 4],
    /// Terrain's `move_cost`, furniture's `move_cost_mod`.
    pub move_cost: i32,
    /// Light the tile emits.
    pub light: i32,
    /// How much the tile covers.
    pub coverage: i32,
    /// The flag set, sorted, after replacement, extension and deletion.
    pub flags: BTreeSet<String>,
}

impl MapData {
    /// Whether this prototype carries a flag, by its reference name (`FLAMMABLE_HARD`, `NO_FLOOR`, ...).
    #[must_use]
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.contains(flag)
    }

    /// Whether a creature may stand on this square.
    ///
    /// Reference: `map::passable_through` (`map.cpp:2419-2422`) - `passable( p ) && ( has_floor( p ) || has_floor_or_support( p ) )`.
    /// With no level below to ask, the floor has to be the terrain's own, so this is the rule for a single-level test: room
    /// (`move_cost != 0`) **and** no `NO_FLOOR` flag. Open air has the room and not the floor, which is why a wall and a hole are
    /// different refusals.
    #[must_use]
    pub fn passable_through(&self) -> bool {
        self.move_cost != 0 && !self.flags.contains("NO_FLOOR")
    }

    /// The name the reference prints for one season's colour.
    #[must_use]
    pub fn color_name(&self, season: usize) -> &'static str {
        string_from_color(self.color.get(season).copied().unwrap_or(self.color[0]))
    }
}

/// A prototype as it was loaded, kept so a child can be resolved later.
#[derive(Debug, Clone)]
struct Loaded {
    kind: MapDataKind,
    id: String,
    base: Option<String>,
    json: Object,
    /// True for an `abstract` template, which is loadable as a base but is not
    /// a prototype of its own.
    template: bool,
}

/// Loads every terrain and furniture object in the given directories, in the
/// reference's file order, resolving inheritance across files.
///
/// # Errors
///
/// Fails when a file cannot be read or an object cannot be loaded.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<Vec<MapData>, LoadError> {
    let mut loaded = Vec::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            if let Some(value) = crate::read_json(&path) {
                collect(&value, &mut loaded);
            }
        }
    }
    let mut resolved = Resolver::new("core").run(loaded)?;
    // Both factories insert a null prototype before the first real object and
    // keep it (`mapdata.cpp:849-899`): terrain has an extra undefined-id entry
    // and furniture one that cannot be walked through.
    for (kind, id, extra_flag) in [
        (MapDataKind::Terrain, "t_null", Some("DIGGABLE")),
        (MapDataKind::Furniture, "f_null", None),
    ] {
        if resolved
            .iter()
            .any(|entry| entry.kind == kind && entry.id == id)
        {
            continue;
        }
        let mut flags = BTreeSet::new();
        flags.insert("TRANSPARENT".to_owned());
        if let Some(extra) = extra_flag {
            flags.insert(extra.to_owned());
        }
        resolved.insert(
            0,
            MapData {
                kind,
                id: id.to_owned(),
                name: "nothing".to_owned(),
                symbol: [i32::from(b' '); 4],
                color: std::array::from_fn(|_| color_from_string("white").unwrap_or_default()),
                move_cost: 0,
                light: 0,
                coverage: 0,
                flags,
            },
        );
    }
    Ok(resolved)
}

/// Loads every terrain and furniture object in a parsed file.
///
/// # Errors
///
/// Reports the first object that cannot be loaded, naming its id.
pub fn load_objects(value: &Value, src: &str) -> Result<Vec<MapData>, LoadError> {
    let mut loaded = Vec::new();
    collect(value, &mut loaded);
    let mut resolved = Resolver::new(src);
    resolved.run(loaded)
}

/// Why a prototype could not be loaded.
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

fn collect(value: &Value, out: &mut Vec<Loaded>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, out);
            }
        }
        Value::Object(object) => {
            let kind = match string(object, "type").as_deref() {
                Some("terrain") => Some(MapDataKind::Terrain),
                Some("furniture") => Some(MapDataKind::Furniture),
                _ => None,
            };
            if let Some(kind) = kind {
                let concrete = string(object, "id");
                let template = concrete.is_none() && object.get("abstract").is_some();
                let id = concrete.or_else(|| string(object, "abstract"));
                if let Some(id) = id {
                    out.push(Loaded {
                        kind,
                        id,
                        base: string(object, "copy-from"),
                        json: object.clone(),
                        template,
                    });
                }
            }
        }
        _ => {}
    }
}

/// Resolves inheritance for a batch of loaded objects.
struct Resolver<'a> {
    src: &'a str,
    by_id: BTreeMap<(MapDataKind, String), usize>,
    objects: Vec<Loaded>,
    resolved: BTreeMap<(MapDataKind, String), MapData>,
}

impl<'a> Resolver<'a> {
    fn new(src: &'a str) -> Self {
        Resolver {
            src,
            by_id: BTreeMap::new(),
            objects: Vec::new(),
            resolved: BTreeMap::new(),
        }
    }

    fn run(&mut self, objects: Vec<Loaded>) -> Result<Vec<MapData>, LoadError> {
        self.objects = objects;
        for at in 0..self.objects.len() {
            let key = match self.objects.get(at) {
                Some(loaded) => (loaded.kind, loaded.id.clone()),
                None => continue,
            };
            self.by_id.insert(key, at);
        }
        let mut out = Vec::new();
        for at in 0..self.objects.len() {
            let key = match self.objects.get(at) {
                Some(loaded) => (loaded.kind, loaded.id.clone()),
                None => continue,
            };
            let template = self.objects.get(at).is_some_and(|loaded| loaded.template);
            let resolved = self.resolve(&key, &mut Vec::new())?;
            if !template {
                out.push(resolved);
            }
        }
        let _ = self.src;
        Ok(out)
    }

    fn resolve(
        &mut self,
        key: &(MapDataKind, String),
        stack: &mut Vec<String>,
    ) -> Result<MapData, LoadError> {
        if let Some(done) = self.resolved.get(key) {
            return Ok(done.clone());
        }
        let id = key.1.clone();
        if stack.contains(&id) {
            return Err(LoadError {
                message: format!("cyclic copy-from through {id}: {stack:?}"),
            });
        }
        let Some(at) = self.by_id.get(key).copied() else {
            return Err(LoadError {
                message: format!("{} {id} is not defined", kind_name(key.0)),
            });
        };
        let Some(loaded) = self.objects.get(at).cloned() else {
            return Err(LoadError {
                message: format!("{id} vanished while resolving"),
            });
        };
        stack.push(id.clone());
        let base = match loaded.base.clone() {
            Some(base_id) => {
                let base_key = (loaded.kind, base_id.clone());
                if !self.by_id.contains_key(&base_key) {
                    return Err(LoadError {
                        message: format!("{} {id} copies from unknown {base_id}", kind_name(key.0)),
                    });
                }
                Some(self.resolve(&base_key, stack)?)
            }
            None => None,
        };
        stack.pop();
        let value = apply(base, &loaded)?;
        self.resolved.insert(key.clone(), value.clone());
        Ok(value)
    }
}

fn kind_name(kind: MapDataKind) -> &'static str {
    match kind {
        MapDataKind::Terrain => "terrain",
        MapDataKind::Furniture => "furniture",
    }
}

/// Applies one object's fields on top of its base, as the reference's `load` does.
fn apply(base: Option<MapData>, loaded: &Loaded) -> Result<MapData, LoadError> {
    let json = &loaded.json;
    let id = loaded.id.clone();
    let inherited = base.is_some();
    let mut value = base.unwrap_or_else(|| MapData {
        kind: loaded.kind,
        id: id.clone(),
        name: String::new(),
        symbol: [i32::from(b' '); 4],
        color: std::array::from_fn(|_| color_from_string("white").unwrap_or_default()),
        move_cost: 0,
        light: 0,
        coverage: 0,
        flags: BTreeSet::new(),
    });
    value.kind = loaded.kind;
    value.id.clone_from(&id);

    if let Some(name) = name_of(json) {
        value.name = name;
    }
    if json.get("symbol").is_some() {
        let seasons = season_strings(json, "symbol")?;
        for (slot, text) in value.symbol.iter_mut().zip(&seasons) {
            let mut characters = text.chars();
            match (characters.next(), characters.next()) {
                (Some(character), None) => *slot = character as i32,
                _ => {
                    return Err(LoadError {
                        message: format!("symbol must be one character in {id}"),
                    });
                }
            }
        }
    }
    if let Some(member) = json
        .get("color")
        .map(|_| "color")
        .or_else(|| json.get("bgcolor").map(|_| "bgcolor"))
    {
        let background = member == "bgcolor";
        let seasons = season_strings(json, member)?;
        for (slot, text) in value.color.iter_mut().zip(&seasons) {
            *slot = if background {
                bgcolor_from_string(text)
            } else {
                color_from_string(text).map_err(|error| LoadError {
                    message: format!("color {text:?} in {id}: {error:?}"),
                })?
            };
        }
    }
    let cost_member = match loaded.kind {
        MapDataKind::Terrain => "move_cost",
        MapDataKind::Furniture => "move_cost_mod",
    };
    if let Some(cost) = integer(json, cost_member) {
        value.move_cost = cost;
    }
    if let Some(light) = integer(json, "light_emitted") {
        value.light = light;
    }
    if let Some(coverage) = integer(json, "coverage") {
        value.coverage = coverage;
    }

    // A child that names `flags` replaces the inherited set; `extend` adds and
    // `delete` removes, in that order, exactly as the reference does.
    if inherited && json.get("flags").is_some() {
        value.flags.clear();
    }
    for flag in strings(json, "flags") {
        value.flags.insert(flag);
    }
    if let Some(extend) = object(json, "extend") {
        for flag in strings(extend, "flags") {
            value.flags.insert(flag);
        }
    }
    if let Some(delete) = object(json, "delete") {
        for flag in strings(delete, "flags") {
            value.flags.remove(&flag);
        }
    }
    Ok(value)
}

/// The four strings of a season array: one string fills all four, or four are
/// taken one per season.
fn season_strings(json: &Object, member: &str) -> Result<[String; 4], LoadError> {
    match json.get(member) {
        Some(Value::Str(text)) => Ok([text.clone(), text.clone(), text.clone(), text.clone()]),
        Some(Value::Array(items)) if items.len() == 1 => {
            let Some(Value::Str(text)) = items.first() else {
                return Err(LoadError {
                    message: format!("{member} entries must be strings"),
                });
            };
            Ok([text.clone(), text.clone(), text.clone(), text.clone()])
        }
        Some(Value::Array(items)) if items.len() == 4 => {
            let mut out: [String; 4] = std::array::from_fn(|_| String::new());
            for (slot, item) in out.iter_mut().zip(items) {
                let Value::Str(text) = item else {
                    return Err(LoadError {
                        message: format!("{member} entries must be strings"),
                    });
                };
                slot.clone_from(text);
            }
            Ok(out)
        }
        Some(_) => Err(LoadError {
            message: format!("{member} must be a string or an array of four"),
        }),
        None => Err(LoadError {
            message: format!("missing {member}"),
        }),
    }
}

fn name_of(json: &Object) -> Option<String> {
    match json.get("name") {
        Some(Value::Str(text)) => Some(text.clone()),
        Some(Value::Object(object)) => string(object, "str"),
        _ => None,
    }
}

fn string(object: &Object, member: &str) -> Option<String> {
    match object.get(member) {
        Some(Value::Str(text)) => Some(text.clone()),
        _ => None,
    }
}

fn integer(object: &Object, member: &str) -> Option<i32> {
    match object.get(member) {
        Some(Value::Int(value)) => i32::try_from(*value).ok(),
        _ => None,
    }
}

fn object<'a>(object: &'a Object, member: &str) -> Option<&'a Object> {
    match object.get(member) {
        Some(Value::Object(inner)) => Some(inner),
        _ => None,
    }
}

fn strings(object: &Object, member: &str) -> Vec<String> {
    match object.get(member) {
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

/// Whether a creature may stand here, given what is underneath.
///
/// The second half of the reference's disjunction: a square without a floor of its own still takes a creature when something
/// holds it up, so a bridge over a chasm is passable and the chasm is not.
#[must_use]
pub fn passable_through_with_support(terrain: &MapData, has_support: bool) -> bool {
    terrain.move_cost != 0 && (!terrain.flags.contains("NO_FLOOR") || has_support)
}

/// The single-level form of the same rule, for callers with no level below to ask.
#[must_use]
pub fn passable_through(terrain: &MapData) -> bool {
    passable_through_with_support(terrain, false)
}
