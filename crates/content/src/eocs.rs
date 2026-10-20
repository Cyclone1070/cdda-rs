//! Effect-on-conditions, both the named ones and the inline ones.
//!
//! Reference: `effect_on_condition.cpp:119-133`. An entry of type
//! `effect_on_condition` registers under its `id`. An *inline* one is an object in
//! a slot - `activated_eocs`, `true_eocs`, `result_eocs`, `eocs` and the rest of
//! that family - which the reference loads and registers under the `id` the object
//! carries.
//!
//! The distinction that matters here is where a slot ends and an ordinary object
//! begins. A slot's value *is* the effect list: each element is either a name or an
//! object that defines one. Nothing deeper than that defines an effect, which is why
//! this loader refuses to collect `id` from arbitrary nesting - a sound effect
//! inside a dialogue effect also has an `id`, and collecting it invented seven
//! effects that do not exist.

use cdda_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

/// The slot names that hold effect-on-conditions.
///
/// Reference: the `load_inline_eoc` call sites. Every field named `*_eocs` belongs
/// here (`activated_eocs`, `processed_eocs`, `deactivated_eocs`, `onhit_eocs`,
/// `ondamage_eocs`, `death_eocs`, `true_eocs`, `false_eocs`, `result_eocs`,
/// `failure_eocs`, `run_eocs` and the rest), plus the singular `eoc`, the plural
/// `eocs` that techniques use, and `effect_on_conditions` on use actions.
const SLOT_EXACT: [&str; 3] = ["eoc", "eocs", "effect_on_conditions"];

/// What a load produced.
#[derive(Debug, Clone, Default)]
pub struct Loaded {
    /// Every effect id, sorted.
    pub ids: BTreeSet<String>,
    /// How many came from an entry of their own.
    pub defined: usize,
    /// How many came from a slot.
    pub inline: usize,
}

fn is_slot(name: &str) -> bool {
    SLOT_EXACT.contains(&name) || name.ends_with("_eocs")
}

/// Loads every effect-on-condition id under the given roots.
///
/// # Errors
///
/// Returns an error when a directory cannot be walked.
pub fn load_paths(roots: &[&Path]) -> Result<Loaded, String> {
    let mut ids = BTreeSet::new();
    let mut defined = BTreeSet::new();
    let mut inline = BTreeSet::new();

    for root in roots {
        let files = crate::json_files_in_load_order(root).map_err(|error| error.to_string())?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            let entries = match value.as_ref() {
                Value::Array(entries) => entries.clone(),
                other => vec![other.clone()],
            };
            for entry in entries {
                if let Value::Object(object) = &entry
                    && object.get("type") == Some(&Value::Str("effect_on_condition".to_owned()))
                {
                    collect_ids(object.get("id"), &mut defined);
                }
                descend(&entry, &mut inline);
            }
        }
    }

    ids.extend(defined.iter().cloned());
    // An inline id that matches a defined one is the same effect, so the two sets
    // are counted apart and registered together.
    inline.retain(|id| !defined.contains(id));
    ids.extend(inline.iter().cloned());
    Ok(Loaded {
        ids,
        defined: defined.len(),
        inline: inline.len(),
    })
}

/// Collects the string or strings under an `id` member.
fn collect_ids(value: Option<&Value>, into: &mut BTreeSet<String>) {
    match value {
        Some(Value::Str(id)) => {
            into.insert(id.clone());
        }
        Some(Value::Array(ids)) => {
            for id in ids {
                if let Value::Str(text) = id {
                    into.insert(text.clone());
                }
            }
        }
        _ => {}
    }
}

/// Registers the effects a slot's value defines.
///
/// A slot's value is a list of effects or a single one; each element is a name (a
/// reference to an effect defined elsewhere) or an object that defines one.
fn collect_slot(value: &Value, into: &mut BTreeSet<String>) {
    match value {
        // A bare name is a reference to an effect defined elsewhere, not a
        // definition, so it adds nothing.
        Value::Str(_) | Value::Int(_) | Value::Float(_) | Value::Bool(_) | Value::Null => {}
        Value::Array(items) => {
            for item in items {
                collect_slot(item, into);
            }
        }
        Value::Object(object) => {
            collect_ids(object.get("id"), into);
            // An inline effect can carry further slots of its own - `true_eocs`
            // and `false_eocs` inside a conditional effect are the common case -
            // so its interior is walked, but only through slots.
            descend(&Value::Object(object.clone()), into);
        }
    }
}

/// Walks an object looking only for slot members.
fn descend(node: &Value, into: &mut BTreeSet<String>) {
    match node {
        Value::Object(object) => {
            for (key, value) in object.members() {
                if is_slot(key) {
                    collect_slot(value, into);
                } else {
                    descend(value, into);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                descend(item, into);
            }
        }
        _ => {}
    }
}
