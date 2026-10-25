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
                    if kind == "transform" || kind == "delayed_transform" {
                        Ok(Some(UseAction::Transform(Transform::read(kind, actor)?)))
                    } else {
                        Ok(Some(UseAction::Other { kind: kind.clone() }))
                    }
                }
                _ => Ok(None),
            }
        })
        .filter_map(Result::transpose)
        .collect()
}
