//! The writer's output is byte-identical to the reference's.
//!
//! `crates/json/tools/reference_probe.cpp` has a `writer` mode that walks a
//! parsed flexbuffer through a verbatim copy of `JsonOut`'s layout code
//! (`references/Cataclysm-DDA/src/json.cpp:2226-2440`) and prints, per file, the
//! compact and the pretty text. This test compares this crate's writer against
//! it for a deterministic sample of the reference's own data files: every 50th
//! file plus the three largest.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// The reference's data directory, or `CDDA_DATA_DIR` when set.
fn data_dir() -> PathBuf {
    std::env::var("CDDA_DATA_DIR").map_or_else(
        |_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data"),
        PathBuf::from,
    )
}

/// The probe binary, or `CDDA_JSON_PROBE` when set.
fn probe_path() -> PathBuf {
    std::env::var("CDDA_JSON_PROBE").map_or_else(
        |_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/json-probe/reference_probe"),
        PathBuf::from,
    )
}

/// Every `*.json` file under `dir`, recursively, with its size.
fn json_files(dir: &Path) -> Vec<(PathBuf, u64)> {
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
                let size = entry.metadata().map_or(0, |meta| meta.len());
                found.push((path, size));
            }
        }
    }
    found.sort();
    found
}

/// The files this test compares: every one of them.
///
/// A sample was enough to find the writer's layout rules; it is not enough to
/// claim the writer matches, because the rules that only bite on one shape -
/// a key that needs escaping, a float that lands on a rounding boundary, an
/// empty object nested three deep - live in the files a sample skips. The whole
/// tree is cheap enough to check on every run.
fn sample_files() -> Vec<PathBuf> {
    let files = json_files(&data_dir());
    assert_eq!(files.len(), 6220, "unexpected fixture count under data");
    files.into_iter().map(|(path, _)| path).collect()
}

/// Starts the probe with `paths` on its stdin, fed from its own thread so a full
/// stdout pipe can never stall the probe.
fn start_probe(probe: &Path, paths: &[PathBuf]) -> Option<Child> {
    let listing = paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let spawned = Command::new(probe)
        .arg("writer")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn();
    assert!(spawned.is_ok(), "failed to start the reference probe");
    let mut child = spawned.ok()?;
    let mut stdin = child.stdin.take()?;
    let listing = format!("{listing}\n");
    drop(std::thread::spawn(move || {
        drop(stdin.write_all(listing.as_bytes()));
    }));
    Some(child)
}

/// One `<path>\0<compact>\0<pretty>\0` record.
type Record = (String, Vec<u8>, Vec<u8>);

/// Reads the next record, or `None` at the end of the stream.
fn read_record(reader: &mut BufReader<std::process::ChildStdout>) -> Option<Record> {
    let mut fields = [Vec::new(), Vec::new(), Vec::new()];
    for field in &mut fields {
        let read = reader.read_until(0, field);
        assert!(read.is_ok(), "failed to read the reference probe's output");
        if read.unwrap_or(0) == 0 {
            return None;
        }
        // `read_until` keeps the delimiter.
        let _ = field.pop();
    }
    let [path, compact, pretty] = fields;
    Some((String::from_utf8_lossy(&path).into_owned(), compact, pretty))
}

/// Where two texts first differ, with a little context either side.
fn first_difference(ours: &str, reference: &[u8]) -> String {
    let at = ours
        .bytes()
        .zip(reference)
        .position(|(a, b)| a != *b)
        .unwrap_or(ours.len().min(reference.len()));
    format!(
        "byte {at}: ours {:?} reference {:?}",
        ours.get(at..)
            .unwrap_or_default()
            .chars()
            .take(40)
            .collect::<String>(),
        String::from_utf8_lossy(reference.get(at..).unwrap_or_default())
            .chars()
            .take(40)
            .collect::<String>()
    )
}

#[test]
fn writer_output_matches_the_reference_writer() {
    let probe = probe_path();
    if !probe.is_file() {
        eprintln!(
            "skipped: reference probe not found at {}; build it with \
             crates/json/tools/build_reference_probe.sh",
            probe.display()
        );
        return;
    }
    let sample = sample_files();
    let Some(mut child) = start_probe(&probe, &sample) else {
        return;
    };
    assert!(child.stdout.is_some(), "probe stdout is not piped");
    let Some(stdout) = child.stdout.take() else {
        return;
    };
    let mut reader = BufReader::new(stdout);

    let mut compared = 0_usize;
    let mut mismatches: Vec<String> = Vec::new();
    while let Some((path, compact, pretty)) = read_record(&mut reader) {
        compared += 1;
        let Ok(bytes) = std::fs::read(&path) else {
            mismatches.push(format!("{path}: unreadable"));
            continue;
        };
        let Ok(value) = cdda_json::parse(&bytes) else {
            mismatches.push(format!("{path}: this crate rejected it"));
            continue;
        };
        for (pretty_mode, expected) in [(false, &compact), (true, &pretty)] {
            let mut writer = cdda_json::Writer::new(pretty_mode);
            writer.write_value(&value);
            let actual = writer.into_string();
            if actual.as_bytes() != expected.as_slice() {
                mismatches.push(format!(
                    "{path}: {} output differs at {}",
                    if pretty_mode { "pretty" } else { "compact" },
                    first_difference(&actual, expected)
                ));
            }
        }
    }
    let _ = child.wait();

    assert_eq!(
        compared,
        sample.len(),
        "compared {compared} of {} files",
        sample.len()
    );
    assert!(
        mismatches.is_empty(),
        "{} of {compared} files differ from the reference writer: {:#?}",
        mismatches.len(),
        mismatches
            .get(..5.min(mismatches.len()))
            .unwrap_or_default()
    );
}
