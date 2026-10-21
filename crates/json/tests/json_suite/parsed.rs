//! Parsing the reference tree, once per file for the whole suite.
//!
//! Two gates walk the same 107 MB: one parses `data/json`, the other parses all of `data/`, which contains it. Left
//! alone the second gate parses the first gate's 3,013 files a second time. This remembers the answer for each path,
//! so a file is parsed once however many gates look at it, and the gates still see the whole tree they asked about.

#![allow(clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::OnceLock;

/// Every `.json` file under a directory, sorted, so two gates walk the same order.
#[must_use]
pub fn json_files(dir: &Path) -> Vec<PathBuf> {
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

fn answers() -> &'static Mutex<BTreeMap<PathBuf, Option<String>>> {
    static ONCE: OnceLock<Mutex<BTreeMap<PathBuf, Option<String>>>> = OnceLock::new();
    ONCE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Parses one file, or says why it could not be parsed. The answer is remembered.
pub fn parse_file(path: &Path) -> Option<String> {
    if let Some(known) = answers()
        .lock()
        .expect("the table is not poisoned")
        .get(path)
    {
        return known.clone();
    }
    let failure = match std::fs::read(path) {
        Ok(bytes) => cdda_json::parse(&bytes)
            .err()
            .map(|error| error.to_string()),
        Err(_) => Some("unreadable".to_owned()),
    };
    answers()
        .lock()
        .expect("the table is not poisoned")
        .insert(path.to_path_buf(), failure.clone());
    failure
}

/// Parses every JSON file under a directory and reports how many there were and which failed.
#[must_use]
pub fn parse_all(dir: &Path) -> (usize, Vec<String>) {
    let files = json_files(dir);
    let count = files.len();
    let mut failures = Vec::new();
    for path in &files {
        if let Some(why) = parse_file(path) {
            failures.push(format!("{}: {why}", path.display()));
        }
    }
    (count, failures)
}
