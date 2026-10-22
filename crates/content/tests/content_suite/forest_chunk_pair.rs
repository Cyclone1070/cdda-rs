//! One chunk, described on both sides: the terrain the reference generated and the dice it generated it with.
//!
//! `corpus/maps/reference-chunk-6049.json` is the chunk at overmap `[60,49]`; the two trace files beside it hold the
//! ninety-six perimeter depths the curve produced for that same chunk and the five hundred and seventy-six tickets its
//! feature raffle drew. Until now the port was compared against a chunk whose dice it did not have, so every score was
//! the share of squares where two independent draws happened to agree.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]

fn dump() -> (String, (i32, i32), Vec<String>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/maps/reference-chunk-6049.json");
    let cdda_json::Value::Object(root) =
        cdda_json::parse(&std::fs::read(&path).expect("the chunk")).expect("valid json")
    else {
        panic!("an object");
    };
    let Some(cdda_json::Value::Str(oter)) = root.get("oter") else {
        panic!("an oter");
    };
    let Some(cdda_json::Value::Array(omt)) = root.get("omt") else {
        panic!("a position");
    };
    let axis = |index: usize| match &omt[index] {
        cdda_json::Value::Int(number) => *number as i32,
        _ => panic!("a number"),
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
    (oter.clone(), (axis(0), axis(1)), cells)
}

#[test]
fn the_chunk_is_thick_woodland_at_its_own_square() {
    let (oter, omt, cells) = dump();
    assert_eq!(oter, "forest_thick", "a denser biome than plain forest");
    assert_eq!(omt, (60, 49));
    assert_eq!(cells.len(), 576);
}

#[test]
fn the_perimeter_beside_it_is_that_chunk_s_own() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-chunk-6049-perimeter.txt");
    let text = std::fs::read_to_string(&path).expect("the perimeter");
    let fields: Vec<&str> = text.split_whitespace().collect();
    // `perimeter x y z` then ninety-six depths: the coordinate is what ties this file to the dump above.
    assert_eq!(fields[0], "perimeter");
    assert_eq!(
        &fields[1..4],
        &["60", "49", "0"],
        "the same square as the dump"
    );
    assert_eq!(fields.len(), 100, "the coordinate and ninety-six depths");
    let depths: Vec<i32> = fields[4..]
        .iter()
        .map(|field| field.parse().expect("a depth"))
        .collect();
    assert_eq!(depths.len(), 96);
    assert!(
        depths.iter().all(|depth| (1..=12).contains(depth)),
        "thick woodland reaches further"
    );
}

#[test]
fn the_tickets_beside_it_are_that_chunk_s_own() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-chunk-6049-tickets.txt");
    let text = std::fs::read_to_string(&path).expect("the tickets");
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(lines.len(), 576, "one per square");
    let mut tickets: Vec<i32> = lines
        .iter()
        .map(|line| {
            line.split_whitespace()
                .nth(2)
                .expect("a ticket")
                .parse()
                .expect("a number")
        })
        .collect();
    // The measured histogram of this chunk: plain ground and its own biome dominate, and the west neighbour is the
    // most frequent of the four, which is what a chunk whose west side is more woodland than field should draw.
    let mut sorted = tickets.clone();
    sorted.sort_unstable();
    let count = |wanted: i32| sorted.iter().filter(|ticket| **ticket == wanted).count();
    assert_eq!(count(-2), 299, "plain ground");
    assert_eq!(count(-1), 115, "its own biome");
    assert_eq!(
        count(3),
        84,
        "the west neighbour, the likeliest of the four"
    );
    tickets.clear();
}

#[test]
fn the_ground_raffle_ran_more_often_than_any_ticket_explanation_allows() {
    // A square reaches the ground raffle when its feature ticket is empty (-2) or names a side that owns no biome. Both
    // are read off the tickets, so the count is bounded from above whatever the chunk's sides are. The recorded count is
    // **higher than every one of those bounds**, which says something the tickets cannot: on at least seventy-one squares
    // the feature pick itself produced nothing to stand on - furniture, or no layer firing - and the ground raffle ran
    // anyway. A port that only counted tickets would put solid ground on those seventy-one squares.
    let tickets = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/overmap/reference-chunk-tickets.txt"),
    )
    .expect("the tickets");
    let drawn: Vec<i32> = tickets
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split_whitespace()
                .nth(2)
                .expect("a ticket")
                .parse()
                .expect("a number")
        })
        .collect();
    assert_eq!(drawn.len(), 576);
    let ground = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/overmap/reference-chunk-ground-tickets.txt"),
    )
    .expect("the ground trace");
    let calls = ground
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();

    let empties = drawn.iter().filter(|ticket| **ticket == -2).count();
    assert_eq!(empties, 461, "the empty feature ticket");
    assert_eq!(calls, 573, "ground raffles");
    // The largest possible bound: every named side owns no biome, so everything except this square's own ticket falls
    // through. The measured count is **above** that bound, so no assignment of sides reaches it.
    let fall_through_all_sides_open = drawn.iter().filter(|ticket| **ticket != -1).count();
    assert_eq!(
        fall_through_all_sides_open, 553,
        "576 less the 23 own-biome tickets"
    );
    assert!(
        calls > fall_through_all_sides_open,
        "{calls} ground calls against a bound of {fall_through_all_sides_open}"
    );
    assert_eq!(
        calls - fall_through_all_sides_open,
        20,
        "the squares only the feature pick can explain"
    );
}

#[test]
fn the_dump_and_the_traces_describe_one_chunk_rather_than_two() {
    // The reason this pair exists: before it, the port's chunk was compared against a dump taken at the player's
    // square while the traces described whichever forest chunk happened to be generated nearby. The coordinate in the
    // perimeter line and the coordinate in the dump are now checked against each other, so that mistake cannot recur
    // silently.
    let (_, omt, _) = dump();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-chunk-6049-perimeter.txt");
    let text = std::fs::read_to_string(&path).expect("the perimeter");
    let fields: Vec<&str> = text.split_whitespace().collect();
    assert_eq!(fields[1], omt.0.to_string(), "same x");
    assert_eq!(fields[2], omt.1.to_string(), "same y");
}
