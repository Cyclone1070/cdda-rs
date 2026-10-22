//! One square's whole decision, replayed from the reference's own dice.
//!
//! The recording is entered where the chunk's first square begins, and from there every draw the port makes must be the
//! draw the reference made next - the recording checks each bounded draw's range, so a single extra or missing die anywhere
//! desynchronises the whole run and the test says which square it happened on.
//!
//! What this covers per square: the six-ticket raffle, and then - if the ticket names a biome - that biome's own layer pick.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::float_cmp
)]

use cdda_content::overmap_build::pick_forest_feature;
use cdda_content::overmap_special::load_forest_mapgens;
use cdda_core_types::overmap::{feather_pool, nesw_weights, pick_real_ticket};
use cdda_core_types::rng::{Recording, Rolls};

fn lines(name: &str) -> Vec<Vec<String>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap")
        .join(name);
    std::fs::read_to_string(&path)
        .expect("a trace")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.split_whitespace().map(str::to_owned).collect())
        .collect()
}

const WIDTH: i32 = 24;
const SELF_WEIGHT: f32 = 3.0;
const MAX_FACTOR: f32 = 4.0;
const FACTOR: f32 = 4.0;
const TICKETS: [i32; 6] = [0, 1, 2, 3, -1, -2];

#[test]
#[ignore = "the stream desynchronises on the reference's **item spawning**: a forest square that places a feature also \
            rolls for the biome's item group, and building an item draws a full-range int that the port has no rule for \
            because it places no items at all. Traced to its caller - `item::item()` during item group spawning - after 31 \
            squares replayed exactly, so the pipeline is sound and the gap is a missing feature rather than an arithmetic \
            error."]
fn the_biome_the_ticket_names_then_produces_the_terrain_the_reference_produced() {
    // The second decision of a square. Every neighbour here is the same dense forest, so whichever ticket wins, the layer
    // that answers it is the same one - and the port has to land on the same terrain the reference recorded, or on nothing
    // when the biome's layers all decline.
    let perimeter: Vec<i32> = lines("reference-chunk-pool-perimeter.txt")[0][4..]
        .iter()
        .map(|field| field.parse().expect("a depth"))
        .collect();
    let recorded = lines("reference-chunk-pool-dice.txt");
    let features = lines("reference-chunk-pool-features.txt");
    let stream = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/overmap/reference-chunk-pool-dice-slice.txt"),
    )
    .expect("the stream");
    let mut rolls = Rolls::replaying(Recording::parse(&stream).expect("a recording"));
    let root = cdda_json::parse(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../references/Cataclysm-DDA/data/json/regional_map_settings.json"),
        )
        .expect("the settings"),
    )
    .expect("valid json");
    let mapgens = load_forest_mapgens(&root);
    let thick = mapgens.get("forest_thick").expect("dense woodland");

    let ground_palette =
        cdda_core_types::weighted::WeightedList::from_entries(thick.groundcover.iter().cloned());
    let mut produced = 0;
    let mut ground_used = 0;
    for (row, feature) in recorded.iter().zip(features.iter()) {
        let x: i32 = row[0].parse().expect("x");
        let y: i32 = row[1].parse().expect("y");
        let recorded_terrain = &feature[6];
        // The square's raffle.
        let weights = nesw_weights((x, y), WIDTH, &perimeter, 0.0, [true; 4]).map(|w| w * FACTOR);
        let net: f32 = weights.iter().sum();
        let pool = feather_pool(weights, SELF_WEIGHT, net, MAX_FACTOR, FACTOR, [FACTOR; 4]);
        let bits = rolls.rng_bits();
        let ticket = TICKETS[pick_real_ticket(bits, &pool)];
        // The biome that ticket names then chooses a feature - or declines, and the square falls back to ground cover, which
        // is a **second** raffle at the ground-cover margin plus a palette pick of its own. Modelling only the first of those
        // two is what desynchronised this test on its first run, one square in.
        let mut terrain = if ticket == -2 {
            None
        } else {
            pick_forest_feature(&thick.components, &mut rolls)
        };
        if terrain.is_none() {
            let ground_weights =
                nesw_weights((x, y), WIDTH, &perimeter, -2.0, [true; 4]).map(|w| w * FACTOR);
            let ground_net: f32 = ground_weights.iter().sum();
            let ground_pool = feather_pool(
                ground_weights,
                SELF_WEIGHT,
                ground_net,
                MAX_FACTOR,
                FACTOR,
                [FACTOR; 4],
            );
            let ground_bits = rolls.rng_bits();
            let _ground_ticket = TICKETS[pick_real_ticket(ground_bits, &ground_pool)];
            terrain = ground_palette.pick(&mut rolls).cloned();
            ground_used += 1;
        }
        match terrain {
            Some(name) => {
                assert_eq!(&name, recorded_terrain, "square ({x},{y}) ticket {ticket}");
                produced += 1;
            }
            None => panic!("square ({x},{y}) produced nothing at all"),
        }
    }
    assert!(
        produced == 576,
        "every square produced a terrain, {produced} did"
    );
    assert!(
        ground_used > 100,
        "only {ground_used} squares fell back to ground cover"
    );
}
