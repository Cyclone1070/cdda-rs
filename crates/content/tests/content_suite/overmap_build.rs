//! Putting buildings beside a town's streets.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_content::overmap_build::{PLACEMENT_RETRIES, build_town};
use cdda_content::overmap_special::{
    CitySettings, OvermapLocation, OvermapSpecial, SpecialTerrain,
};
use cdda_core_types::overmap::{Direction, Overmap};
use cdda_core_types::rng::Rolls;

fn special(id: &str, terrain: &str, flags: &[&str]) -> OvermapSpecial {
    OvermapSpecial {
        id: id.to_string(),
        overmaps: vec![
            SpecialTerrain {
                offset: (0, 0, 0),
                terrain: terrain.to_string(),
            },
            SpecialTerrain {
                offset: (1, 0, 0),
                terrain: terrain.to_string(),
            },
        ],
        occurrences: (10, 100),
        flags: flags.iter().map(std::string::ToString::to_string).collect(),
        locations: vec!["field".to_string()],
        city_distance: (-1, 1000),
        city_sizes: (0, 100),
        rotate: true,
        priority: 0,
        connections: Vec::new(),
        subtype: None,
    }
}

fn settings(houses: &[(&str, i32)]) -> CitySettings {
    CitySettings {
        shop_radius: 0,
        shop_sigma: 0,
        park_radius: 0,
        park_sigma: 0,
        houses: houses
            .iter()
            .map(|(id, w)| ((*id).to_string(), *w))
            .collect(),
        shops: Vec::new(),
        parks: Vec::new(),
    }
}

fn locations() -> Vec<OvermapLocation> {
    vec![OvermapLocation {
        id: "field".to_string(),
        terrains: vec!["field".to_string(), "forest".to_string()],
        flags: Vec::new(),
    }]
}

#[test]
fn a_street_with_buildings_puts_them_beside_it() {
    let mut overmap = Overmap::filled("field");
    let streets: Vec<(i32, i32)> = (0..30).map(|x| (x + 10, 20)).collect();
    let specials = vec![special("house_a", "house_terrain", &[])];
    let mut rolls = Rolls::new(7);
    let report = build_town(
        &mut overmap,
        &specials,
        &locations(),
        &settings(&[("house_a", 1)]),
        &streets,
        Direction::East,
        (24, 20),
        4,
        &mut rolls,
    );
    assert!(!report.built.is_empty(), "buildings were placed");
    assert!(
        report.tiles() >= report.built.len(),
        "each writes at least one tile"
    );
    let built_terrain = (0..180)
        .flat_map(|y| (0..180).map(move |x| (x, y)))
        .filter(|(x, y)| overmap.terrain(*x, *y) == Some("house_terrain"))
        .count();
    assert!(built_terrain > 0, "the terrain was written into the map");
}

#[test]
fn a_square_whose_ground_is_wrong_is_refused_and_says_why() {
    let mut overmap = Overmap::filled("lake_surface");
    let streets: Vec<(i32, i32)> = (0..20).map(|x| (x + 10, 20)).collect();
    let specials = vec![special("house_a", "house_terrain", &[])];
    let mut rolls = Rolls::new(7);
    let report = build_town(
        &mut overmap,
        &specials,
        &locations(),
        &settings(&[("house_a", 1)]),
        &streets,
        Direction::East,
        (20, 20),
        4,
        &mut rolls,
    );
    assert!(report.built.is_empty(), "nothing fits on a lake");
    assert!(!report.refused.is_empty(), "and the refusals are recorded");
}

#[test]
fn a_unique_building_is_placed_once() {
    let mut overmap = Overmap::filled("field");
    let streets: Vec<(i32, i32)> = (0..60).map(|x| (x + 5, 40)).collect();
    let specials = vec![special("one_only", "unique_terrain", &["CITY_UNIQUE"])];
    let mut rolls = Rolls::new(11);
    let report = build_town(
        &mut overmap,
        &specials,
        &locations(),
        &settings(&[("one_only", 1)]),
        &streets,
        Direction::East,
        (30, 40),
        4,
        &mut rolls,
    );
    let unique = report
        .built
        .iter()
        .filter(|built| built.id == "one_only")
        .count();
    assert!(unique <= 1, "a city-unique building appears once: {unique}");
    assert_eq!(PLACEMENT_RETRIES, 10, "the reference retries ten times");
}

#[test]
fn a_building_that_is_too_large_for_the_town_is_never_used() {
    let mut overmap = Overmap::filled("field");
    let mut big = special("mansion", "mansion_terrain", &[]);
    big.city_sizes = (8, 20);
    let streets: Vec<(i32, i32)> = (0..40).map(|x| (x + 5, 40)).collect();
    let mut rolls = Rolls::new(3);
    let report = build_town(
        &mut overmap,
        &[big],
        &locations(),
        &settings(&[("mansion", 1)]),
        &streets,
        Direction::East,
        (25, 40),
        2,
        &mut rolls,
    );
    assert!(
        report.built.is_empty(),
        "a town of size two gets no mansion"
    );
}

#[test]
fn the_same_seed_builds_the_same_town() {
    let build = |seed: u32| {
        let mut overmap = Overmap::filled("field");
        let streets: Vec<(i32, i32)> = (0..40).map(|x| (x + 5, 40)).collect();
        let mut rolls = Rolls::new(seed);
        build_town(
            &mut overmap,
            &[
                special("a", "a_terrain", &[]),
                special("b", "b_terrain", &[]),
            ],
            &locations(),
            &settings(&[("a", 50), ("b", 50)]),
            &streets,
            Direction::East,
            (25, 40),
            4,
            &mut rolls,
        )
        .built
    };
    assert_eq!(build(31337), build(31337));
    assert_ne!(
        build(1),
        build(2),
        "and a different seed builds a different town"
    );
}

#[test]
fn both_sides_of_a_street_are_built_on() {
    let mut overmap = Overmap::filled("field");
    let streets: Vec<(i32, i32)> = (0..40).map(|x| (x + 5, 40)).collect();
    let mut rolls = Rolls::new(31337);
    let report = build_town(
        &mut overmap,
        &[special("a", "a_terrain", &[])],
        &locations(),
        &settings(&[("a", 1)]),
        &streets,
        Direction::East,
        (25, 40),
        4,
        &mut rolls,
    );
    let north = report
        .built
        .iter()
        .filter(|built| built.rotation == Direction::South)
        .count();
    let south = report
        .built
        .iter()
        .filter(|built| built.rotation == Direction::North)
        .count();
    assert!(
        north > 0 && south > 0,
        "{north} facing south, {south} facing north"
    );
}

#[test]
fn a_town_built_from_the_reference_s_own_specials_places_real_buildings() {
    // The whole family in one gate: the reference's specials, its locations, its bins, and a town's streets. If any
    // one of the four loaders stopped agreeing with the others, this is where it shows.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/overmap/overmap_special");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .expect("the specials directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();
    let documents: Vec<_> = paths
        .iter()
        .map(|path| cdda_json::parse(&std::fs::read(path).expect("a file")).expect("valid json"))
        .collect();
    let specials = cdda_content::overmap_special::load_specials(&documents);

    let locations_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/overmap/special_locations.json");
    let locations = cdda_content::overmap_special::load_locations(&[cdda_json::parse(
        &std::fs::read(&locations_path).expect("the locations"),
    )
    .expect("valid json")]);

    let region_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/regional_map_settings.json");
    let region =
        cdda_json::parse(&std::fs::read(&region_path).expect("the region")).expect("valid json");
    let city = cdda_content::overmap_special::read_city_settings(&region).expect("city settings");

    let mut overmap = Overmap::filled("field");
    let streets: Vec<(i32, i32)> = (0..60).map(|x| (x + 5, 40)).collect();
    let mut rolls = Rolls::new(31337);
    let report = build_town(
        &mut overmap,
        &specials,
        &locations,
        &city,
        &streets,
        Direction::East,
        (35, 40),
        6,
        &mut rolls,
    );
    assert!(
        !report.built.is_empty(),
        "the reference's own data builds a town"
    );
    assert!(
        report.built.iter().all(|built| !built.id.is_empty()),
        "and every building has a name"
    );
    // Whatever was built, it used the reference's own ids.
    assert!(
        report
            .built
            .iter()
            .all(|built| specials.iter().any(|special| special.id == built.id))
    );
}
