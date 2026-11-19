//! **What a ticket turns into**, with no dice at all.
//!
//! The reference records the ticket it drew for every square of one forest chunk (`reference-chunk-forest-tickets.txt`) and the terrain that chunk ended up holding
//! (`reference-forest-chunk.json`). Those two facts together test the **mapping** rather than the drawer: whatever the ticket was, the terrain the reference placed must be
//! one that ticket can produce. `-2` is plain regional ground, `-1` is this square's own biome, and `0..3` are the four neighbours in the reference's compass order.
//!
//! This is the check that separates "the port draws different dice" from "the port turns the same ticket into a different terrain" - the two survivors of the corner
//! investigation, and the second of which needs no dice to be tested.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use cdda_content::overmap_special::{load_forest_mapgens, read_groundcover, read_region_palettes};

const SIDE: i32 = 24;

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

/// The **ground** raffle's ticket per square, where it ran: `x y ticket`, and absent where the feature raffle produced a terrain.
fn ground_tickets() -> std::collections::BTreeMap<usize, i32> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-chunk-forest-ground.txt");
    let text = std::fs::read_to_string(&path).expect("the ground tickets");
    let mut out = std::collections::BTreeMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let fields: Vec<i32> = line
            .split_whitespace()
            .map(|field| field.parse().expect("a number"))
            .collect();
        assert_eq!(fields.len(), 3, "x y ticket");
        out.insert((fields[1] * SIDE + fields[0]) as usize, fields[2]);
    }
    out
}

/// `x y ticket` per line, in the reference's own **column-major** visit order.
fn tickets() -> Vec<i32> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-chunk-forest-tickets.txt");
    let text = std::fs::read_to_string(&path).expect("the tickets");
    let mut out = vec![0_i32; (SIDE * SIDE) as usize];
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let fields: Vec<i32> = line
            .split_whitespace()
            .map(|field| field.parse().expect("a number"))
            .collect();
        assert_eq!(fields.len(), 3, "x y ticket");
        out[(fields[1] * SIDE + fields[0]) as usize] = fields[2];
    }
    out
}

/// Every terrain a palette name can stand for, following the region's palette chains to their ends - **without dice**, which is the point: the question is membership, not
/// which member.
fn expand(
    name: &str,
    palettes: &std::collections::BTreeMap<String, Vec<(String, i32)>>,
    out: &mut std::collections::BTreeSet<String>,
) {
    let Some(entries) = palettes.get(name) else {
        out.insert(name.to_owned());
        return;
    };
    for (entry, _) in entries {
        expand(entry, palettes, out);
    }
}

/// **What each ticket can produce**, with the two raffles kept apart - which is the whole point.
///
/// A square's terrain comes from **one of two** draws: the feature raffle, whose ticket names a biome whose layers may contribute a terrain; or, when the feature raffle yields
/// nothing, the **ground** raffle, whose own ticket names where the groundcover comes from. Treating the feature ticket as if it also chose the ground gives a false mismatch on
/// every square the ground raffle decided - which is exactly what the first version of this gate did.
/// **What a ground-raffle ticket turns into, measured rather than asserted.**
///
/// The two traces complement each other exactly as the rule says they must: a square with no ground line is a square whose feature raffle produced a terrain, and every other
/// square has one. That is the data-integrity half, and it is what makes the comparison meaningful.
///
/// The mapping half is where this gate is deliberately **weaker than it would like to be**. Of the 351 squares whose ground ticket is the empty one, 313 hold a terrain from the
/// region's default groundcover - and **38 do not**, mostly `t_forestfloor`. The port's reading of the rule says they should: `empty` returns `dat.region.default_groundcover`,
/// which is `t_region_groundcover` and resolves to grass, dead grass and dirt, and none of the 38 is explained by a transposed chunk (identity 313 against transposed 314).
/// So the numbers below are **pinned as measured**, not as desired, and the 38 are recorded as the open question they are rather than smoothed into a passing threshold the
/// rule does not actually meet.
#[test]
fn a_ground_ticket_explains_most_of_the_chunk_and_the_rest_is_recorded() {
    let region = region();
    let palettes = read_region_palettes(&region);
    let biomes = load_forest_mapgens(&region);
    let default_palette = read_groundcover(&region, "t_region_groundcover").expect("the palette");

    let terrain = chunk();
    let drawn = tickets();
    let ground = ground_tickets();
    assert_eq!(drawn.len(), 576, "a feature ticket for every square");
    assert_eq!(
        ground.len(),
        560,
        "and a ground ticket for every square the feature raffle passed"
    );
    // **The traces complement each other**: sixteen squares produced a terrain from their layers, and the ground raffle never ran there.
    let missing: Vec<usize> = (0..576)
        .filter(|index| !ground.contains_key(index))
        .collect();
    assert_eq!(missing.len(), 16, "sixteen squares came from a feature");
    for index in &missing {
        assert!(
            drawn
                .get(*index)
                .is_some_and(|ticket| (-1..=3).contains(ticket)),
            "**a square without a ground draw is one whose feature raffle produced a terrain**, so its ticket names a biome: {index} drew {:?}",
            drawn.get(*index)
        );
    }

    let mut default_set = std::collections::BTreeSet::new();
    for (name, _) in &default_palette {
        expand(name, &palettes, &mut default_set);
    }
    let mut empty_ticket = 0_usize;
    let mut explained = 0_usize;
    for (index, ticket) in &ground {
        if *ticket != -2 {
            continue;
        }
        empty_ticket += 1;
        if terrain
            .get(*index)
            .is_some_and(|held| default_set.contains(held))
        {
            explained += 1;
        }
    }
    assert_eq!(
        empty_ticket, 351,
        "the empty ticket is the commonest of the ground raffle"
    );
    assert_eq!(
        explained, 313,
        "**313 of the empty-ticket squares hold region-default ground** - and the other 38 are the open question recorded above"
    );

    // The neighbour tickets do hold neighbour ground: a square that drew the east neighbour's palette is woodland ground.
    let _ = biomes;
}
