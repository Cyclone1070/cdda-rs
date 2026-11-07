//! Monster types.
//!
//! The reference loads these through the same generic factory as items, so the
//! machinery — abstracts, `copy-from`, deferred resolution — is the same. What
//! differs is which rules read the file, and the biggest difference is that the
//! numbers go through `assign` rather than through a reader: `relative` is
//! checked **before** `proportional` here (`assign.cpp:60-88`), the opposite of
//! the order items use.

use crate::items::{number, string, strings, sub_object};
use cdda_core_types::units::{read_mass, read_volume};
use cdda_json::{Object, Value};
use std::collections::{BTreeMap, BTreeSet};

/// One monster type after inheritance.
#[derive(Debug, Clone, PartialEq)]
pub struct MonsterType {
    /// Its id.
    pub id: String,
    /// The singular name.
    pub name: String,
    /// Its glyph.
    pub symbol: String,
    /// Its colour.
    pub color: NcColor,
    /// Its body type, empty when it has none.
    pub bodytype: String,
    /// Hit points.
    pub hp: i32,
    /// Speed, where a human is 100.
    pub speed: i32,
    /// Base difficulty.
    pub diff: i32,
    /// Melee skill.
    pub melee_skill: i32,
    /// Dodge.
    pub dodge: i32,
    /// How many dice of bonus bash its melee adds.
    pub melee_dice: i32,
    /// How many faces those dice have.
    pub melee_sides: i32,
    /// How much armour those dice ignore.
    pub melee_dice_ap: i32,
    /// The flat part of the type's blow, per damage type, which `monster::melee_attack` starts from before adding its dice as bash (`monster.cpp:2149`).
    ///
    /// The dump states these as reals because the reference holds a `damage_instance`, and every type in it states a whole number; the loader rounds and says so.
    pub melee_bash: i32,
    /// The flat cut part.
    pub melee_cut: i32,
    /// The flat stab part.
    pub melee_stab: i32,
    /// What a blow costs the monster in moves, which `monster::melee_attack` charges before it even swings (`monster.cpp:2108`).
    ///
    /// The reference's default is a hundred moves - one turn's worth - so a monster that has walked cannot also swing, and a monster that swings has no walk left.
    pub attack_cost: i32,
    /// The hit points it mends per hour on its own, when the data states a rate (`mtype::regenerates`).
    pub regenerates: f64,
    /// Its resistance to bash.
    pub armor_bash: i32,
    /// Its resistance to cut.
    pub armor_cut: i32,
    /// Resistance to stabbing damage.
    ///
    /// Reference: `mtype::armor` (`monster.h`) holds a `resistances` object, and `armor_cut` and `armor_stab` are two of its types - a monster can be armoured against a blade's
    /// edge and open to its point, which is why the port keeps them apart rather than borrowing one for the other.
    pub armor_stab: i32,
    /// Morale.
    pub morale: i32,
    /// Whether the monster gets its nerve back at all (`mtype::regen_morale`). 23 of the corpus's 1,154 types say yes.
    pub regen_morale: bool,
    /// What the data says when this monster dies, or empty when it says nothing.
    ///
    /// Reference: `monster_death_effect::death_message` (`mtype.h:280`). **It is not the only death line**: `mdeath::normal` prints "The %s dies!" of its own (`mondeath.cpp:50-56`), and the
    /// reference's own comment admits a body can produce several. The port keeps them apart for the same reason - one is data, the other is the normal branch's own sentence.
    pub death_message: String,
    /// What it leaves behind, from the data's `corpse_type` (`mtype.h:272-285`): `normal`, `broken`, `splatter` or `none`.
    pub corpse_type: String,
    /// The item a `broken` body leaves, when the data names it itself (`mtype::broken_itype`, `mtype.h:367`).
    ///
    /// Empty for almost every type: the reference's own fallback builds `broken_<id>` from the monster's name (`mondeath.cpp:210-222`), and this field is the exception to that rule.
    pub broken_itype: String,
    /// The item group a body drops when it dies (`mtype::death_drops`, `mtype.h:404`).
    ///
    /// This is where a monster's loot comes from - the group is an **inline item group** with its own weights and counts, which the port has as `byproduct_group` for recipes but not
    /// yet for monsters.
    pub death_drops: String,
    /// Whether it mends by the dark instead of in it (`mtype::regenerates_in_dark`, `mtype.h:541`).
    ///
    /// **Only three of the 1,154 types say yes**, and the rule is a curve rather than a number: `50 * e^(-light^2/10000)` hit points a turn (`monster.cpp:3487-3494`), which a torch snuffs
    /// out and a floodlight stops twenty tiles away.
    pub regenerates_in_dark: bool,
    /// The weak points its body has, if any - **an inline set rather than an id**.
    ///
    /// Reference: `mtype::weakpoints` (`mtype.h:444`), a `::weakpoints` object on the type itself, which `monster::deal_damage` selects from (`creature.cpp:1146`). The dump writes the set's
    /// entries and its default, already **expanded** - a monster's inline set goes through `finalize_damage_map` when its type is finalized, unlike the standalone `weakpoint_set` prototypes -
    /// which is why these maps carry every damage type's name and the set dump's do not.
    pub weakpoints: Option<crate::weakpoints::WeakpointSet>,
    /// The weak-point families its body rewards, with the figures **already resolved**.
    ///
    /// Reference: `mtype::families` (`mtype.h:382`), a `weakpoint_families` read by `weakpoint_families::modifier` (`weakpoint.cpp:171-178`): each entry is a proficiency the attacker must know
    /// for the **bonus**, and pays the **penalty** when it does not. The dump writes the resolved figures because the reference's fallback is the proficiency's own default and the port has no
    /// proficiency table carrying those two numbers.
    pub families: Vec<crate::weakpoints::Family>,
    /// How readily it bleeds (`mtype::bleed_rate`, `mtype.h:486`).
    ///
    /// `mtype::get_bleed_type` (`mtype.cpp:412-422`) checks this **before** it looks at a species at all: a rate of zero means the monster never bleeds its species' blood, however the
    /// species is written. The dump defaults it to a hundred, as the reference does.
    pub bleed_rate: i32,
    /// Aggression, from -100 to 100.
    pub aggression: i32,
    /// Weight in milligrams.
    pub weight: i64,
    /// Volume in millilitres.
    pub volume: i64,
    /// Species, sorted.
    pub species: BTreeSet<String>,
    /// Categories, sorted.
    pub categories: BTreeSet<String>,
    /// Materials and their portions.
    pub materials: BTreeMap<String, i32>,
    /// Its flags, which the dump does not carry but spawning rules read.
    pub flags: BTreeSet<String>,
    /// The harvest definition a corpse of this kind butchers into.
    ///
    /// Reference: `mtype::harvest` (`mtype.h`), a `harvest_id`. The dump now carries it, because the port cannot
    /// guess which of the 247 definitions a monster uses and a guess here would be silently wrong meat.
    pub harvest: String,
    /// The definition a dissection yields, which is a different list from butchery.
    pub dissect: String,
    /// How big it is, which decides the base cost of cutting its corpse up.
    pub size: String,
}

/// Why a monster type could not be loaded.
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

/// Loads every monster type under `dirs`, in the reference's order.
///
/// # Errors
///
/// Reports a file that cannot be read, or an object that names a base or a field
/// the content does not define.
/// Loads monster types from an already-parsed list of prototypes - the shape a dump holds.
///
/// The difference from [`load_paths`] is the filtering: a dump is the generator's own list, so every entry in
/// it is a monster and there is no `type` field to look for. Everything after that is the same work - resolve
/// inheritance, derive size from volume, and append the null monster the reference always has.
///
/// # Errors
/// Returns the first prototype that could not be read.
pub fn load_objects(prototypes: &[cdda_json::Value]) -> Result<Vec<MonsterType>, LoadError> {
    let mut objects: BTreeMap<String, Object> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for value in prototypes {
        // Every prototype in a dump is a monster already: there is no `type` field to filter on, and the
        // dump is the list the generator produced.
        if let Value::Object(object) = value
            && let Some(id) = string(object, "id")
        {
            if !objects.contains_key(&id) {
                order.push(id.clone());
            }
            objects.insert(id, object.clone());
        }
    }
    let mut out = Vec::new();
    for id in &order {
        let mut seen = Vec::new();
        out.extend(resolve(id, &objects, &mut seen)?);
    }
    for monster in &mut out {
        monster.size = size_from_volume(monster.volume);
    }
    out.push(null_monster());
    Ok(out)
}

/// Loads every monster type under the given data directories, in the reference's load order.
///
/// # Errors
/// Returns the first prototype that could not be read, or a directory that could not be listed.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<Vec<MonsterType>, LoadError> {
    let mut objects: BTreeMap<String, Object> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            collect(&value, &mut objects, &mut order);
        }
    }
    let mut out = Vec::new();
    for id in &order {
        let mut seen = Vec::new();
        out.extend(resolve(id, &objects, &mut seen)?);
    }
    // The reference derives a creature's size from its volume after loading (`monstergenerator.cpp`), so the
    // size a monster reports is not the size its JSON states, and often its JSON states none at all.
    for monster in &mut out {
        monster.size = size_from_volume(monster.volume);
    }
    out.push(null_monster());
    // A monster flagged `GEN_DORMANT` gets a fake twin that spawns dormant and
    // wakes as the real one (`monstergenerator.cpp:445-448`).
    let dormant: Vec<MonsterType> = out
        .iter()
        .filter(|monster| monster.flags.contains("GEN_DORMANT"))
        .map(|monster| {
            // Everything is copied, then the twin is made harmless and given
            // the four flags that let it sit still until woken
            // (`monstergenerator.cpp:462-500`).
            let mut copy = monster.clone();
            copy.id = format!("pseudo_dormant_{}", monster.id);
            copy.hp = 5;
            copy.speed = 1;
            copy.dodge = 0;
            copy.flags.clear();
            for flag in ["DORMANT", "QUIETDEATH", "REVIVES", "REVIVES_HEALTHY"] {
                copy.flags.insert(flag.to_owned());
            }
            if monster.flags.contains("FILTHY") {
                copy.flags.insert("FILTHY".to_owned());
            }
            copy
        })
        .collect();
    out.extend(dormant);
    Ok(out)
}

fn collect(value: &Value, out: &mut BTreeMap<String, Object>, order: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, out, order);
            }
        }
        Value::Object(object) => {
            if !matches!(object.get("type"), Some(Value::Str(text)) if text == "MONSTER") {
                return;
            }
            // An id may be a list, in which case one object defines several
            // monsters, all of them identical.
            let ids: Vec<String> = match object.get("id") {
                Some(Value::Str(id)) => vec![id.clone()],
                Some(Value::Array(entries)) => entries
                    .iter()
                    .filter_map(|entry| match entry {
                        Value::Str(id) => Some(id.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            if ids.is_empty() {
                let Some(abstract_id) = string(object, "abstract") else {
                    return;
                };
                out.insert(abstract_id, object.clone());
                return;
            }
            for id in ids {
                if !out.contains_key(&id) {
                    order.push(id.clone());
                }
                out.insert(id, object.clone());
            }
        }
        _ => {}
    }
}

fn resolve(
    id: &str,
    objects: &BTreeMap<String, Object>,
    seen: &mut Vec<String>,
) -> Result<Vec<MonsterType>, LoadError> {
    if seen.iter().any(|name| name == id) {
        return Err(LoadError {
            message: format!("monster {id}: copy-from cycle"),
        });
    }
    let Some(object) = objects.get(id) else {
        return Err(LoadError {
            message: format!("monster {id}: not defined"),
        });
    };
    let monster = match object.get("copy-from") {
        Some(Value::Str(base)) => {
            seen.push(id.to_owned());
            let resolved = resolve(base, objects, seen)?;
            seen.pop();
            resolved.into_iter().next()
        }
        _ => None,
    };
    let inherited = monster.is_some();
    let mut monster = monster.unwrap_or_else(MonsterType::new);
    id.clone_into(&mut monster.id);
    apply(&mut monster, object, inherited)?;
    Ok(vec![monster])
}

fn apply(monster: &mut MonsterType, object: &Object, inherited: bool) -> Result<(), LoadError> {
    let id = monster.id.clone();
    // The reference makes both mandatory, but an abstract is allowed to leave
    // them out: the error is logged and loading carries on with an empty value,
    // which is what the content relies on for its shared bases.
    // A name is text, or an object with `str` for the singular.
    if let Some(name) = name_of(object) {
        monster.name = name;
    } else if !inherited {
        let _ = id;
    }
    if let Some(symbol) = string(object, "symbol") {
        monster.symbol = symbol;
    }
    if let Some(color) = string(object, "color") {
        monster.color =
            cdda_core_types::colors::color_from_string(&color).map_err(|error| LoadError {
                message: format!("monster {id}: color {color:?}: {error:?}"),
            })?;
    }
    if let Some(bodytype) = string(object, "bodytype") {
        monster.bodytype = bodytype;
    }
    monster.hp = count(object, "hp", monster.hp)?;
    monster.speed = count(object, "speed", monster.speed)?;
    if let Some(harvest) = string(object, "harvest") {
        monster.harvest = harvest;
    }
    if let Some(dissect) = string(object, "dissect") {
        monster.dissect = dissect;
    }
    // `size` is not read from JSON at all: the reference overwrites it from the creature's volume once every
    // type is loaded (`monstergenerator.cpp`, `mon.size = volume_to_size( mon.volume )`). Reading the field here
    // would be reading something the reference ignores.
    monster.diff = count(object, "diff", monster.diff)?;
    monster.melee_skill = count(object, "melee_skill", monster.melee_skill)?;
    monster.melee_dice = count(object, "melee_dice", monster.melee_dice)?;
    monster.melee_sides = count(object, "melee_sides", monster.melee_sides)?;
    monster.melee_dice_ap = count(object, "melee_dice_ap", monster.melee_dice_ap)?;
    monster.melee_bash = real(object, "melee_bash", monster.melee_bash);
    monster.melee_cut = real(object, "melee_cut", monster.melee_cut);
    monster.melee_stab = real(object, "melee_stab", monster.melee_stab);
    monster.attack_cost = count(object, "attack_cost", monster.attack_cost)?;
    // Armour is read flat in the dump (`resistance_values` writes one integer per damage type) rather
    // than as the `armor` object the game's JSON uses, because the dump is what this loader is fed.
    monster.armor_bash = count(object, "armor_bash", monster.armor_bash)?;
    monster.armor_cut = count(object, "armor_cut", monster.armor_cut)?;
    monster.armor_stab = count(object, "armor_stab", monster.armor_stab)?;
    // A stated rate is a float in the data, so it is read as one rather than through the integer helper - `regenerates: 0.5` is half a hit point an hour and a `count`
    // reader would make it zero.
    match object.get("regenerates") {
        Some(Value::Float(rate)) => monster.regenerates = *rate,
        Some(Value::Int(rate)) => {
            #[allow(clippy::cast_precision_loss)]
            {
                monster.regenerates = *rate as f64;
            }
        }
        _ => {}
    }
    monster.dodge = count(object, "dodge", monster.dodge)?;
    monster.morale = count(object, "morale", monster.morale)?;
    monster.aggression = count(object, "aggression", monster.aggression)?;
    // A plain JSON boolean in the dump, so it is read directly rather than through a helper that would need its own gate.
    if let Some(cdda_json::Value::Bool(said)) = object.get("regen_morale") {
        monster.regen_morale = *said;
    }
    monster.bleed_rate = count(object, "bleed_rate", monster.bleed_rate)?;
    if let Some(cdda_json::Value::Bool(dark)) = object.get("regenerates_in_dark") {
        monster.regenerates_in_dark = *dark;
    }
    // **The body's weak points, when the type states any.** The dump writes the set's list and its default under one object, and a type with nothing to say has an empty list and the
    // reference's default - which is the same shape, so the reader decides by the list rather than by the key's presence.
    if let Some(cdda_json::Value::Array(families)) = object
        .get("weakpoints")
        .and_then(|set| match set {
            cdda_json::Value::Object(inner) => inner.get("families"),
            _ => None,
        })
    {
        monster.families = crate::weakpoints::read_families(families);
    }
    if let Some(cdda_json::Value::Object(set)) = object.get("weakpoints")
        && let Some(cdda_json::Value::Array(entries)) = set.get("list")
        && !entries.is_empty()
    {
        let mut weakpoints = Vec::new();
        for entry in entries {
            weakpoints.push(
                crate::weakpoints::read_weakpoint(entry).map_err(|error| LoadError {
                    message: error.message,
                })?,
            );
        }
        let default = match set.get("default") {
            Some(entry) => crate::weakpoints::read_weakpoint(entry).map_err(|error| LoadError {
                message: error.message,
            })?,
            None => {
                return Err(LoadError {
                    message: format!("the monster {} has weak points but no default", monster.id),
                });
            }
        };
        monster.weakpoints = Some(crate::weakpoints::WeakpointSet {
            id: String::new(),
            weakpoints,
            default,
        });
    }
    // The two strings the death path reads, with the reference's own defaults: no message, and a normal corpse.
    if let Some(cdda_json::Value::Str(said)) = object.get("death_message") {
        monster.death_message.clone_from(said);
    }
    if let Some(cdda_json::Value::Str(kind)) = object.get("corpse_type") {
        monster.corpse_type.clone_from(kind);
    }
    if let Some(cdda_json::Value::Str(item)) = object.get("broken_itype") {
        monster.broken_itype.clone_from(item);
    }
    if let Some(cdda_json::Value::Str(group)) = object.get("death_drops") {
        monster.death_drops.clone_from(group);
    }
    monster.weight = assigned(object, "weight", monster.weight, Unit::Mass, 1);
    monster.volume = assigned(object, "volume", monster.volume, Unit::Volume, 1);
    species_and_categories(monster, object);
    materials(monster, object);
    flags(monster, object);
    Ok(())
}

/// The singular name, from text or from an object with `str`.
fn name_of(object: &Object) -> Option<String> {
    match object.get("name") {
        Some(Value::Str(text)) => Some(text.clone()),
        // `str_sp` states one form for both singular and plural.
        Some(Value::Object(inner)) => string(inner, "str").or_else(|| string(inner, "str_sp")),
        _ => None,
    }
}

/// A plain whole number, adjusted the way `assign` does.
/// A member the dump states as a real and the world holds as a whole number, rounded the way the reference rounds damage before it is taken off hit points.
fn real(object: &Object, member: &str, current: i32) -> i32 {
    let Some(value) = object.get(member) else {
        return current;
    };
    let number = match value {
        Value::Int(number) => i64::from(i32::try_from(*number).unwrap_or(current)),
        Value::Float(number) => {
            #[allow(clippy::cast_possible_truncation)]
            let whole = *number as i64;
            whole
        }
        _ => return current,
    };
    i32::try_from(number).unwrap_or(current)
}

fn count(object: &Object, member: &str, current: i32) -> Result<i32, LoadError> {
    let value = assigned(object, member, i64::from(current), Unit::Number, 1);
    i32::try_from(value).map_err(|_| LoadError {
        message: format!("monster {member}: {value} does not fit a count"),
    })
}

/// Which unit a `assign`ed number is in.
#[derive(Clone, Copy)]
enum Unit {
    /// A plain count.
    Number,
    /// Milligrams, with a bare number meaning grams.
    Mass,
    /// Millilitres.
    Volume,
}

/// A number that may be stated, adjusted, or scaled — in `assign`'s order.
///
/// `assign` checks `relative` first and `proportional` second
/// (`assign.cpp:60-88`), so an object stating both gets the sum, not the product.
/// Items go the other way round; see `items::adjusted`.
fn assigned(object: &Object, member: &str, current: i64, unit: Unit, int_scale: i64) -> i64 {
    let reader = match unit {
        Unit::Mass => read_mass,
        Unit::Volume => read_volume,
        Unit::Number => read_count,
    };
    let parse = |value: &Value, scale: i64| -> Option<i64> {
        match value {
            Value::Int(number) => Some(number * scale),
            Value::Str(text) => reader(text).ok(),
            Value::Float(number) =>
            {
                #[allow(clippy::cast_possible_truncation)]
                Some(*number as i64)
            }
            _ => None,
        }
    };
    if let Some(relative) = sub_object(object, "relative")
        && let Some(value) = relative_value(relative, member, &parse, int_scale)
    {
        return current + value;
    }
    if let Some(proportional) = sub_object(object, "proportional")
        && let Some(scalar) = number(proportional, member)
    {
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        return (current as f64 * scalar) as i64;
    }
    object
        .get(member)
        .and_then(|value| parse(value, int_scale))
        .unwrap_or(current)
}

fn relative_value(
    object: &Object,
    member: &str,
    parse: &dyn Fn(&Value, i64) -> Option<i64>,
    int_scale: i64,
) -> Option<i64> {
    object
        .get(member)
        .and_then(|value| parse(value, int_scale))
        .or(None)
}

/// A bare number is already a count.
fn read_count(text: &str) -> Result<i64, cdda_core_types::units::UnitError> {
    text.parse()
        .map_err(|_| cdda_core_types::units::UnitError::NumberExpected)
}

/// Species and categories are sets: stated in the object they replace what was
/// inherited, and `extend` and `delete` adjust it instead
/// (`generic_factory.h:1153-1170`).
fn species_and_categories(monster: &mut MonsterType, object: &Object) {
    if object.get("species").is_some() {
        monster.species.clear();
        monster.species.extend(strings(object, "species"));
    }
    if object.get("categories").is_some() {
        monster.categories.clear();
        monster.categories.extend(strings(object, "categories"));
    }
    if let Some(extend) = sub_object(object, "extend") {
        monster.species.extend(strings(extend, "species"));
        monster.categories.extend(strings(extend, "categories"));
    }
    if let Some(delete) = sub_object(object, "delete") {
        for species in strings(delete, "species") {
            monster.species.remove(&species);
        }
        for category in strings(delete, "categories") {
            monster.categories.remove(&category);
        }
    }
}

/// Flags are a set: stated in the object they replace what was inherited, and
/// `extend` and `delete` adjust it instead.
fn flags(monster: &mut MonsterType, object: &Object) {
    if object.get("flags").is_some() {
        monster.flags.clear();
        monster.flags.extend(strings(object, "flags"));
    }
    if let Some(extend) = sub_object(object, "extend") {
        monster.flags.extend(strings(extend, "flags"));
    }
    if let Some(delete) = sub_object(object, "delete") {
        for flag in strings(delete, "flags") {
            monster.flags.remove(&flag);
        }
    }
}

/// Materials are read as tags, one portion each, and anything left without one
/// is made of flesh (`mtype.cpp:761-771`).
fn materials(monster: &mut MonsterType, object: &Object) {
    if object.get("material").is_some() {
        monster.materials.clear();
        for material in strings(object, "material") {
            monster.materials.insert(material, 1);
        }
    }
    if monster.materials.is_empty() {
        monster.materials.insert("flesh".to_owned(), 1);
    }
}

/// The monster the reference makes for itself so that a null id resolves.
///
/// It is a default-constructed type, which is why its numbers look arbitrary:
/// they are the field initialisers, and the dump is where they were read from.
/// How big a creature is, from how much space it takes up.
///
/// Reference: `volume_to_size` (`monstergenerator.cpp`), whose thresholds are inclusive on the lower side:
/// 7,500 ml is still tiny, 46,250 still small, 108,000 still medium, 483,750 still large, anything more is huge.
#[must_use]
pub fn size_from_volume(volume_ml: i64) -> String {
    let name = if volume_ml <= 7_500 {
        "tiny"
    } else if volume_ml <= 46_250 {
        "small"
    } else if volume_ml <= 108_000 {
        "medium"
    } else if volume_ml <= 483_750 {
        "large"
    } else {
        "huge"
    };
    name.to_owned()
}

fn null_monster() -> MonsterType {
    let mut monster = MonsterType::new();
    "mon_null".clone_into(&mut monster.id);
    "human".clone_into(&mut monster.name);
    " ".clone_into(&mut monster.symbol);
    monster.hp = 1;
    monster.weight = 81_499_000;
    monster.volume = 62_499;
    monster.materials.insert("flesh".to_owned(), 1);
    monster
}

impl MonsterType {
    fn new() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            symbol: String::new(),
            color: cdda_core_types::colors::color_from_string("white")
                .unwrap_or_else(|_| cdda_core_types::colors::NcColor::from_attribute(0)),
            bodytype: String::new(),
            hp: 0,
            speed: 0,
            diff: 0,
            melee_skill: 0,
            dodge: 0,
            morale: 0,
            // `mtype::regen_morale` starts false: most monsters do not get their nerve back, and the 23 that do say so.
            regen_morale: false,
            // `mtype::bleed_rate` starts at a hundred (`mtype.h:486`), which is the ordinary case: a species' bleed type is used unless the data says otherwise.
            bleed_rate: 100,
            // Most types do not mend in the dark (`mtype::regenerates_in_dark` starts false).
            regenerates_in_dark: false,
            // Most types state no weak points at all; the ones that do say so in their own row.
            weakpoints: None,
            // A type with no families rewards no proficiency.
            families: Vec::new(),
            // `monster_death_effect` starts with no message and a normal corpse (`mtype.h:274-285`).
            death_message: String::new(),
            corpse_type: "normal".to_owned(),
            broken_itype: String::new(),
            death_drops: String::new(),
            aggression: 0,
            // The reference's own defaults: no dice (`mtype::melee_dice` starts at zero) and no armour.
            melee_dice: 0,
            melee_sides: 0,
            melee_dice_ap: 0,
            melee_bash: 0,
            melee_cut: 0,
            melee_stab: 0,
            attack_cost: 100,
            armor_bash: 0,
            armor_cut: 0,
            armor_stab: 0,
            // The reference's own default: no stated rate, so the fallbacks decide (`mtype::mtype()` leaves `regenerates` at zero).
            regenerates: 0.0,
            // The reference's own defaults for a type that states neither.
            weight: 81_499_000,
            volume: 62_499,
            species: BTreeSet::new(),
            categories: BTreeSet::new(),
            materials: BTreeMap::new(),
            flags: BTreeSet::new(),
            // The reference's own defaults (`mtype::mtype()`): a human harvest, no dissection, medium sized.
            harvest: "human".to_owned(),
            dissect: String::new(),
            size: "medium".to_owned(),
        }
    }
}

use cdda_core_types::colors::NcColor;

/// One monster species, as far as the bleed rule needs it.
///
/// Reference: `species_type` (`monstergenerator.h:33-53`), whose `bleeds` field is what `mtype::get_bleed_type` falls through to (`mtype.cpp:412-422`). The dump carries one entry per
/// species a monster names - **31 of them** - with the field id the data gives, `fd_null` included because the data spells it out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeciesType {
    /// Its id, as the monsters name it.
    pub id: String,
    /// The field it bleeds, or empty when the data says nothing.
    pub bleeds: String,
}

/// Reads the species table from a monster dump.
///
/// The species are **not** in the prototype list: they are their own table beside it, which is the reference's own shape (`MonsterGenerator`'s `mon_species`), and a loader that
/// searched the prototypes for them would find monsters instead.
#[must_use]
pub fn species_from_dump(value: &cdda_json::Value) -> Vec<SpeciesType> {
    let cdda_json::Value::Object(root) = value else {
        return Vec::new();
    };
    let Some(cdda_json::Value::Array(entries)) = root.get("species") else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let cdda_json::Value::Object(object) = entry else {
                return None;
            };
            let id = string(object, "id")?;
            let bleeds = string(object, "bleeds").unwrap_or_default();
            Some(SpeciesType { id, bleeds })
        })
        .collect()
}

/// Loads the species tables from the given data files.
///
/// # Errors
/// Returns the first file that could not be read.
pub fn load_species_paths(dirs: &[&std::path::Path]) -> Result<Vec<SpeciesType>, LoadError> {
    let mut out = Vec::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            out.extend(species_from_dump(&value));
        }
    }
    Ok(out)
}
