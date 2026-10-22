//! The reference's generated map chunk, checked against the rules that chose it.
//!
//! `corpus/maps/reference-forest-chunk.json` is one 24x24 map chunk the reference generated, with the overmap
//! terrain it belongs to and the position it sits at. The port does not generate map chunks yet, so this gate checks
//! what it *can*: that the two levels agree with each other - the overmap rule says this square is woodland, and the
//! reference's own chunk is woodland - and that the chunk is shaped the way the port's level would have to be.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]

use cdda_core_types::noise::forest_noise;
use cdda_core_types::overmap::{ForestGrowth, ForestSettings};

fn chunk() -> (String, (i32, i32), Vec<Vec<String>>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/maps/reference-forest-chunk.json");
    let value = cdda_json::parse(&std::fs::read(&path).expect("the chunk")).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("an object");
    };
    let oter = match root.get("oter") {
        Some(cdda_json::Value::Str(text)) => text.clone(),
        _ => panic!("an oter"),
    };
    let Some(cdda_json::Value::Array(omt)) = root.get("omt") else {
        panic!("a position");
    };
    let axis = |index: usize| match &omt[index] {
        cdda_json::Value::Int(n) => *n as i32,
        _ => panic!("a number"),
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("rows") else {
        panic!("rows");
    };
    let rows = rows
        .iter()
        .map(|row| match row {
            cdda_json::Value::Array(cells) => cells
                .iter()
                .map(|cell| match cell {
                    cdda_json::Value::Str(text) => text.clone(),
                    _ => panic!("a terrain id"),
                })
                .collect(),
            _ => panic!("a row"),
        })
        .collect();
    (oter, (axis(0), axis(1)), rows)
}

#[test]
fn the_chunk_is_one_full_map_square() {
    let (oter, position, rows) = chunk();
    assert_eq!(
        oter, "forest",
        "the reference chose woodland for this square"
    );
    assert_eq!(position, (-100, 264), "and says where it is");
    assert_eq!(rows.len(), 24, "a map chunk is 24 squares a side");
    assert!(rows.iter().all(|row| row.len() == 24), "square");
}

#[test]
fn the_overmap_rule_and_the_reference_s_chunk_agree_about_this_square() {
    // The seed is the one the world was generated with; the noise is the port's own port of the reference's field.
    const SEED: u32 = 1_370_934_862;
    let (_, (x, y), _) = chunk();
    let adjust = ForestGrowth::default().adjust((x.div_euclid(180), y.div_euclid(180)));
    let noise = forest_noise(x, y, SEED) + adjust;
    let settings = ForestSettings {
        threshold: 0.2,
        threshold_thick: 0.25,
    };
    assert!(
        noise > settings.threshold,
        "the reference put woodland here, so the rule must call it woodland: noise {noise} with adjustment {adjust}"
    );
}

#[test]
fn the_chunk_is_made_of_a_handful_of_kinds_of_ground() {
    let (_, _, rows) = chunk();
    let mut kinds: Vec<&str> = rows.iter().flatten().map(String::as_str).collect();
    kinds.sort_unstable();
    kinds.dedup();
    // Fourteen kinds, and the count is pinned: a chunk of woodland drawn from the reference's own generator has a
    // known vocabulary, and a change here means the generator or the data moved.
    assert_eq!(kinds.len(), 14, "{kinds:?}");
    assert!(
        kinds.contains(&"t_grass"),
        "grass is the most common ground in a forest"
    );
    assert!(kinds.contains(&"t_underbrush"), "and there is undergrowth");
    assert!(
        kinds.iter().all(|kind| kind.starts_with("t_")),
        "every id is a terrain"
    );
}

#[test]
fn the_chunk_is_not_one_flat_kind_of_ground() {
    let (_, _, rows) = chunk();
    let mut counts = std::collections::BTreeMap::new();
    for row in &rows {
        for cell in row {
            *counts.entry(cell.as_str()).or_insert(0_usize) += 1;
        }
    }
    let most_common = counts.values().copied().max().unwrap_or(0);
    // My first version of this test claimed no kind covers half the chunk, and the reference says otherwise: grass
    // covers **404 of 576 squares**, seven tenths of it. A forest chunk is grass with patches of dead grass, forest
    // floor and dirt in it, not an even mix - so what is worth pinning is that it is dominated by one kind and still
    // has more than a dozen others.
    assert!(
        most_common > 24 * 24 / 2,
        "one kind dominates the chunk: {most_common} of 576"
    );
    assert!(
        counts.len() >= 10,
        "and the rest is varied: {} kinds",
        counts.len()
    );
    assert_eq!(counts.get("t_grass").copied().unwrap_or(0), 404);
}
