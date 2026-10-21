//! The parse cache: what it stores, when it distrusts what it stored, and how it
//! behaves when the stored copy is damaged.
//!
//! The reference keeps a per-file parse result on disk keyed by the source's
//! modification time (`src/flexbuffer_cache.cpp:46-56`, `:176-187`), so these
//! tests pin the same three rules: a hit avoids the parse, a changed source is a
//! miss, and anything unreadable is a miss rather than an error.

// Tests are allowed to fail loudly: a test that cannot say what went wrong is
// not a test. The library keeps the strict lints.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cdda_json::cache::{CacheStats, DiskCache};
use cdda_json::{Value, parse};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

/// A directory of this test's own, removed and recreated on entry. The crate has
/// no dependencies, so this is the whole of what a temp-dir crate would give us.
fn tempdir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cdda-json-cache-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap_or(());
    dir
}

/// Moves a file's modification time forward, so the mtime rule can be tested
/// without sleeping.
fn touch(path: &Path, seconds: u64) {
    let file = fs::File::options()
        .write(true)
        .open(path)
        .unwrap_or_else(|error| panic!("open {}: {error}", path.display()));
    let when = SystemTime::now() + std::time::Duration::from_secs(seconds);
    file.set_modified(when)
        .unwrap_or_else(|error| panic!("set_modified: {error}"));
}

/// Every file under a directory, recursively.
fn cache_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or(());
    }
    fs::write(path, text).unwrap_or(());
}

/// The value the parser produces for the same text, so a cached read can be
/// compared against a fresh parse rather than against a hand-written literal.
fn fresh(text: &str) -> Value {
    match parse(text.as_bytes()) {
        Ok(value) => value,
        Err(error) => panic!("fixture does not parse: {error}"),
    }
}

#[test]
fn a_second_read_comes_from_the_cache() {
    let dir = tempdir("second-read");
    let source = dir.join("data/thing.json");
    write(&source, r#"{"a": 1, "b": [true, null, "x"]}"#);

    let cache = DiskCache::new(dir.join("cache"), &dir).expect("cache");
    let first = cache.parse_and_cache(&source).expect("first read");
    let second = cache.parse_and_cache(&source).expect("second read");

    assert_eq!(first, fresh(r#"{"a": 1, "b": [true, null, "x"]}"#));
    assert_eq!(first, second);
    assert_eq!(cache.stats(), CacheStats { hits: 1, misses: 1 });
}

#[test]
fn a_changed_source_is_a_miss() {
    let dir = tempdir("changed-source");
    let source = dir.join("data/thing.json");
    write(&source, r#"{"a": 1}"#);

    let cache = DiskCache::new(dir.join("cache"), &dir).expect("cache");
    let first = cache.parse_and_cache(&source).expect("first read");
    assert_eq!(first, fresh(r#"{"a": 1}"#));

    write(&source, r#"{"a": 2}"#);
    touch(&source, 5);
    let second = cache.parse_and_cache(&source).expect("second read");

    assert_eq!(second, fresh(r#"{"a": 2}"#));
    assert_eq!(cache.stats(), CacheStats { hits: 0, misses: 2 });
}

#[test]
fn a_damaged_entry_is_a_miss_and_never_an_error() {
    let dir = tempdir("damaged-entry");
    let source = dir.join("data/thing.json");
    write(&source, r#"{"a": 1}"#);

    let cache = DiskCache::new(dir.join("cache"), &dir).expect("cache");
    let first = cache.parse_and_cache(&source).expect("first read");
    assert_eq!(first, fresh(r#"{"a": 1}"#));

    // Every cache file for this source is now rubbish: truncated, empty, and
    // prefixed with something else entirely.
    let entries = cache_files(&dir.join("cache"));
    assert_eq!(entries.len(), 1, "one source should produce one entry");
    for entry in entries {
        fs::write(&entry, b"not a cache file").unwrap_or(());
    }

    let second = cache
        .parse_and_cache(&source)
        .expect("a damaged entry is a miss");
    assert_eq!(second, first);
    assert_eq!(cache.stats(), CacheStats { hits: 0, misses: 2 });
}

#[test]
fn opening_the_cache_drops_what_it_cannot_use() {
    let dir = tempdir("prune");
    let cache_dir = dir.join("cache");
    let source = dir.join("data/thing.json");
    write(&source, r#"{"a": 1}"#);

    let cache = DiskCache::new(&cache_dir, &dir).expect("cache");
    let _ = cache.parse_and_cache(&source).expect("first read");

    // An older entry for the same source, and a file that is not an entry at all.
    let newest = cache_files(&cache_dir);
    assert_eq!(newest.len(), 1);
    let name = newest
        .first()
        .and_then(|path| path.file_name())
        .map(|n| n.to_string_lossy().into_owned());
    let Some(name) = name else {
        panic!("entry has no name")
    };
    let older = cache_dir.join("data").join(
        name.replace(".cache", "")
            .replace(|c: char| c.is_ascii_digit(), "1")
            + ".cache",
    );
    fs::write(&older, b"stale").unwrap_or(());
    let junk = cache_dir.join("junk.txt");
    fs::write(&junk, b"not an entry").unwrap_or(());

    let reopened = DiskCache::new(&cache_dir, &dir).expect("reopen");

    let left = cache_files(&cache_dir);
    assert_eq!(
        left, newest,
        "the older entry and the junk file should be gone"
    );
    assert_eq!(reopened.stats(), CacheStats { hits: 0, misses: 0 });
}
