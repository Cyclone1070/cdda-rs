//! The port's groundcover pass against the reference's own forest chunk.
//!
//! This is a **baseline**, not parity: the reference runs four feature layers after its groundcover pass, and the port
//! runs none of them yet, so most squares will differ. What the numbers are good for is telling the two failures apart
//! - wrong ground where the port should have been right, and missing features where the port has not written that pass
//! at all.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::overmap_build::forest_ground_pass;
use cdda_content::overmap_special::{OvermapLocation, read_forest_mapgen, read_groundcover};
use cdda_core_types::overmap::{forest_border_depth, forest_perimeter_depth};
use cdda_core_types::rng::Rolls;

fn region() -> cdda_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/regional_map_settings.json");
    cdda_json::parse(&std::fs::read(&path).expect("the region")).expect("valid json")
}

fn chunk() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/maps/reference-forest-chunk.json");
    let cdda_json::Value::Object(root) =
        cdda_json::parse(&std::fs::read(&path).expect("the chunk")).expect("valid json")
    else {
        panic!("an object");
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("rows") else {
        panic!("rows");
    };
    let mut cells = Vec::new();
    for row in rows {
        let cdda_json::Value::Array(cells_in_row) = row else {
            panic!("a row");
        };
        for cell in cells_in_row {
            match cell {
                cdda_json::Value::Str(text) => cells.push(text.clone()),
                _ => panic!("a terrain id"),
            }
        }
    }
    cells
}

#[test]
fn the_ground_pass_produces_a_full_chunk_of_named_ground() {
    let region = region();
    let forest = read_forest_mapgen(&region, "forest").expect("forest settings");
    let self_palette =
        read_groundcover(&region, "t_region_groundcover_forest").expect("the palette");
    let default_palette = read_groundcover(&region, "t_region_groundcover").expect("the palette");
    let border = forest_border_depth(
        [(1920, 4800), (1944, 4800), (1920, 4824), (1944, 4824)],
        5,
        1.0,
    );
    let mut rolls = Rolls::new(31337);
    let perimeter = forest_perimeter_depth(border, 5, &mut rolls);
    let ground = forest_ground_pass(
        24,
        &perimeter,
        &self_palette,
        &default_palette,
        3.0,
        3.0,
        &mut rolls,
    );
    assert_eq!(ground.len(), 576, "twenty-four squares a side");
    assert!(
        ground.iter().all(|id| id.starts_with("t_")),
        "every square is a terrain"
    );
    let kinds: std::collections::BTreeSet<&String> = ground.iter().collect();
    // Six kinds: the forest palette's five plus `t_dirt`, which comes from the region's default ground through the
    // neighbour and empty tickets.
    assert_eq!(kinds.len(), 6, "the palettes' kinds: {kinds:?}");
}

#[test]
fn the_ground_pass_and_the_reference_agree_on_a_share_of_the_chunk() {
    // The baseline: how much of the reference's chunk the port's ground pass already gets right, measured rather than
    // guessed. Two things keep it from being higher and both are known: the palette is only one of the inputs - the
    // neighbours' own palettes are not loaded - and the reference's four feature layers overwrite much of its ground.
    let region = region();
    let forest = read_forest_mapgen(&region, "forest").expect("forest settings");
    let self_palette =
        read_groundcover(&region, "t_region_groundcover_forest").expect("the palette");
    let default_palette = read_groundcover(&region, "t_region_groundcover").expect("the palette");
    let border = forest_border_depth(
        [(1920, 4800), (1944, 4800), (1920, 4824), (1944, 4824)],
        5,
        1.0,
    );
    let mut rolls = Rolls::new(31337);
    let perimeter = forest_perimeter_depth(border, 5, &mut rolls);
    let ground = forest_ground_pass(
        24,
        &perimeter,
        &self_palette,
        &default_palette,
        3.0,
        3.0,
        &mut rolls,
    );
    let reference = chunk();
    let agree = ground
        .iter()
        .zip(reference.iter())
        .filter(|(ours, theirs)| ours == theirs)
        .count();
    let _ = forest;
    let _ = OvermapLocation {
        id: String::new(),
        terrains: Vec::new(),
        flags: Vec::new(),
    };
    // Measured at **122 of 576** with the reference's own chunk, against roughly 64 that chance alone would give - so
    // the pass tracks the reference rather than scattering ground at random. The remaining deficit is the four feature
    // layers, which overwrite most of what this pass lays down. Quoting the number here means a later change that makes
    // it worse is visible in the test's own history rather than only in a diff.
    eprintln!("the ground pass agrees on {agree} of 576 squares");
    assert!(
        agree > 100,
        "the ground pass agrees with the reference on {agree} of 576 squares - below a hundred would mean it is \
         producing ground the reference never lays down"
    );
    assert!(
        agree < 576,
        "and it cannot be all of them: the feature layers are not written yet"
    );
}
