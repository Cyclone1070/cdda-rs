//! The port's groundcover pass against the reference's own forest chunk.
//!
//! This is a **baseline**, not parity: the reference runs four feature layers after its groundcover pass, and the port
//! runs none of them yet, so most squares will differ. What the numbers are good for is telling the two failures apart:
//! wrong ground where the port should have been right, and missing features where the port has not written that pass
//! at all.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::overmap_build::forest_ground_pass;
use cdda_content::overmap_special::{
    OvermapLocation, load_forest_mapgens, max_sparseness, read_forest_mapgen, read_groundcover,
    read_region_palettes,
};
use cdda_core_types::overmap::{forest_border_depth, forest_perimeter_depth};
use cdda_core_types::rng::Rolls;

fn region() -> cdda_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/regional_map_settings.json");
    cdda_json::parse(&std::fs::read(&path).expect("the region")).expect("valid json")
}

fn chunk() -> Vec<String> {
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
    let mut cells = Vec::new();
    for row in rows {
        let cdda_json::Value::Array(cells_in_row) = row else {
            panic!("a row");
        };
        for cell in cells_in_row {
            match cell {
                cdda_json::Value::Str(text) => cells.push(text.clone()),
                _ => panic!("a terrain id"),
            }
        }
    }
    cells
}

#[test]
fn the_ground_pass_produces_a_full_chunk_of_named_ground() {
    let region = region();
    let forest = read_forest_mapgen(&region, "forest").expect("forest settings");
    let self_palette =
        read_groundcover(&region, "t_region_groundcover_forest").expect("the palette");
    let default_palette = read_groundcover(&region, "t_region_groundcover").expect("the palette");
    let palettes = read_region_palettes(&region);
    let biomes = load_forest_mapgens(&region);
    let border = forest_border_depth(
        [(1920, 4800), (1944, 4800), (1920, 4824), (1944, 4824)],
        5,
        1.0,
    );
    let mut rolls = Rolls::new(31337);
    let perimeter = forest_perimeter_depth(border, 5, &mut rolls);
    let ground = forest_ground_pass(
        24,
        &perimeter,
        &self_palette,
        &default_palette,
        &forest.components,
        &palettes,
        // The ceiling is the region's **densest** forest - thick forest at 4 - not ordinary forest at 3. Using the
        // square's own value tilts the chunk towards woodland, which is what the first measurement showed.
        f32::from(i16::try_from(max_sparseness(&region)).expect("a small factor")),
        3.0,
        // The world dump says what surrounds this square: a road to the north, forest to the east, open ground to the
        // south and forest to the west. The forests own biomes and the other two do not, which is most of the rule.
        [None, biomes.get("forest"), None, biomes.get("forest")],
        &mut rolls,
    );
    assert_eq!(ground.len(), 576, "twenty-four squares a side");
    assert!(
        ground.iter().all(|id| id.starts_with("t_")),
        "every square is a terrain"
    );
    let kinds: std::collections::BTreeSet<&String> = ground.iter().collect();
    // The chunk is ground plus features now, so the vocabulary is wide: the palettes' kinds, the trees the region's
    // tree palette resolves to, shrubs, undergrowth and the odd fern or trunk.
    assert!(
        kinds.len() > 10,
        "a forest is varied: {} kinds",
        kinds.len()
    );
    assert!(kinds.contains(&"t_grass".to_string()) && kinds.contains(&"t_forestfloor".to_string()));
    assert!(
        kinds.iter().all(|id| id.starts_with("t_")),
        "and every id is a terrain rather than a palette name: {kinds:?}"
    );
}

#[test]
fn the_ground_pass_and_the_reference_agree_on_a_share_of_the_chunk() {
    // The baseline: how much of the reference's chunk the port's ground pass already gets right, measured rather than
    // guessed. Two things keep it from being higher and both are known: the palette is only one of the inputs - the
    // neighbours' own palettes are not loaded - and the reference's four feature layers overwrite much of its ground.
    let region = region();
    let forest = read_forest_mapgen(&region, "forest").expect("forest settings");
    let self_palette =
        read_groundcover(&region, "t_region_groundcover_forest").expect("the palette");
    let default_palette = read_groundcover(&region, "t_region_groundcover").expect("the palette");
    let palettes = read_region_palettes(&region);
    let biomes = load_forest_mapgens(&region);
    let border = forest_border_depth(
        [(1920, 4800), (1944, 4800), (1920, 4824), (1944, 4824)],
        5,
        1.0,
    );
    let mut rolls = Rolls::new(31337);
    let perimeter = forest_perimeter_depth(border, 5, &mut rolls);
    let ground = forest_ground_pass(
        24,
        &perimeter,
        &self_palette,
        &default_palette,
        &forest.components,
        &palettes,
        // The ceiling is the region's **densest** forest - thick forest at 4 - not ordinary forest at 3. Using the
        // square's own value tilts the chunk towards woodland, which is what the first measurement showed.
        f32::from(i16::try_from(max_sparseness(&region)).expect("a small factor")),
        3.0,
        // The world dump says what surrounds this square: a road to the north, forest to the east, open ground to the
        // south and forest to the west. The forests own biomes and the other two do not, which is most of the rule.
        [None, biomes.get("forest"), None, biomes.get("forest")],
        &mut rolls,
    );
    let reference = chunk();
    let agree = ground
        .iter()
        .zip(reference.iter())
        .filter(|(ours, theirs)| ours == theirs)
        .count();
    let _ = OvermapLocation {
        id: String::new(),
        terrains: Vec::new(),
        flags: Vec::new(),
    };
    // Measured at **291 of 576** with the reference's own chunk, against roughly 64 that chance alone would give. The
    // number moved 122 -> 183 -> 242 -> 285 -> 305 -> 291, from three fixes that each came out of the measurement: resolving
    // the region's **palette chains** rather than stopping at the first name, treating a **furniture** feature as
    // something that does not replace the ground under it, and scaling the four neighbours by the square's own
    // sparseness while this square's ticket stays at a fixed three - the last of which is worth the most, because the
    // scaling is not symmetric and dropping it lets the forest claim half again as much of the chunk as it should.
    // The fourth fix was the ceiling: the feathering compares against the region's **densest** forest - thick forest
    // at four - not the square's own three, and using the square's own value hands the forest too much of the chunk.
    // The last change took the score **down**, from 305 to 291, and is kept: the pass was measured with all four
    // neighbours treated as open field, and the reference's own world says this square has forest to the east and west
    // and a road to the north. Reading the real neighbours out of the world dump and giving the two forests their own
    // densities and palettes is the truer rule, and a lower score from a truer rule is the right trade - the old
    // number was flattered by two neighbours that do not exist.
    let mut counts = std::collections::BTreeMap::new();
    for id in &ground {
        *counts.entry(id.clone()).or_insert(0_usize) += 1;
    }
    let first_difference = ground
        .iter()
        .zip(reference.iter())
        .position(|(ours, theirs)| ours != theirs)
        .unwrap_or(576);
    eprintln!(
        "the ground pass agrees on {agree} of 576 squares, first difference at square {first_difference} \
         (row {}, column {}): ours {:?} against theirs {:?}",
        first_difference / 24,
        first_difference % 24,
        ground.get(first_difference),
        reference.get(first_difference)
    );
    // Where the two first disagree: **square zero**, not somewhere in the middle. The port lays grass and the
    // reference lays forest floor. So the difference is structural rather than accumulated drift - the very first
    // decision a square makes is already different, and no amount of matching dice afterwards would fix it. That
    // narrows the next probe to one square with known dice instead of 576 comparisons.
    assert_eq!(
        first_difference, 0,
        "the disagreement starts at the first square"
    );
    assert_eq!(
        ground[0], "t_grass",
        "the port calls the corner square grass"
    );
    assert_eq!(
        reference[0], "t_forestfloor",
        "and the reference calls it forest floor"
    );
    assert!(
        agree > 275,
        "the ground pass agrees with the reference on {agree} of 576 squares - below a hundred would mean it is \
         producing ground the reference never lays down"
    );
    assert!(
        agree < 576,
        "and it cannot be all of them: the feature layers are not written yet"
    );
}
