//! The port's woodland against the reference's own generated world.
//!
//! `corpus/overmap/reference-world.json` and `reference-world-noise.json` came from one oracle run with one seed, so
//! the picture, the noise field and the seed describe the same world. That makes the strongest comparison the port
//! has had: the reference's **final terrain** against what the port's rules predict from the same noise.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap
)]

use cdda_content::overmap_special::read_forest_settings;
use cdda_core_types::overmap::ForestGrowth;

fn json(path: &str) -> cdda_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap")
        .join(path);
    cdda_json::parse(&std::fs::read(&path).expect("a corpus file")).expect("valid json")
}

fn rows(value: &cdda_json::Value) -> Vec<Vec<char>> {
    let cdda_json::Value::Object(root) = value else {
        panic!("an object");
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("rows") else {
        panic!("rows");
    };
    rows.iter()
        .map(|row| match row {
            cdda_json::Value::Str(text) => text.chars().collect(),
            _ => panic!("a row"),
        })
        .collect()
}

fn noise(value: &cdda_json::Value) -> Vec<Vec<f32>> {
    let cdda_json::Value::Object(root) = value else {
        panic!("an object");
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("rows") else {
        panic!("rows");
    };
    rows.iter()
        .map(|row| match row {
            cdda_json::Value::Array(cells) => cells
                .iter()
                .map(|cell| match cell {
                    cdda_json::Value::Float(n) => *n as f32,
                    cdda_json::Value::Int(n) => *n as f32,
                    _ => panic!("a number"),
                })
                .collect(),
            _ => panic!("a row"),
        })
        .collect()
}

fn centre(value: &cdda_json::Value) -> (i32, i32) {
    let cdda_json::Value::Object(root) = value else {
        panic!("an object");
    };
    let Some(cdda_json::Value::Array(xy)) = root.get("centre") else {
        panic!("a centre");
    };
    let axis = |index: usize| match &xy[index] {
        cdda_json::Value::Int(n) => *n as i32,
        _ => panic!("a number"),
    };
    (axis(0), axis(1))
}

#[test]
fn the_reference_s_world_is_reproduced_tile_for_tile_where_nothing_else_overwrote_it() {
    let picture = json("reference-world.json");
    let field = json("reference-world-noise.json");
    let glyphs = rows(&picture);
    let values = noise(&field);
    let (centre_x, centre_y) = centre(&picture);

    // The region's own thresholds, not the header's defaults.
    let region_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/regional_map_settings.json");
    let region =
        cdda_json::parse(&std::fs::read(&region_path).expect("the region")).expect("valid json");
    let thresholds = read_forest_settings(&region).expect("forest settings");
    assert!(
        (thresholds.threshold - 0.2).abs() < 1e-6,
        "the file's threshold overrides the header's: {}",
        thresholds.threshold
    );

    // The overmap this world sits in, and the growth that goes with being away from the origin.
    let overmap_position = (centre_x.div_euclid(180), centre_y.div_euclid(180));
    let adjust = ForestGrowth::default().adjust(overmap_position);

    let mut agree = 0;
    let mut overwritten = 0;
    let mut disagree = Vec::new();
    for (dy, row) in glyphs.iter().enumerate() {
        for (dx, glyph) in row.iter().enumerate() {
            let predicted = values[dy][dx] + adjust > thresholds.threshold;
            let forest = *glyph == 'F';
            if predicted == forest {
                agree += 1;
            } else if !matches!(glyph, 'F' | '.' | '~') {
                // A road, a building or a slope stands here: some later rule wrote over the woodland, which is the
                // reference's own order - forests first, then the town.
                overwritten += 1;
            } else {
                disagree.push((
                    centre_x + dx as i32 - 20,
                    centre_y + dy as i32 - 20,
                    *glyph,
                    values[dy][dx],
                ));
            }
        }
    }
    assert!(
        disagree.is_empty(),
        "{} tiles disagree between the noise rule and the reference's own terrain: {:?}",
        disagree.len(),
        &disagree[..disagree.len().min(5)]
    );
    assert!(
        agree + overwritten == 1681,
        "{agree} agreed, {overwritten} overwritten by the town"
    );
    assert!(
        agree > 1300,
        "most of the map is decided by the rule alone: {agree}"
    );
}

#[test]
fn the_region_s_thresholds_are_lower_than_the_header_s_defaults() {
    let region_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/regional_map_settings.json");
    let region =
        cdda_json::parse(&std::fs::read(&region_path).expect("the region")).expect("valid json");
    let loaded = read_forest_settings(&region).expect("forest settings");
    let header = cdda_core_types::overmap::ForestSettings::default();
    assert!(
        (loaded.threshold - 0.2).abs() < 1e-6,
        "the file asks for 0.2"
    );
    assert!(
        (loaded.threshold_thick - 0.25).abs() < 1e-6,
        "and 0.25 for thick"
    );
    assert!(
        loaded.threshold < header.threshold && loaded.threshold_thick < header.threshold_thick,
        "both are below the header's defaults: {loaded:?} against {header:?}"
    );
}

#[test]
fn the_growth_pushes_woodland_outwards_from_the_origin() {
    let growth = cdda_core_types::overmap::ForestGrowth::default();
    assert!(
        (growth.adjust((0, 0)) - 0.0).abs() < 1e-6,
        "no shift at the origin"
    );
    assert!(
        (growth.adjust((-1, 0)) - 0.02).abs() < 1e-6,
        "one map west: the west rate"
    );
    assert!(
        (growth.adjust((0, -1)) - 0.04).abs() < 1e-6,
        "one map north: the north rate"
    );
    assert!((growth.adjust((1, 0)) - 0.0).abs() < 1e-6, "nothing east");
    assert!(
        (growth.adjust((2, -3)) - 0.12).abs() < 1e-6,
        "and they add up: three norths"
    );
}
