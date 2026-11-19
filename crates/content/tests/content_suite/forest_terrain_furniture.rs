//! **Furniture that follows the ground.** Reference: `load_forest_biome` (`regional_settings.cpp:144-160`), `set_terrain_dependent_furniture`
//! (`mapgen_functions.cpp:1094-1116`), and the region's own data, which is the red gate here: the JSON says exactly one terrain in exactly one biome carries such furniture.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::overmap_special::{load_forest_mapgens, terrain_dependent_furniture_roll};

fn region() -> cdda_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/regional_map_settings.json");
    cdda_json::parse(&std::fs::read(&path).expect("the region")).expect("valid json")
}

/// **The data is nested, and only shallow water in the swamp biome carries furniture.** The reference's `terrain_furniture` is a map from a terrain id to its own chance and
/// its own weighted furniture; reading it as a flat weighted list - which the port used to do - turns `chance` and `clear_furniture` into furniture ids.
#[test]
fn only_shallow_water_in_the_swamp_carries_furniture() {
    let biomes = load_forest_mapgens(&region());
    assert_eq!(biomes.len(), 3, "three biomes in the region");

    let water = biomes.get("forest_water").expect("the swamp biome").clone();
    assert_eq!(
        water.terrain_furniture.len(),
        1,
        "**exactly one terrain in the swamp has ground furniture**: {:?}",
        water.terrain_furniture.keys().collect::<Vec<_>>()
    );
    let murky = water
        .terrain_furniture
        .get("t_water_murky")
        .expect("shallow water");
    assert_eq!(murky.chance, 2, "one roll in two");
    assert!(!murky.clear_furniture, "and it adds rather than replaces");
    assert_eq!(
        murky.furniture,
        vec![("f_region_water_plant".to_owned(), 1)],
        "placing a water plant"
    );

    // **The woodland biomes carry none**, which is the fact that decides whether the reference's chunk generator makes a draw per square there.
    for name in ["forest", "forest_thick"] {
        let biome = biomes.get(name).expect("a woodland biome");
        assert!(
            biome.terrain_furniture.is_empty(),
            "**{name} has no terrain-dependent furniture at all**"
        );
    }
}

/// **A roll happens only where there is furniture with weight.** Reference: `set_terrain_dependent_furniture` returns before `one_in( chance )` when the terrain has no entry, and
/// again when the weighted list is empty - so an empty entry consumes **no** die.
#[test]
fn furniture_with_no_weight_rolls_no_die() {
    use cdda_content::overmap_special::TerrainDependentFurniture;

    let empty = TerrainDependentFurniture {
        chance: 2,
        clear_furniture: false,
        furniture: Vec::new(),
    };
    assert_eq!(
        terrain_dependent_furniture_roll(&empty),
        None,
        "**no weight, no roll** - the reference returns before `one_in`"
    );

    let weightless = TerrainDependentFurniture {
        chance: 2,
        clear_furniture: false,
        furniture: vec![("f_region_water_plant".to_owned(), 0)],
    };
    assert_eq!(
        terrain_dependent_furniture_roll(&weightless),
        None,
        "a zero weight is the same as no furniture"
    );

    let real = TerrainDependentFurniture {
        chance: 2,
        clear_furniture: false,
        furniture: vec![("f_region_water_plant".to_owned(), 1)],
    };
    assert_eq!(
        terrain_dependent_furniture_roll(&real),
        Some((2, real.furniture.as_slice())),
        "and one roll in two where it does"
    );
}
