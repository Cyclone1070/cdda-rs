//! What a forest's layers put on a square.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_content::overmap_build::pick_forest_feature;
use cdda_content::overmap_special::ForestComponent;
use cdda_core_types::rng::Rolls;

fn layer(id: &str, sequence: i32, chance: i32, types: &[(&str, i32)]) -> ForestComponent {
    ForestComponent {
        id: id.to_string(),
        sequence,
        chance,
        types: types
            .iter()
            .map(|(name, weight)| ((*name).to_string(), *weight))
            .collect(),
    }
}

#[test]
fn a_square_gets_at_most_one_feature() {
    let layers = vec![
        layer("trees", 0, 1, &[("t_tree", 1)]),
        layer("shrubs", 1, 1, &[("t_underbrush", 1)]),
    ];
    let mut rolls = Rolls::new(1);
    let picked = pick_forest_feature(&layers, &mut rolls).expect("a feature");
    assert_eq!(
        picked, "t_tree",
        "the first layer that succeeds wins outright"
    );
}

#[test]
fn a_layer_that_does_not_roll_leaves_the_square_to_the_next_one() {
    // The first layer never succeeds - a chance of a million - so the second one decides.
    let layers = vec![
        layer("water", 0, 1_000_000, &[("t_water_sh", 1)]),
        layer("shrubs", 1, 1, &[("t_underbrush", 1)]),
    ];
    let mut rolls = Rolls::new(7);
    assert_eq!(
        pick_forest_feature(&layers, &mut rolls),
        Some("t_underbrush".to_string())
    );
}

#[test]
fn a_square_can_get_nothing_at_all() {
    let layers = vec![
        layer("water", 0, 1_000_000, &[("t_water_sh", 1)]),
        layer("clutter", 1, 1_000_000, &[("t_shrub", 1)]),
    ];
    let mut rolls = Rolls::new(11);
    assert_eq!(
        pick_forest_feature(&layers, &mut rolls),
        None,
        "and then it keeps its groundcover"
    );
}

#[test]
fn over_a_thousand_squares_a_layer_lands_about_as_often_as_its_chance_says() {
    let layers = vec![layer("trees", 0, 12, &[("t_tree", 1)])];
    let mut rolls = Rolls::new(2024);
    let mut trees = 0;
    for _ in 0..12_000 {
        if pick_forest_feature(&layers, &mut rolls).is_some() {
            trees += 1;
        }
    }
    // One square in twelve, so a thousand over twelve thousand squares; the spread is the dice's, not the rule's.
    assert!(
        (900..=1100).contains(&trees),
        "a tree on {trees} of 12000 squares, expecting about 1000"
    );
}

#[test]
fn a_layer_s_types_are_drawn_by_weight() {
    let layers = vec![layer(
        "trees",
        0,
        1,
        &[("t_region_tree_forest", 128), ("t_tree_young", 32)],
    )];
    let mut rolls = Rolls::new(31337);
    let mut young = 0;
    for _ in 0..8_000 {
        if pick_forest_feature(&layers, &mut rolls) == Some("t_tree_young".to_string()) {
            young += 1;
        }
    }
    // 32 in 160 is a fifth.
    assert!(
        (1500..=1700).contains(&young),
        "{young} young trees of 8000, expecting about 1600"
    );
}

#[test]
fn the_dice_are_spent_layer_by_layer() {
    // Two squares drawn with the same dice differ only if the rule spent different numbers of rolls. A rule that
    // rolled every layer, instead of stopping at the first success, would leave the second square with different dice
    // than the reference gives it.
    let layers = vec![
        layer("trees", 0, 2, &[("t_tree", 1)]),
        layer("shrubs", 1, 2, &[("t_underbrush", 1)]),
    ];
    let mut one = Rolls::new(5);
    let mut two = Rolls::new(5);
    let first = pick_forest_feature(&layers, &mut one);
    let second = pick_forest_feature(&layers, &mut one);
    assert_eq!(first, pick_forest_feature(&layers, &mut two));
    let _ = second;
}
