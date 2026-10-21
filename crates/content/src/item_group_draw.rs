//! Drawing items out of a group, the way the reference does.
//!
//! Reference: `Item_spawn_data::create` and friends (`src/item_group.cpp`). A group is either a
//! **collection** - take every entry, each with its own chance - or a **distribution** - take exactly one, chosen by
//! weight. Getting that backwards is the difference between a crate that holds five things and a crate that holds
//! one, and both look plausible in code.
//!
//! The definitions here are read from JSON rather than from the flattened dump, because drawing needs the
//! probabilities and the nesting that the dump's totals have already summed away.

use cdda_core_types::rng::Rolls;
use cdda_json::{Object, Value};
use std::collections::BTreeMap;

/// One entry in a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// A specific item, with a chance out of a hundred and a count.
    Item {
        /// The item's id.
        id: String,
        /// Chance out of a hundred, before the reference's roll.
        probability: i32,
        /// How few and how many.
        count: (i32, i32),
    },
    /// Another group, with a chance out of a hundred.
    Group {
        /// The nested group's id.
        id: String,
        /// Chance out of a hundred, before the reference's roll.
        probability: i32,
    },
    /// The reference's explicit "nothing", which occupies weight in a distribution.
    Nothing {
        /// Chance out of a hundred.
        probability: i32,
    },
}

impl Entry {
    /// How much of a distribution's weight this entry occupies.
    fn weight(&self) -> i32 {
        match self {
            Self::Item { probability, .. }
            | Self::Group { probability, .. }
            | Self::Nothing { probability } => *probability,
        }
    }
}

/// A group as the JSON states it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    /// Its id.
    pub id: String,
    /// True when the group picks one entry, false when it takes them all.
    pub distribution: bool,
    /// What it can produce.
    pub entries: Vec<Entry>,
}

/// Reads every group definition in a file or list.
#[must_use]
pub fn load_definitions(value: &Value) -> BTreeMap<String, Definition> {
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
            && kind != "item_group"
        {
            continue;
        }
        let Some(Value::Str(id)) = object.get("id") else {
            continue;
        };
        let distribution =
            matches!(object.get("subtype"), Some(Value::Str(kind)) if kind == "distribution");
        let entries = match object.get("entries") {
            Some(Value::Array(list)) => list.iter().filter_map(read_entry).collect(),
            _ => Vec::new(),
        };
        out.insert(
            id.clone(),
            Definition {
                id: id.clone(),
                distribution,
                entries,
            },
        );
    }
    out
}

/// The chance an entry is taken, out of a hundred.
///
/// The reference writes this as `prob` in its data and calls it `probability` in code; both spellings are read
/// because both appear, and a default of a hundred is the reference's own for an entry that states none.
fn probability(object: &Object) -> i32 {
    for key in ["prob", "probability"] {
        if let Some(Value::Int(value)) = object.get(key) {
            return i32::try_from(*value).unwrap_or(100);
        }
    }
    100
}

fn count_range(object: &Object) -> (i32, i32) {
    let read = |value: Option<&Value>| match value {
        Some(Value::Int(number)) => i32::try_from(*number).unwrap_or(1),
        Some(Value::Array(pair)) => match pair.first() {
            Some(Value::Int(number)) => i32::try_from(*number).unwrap_or(1),
            _ => 1,
        },
        _ => 1,
    };
    let low = read(object.get("count"));
    let high = match object.get("count") {
        Some(Value::Array(pair)) => match pair.get(1) {
            Some(Value::Int(number)) => i32::try_from(*number).unwrap_or(low),
            _ => low,
        },
        _ => low,
    };
    (low, high.max(low))
}

fn read_entry(value: &Value) -> Option<Entry> {
    let Value::Object(object) = value else {
        return None;
    };
    let probability = probability(object);
    // `null` is the reference's explicit "nothing", and it is written as an item, so it has to be recognised
    // before the item branch or it becomes an item that does not exist.
    if matches!(object.get("item"), Some(Value::Str(id)) if id == "null") {
        return Some(Entry::Nothing { probability });
    }
    if let Some(Value::Str(id)) = object.get("item") {
        return Some(Entry::Item {
            id: id.clone(),
            probability,
            count: count_range(object),
        });
    }
    if let Some(Value::Str(id)) = object.get("group") {
        return Some(Entry::Group {
            id: id.clone(),
            probability,
        });
    }
    None
}

/// Draws what a group produces.
///
/// A **collection** walks its entries and takes each one that its chance allows. A **distribution** picks exactly
/// one, by weight. Nested groups are expanded in place, so a group of groups is a group of items.
///
/// Returns what was drawn as `(item id, count)` pairs, in the order the reference would create them.
pub fn draw(
    definitions: &BTreeMap<String, Definition>,
    id: &str,
    rolls: &mut Rolls,
) -> Vec<(String, i32)> {
    let mut out = Vec::new();
    draw_into(definitions, id, rolls, &mut out, 0);
    out
}

fn draw_into(
    definitions: &BTreeMap<String, Definition>,
    id: &str,
    rolls: &mut Rolls,
    out: &mut Vec<(String, i32)>,
    depth: usize,
) {
    // A group that names itself would otherwise draw forever; the reference rejects such data at load time.
    if depth > 8 {
        return;
    }
    let Some(definition) = definitions.get(id) else {
        return;
    };
    if definition.distribution {
        let total: i32 = definition.entries.iter().map(Entry::weight).sum();
        if total <= 0 {
            return;
        }
        let mut roll = rolls.rng(1, total);
        for entry in &definition.entries {
            let weight = entry.weight();
            if roll > weight {
                roll -= weight;
                continue;
            }
            match entry {
                Entry::Item { id, count, .. } => {
                    out.push((id.clone(), rolls.rng(count.0.max(1), count.1.max(1))));
                }
                Entry::Group { id, .. } => draw_into(definitions, id, rolls, out, depth + 1),
                Entry::Nothing { .. } => {}
            }
            return;
        }
        return;
    }
    for entry in &definition.entries {
        match entry {
            Entry::Item {
                id,
                probability,
                count,
            } => {
                if *probability < 100 && rolls.rng(1, 100) > *probability {
                    continue;
                }
                out.push((id.clone(), rolls.rng(count.0.max(1), count.1.max(1))));
            }
            Entry::Group { id, probability } => {
                if *probability < 100 && rolls.rng(1, 100) > *probability {
                    continue;
                }
                draw_into(definitions, id, rolls, out, depth + 1);
            }
            Entry::Nothing { .. } => {}
        }
    }
}
