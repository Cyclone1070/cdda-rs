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
        /// The holiday this entry belongs to, when it belongs to one (`Item_spawn_data::event`).
        ///
        /// A holiday entry carries two probabilities: its raw weight, which it keeps in the distribution's total, and its
        /// chance once the holiday gate has been applied, which is zero outside its holiday (`get_probability`, `src/item_group.cpp:901-920`).
        event: Option<String>,
    },
    /// Another group, with a chance out of a hundred.
    Group {
        /// The nested group's id.
        id: String,
        /// Chance out of a hundred, before the reference's roll.
        probability: i32,
        /// The holiday this group belongs to, when it belongs to one.
        event: Option<String>,
    },
    /// The reference's explicit "nothing", which occupies weight in a distribution.
    Nothing {
        /// Chance out of a hundred.
        probability: i32,
    },
}

impl Entry {
    /// The same entry with a different weight, which is how a collection's chances are capped.
    fn with_weight(self, weight: i32) -> Self {
        match self {
            Self::Item {
                id,
                count,
                event,
                probability: _,
            } => Self::Item {
                id,
                probability: weight,
                count,
                event,
            },
            Self::Group {
                id,
                event,
                probability: _,
            } => Self::Group {
                id,
                probability: weight,
                event,
            },
            Self::Nothing { .. } => Self::Nothing {
                probability: weight,
            },
        }
    }

    /// Whether the entry belongs to a holiday at all (`Item_spawn_data::is_event_based`).
    fn is_event_based(&self) -> bool {
        match self {
            Self::Item { event, .. } | Self::Group { event, .. } => event.is_some(),
            Self::Nothing { .. } => false,
        }
    }

    /// The chance once the holiday gate has been applied (`get_probability( false )`): zero outside its holiday.
    fn in_season_probability(&self, holiday: Option<&str>) -> i32 {
        match self {
            Self::Item {
                probability, event, ..
            }
            | Self::Group {
                probability, event, ..
            } => match event {
                Some(only) if holiday != Some(only.as_str()) => 0,
                _ => *probability,
            },
            Self::Nothing { probability } => *probability,
        }
    }

    /// How much of a distribution's weight this entry occupies (`get_probability( true )`, the raw weight).
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
        let entries: Vec<Entry> = match object.get("entries") {
            Some(Value::Array(list)) => list
                .iter()
                .filter_map(read_entry)
                // `Item_group::add_entry` drops an entry whose weight is not positive, so a `prob: 0` entry is not in the group at all
                // (`src/item_group.cpp:813-815`).
                .filter(|entry| entry.weight() > 0)
                .map(|entry| {
                    if distribution {
                        entry
                    } else {
                        // A collection's chances are percentages, and the reference caps them at a hundred (`src/item_group.cpp:818-820`).
                        let capped = entry.weight().min(100);
                        entry.with_weight(capped)
                    }
                })
                .collect(),
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
    // The reference's holiday gate lives on every spawn-data entry (`Item_spawn_data::event`), written as `event` in the data.
    let event = match object.get("event") {
        Some(Value::Str(holiday)) if holiday != "none" => Some(holiday.clone()),
        _ => None,
    };
    if let Some(Value::Str(id)) = object.get("item") {
        return Some(Entry::Item {
            id: id.clone(),
            probability,
            count: count_range(object),
            event,
        });
    }
    if let Some(Value::Str(id)) = object.get("group") {
        return Some(Entry::Group {
            id: id.clone(),
            probability,
            event,
        });
    }
    None
}

/// Draws what a group produces, with no holiday in season.
///
/// A caller that knows the date uses [`draw_with_holiday`]; this is the form every other caller wants.
///
/// # Panics
///
/// Panics if a replayed recording runs out of dice, as every replayed draw does.
#[must_use]
pub fn draw(
    definitions: &BTreeMap<String, Definition>,
    id: &str,
    rolls: &mut Rolls,
) -> Vec<(String, i32)> {
    draw_with_holiday(definitions, id, rolls, None)
}

/// **Draws at a spawn rate**, which is what a map or a vehicle spawn asks for.
///
/// Reference: `item_group::items_from( id, birthday, flags )` with `spawn_flags::use_spawn_rate` - inside `Single_item_creator::create` (`item_group.cpp:285-296`), each item made rolls
/// `rng_float( 0, 1 )` and is **skipped when that is greater than the rate**. The rate is the `ITEM_SPAWNRATE` option, read one item at a time, so a rate of zero makes a group draw
/// nothing at all and a rate of one leaves it exactly as it was.
///
/// **Named, not guessed:** the reference exempts `MISSION_ITEM`s from the die; the port's draw carries no item flags and exempts nothing.
#[must_use]
pub fn draw_at_rate(
    definitions: &BTreeMap<String, Definition>,
    id: &str,
    rolls: &mut Rolls,
    spawn_rate: f64,
) -> Vec<(String, i32)> {
    let mut out = Vec::new();
    draw_into_at_rate(
        definitions,
        id,
        rolls,
        &mut out,
        &mut Vec::new(),
        None,
        Some(spawn_rate),
    );
    out
}

/// Draws what a group produces, against the holiday the calendar is in.
///
/// Reference: `Item_group::create` (`src/item_group.cpp:832-864`), which asks `rng( 0, 99 )` per entry of a **collection** and `rng( 0, sum_prob - 1 )` once for a
/// **distribution**, then subtracts each entry's raw weight and takes the first that takes the running total below zero. The holiday test comes between the two
/// subtractions: an event-based entry outside its holiday has an in-season chance of zero, and is passed over **even when the die chose it**
/// (`( ev_based && prob == 0 ) || p >= 0` → continue).
///
/// # Panics
///
/// Panics if a replayed recording runs out of dice.
#[must_use]
pub fn draw_with_holiday(
    definitions: &BTreeMap<String, Definition>,
    id: &str,
    rolls: &mut Rolls,
    holiday: Option<&str>,
) -> Vec<(String, i32)> {
    let mut out = Vec::new();
    draw_into(definitions, id, rolls, &mut out, &mut Vec::new(), holiday);
    out
}

/// How many of an entry to make, drawing a die only when the range can vary.
///
/// Reference: `Single_item_creator::create` (`src/item_group.cpp:277-284`): `cnt = ( modifier_count.first == modifier_count.second ) ? modifier_count.first : rng(
/// modifier_count.first, modifier_count.second )`. A range of one value is not a range: the reference asks the dice nothing, so a port that drew `rng( 5, 5 )` would be one
/// die ahead of the reference for the rest of the session. A count of zero is a count of zero - the reference's `for( ; cnt > 0; cnt-- )` makes nothing.
fn count_to_make(count: (i32, i32), rolls: &mut Rolls) -> usize {
    let made = if count.0 == count.1 {
        count.0
    } else {
        rolls.rng(count.0, count.1)
    };
    usize::try_from(made).unwrap_or(0)
}

/// Expands one group into `out`, refusing to re-enter a group it is already inside.
///
/// Reference: `Single_item_creator::create` (`src/item_group.cpp:307-312`) keeps a `RecursionList` of the groups the expansion is inside, pushes the nested group before
/// expanding it and pops it after, and treats a repeat as an **error**: `debugmsg( "recursion in item spawn list %s" )`, and the branch ends. The test is the **set of
/// names**, not a depth - a cycle of any length stops at its second visit, and a legally deep chain is drawn in full.
fn draw_into(
    definitions: &BTreeMap<String, Definition>,
    id: &str,
    rolls: &mut Rolls,
    out: &mut Vec<(String, i32)>,
    chain: &mut Vec<String>,
    holiday: Option<&str>,
) {
    draw_into_at_rate(definitions, id, rolls, out, chain, holiday, None);
}

/// The same expansion, with the spawn rate the reference reads one item at a time.
fn draw_into_at_rate(
    definitions: &BTreeMap<String, Definition>,
    id: &str,
    rolls: &mut Rolls,
    out: &mut Vec<(String, i32)>,
    chain: &mut Vec<String>,
    holiday: Option<&str>,
    spawn_rate: Option<f64>,
) {
    if chain.iter().any(|inside| inside == id) {
        // The reference reports this and returns; the port has no message sink here, so the branch simply ends. What matters is that it ends **here** and not
        // eight levels down, which is what a depth guard does.
        return;
    }
    let Some(definition) = definitions.get(id) else {
        return;
    };
    chain.push(id.to_owned());
    expand(
        definitions,
        definition,
        rolls,
        out,
        chain,
        holiday,
        spawn_rate,
    );
    chain.pop();
}

/// The two group kinds, once the recursion check has been made.
fn expand(
    definitions: &BTreeMap<String, Definition>,
    definition: &Definition,
    rolls: &mut Rolls,
    out: &mut Vec<(String, i32)>,
    chain: &mut Vec<String>,
    holiday: Option<&str>,
    spawn_rate: Option<f64>,
) {
    if definition.distribution {
        // `sum_prob` is the sum of the **raw** weights, holiday entries included, and the whole draw is one question
        // (`int p = rng( 0, sum_prob - 1 )`, `src/item_group.cpp:844`).
        let total: i32 = definition.entries.iter().map(Entry::weight).sum();
        if total <= 0 {
            return;
        }
        let mut remaining = rolls.rng(0, total - 1);
        for entry in &definition.entries {
            remaining -= entry.weight();
            // The holiday test sits **after** the subtraction and before the answer: an event-based entry whose in-season chance is zero is passed over,
            // and its weight has already been taken off the running total (`src/item_group.cpp:846-856`).
            if (entry.is_event_based() && entry.in_season_probability(holiday) == 0)
                || remaining >= 0
            {
                continue;
            }
            match entry {
                Entry::Item { id, count, .. } => {
                    let made = kept_at_rate(count_to_make(*count, rolls), spawn_rate, rolls);
                    if made > 0 {
                        out.push((id.clone(), i32::try_from(made).unwrap_or(i32::MAX)));
                    }
                }
                Entry::Group { id, .. } => {
                    draw_into_at_rate(definitions, id, rolls, out, chain, holiday, spawn_rate);
                }
                Entry::Nothing { .. } => {}
            }
            return;
        }
        return;
    }
    for entry in &definition.entries {
        // `rng( 0, 99 ) >= get_probability( false )` → skip (`src/item_group.cpp:837-841`).
        if rolls.rng(0, 99) >= entry.in_season_probability(holiday) {
            continue;
        }
        match entry {
            Entry::Item { id, count, .. } => {
                let made = kept_at_rate(count_to_make(*count, rolls), spawn_rate, rolls);
                if made > 0 {
                    out.push((id.clone(), i32::try_from(made).unwrap_or(i32::MAX)));
                }
            }
            Entry::Group { id, .. } => {
                draw_into_at_rate(definitions, id, rolls, out, chain, holiday, spawn_rate);
            }
            Entry::Nothing { .. } => {}
        }
    }
}

/// **How many of a batch survive the spawn rate**: one die per item, kept unless the die is greater than the rate.
///
/// Reference: `item_group.cpp:289-292` - `rng_float( 0, 1 ) > spawn_rate` skips the item, so a rate of one keeps everything and a rate of zero keeps nothing (a die exactly equal to
/// the rate survives, which is the reference's `>` and not a `<`).
fn kept_at_rate(made: usize, spawn_rate: Option<f64>, rolls: &mut Rolls) -> usize {
    // **No rate means no die**, which is `items_from` **without** `spawn_flags::use_spawn_rate`: the reference rolls nothing there, and a port that rolled anyway would be one die
    // ahead of every replay that uses the plain draw.
    let Some(rate) = spawn_rate else {
        return made;
    };
    // **The die is rolled even at a rate of one**, because the reference rolls it: `rng_float` can only fail on `> 1.0`, which its range cannot produce, so the answer is always the
    // whole batch - but the stream has moved by one die per item, and a replay that skipped them would fall out of step for the rest of the session.
    let mut kept = 0;
    for _ in 0..made {
        if rolls.rng_float(0.0, 1.0) <= rate {
            kept += 1;
        }
    }
    kept
}
