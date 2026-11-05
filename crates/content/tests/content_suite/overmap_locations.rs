//! The locations a special may be placed on, against the reference's own definitions.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::overmap_special::{OvermapLocation, load_locations};

fn locations() -> Vec<OvermapLocation> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/overmap/special_locations.json");
    let bytes = std::fs::read(&path).expect("the locations file");
    let document = cdda_json::parse(&bytes).expect("valid json");
    load_locations(&[document])
}

fn find(id: &str) -> OvermapLocation {
    locations()
        .into_iter()
        .find(|location| location.id == id)
        .expect("the location")
}

#[test]
fn every_location_in_the_reference_is_read() {
    assert_eq!(locations().len(), 35, "thirty-five locations");
}

#[test]
fn forest_is_the_terrains_the_reference_lists_for_it() {
    let forest = find("forest");
    assert!(forest.test("forest", &[]));
    assert!(forest.test("forest_thick", &[]));
    assert!(forest.test("forest_trail", &[]));
    assert!(!forest.test("field", &[]), "a field is not a forest");
    assert!(!forest.test("lake_surface", &[]));
}

#[test]
fn land_includes_field_and_woodland_both() {
    let land = find("land");
    for terrain in [
        "field",
        "forest",
        "forest_thick",
        "forest_water",
        "meadow_core",
    ] {
        assert!(land.test(terrain, &[]), "{terrain} is land");
    }
    assert!(!land.test("lake_surface", &[]), "water is not land");
}

#[test]
fn a_location_written_against_flags_matches_by_flag() {
    // None of the reference's locations use flags today, so this checks the rule the type allows rather than the
    // data: a location with a flag matches a terrain carrying it.
    let by_flag = OvermapLocation {
        id: "by_flag".to_string(),
        terrains: Vec::new(),
        flags: vec!["MAN_MADE".to_string()],
    };
    assert!(by_flag.test("field", &["MAN_MADE"]));
    assert!(!by_flag.test("field", &[]));
}
