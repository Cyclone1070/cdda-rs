//! Reading the ids a registry holds out of the content files.

use crate::{IdsByType, json_files_in_load_order};
use cdda_json::{Object, Value};
use std::fmt;
use std::path::Path;

/// A registry: one JSON `type`, with the concrete ids it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    /// The `type` string, e.g. `ITEM` or `MONSTER`.
    pub type_name: String,
    /// Concrete ids, in the order they were first loaded.
    pub ids: Vec<String>,
}

/// Why a scan failed.
#[derive(Debug)]
pub enum ScanError {
    /// The directory could not be read.
    Io(std::io::Error),
    /// A file did not parse.
    Parse {
        /// The file that failed.
        path: String,
        /// What the parser said.
        message: String,
    },
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::Io(error) => write!(f, "{error}"),
            ScanError::Parse { path, message } => write!(f, "{path}: {message}"),
        }
    }
}

impl std::error::Error for ScanError {}

/// Scans every file under `dir`, collecting each registry's concrete ids.
///
/// A prototype is *concrete* when it names itself with the id member; an
/// `abstract` object is a template and belongs to no registry's list. Duplicates
/// keep their first position, as the reference's list does.
pub fn scan(dir: &Path) -> Result<Vec<Registry>, ScanError> {
    let mut registries: IdsByType = IdsByType::new();
    for path in json_files_in_load_order(dir).map_err(ScanError::Io)? {
        let bytes = std::fs::read(&path).map_err(ScanError::Io)?;
        let value = cdda_json::parse(&bytes).map_err(|error| ScanError::Parse {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        collect(&value, &mut registries);
    }
    Ok(registries
        .into_iter()
        .map(|(type_name, ids)| Registry { type_name, ids })
        .collect())
}

/// Walks a value, recording the id of every object that has one.
fn collect(value: &Value, out: &mut IdsByType) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, out);
            }
        }
        Value::Object(object) => record(object, out),
        _ => {}
    }
}

/// Records one object's contribution to its registry.
fn record(object: &Object, out: &mut IdsByType) {
    let Some(Value::Str(type_name)) = object.get("type") else {
        return;
    };
    // An abstract is a template; it never enters the concrete list.
    if object.get("abstract").is_some() {
        return;
    }
    let Some(id) = id_of(object) else {
        return;
    };
    let entry = out.entry(type_name.clone()).or_default();
    if !entry.contains(&id) {
        entry.push(id);
    }
}

/// The id member, under the names the reference's factories use.
fn id_of(object: &Object) -> Option<String> {
    for member in ["id", "ident"] {
        match object.get(member) {
            Some(Value::Str(id)) => return Some(id.clone()),
            Some(Value::Object(_)) | Some(Value::Array(_)) => return None,
            _ => continue,
        }
    }
    None
}
