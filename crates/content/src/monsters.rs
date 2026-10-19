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
    /// Morale.
    pub morale: i32,
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
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<Vec<MonsterType>, LoadError> {
    let mut objects: BTreeMap<String, Object> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let bytes = std::fs::read(&path).map_err(|error| LoadError {
                message: format!("{}: {error}", path.display()),
            })?;
            let Ok(value) = cdda_json::parse(&bytes) else {
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
    monster.diff = count(object, "diff", monster.diff)?;
    monster.melee_skill = count(object, "melee_skill", monster.melee_skill)?;
    monster.dodge = count(object, "dodge", monster.dodge)?;
    monster.morale = count(object, "morale", monster.morale)?;
    monster.aggression = count(object, "aggression", monster.aggression)?;
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
            aggression: 0,
            // The reference's own defaults for a type that states neither.
            weight: 81_499_000,
            volume: 62_499,
            species: BTreeSet::new(),
            categories: BTreeSet::new(),
            materials: BTreeMap::new(),
            flags: BTreeSet::new(),
        }
    }
}

use cdda_core_types::colors::NcColor;
