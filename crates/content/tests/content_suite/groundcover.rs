//! The ground a region lays down before anything else is placed on it.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::overmap_special::read_groundcover;

fn region() -> cdda_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/regional_map_settings.json");
    cdda_json::parse(&std::fs::read(&path).expect("the region")).expect("valid json")
}

fn weight(palette: &[(String, i32)], id: &str) -> i32 {
    palette
        .iter()
        .find(|(name, _)| name == id)
        .map_or(0, |(_, w)| *w)
}

#[test]
fn the_forest_palette_is_the_reference_s_own_weights() {
    let palette = read_groundcover(&region(), "t_region_groundcover_forest").expect("the palette");
    assert_eq!(palette.len(), 5, "five kinds of forest ground");
    assert_eq!(
        weight(&palette, "t_forestfloor"),
        10,
        "forest floor, ten times the rest"
    );
    assert_eq!(weight(&palette, "t_grass"), 1);
    assert_eq!(weight(&palette, "t_grass_long"), 1);
    assert_eq!(weight(&palette, "t_moss"), 1);
    assert_eq!(weight(&palette, "t_grass_dead"), 1);
}

#[test]
fn the_default_palette_weighs_grass_against_dead_grass_and_dirt() {
    let palette = read_groundcover(&region(), "t_region_groundcover").expect("the palette");
    assert_eq!(weight(&palette, "t_grass"), 12000);
    assert_eq!(weight(&palette, "t_grass_dead"), 2000);
    assert_eq!(weight(&palette, "t_dirt"), 1000);
    // The file's own comment says the weights are large so a mod can add a rare terrain; pinning the ratio rather
    // than the numbers would survive a rebalance, but the numbers are what the reference has.
    assert_eq!(palette.len(), 3);
}

#[test]
fn the_reference_s_forest_chunk_carries_ground_the_forest_palette_does_not_contain() {
    // A finding rather than a rule: the chunk's most common ground is `t_grass` at 404 of 576 squares, while the
    // forest palette makes `t_forestfloor` ten times likelier than grass. So the chunk is **not** the palette alone -
    // the generator blends a square towards its neighbouring biomes, and this square's neighbours are open ground.
    // The port has to reproduce that blending before it can hope to match the chunk.
    let palette = read_groundcover(&region(), "t_region_groundcover_forest").expect("the palette");
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
    let mut kinds: Vec<&str> = Vec::new();
    for row in rows {
        let cdda_json::Value::Array(cells) = row else {
            panic!("a row");
        };
        for cell in cells {
            if let cdda_json::Value::Str(text) = cell {
                kinds.push(text.as_str());
            }
        }
    }
    assert!(kinds.contains(&"t_dirt"), "the chunk has dirt");
    assert_eq!(
        weight(&palette, "t_dirt"),
        0,
        "and the forest palette has none"
    );
    assert!(
        kinds.contains(&"t_grass"),
        "the chunk's most common ground is grass"
    );
    assert!(
        weight(&palette, "t_forestfloor") > weight(&palette, "t_grass"),
        "while the palette prefers forest floor ten to one - so the mix in the chunk comes from somewhere else"
    );
}
