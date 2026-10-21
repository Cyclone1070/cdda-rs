//! The cache against the real content: every file, twice.
//!
//! The point of a cache is that it changes nothing but the time taken. This test
//! proves the first half on all 6,220 shipped files: a value read from the cache
//! is identical, member for member, to a value parsed from the source.

// Tests are allowed to fail loudly: a test that cannot say what went wrong is
// not a test. The library keeps the strict lints.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cdda_json::Value;
use cdda_json::cache::{CacheStats, DiskCache};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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

/// Every `*.json` file under `dir`, recursively, in a stable order.
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
            } else if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// A digest of a value that ignores nothing: type, every number's exact bits,
/// every string, every key, and the order of members.
fn digest(value: &Value, out: &mut u64) {
    fn mix(bytes: &[u8], out: &mut u64) {
        for byte in bytes {
            *out ^= u64::from(*byte);
            *out = out.wrapping_mul(0x100_0000_01b3);
        }
    }
    match value {
        Value::Null => mix(b"n", out),
        Value::Bool(flag) => mix(if *flag { b"t" } else { b"f" }, out),
        Value::Int(number) => {
            mix(b"i", out);
            mix(&number.to_le_bytes(), out);
        }
        Value::Float(number) => {
            mix(b"d", out);
            mix(&number.to_bits().to_le_bytes(), out);
        }
        Value::Str(text) => {
            mix(b"s", out);
            mix(text.as_bytes(), out);
        }
        Value::Array(items) => {
            mix(b"[", out);
            for item in items {
                digest(item, out);
            }
            mix(b"]", out);
        }
        Value::Object(object) => {
            mix(b"{", out);
            for (key, item) in object.members() {
                mix(key.as_bytes(), out);
                digest(item, out);
            }
            mix(b"}", out);
        }
    }
}

fn read_all(cache: &DiskCache, files: &[PathBuf]) -> (u64, Duration, Vec<String>) {
    let started = Instant::now();
    let mut combined = 0xcbf2_9ce4_8422_2325u64;
    let mut failures = Vec::new();
    for file in files {
        match cache.parse_and_cache(file) {
            Ok(value) => digest(&value, &mut combined),
            Err(error) => failures.push(format!("{}: {error}", file.display())),
        }
    }
    (combined, started.elapsed(), failures)
}

#[test]
fn cached_values_are_identical_to_parsed_values() {
    let data = data_dir();
    let files = json_files(&data);
    assert!(
        files.len() > 6000,
        "expected the whole content set, got {}",
        files.len()
    );

    let cache_dir = std::env::temp_dir().join("cdda-json-cache-corpus");
    let _ = std::fs::remove_dir_all(&cache_dir);

    let cold = DiskCache::new(&cache_dir, &data).expect("cache");
    let (cold_digest, cold_time, cold_failures) = read_all(&cold, &files);
    assert!(
        cold_failures.is_empty(),
        "cold read failed: {:?}",
        &cold_failures[..cold_failures.len().min(3)]
    );
    assert_eq!(
        cold.stats(),
        CacheStats {
            hits: 0,
            misses: files.len() as u64
        }
    );

    let warm = DiskCache::new(&cache_dir, &data).expect("cache");
    let (warm_digest, warm_time, warm_failures) = read_all(&warm, &files);
    assert!(
        warm_failures.is_empty(),
        "warm read failed: {:?}",
        &warm_failures[..warm_failures.len().min(3)]
    );
    assert_eq!(
        warm.stats(),
        CacheStats {
            hits: files.len() as u64,
            misses: 0
        }
    );

    assert_eq!(cold_digest, warm_digest, "cache changed a value");
    println!(
        "{} files: cold {:?}, warm {:?}",
        files.len(),
        cold_time,
        warm_time
    );

    let _ = std::fs::remove_dir_all(&cache_dir);
}
