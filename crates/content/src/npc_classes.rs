//! NPC classes: what an NPC is before they are given a personality.
//!
//! Reference: `npc_class.cpp`. A class holds the skill levels, traits, bionics, spells, worn gear and carried items an NPC of
//! that class starts with, and every NPC is built from exactly one. The port loads the identity of each class - its id and the
//! name the player sees - because those are what mapgen and dialogue refer to; the skill and gear tables follow as the pieces
//! that consume them are ported.

use std::collections::BTreeMap;

use cdda_json::Value;

use crate::monsters::LoadError;

/// One NPC class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcClass {
    /// Its id, as the data writes it.
    pub id: String,
    /// The name the player sees, empty when the data names none.
    pub name: String,
    /// The description shown when the class is chosen, empty when the data gives none.
    pub job_description: String,
    /// Whether the class is offered in the character creator's common list.
    pub common: bool,
}

/// Loads every NPC class under `dirs`.
///
/// # Errors
///
/// Reports a file that cannot be read or a class without an id.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<Vec<NpcClass>, LoadError> {
    let mut by_id: BTreeMap<String, NpcClass> = BTreeMap::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            let Value::Array(objects) = value.as_ref() else {
                continue;
            };
            for object in objects {
                let Some(class) = read_class(object) else {
                    continue;
                };
                // Later copies replace earlier ones, as the reference's generic factory does for a repeated id.
                by_id.insert(class.id.clone(), class);
            }
        }
    }
    Ok(by_id.into_values().collect())
}

fn read_class(object: &Value) -> Option<NpcClass> {
    let Value::Object(fields) = object else {
        return None;
    };
    match fields.get("type") {
        Some(Value::Str(kind)) if kind == "npc_class" => {}
        _ => return None,
    }
    let Some(Value::Str(id)) = fields.get("id") else {
        return None;
    };
    Some(NpcClass {
        id: id.clone(),
        name: read_name(fields.get("name")),
        job_description: match fields.get("job_description") {
            Some(Value::Str(text)) => text.clone(),
            _ => String::new(),
        },
        common: matches!(fields.get("common"), Some(Value::Bool(true))),
    })
}

/// The name of a class, which the data writes either as a bare string or as a translated string object.
fn read_name(value: Option<&Value>) -> String {
    match value {
        Some(Value::Str(text)) => text.clone(),
        Some(Value::Object(fields)) => match fields.get("str") {
            Some(Value::Str(text)) => text.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}
