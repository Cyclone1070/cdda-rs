//! The reference's own die, put through the port's six-ticket raffle.
//!
//! Each square of the chunk records **which die of the whole run it spent** and **which ticket that die bought**. The port
//! recomputes the six weights from the perimeter, so this test ignores the reference's weights entirely: it takes the die,
//! runs the port's raffle, and asks whether the same ticket comes out. That is the first comparison in this project where a
//! generated decision is reproduced from the reference's own randomness rather than matched in aggregate.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::float_cmp
)]

use cdda_core_types::overmap::{feather_pool, nesw_weights, pick_real_ticket};

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

/// The ticket values the reference's pool holds, in the order it adds them.
const TICKETS: [i32; 6] = [0, 1, 2, 3, -1, -2];

#[test]
fn the_reference_s_own_die_buys_the_same_ticket_out_of_the_port_s_pool() {
    let perimeter: Vec<i32> = lines("reference-chunk-pool-perimeter.txt")[0][4..]
        .iter()
        .map(|field| field.parse().expect("a depth"))
        .collect();
    let rows = lines("reference-chunk-pool-dice.txt");
    assert_eq!(rows.len(), 576);
    let mut agreed = 0;
    for row in &rows {
        let x: i32 = row[0].parse().expect("x");
        let y: i32 = row[1].parse().expect("y");
        let recorded_ticket: i32 = row[2].parse().expect("a ticket");
        let bits: u32 = row[3].parse().expect("a die");
        let weights = nesw_weights((x, y), WIDTH, &perimeter, 0.0, [true; 4]).map(|w| w * FACTOR);
        let net: f32 = weights.iter().sum();
        let pool = feather_pool(weights, SELF_WEIGHT, net, MAX_FACTOR, FACTOR, [FACTOR; 4]);
        let index = pick_real_ticket(bits, &pool);
        let ticket = TICKETS[index];
        assert_eq!(
            ticket, recorded_ticket,
            "square ({x},{y}) spent die {bits} and bought {recorded_ticket}, the port buys {ticket} from {pool:?}"
        );
        agreed += 1;
    }
    assert_eq!(agreed, 576);
}

#[test]
fn the_chunk_uses_every_direction_and_never_the_empty_ticket() {
    // A property of the data that makes the test above meaningful: in a chunk surrounded by dense forest on all eight sides
    // the empty ticket's weight is zero, so all 576 squares land on a real direction or on this square's own biome - and
    // they land on all four directions, not just one.
    let rows = lines("reference-chunk-pool-dice.txt");
    let mut counts = std::collections::BTreeMap::new();
    for row in &rows {
        *counts.entry(row[2].clone()).or_insert(0_usize) += 1;
    }
    assert_eq!(
        counts.get("-2"),
        None,
        "no square fell through to the region's ground here"
    );
    for direction in ["0", "1", "2", "3"] {
        assert!(
            counts.get(direction).copied().unwrap_or(0) > 30,
            "direction {direction}: {counts:?}"
        );
    }
}
