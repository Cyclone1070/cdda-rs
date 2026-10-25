//! What using an item does, as far as the data says.
//!
//! Reference: `class use_function` and `class iuse_actor` (`src/iuse.h:279-300`), whose actors are loaded by `iuse_actor::load` and dispatched by the `"type"` member of
//! a `use_action` object. The port reads the **data** for the actors it has ported and keeps the rest by name, so a type's list is complete even while the behaviours
//! behind it are still arriving: an actor the port does not know is [`UseAction::Other`], which is honest about what is missing instead of dropping it.
//!
//! **Legacy string ids are actions too.** A `"use_action": "ALCOHOL"` is an `iuse_function_wrapper` whose type is that very string (`src/item_factory.cpp:1807-1818`),
//! so it is kept the same way an actor's type is — the difference is only that its behaviour lives in a function rather than in a loaded actor.

use crate::constructions::duration_in_moves;
use crate::items::LoadError;
use cdda_json::{Object, Value};

/// One action an item can be used for.
#[derive(Debug, Clone, PartialEq)]
pub enum UseAction {
    /// A transformation: the item becomes another item, or a container holding one.
    Transform(Transform),
    /// Taking the thing: effects, fields, stat changes, vitamins and the item left behind.
    ConsumeDrug(ConsumeDrug),
    /// Anything the port has not ported yet, kept by the name the data gives it — an actor's `"type"`, or a legacy string id.
    Other {
        /// The actor type, or the legacy id.
        kind: String,
    },
}

/// The `transform` actor's data, and `delayed_transform`'s, which derives from it.
///
/// Reference: `class iuse_transform` (`src/iuse_actor.h:44-110`) and `iuse_transform::load` (`src/iuse_actor.cpp:216-250`). Every field is about the **result** of the
/// transformation, not the act of it: what the item becomes, where the new charges come from, and what must be true before it may happen.
// Five independent flags, and they are the actor's own shape (`src/iuse_actor.h:44-110`): a data mirror with fewer fields would have to invent a grouping the
// reference does not have, which is exactly how a mirror stops being one.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq)]
pub struct Transform {
    /// The actor's own type: `transform`, or `delayed_transform`, which the reference loads with the same loader and a `not_ready_msg`.
    pub action: String,
    /// The charge cost of the action; `-1` when the actor does not state one, which the reference means as "the item's own use cost".
    pub cost: i32,
    /// The item type it becomes, or empty when the result comes from a group.
    pub target: String,
    /// The item group the result is drawn from, or empty. The reference refuses an actor that states both.
    pub target_group: String,
    /// An item type to put the result **inside**, or empty.
    pub container: String,
    /// Which variant of the target to choose; `<any>` when the actor does not say.
    pub variant_type: String,
    /// Whether the container the result is put in is sealed.
    pub sealed: bool,
    /// How many charges the result gets, or `-1` to keep the item's own.
    pub target_charges: i32,
    /// The ammunition type to set on the result, or empty.
    pub target_ammo: String,
    /// A timer to start on the result, in seconds; zero for none.
    pub target_timer_seconds: i64,
    /// Whether the result is active.
    pub active: bool,
    /// Whether the item must be worn first.
    pub need_worn: bool,
    /// Whether the item must be wielded first.
    pub need_wielding: bool,
    /// Whether the item must be empty first.
    pub need_empty: bool,
    /// Moves the transformation costs the one who does it.
    pub moves: i32,
    /// Fire charges the item must have, from a fire nearby.
    pub need_fire: i32,
    /// Charges the item must hold before it may be transformed.
    pub need_charges: i32,
    /// The charges to choose from at random, which the reference refuses to combine with a fixed count.
    pub rand_target_charges: Vec<i32>,
}

impl Transform {
    /// Reads the actor's data out of its JSON object.
    ///
    /// # Errors
    ///
    /// If a field is present with the wrong shape.
    pub fn read(action: &str, object: &Object) -> Result<Self, LoadError> {
        let text = |key: &str| match object.get(key) {
            Some(Value::Str(value)) => value.clone(),
            _ => String::new(),
        };
        let flag = |key: &str, fallback: bool| match object.get(key) {
            Some(Value::Bool(value)) => *value,
            _ => fallback,
        };
        let whole = |key: &str, fallback: i32| match object.get(key) {
            Some(Value::Int(value)) => i32::try_from(*value).unwrap_or(fallback),
            _ => fallback,
        };
        let rand_target_charges = match object.get("rand_target_charges") {
            Some(Value::Array(entries)) => entries
                .iter()
                .filter_map(|entry| match entry {
                    Value::Int(value) => i32::try_from(*value).ok(),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        Ok(Self {
            action: action.to_owned(),
            cost: whole("cost", -1),
            target: text("target"),
            target_group: text("target_group"),
            container: text("container"),
            variant_type: match object.get("variant_type") {
                Some(Value::Str(value)) => value.clone(),
                _ => "<any>".to_owned(),
            },
            // **Sealed is the odd default**: an actor that says nothing about sealing means a sealed container, not an open one (`src/iuse_actor.h:61-62`).
            sealed: flag("sealed", true),
            target_charges: whole("target_charges", -1),
            target_ammo: text("target_ammo"),
            target_timer_seconds: match object.get("target_timer") {
                Some(value @ (Value::Str(_) | Value::Int(_))) => {
                    duration_in_moves(value) / 100
                }
                _ => 0,
            },
            active: flag("active", false),
            need_worn: flag("need_worn", false),
            need_wielding: flag("need_wielding", false),
            need_empty: flag("need_empty", false),
            moves: whole("moves", 0),
            need_fire: whole("need_fire", 0),
            need_charges: whole("need_charges", 0),
            rand_target_charges,
        })
    }
}

/// Reads a type's `use_action`, which the data may write as one entry or a list of them.
///
/// Reference: `item_factory.cpp`'s `use_function_reader_map` and `use_function_reader_single`: the member may be a single action or an array of them, and every
/// entry is either a legacy string id or an object with a `"type"`.
///
/// # Errors
///
/// If an entry is neither a string nor an object, or a ported actor's fields are malformed.
pub fn read_use_actions(object: &Object) -> Result<Vec<UseAction>, LoadError> {
    let Some(value) = object.get("use_action") else {
        return Ok(Vec::new());
    };
    let entries: Vec<&Value> = match value {
        Value::Array(entries) => entries.iter().collect(),
        other => vec![other],
    };
    entries
        .into_iter()
        .map(|entry| -> Result<Option<UseAction>, LoadError> {
            match entry {
                Value::Str(id) => Ok(Some(UseAction::Other { kind: id.clone() })),
                Value::Object(actor) => {
                    let Some(Value::Str(kind)) = actor.get("type") else {
                        return Ok(None);
                    };
                    read_actor(kind, actor)
                }
                _ => Ok(None),
            }
        })
        .filter_map(Result::transpose)
        .collect()
}

/// One effect a drug gives, for how long, and where.
///
/// Reference: `struct effect_data` (`src/iuse_actor.h:234-242`) and `load_effect_data` (`src/iuse_actor.cpp:837-847`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrugEffect {
    /// The effect id.
    pub id: String,
    /// How long it lasts, in seconds.
    pub duration_seconds: i64,
    /// The body part it is applied to; `bp_null` when the actor does not say.
    pub bp: String,
    /// Whether it never wears off.
    pub permanent: bool,
}

/// Damage a drug does over time, which is how a poison is written.
///
/// Reference: `class damage_over_time_data` (`src/damage.h:193-204`). `amount` is a **double** even though almost every entry in the content is whole, so it is kept
/// as one rather than rounded on the way in.
#[derive(Debug, Clone, PartialEq)]
pub struct DrugDamage {
    /// The damage type, e.g. `poison`.
    pub damage_type: String,
    /// How long it keeps doing damage, in seconds.
    pub duration_seconds: i64,
    /// How much damage in total.
    pub amount: f64,
    /// The body parts it applies to.
    pub bps: Vec<String>,
}

/// The `consume_drug` actor's data.
///
/// Reference: `class consume_drug_iuse` (`src/iuse_actor.h:247-283`) and its `load` (`src/iuse_actor.cpp:849-871`). It is a list of consequences rather than one
/// action, which is why almost every field is a collection: effects with durations and body parts, fields to produce in the world, stat adjustments, and vitamins
/// as a **range** to roll between.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsumeDrug {
    /// The actor's own type: `consume_drug`.
    pub action: String,
    /// Moves the act costs, which defaults to a whole turn rather than to zero.
    pub moves: i32,
    /// What the player is told.
    pub activation_message: String,
    /// The item type left behind, or `null` for none — the reference's default `NULL_ID`, whose **name is the string "null"**.
    pub used_up_item: String,
    /// Fields to put into the world, by field id, with the intensity.
    pub fields_produced: std::collections::BTreeMap<String, i32>,
    /// Stat adjustments, by stat name.
    pub stat_adjustments: std::collections::BTreeMap<String, i32>,
    /// Charges of another item the act consumes, e.g. fire.
    pub charges_needed: std::collections::BTreeMap<String, i32>,
    /// Tools the act needs, which are not consumed. A **negative** count means the tool is optional in the reference, so the sign is data.
    pub tools_needed: std::collections::BTreeMap<String, i32>,
    /// Vitamins gained, each with the low and high end of the range to roll.
    ///
    /// **A map, not a list.** The reference stores these in a `std::map<vitamin_id, std::pair<int, int>>` (`src/iuse_actor.h:263-264`), so they come back **sorted by
    /// id** however the data wrote them — and a duplicate id keeps the **first** one, because the loader `emplace`s. Reading the array into a list preserves the
    /// file's order instead and disagrees on 30 of the 108 drugs.
    pub vitamins: std::collections::BTreeMap<String, (i32, i32)>,
    /// The effects.
    pub effects: Vec<DrugEffect>,
    /// The damage over time.
    pub damage_over_time: Vec<DrugDamage>,
}

impl ConsumeDrug {
    /// Reads the actor's data out of its JSON object.
    ///
    /// # Errors
    ///
    /// If a member is present with the wrong shape.
    pub fn read(action: &str, object: &Object) -> Result<Self, LoadError> {
        let translation = |key: &str| match object.get(key) {
            Some(Value::Str(value)) => value.clone(),
            Some(Value::Object(value)) => match value.get("str") {
                Some(Value::Str(value)) => value.clone(),
                _ => String::new(),
            },
            _ => String::new(),
        };
        let int_map = |key: &str| match object.get(key) {
            Some(Value::Object(map)) => map
                .members()
                .iter()
                .filter_map(|(id, value)| match value {
                    Value::Int(value) => Some((id.clone(), i32::try_from(*value).ok()?)),
                    _ => None,
                })
                .collect(),
            _ => std::collections::BTreeMap::new(),
        };
        let vitamins = read_vitamins(object);
        let effects = read_effects(object);
        let damage_over_time = read_damage_over_time(object);
        Ok(Self {
            action: action.to_owned(),
            moves: match object.get("moves") {
                Some(Value::Int(value)) => i32::try_from(*value).unwrap_or(100),
                // `int moves = 100` is the actor's member default, and it is a whole turn rather than nothing.
                _ => 100,
            },
            activation_message: translation("activation_message"),
            used_up_item: match object.get("used_up_item") {
                Some(Value::Str(item)) => item.clone(),
                _ => "null".to_owned(),
            },
            fields_produced: int_map("fields_produced"),
            stat_adjustments: int_map("stat_adjustments"),
            charges_needed: int_map("charges_needed"),
            tools_needed: int_map("tools_needed"),
            vitamins,
            effects,
            damage_over_time,
        })
    }
}

/// The vitamins, as the reference keeps them: a map, sorted by id, first entry winning on a duplicate.
/// The vitamins, as the reference keeps them: a **map**, sorted by id, the first entry winning on a duplicate id.
///
/// Reference: `src/iuse_actor.cpp:866-870`, which `emplace`s into a `std::map<vitamin_id, std::pair<int,int>>` and makes `hi` equal `lo` when the third element of the
/// array is missing. Reading the array into a list keeps the file's order instead, which disagrees on 30 of the 108 drugs.
fn read_vitamins(object: &Object) -> std::collections::BTreeMap<String, (i32, i32)> {
    let Some(Value::Array(entries)) = object.get("vitamins") else {
        return std::collections::BTreeMap::new();
    };
    let mut out = std::collections::BTreeMap::new();
    for entry in entries {
        let Value::Array(parts) = entry else {
            continue;
        };
        let Some(Value::Str(id)) = parts.first() else {
            continue;
        };
        let Some(lo) = parts.get(1).and_then(int_of) else {
            continue;
        };
        let hi = parts.get(2).and_then(int_of).unwrap_or(lo);
        // `emplace` keeps the first entry for an id that appears twice.
        out.entry(id.clone()).or_insert((lo, hi));
    }
    out
}

/// The effects, in the order the data lists them.
fn read_effects(object: &Object) -> Vec<DrugEffect> {
    let Some(Value::Array(entries)) = object.get("effects") else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let Value::Object(effect) = entry else {
                return None;
            };
            let Some(Value::Str(id)) = effect.get("id") else {
                return None;
            };
            Some(DrugEffect {
                id: id.clone(),
                duration_seconds: duration_seconds(effect.get("duration")),
                bp: match effect.get("bp") {
                    Some(Value::Str(bp)) => bp.clone(),
                    _ => "bp_null".to_owned(),
                },
                permanent: matches!(effect.get("permanent"), Some(Value::Bool(true))),
            })
        })
        .collect()
}

/// The damage over time, whose `amount` is a double.
fn read_damage_over_time(object: &Object) -> Vec<DrugDamage> {
    let Some(Value::Array(entries)) = object.get("damage_over_time") else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let Value::Object(dot) = entry else {
                return None;
            };
            let Some(Value::Str(kind)) = dot.get("type") else {
                return None;
            };
            Some(DrugDamage {
                damage_type: kind.clone(),
                duration_seconds: duration_seconds(dot.get("duration")),
                amount: match dot.get("amount") {
                    Some(Value::Float(amount)) => *amount,
                    Some(Value::Int(amount)) => {
                        #[allow(clippy::cast_precision_loss)]
                        let amount = *amount as f64;
                        amount
                    }
                    _ => 0.0,
                },
                bps: match dot.get("bps") {
                    Some(Value::Array(parts)) => parts
                        .iter()
                        .filter_map(|part| match part {
                            Value::Str(part) => Some(part.clone()),
                            _ => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                },
            })
        })
        .collect()
}

/// A whole number out of a JSON value.
fn int_of(value: &Value) -> Option<i32> {
    match value {
        Value::Int(value) => i32::try_from(*value).ok(),
        _ => None,
    }
}

/// A duration written either as a string in the game's grammar or as a whole number of turns, in seconds.
///
/// Reference: `load_effect_data` (`src/iuse_actor.cpp:839-844`), which reads a string through the duration reader and an int through `from_turns`.
fn duration_seconds(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Str(text)) => crate::constructions::parse_duration(text),
        Some(Value::Int(turns)) => *turns,
        _ => 0,
    }
}

/// Reads one actor out of its object, if the port has it.
///
/// # Errors
///
/// If a ported actor's fields are malformed.
pub fn read_actor(kind: &str, object: &Object) -> Result<Option<UseAction>, LoadError> {
    match kind {
        "transform" | "delayed_transform" => Ok(Some(UseAction::Transform(Transform::read(kind, object)?))),
        "consume_drug" => Ok(Some(UseAction::ConsumeDrug(ConsumeDrug::read(kind, object)?))),
        _ => Ok(Some(UseAction::Other {
            kind: kind.to_owned(),
        })),
    }
}
