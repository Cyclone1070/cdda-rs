//! Every weak-point set a monster can have.
//!
//! Reference: `struct weakpoints` (`src/weakpoint.h:180-205`) - a list of `weakpoint` entries and a **default** one that answers the hit when no entry's share is rolled. The figures
//! [`crate::weakpoints`]' rules read are loaded here from the oracle's own dump, which is what keeps the data's numbers out of the code.
//!
//! The port's own rule module lives in `cdda_sim::weakpoints`; this half is the data, and it deliberately knows nothing about what a weak point does.

use crate::generated::LoadError;
use cdda_json::Value;
use std::collections::BTreeMap;

/// One weak point, as the data states it.
#[derive(Debug, Clone, PartialEq)]
pub struct Weakpoint {
    /// Its id, which is what the data calls it.
    pub id: String,
    /// The percent chance of hitting it before skill is considered.
    pub coverage: f64,
    /// Whether hitting it helps the attacker (`is_good`).
    pub helps_attacker: bool,
    /// Whether it is the head, which the reference marks for its own messages.
    pub is_head: bool,
    /// `armor_mult`, by damage type.
    pub armour_multiplier: BTreeMap<String, f64>,
    /// `armor_penalty`, by damage type.
    pub armour_penalty: BTreeMap<String, f64>,
    /// `damage_mult`, by damage type.
    pub damage_multiplier: BTreeMap<String, f64>,
    /// `crit_mult`, by damage type.
    pub crit_multiplier: BTreeMap<String, f64>,
    /// The coverage multiplier per attack type, in the reference's own order: `NONE`, melee bash, melee cut, melee stab, projectile.
    pub coverage_multiplier: Vec<f64>,
    /// The difficulty per attack type, in the same order.
    pub difficulty: Vec<f64>,
    /// What a blow into it can trigger (`weakpoint_effect`).
    pub effects: Vec<TriggeredEffect>,
}

/// Something a struck weak point can do to the body it was found on.
///
/// Reference: `weakpoint_effect` (`weakpoint.h:68-92`). The port keeps the rule's inputs and names the two it cannot honour - `effect_on_conditions` and the message - in `docs/parity.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggeredEffect {
    /// The effect's id.
    pub effect: String,
    /// The percent chance it fires.
    pub chance: f64,
    /// Whether it never wears off.
    pub permanent: bool,
    /// How many turns it lasts, as a range.
    pub duration: (i32, i32),
    /// How strong it is, as a range.
    pub intensity: (i32, i32),
    /// The share of the body's maximum hit points the blow must take, as a percent range.
    pub damage_required: (f64, f64),
}

/// A set of weak points and the default one.
#[derive(Debug, Clone, PartialEq)]
pub struct WeakpointSet {
    /// Its id, which a monster type names.
    pub id: String,
    /// The entries, in the data's order - **which is the order the walk tries them in**.
    pub weakpoints: Vec<Weakpoint>,
    /// The weak point that answers a roll nothing else took.
    pub default: Weakpoint,
}

/// Reads the effects a weak point can trigger.
///
/// # Errors
/// None: an entry that states nothing readable is skipped rather than failing the whole set, because an effect the data leaves half-written must not stop a game from loading.
fn read_effects(value: Option<&Value>) -> Vec<TriggeredEffect> {
    let Some(Value::Array(entries)) = value else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries {
        let Value::Object(object) = entry else {
            continue;
        };
        let Some(Value::Str(effect)) = object.get("effect") else {
            continue;
        };
        let number = |key: &str, fallback: f64| -> f64 {
            match object.get(key) {
                Some(Value::Float(found)) => *found,
                Some(Value::Int(found)) => *found as f64,
                _ => fallback,
            }
        };
        let pair = |key: &str, fallback: (f64, f64)| -> (f64, f64) {
            match object.get(key) {
                Some(Value::Array(ends)) if ends.len() == 2 => (
                    ends.first().and_then(as_number).unwrap_or(fallback.0),
                    ends.get(1).and_then(as_number).unwrap_or(fallback.1),
                ),
                _ => fallback,
            }
        };
        #[allow(clippy::cast_possible_truncation)]
        let whole = |value: f64| value as i32;
        out.push(TriggeredEffect {
            effect: effect.clone(),
            chance: number("chance", 100.0),
            permanent: matches!(object.get("permanent"), Some(Value::Bool(true))),
            duration: {
                let (first, second) = pair("duration", (0.0, 0.0));
                (whole(first), whole(second))
            },
            intensity: {
                let (first, second) = pair("intensity", (1.0, 1.0));
                (whole(first), whole(second))
            },
            damage_required: pair("damage_required", (0.0, 100.0)),
        });
    }
    out
}

/// A JSON number, whole or fractional.
fn as_number(value: &Value) -> Option<f64> {
    match value {
        Value::Float(found) => Some(*found),
        Value::Int(found) => Some(*found as f64),
        _ => None,
    }
}

/// Reads one weak point, from the dump's own shape.
///
/// # Errors
/// Returns an error when the entry states no id.
pub fn read_weakpoint(value: &Value) -> Result<Weakpoint, LoadError> {
    let Value::Object(object) = value else {
        return Err(LoadError {
            message: "a weak point that is not an object".to_owned(),
        });
    };
    let Some(Value::Str(id)) = object.get("id") else {
        return Err(LoadError {
            message: "a weak point with no id".to_owned(),
        });
    };
    let flag = |key: &str, fallback: bool| -> bool {
        match object.get(key) {
            Some(Value::Bool(found)) => *found,
            _ => fallback,
        }
    };
    let number = |key: &str| -> f64 {
        match object.get(key) {
            Some(Value::Float(found)) => *found,
            Some(Value::Int(found)) => *found as f64,
            _ => 0.0,
        }
    };
    let table = |key: &str| -> BTreeMap<String, f64> {
        let mut out = BTreeMap::new();
        if let Some(Value::Object(entries)) = object.get(key) {
            for (damage_type, amount) in entries.members() {
                let amount = match amount {
                    Value::Float(found) => Some(*found),
                    Value::Int(found) => Some(*found as f64),
                    _ => None,
                };
                if let Some(amount) = amount {
                    out.insert(damage_type.clone(), amount);
                }
            }
        }
        out
    };
    let list = |key: &str| -> Vec<f64> {
        match object.get(key) {
            Some(Value::Array(entries)) => entries
                .iter()
                .filter_map(|entry| match entry {
                    Value::Float(found) => Some(*found),
                    Value::Int(found) => Some(*found as f64),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    };
    Ok(Weakpoint {
        id: id.clone(),
        coverage: number("coverage"),
        helps_attacker: flag("is_good", true),
        is_head: flag("is_head", false),
        armour_multiplier: table("armor_mult"),
        armour_penalty: table("armor_penalty"),
        damage_multiplier: table("damage_mult"),
        crit_multiplier: table("crit_mult"),
        // The reference's default weak point has **no coverage and no entries**, and the dump states that as an empty list rather than a guessed default.
        coverage_multiplier: list("coverage_mult"),
        difficulty: list("difficulty"),
        effects: read_effects(object.get("effects")),
    })
}

/// Expands a map's `all` entry into every damage type the port knows.
///
/// Reference: `finalize_damage_map` (`damage.cpp:789-829`) - `all` is every type's default, `physical` and `non_physical` fill their own halves, and **a type named outright keeps its own figure**.
/// The port's three damage types are all physical, so only `all` can reach them; the two halves are named in `docs/parity.md` rather than guessed at.
///
/// **This has to run, because the dump writes the maps before it does**: the harness stops at `check_mod_data`, which loads weak points but does not finalize them, so a set whose data says
/// `{ "damage_mult": { "all": 0.5 } }` arrives with the keyword still in it - and no rule ever asks about a damage type called `all`.
#[must_use]
pub fn expand_all(map: &BTreeMap<String, f64>, default: f64) -> BTreeMap<String, f64> {
    let all = map.get("all").copied().unwrap_or(default);
    let mut out = BTreeMap::new();
    for damage_type in ["bash", "cut", "stab"] {
        out.insert(damage_type.to_owned(), map.get(damage_type).copied().unwrap_or(all));
    }
    out
}

/// Reads a list of sets, in the order the dump states them.
///
/// # Errors
/// Returns the first set or entry that could not be read.
pub fn load_objects(sets: &[Value]) -> Result<Vec<WeakpointSet>, LoadError> {
    let mut out = Vec::new();
    for value in sets {
        let Value::Object(object) = value else {
            continue;
        };
        let Some(Value::Str(id)) = object.get("id") else {
            return Err(LoadError {
                message: "a weak-point set with no id".to_owned(),
            });
        };
        let mut weakpoints = Vec::new();
        if let Some(Value::Array(entries)) = object.get("weakpoints") {
            for entry in entries {
                weakpoints.push(read_weakpoint(entry)?);
            }
        }
        let default = match object.get("default") {
            Some(entry) => read_weakpoint(entry)?,
            None => {
                return Err(LoadError {
                    message: format!("the weak-point set {id} has no default"),
                });
            }
        };
        out.push(WeakpointSet {
            id: id.clone(),
            weakpoints,
            default,
        });
    }
    Ok(out)
}
