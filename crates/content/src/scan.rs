//! Reading the ids a registry holds out of the content files.

use crate::{IdsByType, id_member, json_files_in_load_order, null_prototype, registry_for};
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

/// Scans directories in order, collecting each registry's concrete ids.
///
/// # Errors
///
/// Returns an error when a file cannot be read or does not parse. A content file
/// that does not parse is not something to carry on past.
///
/// The core content is two directories and the order matters: the engine's own
/// data (`data/core`, which holds the sentinel prototypes) is loaded before the
/// content pack (`data/json`), because a factory inserts a prototype the first
/// time it sees the id and keeps the first one
/// (`game.cpp:599-600`, `init.cpp:487`).
///
/// A prototype is *concrete* when it names itself with the id member; an
/// `abstract` object is a template and belongs to no registry's list. Duplicates
/// keep their first position, as the reference's list does.
pub fn scan(dirs: &[&Path]) -> Result<Vec<Registry>, ScanError> {
    let mut registries: IdsByType = IdsByType::new();
    for dir in dirs {
        scan_one(dir, &mut registries)?;
    }
    Ok(registries
        .into_iter()
        .map(|(type_name, ids)| Registry { type_name, ids })
        .collect())
}

/// Scans one directory into a registry set.
fn scan_one(dir: &Path, registries: &mut IdsByType) -> Result<(), ScanError> {
    for path in json_files_in_load_order(dir).map_err(ScanError::Io)? {
        let value = crate::read_json(&path).ok_or_else(|| ScanError::Parse {
            path: path.display().to_string(),
            message: "file did not parse".to_owned(),
        })?;
        collect(&value, registries);
    }
    Ok(())
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
    let Some(Value::Str(json_type)) = object.get("type") else {
        return;
    };
    // An abstract is a template; it never enters the concrete list.
    if object.get("abstract").is_some() {
        return;
    }
    let registry = registry_for(json_type).to_owned();
    let Some(id) = id_of(object, id_member(&registry)) else {
        return;
    };
    let entry = out.entry(registry).or_default();
    // A factory inserts its null prototype before reading its first object, so
    // it is the first entry in the registry's list.
    if entry.is_empty()
        && let Some(null_id) = null_prototype(json_type)
    {
        entry.push(null_id.to_owned());
    }
    if !entry.contains(&id) {
        entry.push(id);
    }
}

/// The id of one object, read from the member its registry uses.
fn id_of(object: &Object, member: &str) -> Option<String> {
    match object.get(member) {
        Some(Value::Str(id)) => Some(id.clone()),
        _ => None,
    }
}
