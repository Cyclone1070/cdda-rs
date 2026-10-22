//! Two chunks at the same moment in their lives.
//!
//! The reference's forest generation leaves placeholders in the map and a later sweep resolves them; the port now does the
//! same, in the same two steps. So its output can finally be compared with the reference's chunk **as generated** - names
//! against names - instead of against a finished map that has been through every later pass.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::collections::BTreeMap;

use cdda_content::overmap_build::{forest_ground_pass_placeholders, resolve_chunk_placeholders};
use cdda_core_types::rng::Rolls;

fn chunk() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-chunk-placed.txt");
    std::fs::read_to_string(&path)
        .expect("the placed chunk")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split_whitespace()
                .nth(5)
                .expect("a terrain")
                .to_owned()
        })
        .collect()
}

#[test]
fn the_reference_chunk_as_generated_is_sixty_percent_placeholders() {
    // The stage the port now stops at, measured on the reference's own chunk: **567 of 576** squares still hold a name
    // beginning `t_region_`, and only nine are terrains the raffle placed directly. My first version said four hundred,
    // carrying a different run's number into this chunk; the reference is far more placeholder-heavy than that.
    let cells = chunk();
    assert_eq!(cells.len(), 576);
    let placeholders = cells
        .iter()
        .filter(|cell| cell.starts_with("t_region_"))
        .count();
    assert_eq!(placeholders, 567);
    assert_eq!(
        576 - placeholders,
        9,
        "the squares that skipped the palettes"
    );
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for cell in &cells {
        *counts.entry(cell.as_str()).or_insert(0) += 1;
    }
    assert_eq!(counts.get("t_region_groundcover").copied(), Some(258));
    assert_eq!(
        counts.get("t_region_groundcover_forest").copied(),
        Some(253)
    );
}

#[test]
fn the_port_stops_at_placeholders_and_the_separate_sweep_finishes_the_job() {
    // The architecture the reference has and the port now shares: generation leaves names, resolution replaces them. The
    // same inputs run twice - once stopping at the generator, once going on to the sweep - so the difference between the
    // two calls is exactly the second step.
    let side = 4;
    let perimeter = vec![2_i32; (side * side) as usize];
    let mut palettes = BTreeMap::new();
    palettes.insert("t_region_test".to_owned(), vec![("t_grass".to_owned(), 1)]);
    let palette = vec![("t_region_test".to_owned(), 1)];
    let layers: Vec<cdda_content::overmap_special::ForestComponent> = Vec::new();
    let neighbours = [None; 4];

    let raw = forest_ground_pass_placeholders(
        side,
        &perimeter,
        &palette,
        &palette,
        &layers,
        &palettes,
        4.0,
        3.0,
        neighbours,
        &mut Rolls::new(7),
    );
    assert_eq!(raw.len(), 16);
    assert!(
        raw.iter().all(|cell| cell == "t_region_test"),
        "the generator's own output is the placeholder: {raw:?}"
    );

    let mut cells = raw;
    let resolved = resolve_chunk_placeholders(&mut cells, &palettes, &mut Rolls::new(7));
    assert_eq!(resolved, 16, "every square needed the second step");
    assert!(cells.iter().all(|cell| cell == "t_grass"), "{cells:?}");
}
