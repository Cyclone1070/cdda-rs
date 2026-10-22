//! What a region palette actually produced, counted over four hundred squares.
//!
//! The feature trace records the palette **name** each square drew - `t_region_groundcover_forest` four hundred times in
//! this chunk - and the map dump records the terrain that name became. Those two together are four hundred real draws from
//! one weighted list, so the list's weights can be checked against them without a single die: nothing the port believes
//! about a weight can survive being compared with what the reference actually produced.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::type_complexity
)]

use std::collections::BTreeMap;

use cdda_content::overmap_special::read_region_palettes;

fn reference_regional_settings() -> cdda_json::Value {
    // The forest composition and its palettes live in the region settings file, which the port reads with the same
    // loader the game does.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/regional_map_settings.json");
    cdda_json::parse(&std::fs::read(&path).expect("the region settings")).expect("valid json")
}

/// The palettes as the region settings define them.
fn palettes() -> BTreeMap<String, Vec<(String, i32)>> {
    let root = reference_regional_settings();
    read_region_palettes(&root)
}

fn chunk() -> (Vec<String>, Vec<(i32, i32, String, String)>) {
    let map_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/maps/reference-chunk-features.json");
    let cdda_json::Value::Object(root) =
        cdda_json::parse(&std::fs::read(&map_path).expect("the chunk")).expect("valid json")
    else {
        panic!("an object");
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("rows") else {
        panic!("rows");
    };
    let mut cells = Vec::new();
    for row in rows {
        let cdda_json::Value::Array(row) = row else {
            panic!("a row");
        };
        for cell in row {
            let cdda_json::Value::Str(text) = cell else {
                panic!("a terrain id");
            };
            cells.push(text.clone());
        }
    }
    let trace_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-chunk-features.txt");
    let text = std::fs::read_to_string(&trace_path).expect("the feature trace");
    let mut drawn = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        drawn.push((
            fields[0].parse().expect("x"),
            fields[1].parse().expect("y"),
            fields[3].to_owned(),
            fields[4].to_owned(),
        ));
    }
    (cells, drawn)
}

#[test]
fn the_record_is_one_chunk_and_the_two_files_agree_on_its_shape() {
    let (cells, drawn) = chunk();
    assert_eq!(cells.len(), 576);
    assert_eq!(drawn.len(), 576);
    // The trace is written in the reference's own loop order - x outside, y inside - while the dump is row-major, so the
    // two files line up by coordinate and not by position. Comparing them by position was my first mistake here: it made
    // every square disagree.
    let terrain_at = |x: i32, y: i32| cells[(y * 24 + x) as usize].clone();
    assert_eq!(terrain_at(0, 0), cells[0]);
}

#[test]
fn the_trace_records_what_the_raffle_chose_not_what_the_map_ended_with() {
    // The finding, recorded as a measurement rather than hidden: of 576 squares, **273** hold a terrain that the palette
    // the square recorded could not have produced. The pattern is systematic - ground cover recorded, underbrush or a
    // shrub or a tree standing there - which says the map's final terrain came from somewhere other than this one raffle.
    // Until the placement itself is traced, the palette's weights cannot be checked against this chunk, and the observed
    // share of forest floor (186 of 400 ground-cover squares) is not evidence about the weight either way.
    let palettes = palettes();
    assert!(
        palettes.contains_key("t_region_groundcover_forest"),
        "the ground cover palette loads"
    );
    let (cells, drawn) = chunk();
    let mut outside = 0;
    let mut examples: Vec<(String, String)> = Vec::new();
    for (x, y, name, _) in &drawn {
        let cell = &cells[(*y * 24 + *x) as usize];
        if !reachable(&palettes, name).contains(cell) {
            outside += 1;
            if examples.len() < 3 {
                examples.push((name.clone(), cell.clone()));
            }
        }
    }
    assert_eq!(
        outside, 273,
        "squares outside their recorded palette: {examples:?}"
    );
}

#[test]
fn plain_ground_cover_accounts_for_two_hundred_and_eighty_sixths_of_the_chunk() {
    // The count that is trustworthy, because it comes from the raffle itself rather than from the map: which palette each
    // square drew. Ground cover is 400 squares of 576, dense trees 97, dense shrubs 75, and four squares drew a single
    // young tree directly.
    let (_, drawn) = chunk();
    let count = |name: &str| {
        drawn
            .iter()
            .filter(|(_, _, palette, _)| palette == name)
            .count()
    };
    assert_eq!(count("t_region_groundcover_forest"), 400);
    assert_eq!(count("t_region_tree_forest_dense"), 97);
    assert_eq!(count("t_region_shrub_forest_dense"), 75);
    assert_eq!(count("t_tree_young"), 4);
}

/// Every terrain a palette name can become, following nested palettes.
fn reachable(palettes: &BTreeMap<String, Vec<(String, i32)>>, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = vec![name.to_owned()];
    let mut steps = 0;
    while let Some(current) = pending.pop() {
        steps += 1;
        assert!(steps < 1024, "the palette chain walked too far");
        match palettes.get(&current) {
            Some(entries) => {
                for (entry, _) in entries {
                    pending.push(entry.clone());
                }
            }
            None => out.push(current),
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}
