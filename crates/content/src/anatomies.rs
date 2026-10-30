//! Which parts a body has.
//!
//! Reference: `anatomy` (`src/anatomy.h:20-64`), a list of `bodypart_id` that `anatomy::select_body_part` walks.
//! In the reference the list is reachable only through a file-local factory, so the port reads the same JSON the
//! reference reads - `"type": "anatomy"`, `id` and `parts` - and holds it to the dumped body parts beside it.

/// One body's membership.
#[derive(Debug, Clone, PartialEq)]
pub struct Anatomy {
    /// Its id, which is what a creature's body type names.
    pub id: String,
    /// The parts it has, in the order the data states them.
    pub parts: Vec<String>,
}

/// Reads one prototype.
///
/// # Errors
/// Returns a message when the prototype states no id or no parts.
pub fn read_prototype(object: &cdda_json::Object) -> Result<Anatomy, crate::generated::LoadError> {
    let Some(id) = object.get("id").and_then(|value| match value {
        cdda_json::Value::Str(text) => Some(text.clone()),
        _ => None,
    }) else {
        return Err(crate::generated::LoadError {
            message: "an anatomy with no id".to_owned(),
        });
    };
    let Some(cdda_json::Value::Array(parts)) = object.get("parts") else {
        return Err(crate::generated::LoadError {
            message: format!("anatomy {id} states no parts"),
        });
    };
    let parts = parts
        .iter()
        .filter_map(|part| match part {
            cdda_json::Value::Str(text) => Some(text.clone()),
            _ => None,
        })
        .collect();
    Ok(Anatomy { id, parts })
}

/// Reads a list of prototypes, keeping only the ones that claim to be anatomies.
///
/// # Errors
/// Returns the first anatomy that could not be read.
pub fn load_objects(
    prototypes: &[cdda_json::Value],
) -> Result<Vec<Anatomy>, crate::generated::LoadError> {
    let mut out = Vec::new();
    for value in prototypes {
        let cdda_json::Value::Object(object) = value else {
            continue;
        };
        let is_anatomy = matches!(
            object.get("type"),
            Some(cdda_json::Value::Str(text)) if text == "anatomy"
        );
        if is_anatomy {
            out.push(read_prototype(object)?);
        }
    }
    Ok(out)
}
