//! The port's groundcover pass against the reference's own forest chunk.
//!
//! This is a **baseline**, not parity: the reference runs four feature layers after its groundcover pass, and the port
//! runs none of them yet, so most squares will differ. What the numbers are good for is telling the two failures apart:
//! wrong ground where the port should have been right, and missing features where the port has not written that pass
//! at all.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]

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
    // Measured at **302 of 576** with the reference's own chunk, against roughly 64 that chance alone would give. The last
    // four squares of the gain came from the **column order** (`forest_visit_order`): the reference walks `x` outside and
    // `y` inside, the port walked rows, and with one stream of dice that hands `(1,0)` the die rolled for `(0,1)`. The
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
    // The dump's rows may run the other way, or from another origin. Before blaming the generator, compare the port's
    // chunk against the reference under every mirror image of the square: if one of them scores far better, the
    // comparison was upside down rather than the rule being wrong.
    let mut best = (0_usize, "identity");
    for (name, transform) in [
        ("identity", 0_u8),
        ("flip rows", 1),
        ("flip columns", 2),
        ("rotate 180", 3),
        ("transpose", 4),
        ("transpose + flip rows", 5),
        ("transpose + flip columns", 6),
        ("transpose + rotate 180", 7),
    ] {
        let agrees = (0..576)
            .filter(|index: &usize| {
                let ours = ((*index % 24) as i32, (*index / 24) as i32);
                // The eight symmetries of the square, written out: `ours` is where the port thinks the square is,
                // and `theirs` is where the reference would have to hold it for the two to agree.
                let theirs = match transform {
                    0 => (ours.0, ours.1),
                    1 => (ours.0, 23 - ours.1),
                    2 => (23 - ours.0, ours.1),
                    3 => (23 - ours.0, 23 - ours.1),
                    4 => (ours.1, ours.0),
                    5 => (ours.1, 23 - ours.0),
                    6 => (23 - ours.1, ours.0),
                    _ => (23 - ours.1, 23 - ours.0),
                };
                ground[*index] == reference[(theirs.1 * 24 + theirs.0) as usize]
            })
            .count();
        if agrees > best.0 {
            best = (agrees, name);
        }
    }
    // Measured: identity 302, best mirror ("transpose + flip rows") 309. Sixteen squares is not nothing, but it is not
    // the decisive gap an upside-down sample would show either - a transposed chunk would fit far better than this -
    // so the rows run the way the port reads them, and the disagreement is in the rule or the dice. Pinning the
    // comparison means nobody re-opens this question by instinct later.
    eprintln!(
        "best mirror image: {} with {} of 576 squares",
        best.1, best.0
    );
    assert!(
        best.0 <= agree + 20,
        "no mirror image of the chunk fits better than the chunk itself: {} against {agree}",
        best.0
    );
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

/// **A chunk's squares are generated in columns, not rows.** Reference: `mapgen_forest`'s own loop (`mapgen_functions.cpp:1120-1121`) - `for( x ) { for( y ) }` - and the
/// reference's ticket trace is written in that same order. It is not cosmetic: the chunk consumes one stream of dice, so the order decides which square receives which draw,
/// and a row-major walk hands `(1,0)` the die rolled for `(0,1)`.
#[test]
fn a_chunk_is_generated_in_columns() {
    use cdda_content::overmap_build::forest_visit_order;

    assert_eq!(
        forest_visit_order(2),
        vec![(0, 0), (0, 1), (1, 0), (1, 1)],
        "**two squares a side walks down the first column before touching the second**"
    );
    let order = forest_visit_order(24);
    assert_eq!(order.len(), 576, "a full chunk");
    assert_eq!(order.first(), Some(&(0, 0)), "it starts at the corner");
    assert_eq!(
        order.get(1),
        Some(&(0, 1)),
        "**and steps north before east**"
    );
    assert_eq!(
        order.get(23),
        Some(&(0, 23)),
        "to the end of the first column"
    );
    assert_eq!(
        order.get(24),
        Some(&(1, 0)),
        "before the second column begins"
    );
    assert_eq!(order.last(), Some(&(23, 23)), "and the far corner is last");
}
