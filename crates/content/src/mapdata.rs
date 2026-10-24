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
    /// Whether light and sight pass through this square.
    ///
    /// Reference: `map_data_common_t::transparent` (`src/mapdata.h:573`), a **stored** bool rather than a derived one, and `false` for a new object. `set_flag`
    /// makes it transparent for `TRANSPARENT` or `TRANSLUCENT`; a child that names its own `flags` array calls `unset_flags`,
    /// which clears it (`mapdata.cpp:1112-1114`); and `unset_flag` clears it for `TRANSPARENT` while leaving it alone for `TRANSLUCENT`
    /// (`mapdata.cpp:892-905`). The asymmetries are the reference's, and they are why this cannot be recomputed from `flags`.
    pub transparent: bool,
    /// How hard the tile is to break, when it can be broken at all.
    ///
    /// Reference: `map_data_common_t::bash` (`src/mapdata.h:433`), an `optional` block: a tile with **no** block cannot be bashed, and a tile with a block whose `str_min` is
    /// `-1` cannot be bashed either but is a different thing — the reference's `bash_ter_furn` tests `if( !bash || ( bash->destroy_only && !params.destroy ) )` on the
    /// block's presence, so a block that merely cannot be reached still answers to `destroy`.
    pub bash: Option<BashInfo>,
}

/// One tile's bash block: how much strength it takes, and what it becomes.
///
/// Reference: `map_common_bash_info` with `map_ter_bash_info` and `map_furn_bash_info` (`src/mapdata.h:40-84`). The two subclasses differ in exactly one field each — terrain
/// names the terrain it turns into and, separately, what it turns into when bashed from above; furniture names the furniture it turns into.
///
/// **Absence is a value.** Every numeric field is `-1` when the data does not name it, and `str_min` of `-1` is how the reference spells "this block cannot be bashed at all".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BashInfo {
    /// The strength at or below which the bash never succeeds.
    pub str_min: i32,
    /// The strength at or above which it always does; between the two, one roll decides.
    pub str_max: i32,
    /// `str_min` when the tile is braced by furniture beside it.
    pub str_min_blocked: i32,
    /// `str_max` in the same case.
    pub str_max_blocked: i32,
    /// `str_min` when something supports the tile from below.
    pub str_min_supported: i32,
    /// `str_max` in the same case.
    pub str_max_supported: i32,
    /// Only reachable by a `destroy`, never by an ordinary bash.
    pub destroy_only: bool,
    /// This terrain is the roof of the tile below, and bashing it should take that too.
    pub bash_below: bool,
    /// How loud breaking it is, or `-1` for silent.
    pub sound_vol: i32,
    /// How loud failing to break it is, or `-1` for silent.
    pub sound_fail_vol: i32,
    /// The **named** item group it drops.
    ///
    /// A fresh block that says nothing about items gets the reference's `EMPTY_GROUP` (`src/mapdata.cpp:410-412`), which is a real group with a real name — so the default
    /// here is that group and not "none". An inline array is turned into an anonymous group with no name to write down, which is recorded as `None`: a declared gap rather
    /// than a silent one.
    pub drop_group: Option<String>,
    /// What it becomes: terrain's `ter_set`, furniture's `furn_set`.
    pub set: String,
    /// What terrain becomes when bashed from above, which defaults to [`BashInfo::set`]; furniture has no such field.
    pub set_from_above: Option<String>,
}

impl Default for BashInfo {
    /// The reference's own defaults: every number `-1`, which is its spelling for "not given", and the empty item group.
    ///
    /// `map_common_bash_info` (`src/mapdata.h:40-63`) is initialised that way, and `load` only overwrites what the JSON names — so a block that says nothing at all is not a
    /// block of zeroes.
    fn default() -> Self {
        Self {
            str_min: -1,
            str_max: -1,
            str_min_blocked: -1,
            str_max_blocked: -1,
            str_min_supported: -1,
            str_max_supported: -1,
            destroy_only: false,
            bash_below: false,
            sound_vol: -1,
            sound_fail_vol: -1,
            drop_group: Some("EMPTY_GROUP".to_owned()),
            set: String::new(),
            set_from_above: None,
        }
    }
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
                // `null_terrain_t` and `null_furniture_t` set the bool themselves (`mapdata.cpp:555`, `:576`), and nothing loads over them, so the reference's
                // own dump has both **transparent** - unlike an ordinary new prototype, which starts opaque.
                transparent: true,
                bash: None,
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

/// Whether a flag makes a prototype transparent.
///
/// Reference: `map_data_common_t::set_flag` (`mapdata.cpp:876-883`), which treats both flags alike: `TRANSLUCENT` passes **light** without passing detail, and
/// the light cache only asks about light.
fn makes_transparent(flag: &str) -> bool {
    flag == "TRANSPARENT" || flag == "TRANSLUCENT"
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
        // A new prototype starts **opaque** (`mapdata.h:573`). The only two things that start transparent are the code-made null prototypes, and the
        // shipped `t_null`/`f_null` override even that by naming their own flags - which is why this is data-pinned rather than reasoned.
        transparent: false,
        bash: None,
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
        // `unset_flags` clears the stored bool as well (`mapdata.cpp:907-912`).
        value.transparent = false;
    }
    for flag in strings(json, "flags") {
        if makes_transparent(&flag) {
            value.transparent = true;
        }
        value.flags.insert(flag);
    }
    if let Some(extend) = object(json, "extend") {
        for flag in strings(extend, "flags") {
            if makes_transparent(&flag) {
                value.transparent = true;
            }
            value.flags.insert(flag);
        }
    }
    if let Some(delete) = object(json, "delete") {
        for flag in strings(delete, "flags") {
            value.flags.remove(&flag);
            // `unset_flag` (`mapdata.cpp:892-905`) clears the bool for `TRANSPARENT` only, and only when it was already set. Clearing `TRANSLUCENT` leaves it alone,
            // which is the reference's own asymmetry and the reason a window that deletes one flag still passes light.
            if flag == "TRANSPARENT" && value.transparent {
                value.transparent = false;
            }
        }
    }
    apply_bash(&mut value.bash, json, loaded.kind);
    Ok(value)
}

/// The four strings of a season array: one string fills all four, or four are
/// taken one per season.
/// Applies a `bash` block over whatever the base had.
///
/// Reference: `map_data_common_t`'s loaders (`src/mapdata.cpp:1224-1231` for terrain, and the furniture equivalent), which keep the inherited block and load the child's
/// object over it when there is one — so a child that names only `str_min` keeps its parent's `ter_set`. The block is only created when the JSON has one; a tile without a
/// block inherits nothing and gets none.
fn apply_bash(value: &mut Option<BashInfo>, json: &Object, kind: MapDataKind) {
    let Some(Value::Object(block)) = json.get("bash") else {
        return;
    };
    let fresh = value.is_none();
    let mut info = value.take().unwrap_or_default();
    for (member, slot) in [
        ("str_min", &mut info.str_min),
        ("str_max", &mut info.str_max),
        ("str_min_blocked", &mut info.str_min_blocked),
        ("str_max_blocked", &mut info.str_max_blocked),
        ("str_min_supported", &mut info.str_min_supported),
        ("str_max_supported", &mut info.str_max_supported),
        ("sound_vol", &mut info.sound_vol),
        ("sound_fail_vol", &mut info.sound_fail_vol),
    ] {
        if let Some(number) = integer(block, member) {
            *slot = number;
        }
    }
    if let Some(flag) = block.get("destroy_only") {
        info.destroy_only = matches!(flag, Value::Bool(true));
    }
    if let Some(flag) = block.get("bash_below") {
        info.bash_below = matches!(flag, Value::Bool(true));
    }
    // **Absent and anonymous are different.** `items` as a string is a group the port can name; as an array it is a group the reference mints an id for, which no port can
    // reproduce, so it is recorded as "there are items" and left unnamed.
    if let Some(items) = block.get("items") {
        info.drop_group = match items {
            Value::Str(name) => Some(name.clone()),
            // An inline array: the reference mints an anonymous group for it, and its id is not a name any port could reproduce.
            _ => None,
        };
    }
    if let Some(set) = string(block, "ter_set").or_else(|| string(block, "furn_set")) {
        info.set = set;
    }
    // `ter_set_bashed_from_above` is `optional( jo, was_loaded, ..., ter_set )` (`src/mapdata.cpp:423`), and that default is applied **only when the object is fresh**:
    // a child that renames `ter_set` and says nothing about the above variant keeps the value its parent resolved — which is why a carpet over a floor still bashes from
    // above into `t_floor` and not into the carpet's own base. Furniture has no such field at all.
    let above = string(block, "ter_set_bashed_from_above");
    info.set_from_above = match kind {
        MapDataKind::Terrain => Some(match above {
            Some(named) => named,
            None if fresh => info.set.clone(),
            None => info.set_from_above.unwrap_or_else(|| info.set.clone()),
        }),
        // Furniture's `furn_set` is `optional( ..., furn_f_null )` (`src/mapdata.cpp:429`), so a block that names no furniture becomes `f_null` — a real id the map can
        // stand on, not an empty string.
        MapDataKind::Furniture => {
            if info.set.is_empty() {
                "f_null".clone_into(&mut info.set);
            }
            None
        }
    };
    *value = Some(info);
}

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
