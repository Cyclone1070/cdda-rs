//! The `json_flag` type: the behaviours a flag *has*, as opposed to the fact that an item carries it.
//!
//! Reference: `json_flag` (`src/flag.h`, loaded in `src/json_flag.cpp`). An item's flags are a set of ids, and almost everything the game asks of a flag is "does
//! the item carry it". A handful of flags carry a number instead, and `taste_mod` is the one that changes a value rather than a decision: a food flagged
//! `BAD_TASTE` is worth **5 less fun** than its own data says, everywhere and in every eater state (`src/item.cpp:8041-8056`).
//!
//! Only the members this crate needs are read. The loader keeps every `json_flag` it finds so that a later ported system can ask about the rest without a second
//! sweep of the content.

use crate::mapdata::LoadError;
use cdda_json::{Object, Value};
use std::collections::BTreeMap;

/// One `json_flag`: its id and the numbers it carries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JsonFlag {
    /// The flag's id, which is what an item's flag set holds.
    pub id: String,
    /// How much fun carrying this flag adds to a food's own taste. Negative for a flag that tastes bad; absent means nothing.
    pub taste_mod: i32,
}

/// Reads every `json_flag` out of the given paths.
///
/// # Errors
///
/// If a member has the wrong type.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<BTreeMap<String, JsonFlag>, LoadError> {
    let mut flags: BTreeMap<String, JsonFlag> = BTreeMap::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            let entries: Vec<&Value> = match &*value {
                Value::Array(entries) => entries.iter().collect(),
                other => vec![other],
            };
            for entry in entries {
                let Value::Object(object) = entry else {
                    continue;
                };
                if member(object, "type") != Some("json_flag") {
                    continue;
                }
                let Some(id) = member(object, "id") else {
                    continue;
                };
                let flag = JsonFlag {
                    id: id.to_owned(),
                    taste_mod: match object.get("taste_mod") {
                        Some(Value::Int(number)) => i32::try_from(*number).map_err(|_| LoadError {
                            message: format!("{id}: taste_mod out of range: {number}"),
                        })?,
                        Some(other) => {
                            return Err(LoadError {
                                message: format!("{id}: taste_mod is a number, not {other:?}"),
                            });
                        }
                        None => 0,
                    },
                };
                // A later definition replaces an earlier one, as the reference's inserter does.
                flags.insert(id.to_owned(), flag);
            }
        }
    }
    Ok(flags)
}

fn member<'a>(object: &'a Object, name: &str) -> Option<&'a str> {
    match object.get(name) {
        Some(Value::Str(value)) => Some(value.as_str()),
        _ => None,
    }
}
