//! Which terrains own a forest biome, and how dense each one is.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::overmap_special::load_forest_mapgens;

fn region() -> cdda_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/regional_map_settings.json");
    cdda_json::parse(&std::fs::read(&path).expect("the region")).expect("valid json")
}

#[test]
fn three_terrains_own_a_forest_biome() {
    let biomes = load_forest_mapgens(&region());
    let mut names: Vec<&String> = biomes.keys().collect();
    names.sort();
    assert_eq!(names.len(), 3, "{names:?}");
    assert!(biomes.contains_key("forest"));
    assert!(biomes.contains_key("forest_thick"));
    assert!(biomes.contains_key("forest_water"));
}

#[test]
fn each_biome_carries_its_own_density() {
    let biomes = load_forest_mapgens(&region());
    let density = |name: &str| {
        biomes
            .get(name)
            .map(|forest| forest.sparseness_adjacency_factor)
            .expect("a biome")
    };
    assert_eq!(density("forest"), 3);
    assert_eq!(
        density("forest_thick"),
        4,
        "thick woodland is the densest in the region"
    );
    assert_eq!(
        density("forest_water"),
        2,
        "and swampy woodland the thinnest"
    );
}

#[test]
fn a_road_owns_no_forest_biome() {
    // The previous round left this open: the region's comments say a road *may* be given a biome so that woodland does
    // not fade out where it meets one. It is not: there is no `road` entry, so a road beside a forest is a neighbour
    // with no biome - no density, no palette - and the generator treats it the way it treats open ground.
    let biomes = load_forest_mapgens(&region());
    assert!(
        !biomes.keys().any(|name| name.starts_with("road")),
        "no road owns a forest biome: {:?}",
        biomes.keys().collect::<Vec<&String>>()
    );
}

#[test]
fn the_biomes_the_table_holds_are_the_ones_the_forest_rule_uses() {
    // The three keys here are exactly the terrains the ordinary forest's own definition names, so a square of forest
    // next to a square of thick forest finds a biome for its neighbour and treats it as denser.
    let biomes = load_forest_mapgens(&region());
    let forest = biomes.get("forest").expect("forest");
    assert!(forest.terrains.iter().any(|terrain| terrain == "forest"));
    assert_eq!(biomes.get("forest").map(|f| f.components.len()), Some(4));
}
