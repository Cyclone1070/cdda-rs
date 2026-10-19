//! Content registry.
//!
//! The reference keeps one registry per JSON `type` — ninety of them in the core
//! content — each holding the concrete prototypes of that type after inheritance
//! has been resolved. This crate starts from the part that can be verified before
//! any merge rule exists: which ids each registry ends up holding.
//!
//! The order of loading is the reference's: files in filesystem order, then the
//! objects inside each file in document order (`filesystem.cpp:332,372-374`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub mod factory;
pub mod fields;
pub mod generated;
pub mod items;
pub mod mapdata;
pub mod materials;
pub mod monsters;
mod rules;
mod scan;
pub mod traps;

pub use rules::{id_member, null_prototype, registry_for};
pub use scan::{Registry, ScanError, scan};

/// Every `*.json` file under `dir`, in the reference's load order.
///
/// # Errors
///
/// Returns an error only if the starting directory itself cannot be read; a
/// subdirectory that cannot be opened is skipped, as the reference skips it.
///
/// The reference walks a directory level by level, sorting the entries of each
/// level (`filesystem.cpp:332,372-374`), so directories and files interleave by
/// name rather than files-before-directories.
pub fn json_files_in_load_order(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut ordered = Vec::new();
    let mut level = vec![dir.to_path_buf()];
    while !level.is_empty() {
        let mut next = Vec::new();
        let mut files = Vec::new();
        let mut entries = Vec::new();
        for current in &level {
            let Ok(read) = std::fs::read_dir(current) else {
                continue;
            };
            for entry in read.flatten() {
                entries.push(entry.path());
            }
        }
        entries.sort();
        for path in entries {
            if path.is_dir() {
                next.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                files.push(path);
            }
        }
        ordered.extend(files);
        level = next;
    }
    Ok(ordered)
}

/// The concrete ids a registry holds, keyed by the registry's type name.
pub type IdsByType = BTreeMap<String, Vec<String>>;
