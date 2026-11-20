//! **Where cities stand.**
//!
//! Reference: `overmap::place_cities` (`overmap.cpp:6037-6162`), the root of the overmap's own generation: how many cities an overmap holds, how big each is, and where
//! its centre stands. The port had city *tiles* (`cities::set_tiles`) and no city *placement*, so every world's cities were whatever a test laid down.
//!
//! The options the rule reads are the ones the data's own defaults set: `CITY_SIZE` eight and `CITY_SPACING` four from the `world_cities` slider's first level
//! (`data/core/world_option_sliders.json:13`), and `OVERMAP_MAXIMUM_URBANITY` eight (`data/core/external_options.json`). Urbanity and forestosity are the overmap's
//! position on the world's urban/rural gradient; at the origin overmap both are zero, which is where these tests stand.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use bevy_ecs::prelude::*;

/// The options the reference's data defaults to.
fn the_default_options() -> cdda_sim::city_placement::CitySettings {
    cdda_sim::city_placement::CitySettings {
        city_size: 8,
        city_spacing: 4,
        maximum_urbanity: 8,
    }
}

/// **The number of cities follows the coverage formula, and each one is placed inside the map.** Reference: `overmap.cpp:6094-6107` - the count is
/// `roll_remainder( omts_per_overmap * coverage / omts_per_city )`. With the default options at the origin that is `32400 / 2^4 / 216.75 = 9.34`: an overmap holds
/// **nine or ten** cities, and the reference's own table says the same - spacing four covers about six percent of the map, seven cities of size eight.
#[test]
fn an_overmap_holds_the_cities_the_coverage_formula_asks_for() {
    let mut counts: std::collections::BTreeMap<usize, u32> = std::collections::BTreeMap::new();
    for seed in 1..=200u32 {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        let placed = cdda_sim::city_placement::place_cities(
            &mut world,
            (0, 0),
            the_default_options(),
            0,
            0.0,
        );
        *counts.entry(placed.len()).or_insert(0) += 1;
        for city in &placed {
            assert!(
                (2..=55).contains(&city.size),
                "**a city is at least two and at most fifty-five**: {}",
                city.size
            );
            assert!(
                (0..180).contains(&city.centre.0) && (0..180).contains(&city.centre.1),
                "**and its centre is on the overmap**: {:?}",
                city.centre
            );
        }
    }
    assert!(
        counts.keys().all(|count| (9..=10).contains(count)),
        "**the formula asks for nine or ten**: {counts:?}"
    );
    assert!(
        counts.values().sum::<u32>() == 200,
        "every seed answered: {counts:?}"
    );
    assert!(
        counts.len() > 1,
        "**and the seed decides**, so both answers appear: {counts:?}"
    );
}

/// **A city centre is an intersection, and the same seed puts it in the same place.** Reference: `overmap.cpp:6135-6139`: the tile the city centre lands on is set to
/// `oter_road_nesw` - "every city starts with an intersection" - and only a tile that is still the default terrain may host one.
#[test]
fn a_city_centre_is_an_intersection_and_replays() {
    let place = |seed: u32| {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        let placed = cdda_sim::city_placement::place_cities(
            &mut world,
            (0, 0),
            the_default_options(),
            0,
            0.0,
        );
        let roads: Vec<_> = placed
            .iter()
            .map(|city| {
                (
                    city.centre,
                    cdda_sim::overmap::terrain_at(&world, city.centre).map(ToOwned::to_owned),
                )
            })
            .collect();
        (placed, roads)
    };
    // Seed 1 places a full overmap of cities, and the test says so rather than hoping: a seed that placed none would make the assertions below vacuous.
    let (placed, roads) = place(1);
    assert!(!placed.is_empty(), "**this seed places cities**");
    assert!(
        roads
            .iter()
            .all(|(_, terrain)| terrain.as_deref() == Some("road_nesw")),
        "**and every centre is an intersection**: {roads:?}"
    );
    let (again, again_roads) = place(1);
    assert_eq!(
        placed
            .iter()
            .map(|city| (city.centre, city.size))
            .collect::<Vec<_>>(),
        again
            .iter()
            .map(|city| (city.centre, city.size))
            .collect::<Vec<_>>(),
        "**the same seed places the same city**"
    );
    assert_eq!(roads, again_roads, "and sets the same tile");
}

/// **A tile that is already taken cannot host a city centre.** Reference: `overmap.cpp:6133-6134` - the placement is only accepted `if( ter( p ) == default_oter )`, so a
/// second city on a road is refused rather than stacking.
#[test]
fn a_city_centre_is_not_placed_on_a_road() {
    // Find a seed that places one, then tidy the world and ask it to place again with that centre already a road: the tile must be refused.
    let mut placed_centre = None;
    for seed in 1..=200u32 {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        let placed = cdda_sim::city_placement::place_cities(
            &mut world,
            (0, 0),
            the_default_options(),
            0,
            0.0,
        );
        if let Some(city) = placed.first() {
            placed_centre = Some((seed, city.centre));
            break;
        }
    }
    let (seed, centre) = placed_centre.expect("some seed places a city");
    let mut world = World::new();
    cdda_sim::overmap::set_terrain(&mut world, centre, "road_nesw");
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
    let placed =
        cdda_sim::city_placement::place_cities(&mut world, (0, 0), the_default_options(), 0, 0.0);
    assert!(
        placed.iter().all(|city| city.centre != centre),
        "**the road is not built on twice**"
    );
}

/// **A city is never smaller than two**, whatever the size ladder rolls. Reference: `overmap.cpp:6116-6118` - "a city of size 1 is just a crossroad with no buildings at all",
/// so the ladder's tiny branch is clamped up. With a city size of three the tiny branch can roll below two, which is where the clamp is the only thing holding the rule up.
#[test]
fn a_city_is_never_smaller_than_two() {
    let small = cdda_sim::city_placement::CitySettings {
        city_size: 3,
        city_spacing: 4,
        maximum_urbanity: 8,
    };
    let mut seen = 0;
    for seed in 1..=300u32 {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        for city in cdda_sim::city_placement::place_cities(&mut world, (0, 0), small, 0, 0.0) {
            seen += 1;
            assert!(
                city.size >= 2,
                "**the clamp holds where the ladder rolls below two**: {}",
                city.size
            );
        }
    }
    assert!(seen > 0, "the test saw {seen} cities, so it is not vacuous");
}
