//! Byte-for-byte parity with the reference loader.
//!
//! For every JSON file shipped with the reference this test parses the file
//! with this crate and asks a probe built from the reference's own vendored
//! flatbuffers parser (`crates/json/tools/reference_probe.cpp`, built by
//! `crates/json/tools/build_reference_probe.sh`) for its verdict and for an
//! FNV-1a 64 digest of a canonical encoding of the flexbuffer it produced. Both
//! the accept/reject decision and the digest — which covers key order, integer
//! versus float typing, decoded string bytes and NUL handling — must match.
//!
//! The probe is optional: without it the test prints how to build it and is
//! skipped, because it needs a C++ compiler and the pinned reference checkout.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

/// FNV-1a 64 over bytes, the hash the probe uses.
fn fnv(bytes: &[u8]) -> u64 {
    let mut hash = 1_469_598_103_934_665_603_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash
}

/// The probe's canonical encoding, digest included.
fn canonical_digest(value: &cdda_json::Value) -> u64 {
    fn encode(value: &cdda_json::Value, out: &mut String) {
        use std::fmt::Write as _;
        match value {
            cdda_json::Value::Null => out.push_str("null"),
            cdda_json::Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
            cdda_json::Value::Int(number) => {
                let _ = write!(out, "i{number}");
            }
            cdda_json::Value::Float(number) => {
                let _ = write!(out, "f{:016x}", number.to_bits());
            }
            cdda_json::Value::Str(text) => {
                let _ = write!(out, "s{:016x}", fnv(text.as_bytes()));
            }
            cdda_json::Value::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    encode(item, out);
                }
                out.push(']');
            }
            cdda_json::Value::Object(object) => {
                out.push('{');
                for (i, (key, item)) in object.members().iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    // Keys are compared, and hashed, as NUL-terminated C
                    // strings, exactly like flexbuffers does.
                    let bytes = key.as_bytes();
                    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
                    let _ = write!(out, "k{:016x}=", fnv(bytes.get(..end).unwrap_or_default()));
                    encode(item, out);
                }
                out.push('}');
            }
        }
    }
    let mut encoded = String::new();
    encode(value, &mut encoded);
    fnv(encoded.as_bytes())
}

#[test]
fn every_reference_file_agrees_with_the_reference_parser() {
    let probe = probe_path();
    if !probe.is_file() {
        eprintln!(
            "skipped: reference probe not found at {}; build it with \
             crates/json/tools/build_reference_probe.sh",
            probe.display()
        );
        return;
    }
    let files = json_files(&data_dir());
    assert_eq!(files.len(), 6220, "unexpected fixture count under data");

    let listing = files
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let spawned = Command::new(&probe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn();
    assert!(
        spawned.is_ok(),
        "failed to start the reference probe at {}",
        probe.display()
    );
    let Ok(mut child) = spawned else {
        return;
    };
    // The probe answers as it goes, so its stdout has to be drained while the
    // listing is still being written: feed stdin from its own thread.
    let listing_with_newline = format!("{listing}\n");
    assert!(child.stdin.is_some(), "probe stdin is not piped");
    let Some(mut stdin) = child.stdin.take() else {
        return;
    };
    let feeder = std::thread::spawn(move || drop(stdin.write_all(listing_with_newline.as_bytes())));
    let finished = child.wait_with_output();
    assert!(finished.is_ok(), "failed to run the reference probe");
    let Ok(output) = finished else {
        return;
    };
    drop(feeder.join());
    assert!(
        output.status.success(),
        "reference probe exited with {}",
        output.status
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut agreed = 0_usize;
    let mut disagreements = Vec::new();
    for line in stdout.lines() {
        let mut fields = line.split('\t');
        let (Some(path), Some(verdict), Some(payload)) =
            (fields.next(), fields.next(), fields.next())
        else {
            disagreements.push(format!("unparsable probe line: {line}"));
            continue;
        };
        let Ok(bytes) = std::fs::read(path) else {
            disagreements.push(format!("{path}: unreadable"));
            continue;
        };
        let ours = cdda_json::parse(&bytes);
        match (&ours, verdict) {
            (Ok(value), "OK") => {
                let digest = canonical_digest(value);
                if format!("{digest:016x}") == payload {
                    agreed += 1;
                } else {
                    disagreements.push(format!(
                        "{path}: digest {digest:016x} but the reference says {payload}"
                    ));
                }
            }
            (Err(error), "FAIL") => {
                // The reference prefixes its message with the file path.
                if payload.ends_with(&error.text()) {
                    agreed += 1;
                } else {
                    disagreements.push(format!(
                        "{path}: rejected as {error}, reference says {payload}"
                    ));
                }
            }
            _ => {
                disagreements.push(format!(
                    "{path}: we say {ours:?}, the reference says {verdict} {payload}"
                ));
            }
        }
    }
    assert_eq!(
        agreed,
        6220,
        "only {agreed} of 6220 files agreed with the reference; disagreements: {:#?}",
        disagreements
            .get(..10.min(disagreements.len()))
            .unwrap_or_default()
    );
}
