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
#[test]
fn every_file_under_data_json_parses() {
    let (count, failures) = crate::parsed::parse_all(&data_dir().join("json"));
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
    let (count, failures) = crate::parsed::parse_all(&data_dir());
    assert_eq!(count, 6220, "unexpected fixture count under data");
    assert!(
        failures.is_empty(),
        "{} of {count} files failed to parse, first 5: {:#?}",
        failures.len(),
        failures.get(..5.min(failures.len())).unwrap_or_default()
    );
}
