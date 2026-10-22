//! How the reference generates woodland on the map level.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::overmap_special::read_forest_mapgen;

fn region() -> cdda_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/regional_map_settings.json");
    cdda_json::parse(&std::fs::read(&path).expect("the region")).expect("valid json")
}

#[test]
fn the_forest_settings_name_the_terrains_they_apply_to() {
    let forest = read_forest_mapgen(&region(), "forest").expect("forest settings");
    assert_eq!(
        forest.terrains,
        vec!["forest", "special_forest", "special_forest_isherwood"]
    );
    assert_eq!(forest.sparseness_adjacency_factor, 3);
}

#[test]
fn the_layers_run_in_the_order_their_sequence_gives() {
    let forest = read_forest_mapgen(&region(), "forest").expect("forest settings");
    assert_eq!(forest.components.len(), 4, "trees, shrubs, clutter, water");
    let sequences: Vec<i32> = forest
        .components
        .iter()
        .map(|layer| layer.sequence)
        .collect();
    assert_eq!(sequences, vec![0, 1, 2, 3], "and they are sorted by it");
    assert_eq!(forest.components[0].id, "trees");
    assert_eq!(forest.components[1].id, "shrubs_and_flowers");
}

#[test]
fn a_layer_s_chance_is_one_square_in_that_many() {
    let forest = read_forest_mapgen(&region(), "forest").expect("forest settings");
    let chance = |id: &str| {
        forest
            .components
            .iter()
            .find(|layer| layer.id == id)
            .map(|layer| layer.chance)
            .expect("the layer")
    };
    assert_eq!(chance("trees"), 12, "one square in twelve gets a tree");
    assert_eq!(chance("shrubs_and_flowers"), 10);
    assert_eq!(chance("clutter"), 80);
    assert_eq!(chance("water"), 512, "and water is rare");
}

#[test]
fn a_layer_s_types_keep_their_weights() {
    let forest = read_forest_mapgen(&region(), "forest").expect("forest settings");
    let trees = &forest.components[0];
    assert_eq!(trees.types.len(), 2);
    let weight = |id: &str| {
        trees
            .types
            .iter()
            .find(|(name, _)| name == id)
            .map(|(_, weight)| *weight)
            .expect("the type")
    };
    assert_eq!(weight("t_region_tree_forest"), 128);
    assert_eq!(
        weight("t_tree_young"),
        32,
        "young trees are a quarter as likely"
    );
}

#[test]
fn the_groundcover_is_laid_before_any_layer() {
    let forest = read_forest_mapgen(&region(), "forest").expect("forest settings");
    assert_eq!(
        forest.groundcover,
        vec![("t_region_groundcover_forest".to_string(), 1)]
    );
}

#[test]
fn the_items_scattered_through_a_forest_are_named_with_their_chance() {
    let forest = read_forest_mapgen(&region(), "forest").expect("forest settings");
    assert_eq!(forest.item_group, "forest");
    assert_eq!(forest.item_group_chance, 60);
    assert_eq!(forest.item_spawn_iterations, 1);
}

#[test]
fn thick_forest_is_described_by_its_own_settings() {
    let thick = read_forest_mapgen(&region(), "forest_thick").expect("thick forest settings");
    assert!(
        thick
            .terrains
            .iter()
            .any(|terrain| terrain == "forest_thick")
    );
    assert!(
        !thick.components.is_empty(),
        "thick woodland has layers of its own"
    );
    let trees = thick
        .components
        .iter()
        .find(|layer| layer.id == "trees")
        .expect("a tree layer");
    assert!(
        trees.chance != 12,
        "and a different density from ordinary forest: {}",
        trees.chance
    );
}
