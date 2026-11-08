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
    /// What it yields, **after** [`resolve_harvests`] has folded in what it inherits.
    pub entries: Vec<HarvestEntry>,
    /// The definition this one copies, when it states one (`copy-from`).
    pub copy_from: Option<String>,
    /// The entries this one adds to what it inherited (`extend`).
    pub extend: Vec<HarvestEntry>,
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
        // **The half of the definition that is not written down.** Most harvest lists in the content are a `copy-from` plus an `extend`, which is why a loader that reads only `entries` gives the
        // commonest body in the game - the humanoid zombie - an empty one.
        let copy_from = match object.get("copy-from") {
            Some(Value::Str(parent)) => Some(parent.clone()),
            _ => None,
        };
        let extend = match object.get("extend") {
            Some(Value::Object(inner)) => match inner.get("entries") {
                Some(Value::Array(list)) => list.iter().filter_map(read_entry).collect(),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        out.insert(
            id.clone(),
            HarvestDefinition {
                id: id.clone(),
                entries,
                copy_from,
                extend,
            },
        );
    }
    out
}

/// Folds every `copy-from` in a set of harvest definitions into the definitions that inherit from them.
///
/// Reference: `harvest_definition` is a `generic_factory` entry, so its `copy-from` is resolved at finalisation and its `extend.entries` are appended **after** the inherited list, in the order the
/// child writes them. A parent that is itself a copy is resolved first, and a cycle resolves to whatever the pass reached - the reference refuses a cycle outright, and a port that looped on one would
/// hang, so the passes are bounded by the number of definitions.
///
/// **A definition that states `entries` and no `copy-from` keeps exactly what it stated**, which is the reference's own rule: the parent is only consulted when the child names one.
pub fn resolve_harvests(all: &mut BTreeMap<String, HarvestDefinition>) {
    let ids: Vec<String> = all.keys().cloned().collect();
    // The bound is the number of definitions: every pass resolves at least the chains whose parent is already resolved, so a chain of n resolves in n passes at the very worst.
    for _ in 0..ids.len().max(1) {
        let mut changed = false;
        for id in &ids {
            let Some(definition) = all.get(id) else {
                continue;
            };
            let Some(parent_id) = definition.copy_from.clone() else {
                continue;
            };
            let Some(parent) = all.get(&parent_id) else {
                continue;
            };
            // A parent that has not been resolved yet is waited for, unless it inherits in turn and the chain has run as far as it can.
            if parent.copy_from.is_some() && parent.entries.is_empty() && !parent.extend.is_empty() {
                continue;
            }
            let mut entries = parent.entries.clone();
            entries.extend(definition.extend.iter().cloned());
            let inherited = entries != definition.entries;
            if let Some(definition) = all.get_mut(id) {
                if inherited {
                    definition.entries = entries;
                    changed = true;
                }
                definition.copy_from = None;
            }
        }
        if !changed {
            break;
        }
    }
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
