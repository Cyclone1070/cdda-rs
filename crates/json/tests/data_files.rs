//! Every JSON file shipped with the reference parses.
//!
//! The fixtures are read from the pinned reference checkout, never copied, so
//! this test is exactly as strong as the checkout is: if a rule is encoded
//! wrongly, some real file will reject.

use std::path::{Path, PathBuf};

/// The reference's data directory, or `CDDA_DATA_DIR` when set.
fn data_dir() -> PathBuf {
    let root = std::env::var("CDDA_DATA_DIR").map_or_else(
        |_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data"),
        PathBuf::from,
    );
    assert!(
        root.is_dir(),
        "reference data directory not found at {}",
        root.display()
    );
    root
}

/// Every `*.json` file under `dir`, recursively.
fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "json") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Parses every JSON file under `dir` and reports the ones that failed.
fn parse_all(dir: &Path) -> (usize, Vec<String>) {
    let files = json_files(dir);
    let count = files.len();
    let mut failures = Vec::new();
    for path in &files {
        let Ok(bytes) = std::fs::read(path) else {
            failures.push(format!("{}: unreadable", path.display()));
            continue;
        };
        if let Err(error) = cdda_json::parse(&bytes) {
            failures.push(format!("{}: {error}", path.display()));
        }
    }
    (count, failures)
}

#[test]
fn every_file_under_data_json_parses() {
    let (count, failures) = parse_all(&data_dir().join("json"));
    assert_eq!(count, 3013, "unexpected fixture count under data/json");
    assert!(
        failures.is_empty(),
        "{} of {count} files failed to parse, first 5: {:#?}",
        failures.len(),
        failures.get(..5.min(failures.len())).unwrap_or_default()
    );
}

#[test]
fn every_json_file_shipped_with_the_reference_parses() {
    // All of `data/`, which is the 6220 files and 107 MB the comparison gates
    // load.
    let (count, failures) = parse_all(&data_dir());
    assert_eq!(count, 6220, "unexpected fixture count under data");
    assert!(
        failures.is_empty(),
        "{} of {count} files failed to parse, first 5: {:#?}",
        failures.len(),
        failures.get(..5.min(failures.len())).unwrap_or_default()
    );
}
