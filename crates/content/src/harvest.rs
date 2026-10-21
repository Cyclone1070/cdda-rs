//! Harvest definitions: what a butcher can take from a corpse, and how much.
//!
//! Reference: `harvest.h`/`harvest.cpp` and the `harvest` type in `data/json/harvest.json`. A monster names a
//! harvest id; the definition says which drops that means. An entry either gives a **number of items**
//! (`base_num`/`scale_num`, rolled) or a **fraction of the corpse's mass** (`mass_ratio`), and the two are not
//! interchangeable: meat is a proportion of a body, seeds are a count of things.
//!
//! Kept here rather than in the sim because it is content: the numbers come from JSON, and the sim only asks.

use cdda_json::{Object, Value};
use std::collections::BTreeMap;

/// What part of a corpse an entry comes from, which decides how damage to the corpse affects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// Meat.
    Flesh,
    /// Organs.
    Offal,
    /// Hide.
    Skin,
    /// Bones.
    Bone,
    /// Blood.
    Blood,
    /// Something the reference treats as its own kind, kept by name.
    Other,
}

impl Part {
    /// The reference's own spelling, which is what the JSON uses.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "flesh" => Self::Flesh,
            "offal" => Self::Offal,
            "skin" => Self::Skin,
            "bone" => Self::Bone,
            "blood" => Self::Blood,
            _ => Self::Other,
        }
    }
}

/// One thing a corpse gives up.
#[derive(Debug, Clone, PartialEq)]
pub struct HarvestEntry {
    /// The item group or item id that drops.
    pub drop: String,
    /// What part of the body it is.
    pub part: Option<Part>,
    /// A count of items, low and high, before scaling.
    pub base_num: Option<(i32, i32)>,
    /// How the count grows with the corpse, low and high.
    pub scale_num: Option<(f64, f64)>,
    /// A proportion of the corpse's mass, for things measured in bulk.
    pub mass_ratio: Option<f64>,
    /// A ceiling on the count, as the reference writes `max`.
    pub max: Option<i32>,
    /// Any flags the entry carries, sorted.
    pub flags: Vec<String>,
}

/// A named harvest.
#[derive(Debug, Clone, PartialEq)]
pub struct HarvestDefinition {
    /// Its id, which is what a monster names.
    pub id: String,
    /// What it yields.
    pub entries: Vec<HarvestEntry>,
}

/// Reads every harvest definition in a file or list, keyed by id.
#[must_use]
pub fn load_harvests(value: &Value) -> BTreeMap<String, HarvestDefinition> {
    let mut out = BTreeMap::new();
    let single = [value.clone()];
    let entries: &[Value] = match value {
        Value::Array(entries) => entries,
        Value::Object(_) => &single,
        _ => &[],
    };
    for entry in entries {
        let Value::Object(object) = entry else {
            continue;
        };
        if let Some(Value::Str(kind)) = object.get("type")
            && kind != "harvest"
        {
            continue;
        }
        let Some(Value::Str(id)) = object.get("id") else {
            continue;
        };
        let entries = match object.get("entries") {
            Some(Value::Array(list)) => list.iter().filter_map(read_entry).collect(),
            _ => Vec::new(),
        };
        out.insert(
            id.clone(),
            HarvestDefinition {
                id: id.clone(),
                entries,
            },
        );
    }
    out
}

/// A number that may be a single value or a low-high pair, as the reference writes them.
///
/// The JSON holds whole numbers and fractions in the same fields, and the reference reads them all as floats, so
/// the conversion here is the same one it makes.
#[allow(clippy::cast_precision_loss)]
fn pair(object: &Object, key: &str) -> Option<(f64, f64)> {
    match object.get(key)? {
        Value::Int(value) => Some((*value as f64, *value as f64)),
        Value::Float(value) => Some((*value, *value)),
        Value::Array(values) => {
            let first = match values.first()? {
                Value::Int(value) => *value as f64,
                Value::Float(value) => *value,
                _ => return None,
            };
            let second = match values.get(1)? {
                Value::Int(value) => *value as f64,
                Value::Float(value) => *value,
                _ => return None,
            };
            Some((first, second))
        }
        _ => None,
    }
}

#[allow(clippy::cast_precision_loss)]
fn read_entry(value: &Value) -> Option<HarvestEntry> {
    let Value::Object(object) = value else {
        return None;
    };
    let Some(Value::Str(drop)) = object.get("drop") else {
        return None;
    };
    let part = match object.get("type") {
        Some(Value::Str(name)) => Some(Part::from_name(name)),
        _ => None,
    };
    let base = pair(object, "base_num");
    let scale = pair(object, "scale_num");
    #[allow(clippy::cast_precision_loss)]
    let max = match object.get("max") {
        Some(Value::Int(value)) => i32::try_from(*value).ok(),
        _ => None,
    };
    let mass = match object.get("mass_ratio") {
        Some(Value::Int(value)) => Some(*value as f64),
        Some(Value::Float(value)) => Some(*value),
        _ => None,
    };
    let mut flags = Vec::new();
    if let Some(Value::Array(list)) = object.get("flags") {
        flags.extend(list.iter().filter_map(|flag| match flag {
            Value::Str(name) => Some(name.clone()),
            _ => None,
        }));
    }
    flags.sort();
    Some(HarvestEntry {
        drop: drop.clone(),
        part,
        base_num: base.map(|(low, high)| {
            #[allow(clippy::cast_possible_truncation)]
            let pair = (low as i32, high as i32);
            pair
        }),
        scale_num: scale,
        mass_ratio: mass,
        max,
        flags,
    })
}
