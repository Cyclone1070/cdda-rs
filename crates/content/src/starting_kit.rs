//! Professions and scenarios: the choices a new character starts from.
//!
//! Reference: `profession::get_all` and `scenario::get_all` (`src/profession.h:108`,
//! `src/scenario.h:91`), reached through `--dump-starting-kit`. Ids only for now: a profession
//! also carries a starting kit, skills and a scenario link, and those are the next gate.

use cdda_json::{Object, Value};

/// Every profession and scenario id the reference defines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartingKit {
    /// Profession ids, sorted.
    pub professions: Vec<String>,
    /// Scenario ids, sorted.
    pub scenarios: Vec<String>,
}

fn ids_of(object: &Object, key: &str) -> Result<Vec<String>, String> {
    let Some(Value::Array(entries)) = object.get(key) else {
        return Err(format!("the dump has no {key} array"));
    };
    let mut ids = Vec::with_capacity(entries.len());
    for entry in entries {
        let Value::Str(id) = entry else {
            return Err(format!("a {key} id is not a string"));
        };
        ids.push(id.clone());
    }
    Ok(ids)
}

/// Reads the starting-kit dump.
///
/// The counts in the dump are checked against the arrays rather than trusted: a dump that says
/// 320 and lists 319 is broken, and a loader that believed the count would hide the gap.
///
/// # Errors
///
/// Returns the reason the dump could not be read, or that its counts disagree with its contents.
pub fn from_dump(value: &Value) -> Result<StartingKit, String> {
    let Value::Object(root) = value else {
        return Err("the dump's root is not an object".to_string());
    };
    let professions = ids_of(root, "professions")?;
    let scenarios = ids_of(root, "scenarios")?;
    for (key, listed, array) in [
        ("profession_count", "profession_count", &professions),
        ("scenario_count", "scenario_count", &scenarios),
    ] {
        let Some(Value::Int(count)) = root.get(listed) else {
            return Err(format!("the dump has no {key}"));
        };
        if usize::try_from(*count).ok() != Some(array.len()) {
            return Err(format!("{key} disagrees with the ids listed"));
        }
    }
    Ok(StartingKit {
        professions,
        scenarios,
    })
}
