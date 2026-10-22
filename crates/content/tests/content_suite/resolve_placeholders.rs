//! The second draw: placeholders in a generated chunk becoming real terrain.
//!
//! Map generation writes names like `t_region_groundcover_forest` into the map and leaves them there. A separate pass over
//! every square turns them into terrain, drawing one weighted pick each. So a finished chunk's terrain is the generator's
//! choice **plus** a second draw - and the port has to model both, or it produces chunks full of placeholders.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use std::collections::BTreeMap;

use cdda_content::overmap_build::resolve_chunk_placeholders;
use cdda_core_types::rng::Rolls;

fn palettes() -> BTreeMap<String, Vec<(String, i32)>> {
    // Transcribed from `regional_map_settings.json`, one entry per shape the pass has to handle: a palette of terrains, a
    // palette naming another palette, and a terrain that is nobody's placeholder.
    let mut map = BTreeMap::new();
    map.insert(
        "t_region_groundcover".to_owned(),
        vec![
            ("t_grass".to_owned(), 12000),
            ("t_grass_dead".to_owned(), 2000),
        ],
    );
    map.insert(
        "t_region_groundcover_forest".to_owned(),
        vec![("t_forestfloor".to_owned(), 10), ("t_grass".to_owned(), 1)],
    );
    map.insert(
        "t_region_tree_forest".to_owned(),
        vec![("t_region_tree_forest_dense".to_owned(), 1)],
    );
    map.insert(
        "t_region_tree_forest_dense".to_owned(),
        vec![("t_tree".to_owned(), 4), ("t_tree_pine".to_owned(), 1)],
    );
    map
}

#[test]
fn a_chunk_of_placeholders_becomes_a_chunk_of_terrain() {
    let mut cells = vec![
        "t_region_groundcover_forest".to_owned(),
        "t_region_tree_forest".to_owned(),
        "t_dirt".to_owned(),
    ];
    let resolved = resolve_chunk_placeholders(&mut cells, &palettes(), &mut Rolls::new(11));
    assert_eq!(resolved, 2, "two placeholders, one real terrain");
    assert!(!cells[0].starts_with("t_region_"), "{}", cells[0]);
    assert!(
        !cells[1].starts_with("t_region_"),
        "the nested palette resolved too: {}",
        cells[1]
    );
    assert_eq!(
        cells[2], "t_dirt",
        "a real terrain is left exactly as it was"
    );
}

#[test]
fn the_placeholders_become_entries_of_their_own_palette() {
    // The check that the pass draws rather than invents: every placeholder has to land on an entry of its palette, and the
    // entries of a nested palette count as its own.
    let palettes = palettes();
    let mut cells = vec!["t_region_groundcover_forest".to_owned(); 400];
    resolve_chunk_placeholders(&mut cells, &palettes, &mut Rolls::new(31337));
    let allowed = ["t_forestfloor", "t_grass"];
    assert!(
        cells.iter().all(|cell| allowed.contains(&cell.as_str())),
        "{:?}",
        &cells[..3]
    );
    let floor = cells
        .iter()
        .filter(|cell| cell.as_str() == "t_forestfloor")
        .count();
    // Ten to one in the data, and four hundred draws: the commoner entry has to dominate, but not be the only one.
    assert!(floor > 300, "{floor} of 400");
    assert!(floor < 400, "the rarer entry never appeared");
}

#[test]
fn the_same_seed_resolves_the_same_chunk_the_same_way() {
    let palettes = palettes();
    let mut first = vec!["t_region_tree_forest".to_owned(); 64];
    let mut second = first.clone();
    resolve_chunk_placeholders(&mut first, &palettes, &mut Rolls::new(4242));
    resolve_chunk_placeholders(&mut second, &palettes, &mut Rolls::new(4242));
    assert_eq!(first, second);
}

#[test]
fn a_chunk_with_no_placeholders_costs_no_dice() {
    // The pass draws only where it resolves, so a chunk already full of terrain leaves the dice untouched. A port that drew
    // per square regardless would put every later square of the world on different dice.
    let palettes = palettes();
    let mut cells = vec!["t_dirt".to_owned(); 8];
    let mut untouched = Rolls::new(99);
    let expected = untouched.clone();
    let resolved = resolve_chunk_placeholders(&mut cells, &palettes, &mut untouched);
    assert_eq!(resolved, 0);
    assert_eq!(untouched, expected);
}
