//! The city's building lists, from the reference's own regional settings.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_content::overmap_special::{CitySettings, read_city_settings};

fn settings() -> CitySettings {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/regional_map_settings.json");
    let bytes = std::fs::read(&path).expect("the regional settings");
    let cdda_json::Value::Array(regions) = cdda_json::parse(&bytes).expect("valid json") else {
        panic!("the file is a list of regions");
    };
    let region = regions
        .iter()
        .find(|region| {
            let cdda_json::Value::Object(fields) = region else {
                return false;
            };
            matches!(fields.get("city"), Some(cdda_json::Value::Object(_)))
        })
        .expect("a region with city settings");
    read_city_settings(region).expect("city settings")
}

#[test]
fn the_three_bins_are_read_with_their_weights() {
    let settings = settings();
    assert_eq!(settings.houses.len(), 178, "178 kinds of house");
    assert_eq!(settings.shops.len(), 223, "223 kinds of shop");
    assert_eq!(settings.parks.len(), 51, "51 kinds of park");
    assert!(settings.houses.iter().all(|(_, weight)| *weight > 0));
}

#[test]
fn the_radii_are_the_reference_s_own() {
    let settings = settings();
    assert_eq!(settings.shop_radius, 30);
    assert_eq!(settings.shop_sigma, 50);
    assert_eq!(settings.park_radius, 20);
    assert_eq!(settings.park_sigma, 80);
}

#[test]
fn a_heavy_entry_carries_a_heavier_weight_than_a_light_one() {
    let settings = settings();
    let weight = |bin: &[(String, i32)], id: &str| {
        bin.iter()
            .find(|(name, _)| name == id)
            .map_or(0, |(_, weight)| *weight)
    };
    // The reference weights most houses at fifty and one at five; the difference has to survive the read.
    assert_eq!(weight(&settings.houses, "2storyModern01"), 5);
    assert_eq!(weight(&settings.houses, "bungalow01"), 50);
}
