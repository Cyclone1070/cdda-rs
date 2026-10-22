//! The port generating the reference's chunk, at the reference's stage.
//!
//! Same perimeter, same region settings, same rules - the port draws its own dice, so this is not a positional comparison.
//! What it can settle is whether the port produces a chunk of the **shape** the reference produces: almost entirely
//! placeholders, ground cover dominating, and the same handful of palettes. A pass that resolved eagerly would fail the
//! first of those immediately, which is exactly the mistake the last three rounds uncovered.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::collections::BTreeMap;

use cdda_content::overmap_build::forest_ground_pass_placeholders;
use cdda_content::overmap_special::{load_forest_mapgens, read_region_palettes};
use cdda_core_types::rng::Rolls;

fn settings() -> cdda_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/regional_map_settings.json");
    cdda_json::parse(&std::fs::read(&path).expect("the region settings")).expect("valid json")
}

fn perimeter() -> Vec<i32> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-chunk-stage-perimeter.txt");
    std::fs::read_to_string(&path)
        .expect("the perimeter")
        .split_whitespace()
        .skip(4)
        .map(|field| field.parse().expect("a depth"))
        .collect()
}

#[test]
fn the_port_produces_a_chunk_of_placeholders_from_the_reference_s_own_perimeter() {
    let root = settings();
    let mapgens = load_forest_mapgens(&root);
    let palettes = read_region_palettes(&root);
    let perimeter = perimeter();
    assert_eq!(perimeter.len(), 96);
    // The region's default ground cover is the **placeholder** `t_region_groundcover`, exactly as the settings file writes
    // it. Expanding it here into `t_grass`, `t_grass_dead` and `t_dirt` was my first attempt, and it is precisely the
    // eagerness this test exists to catch: it left 55 placeholders instead of hundreds, because it resolved the region's
    // ground before the sweep got to it.
    let default_palette: Vec<(String, i32)> = vec![("t_region_groundcover".to_owned(), 1)];
    let no_layers: Vec<cdda_content::overmap_special::ForestComponent> = Vec::new();

    let forest = mapgens.get("forest").expect("plain woodland is defined");
    let cells = forest_ground_pass_placeholders(
        24,
        &perimeter,
        &forest.groundcover,
        &default_palette,
        &no_layers,
        &palettes,
        4.0,
        f32::from(i16::try_from(forest.sparseness_adjacency_factor).unwrap_or(3)),
        [None; 4],
        &mut Rolls::new(2024),
    );
    assert_eq!(cells.len(), 576);
    // The shape rather than the squares: the port must leave placeholders, because resolution is a later sweep.
    let placeholders = cells
        .iter()
        .filter(|cell| cell.starts_with("t_region_"))
        .count();
    assert!(
        placeholders > 400,
        "the port's generator left {placeholders} placeholders of 576 - it resolved eagerly"
    );
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for cell in &cells {
        *counts.entry(cell.as_str()).or_insert(0) += 1;
    }
    // Ground cover of some kind dominates a woodland chunk, whether it came from this biome's own layer or the region's.
    let ground_like = cells
        .iter()
        .filter(|cell| cell.starts_with("t_region_groundcover"))
        .count();
    assert!(
        ground_like > 300,
        "{ground_like} squares of plain ground cover"
    );
}
