//! The six weights of a forest square's feature raffle, as the reference computed them.
//!
//! Each line is `x y ticket north east south west self empty`: the square, the ticket the draw landed on, and the six
//! weights the pool was built from. With the weights on disk the port's arithmetic can be compared square by square
//! **without any dice involved**, which is the only kind of comparison that can say "the rules agree" rather than "two
//! draws agreed".

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

struct Square {
    ticket: i32,
    pool: [f32; 6],
}

fn trace() -> Vec<Square> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-forest-pools-trace.txt");
    let text = std::fs::read_to_string(&path).expect("the trace");
    let mut squares = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let fields: Vec<f32> = line
            .split_whitespace()
            .map(|field| field.parse().expect("a number"))
            .collect();
        assert_eq!(fields.len(), 9, "two coordinates, a ticket and six weights");
        let mut pool = [0.0_f32; 6];
        pool.copy_from_slice(&fields[3..9]);
        squares.push(Square {
            ticket: fields[2] as i32,
            pool,
        });
    }
    squares
}

#[test]
fn every_square_of_the_chunk_has_its_six_weights() {
    let squares = trace();
    assert_eq!(squares.len(), 576, "one line per square");
    let mut histograms = std::collections::BTreeMap::new();
    for square in &squares {
        *histograms.entry(square.ticket).or_insert(0) += 1;
    }
    // Measured: the empty ticket takes the largest share, this square's own biome the second, and the four neighbours
    // divide the rest - the shape a chunk of dense woodland should have. The counts are pinned as measured rather than
    // as expected: 306 empties and 118 of the square's own biome, which is a share I would have guessed too low.
    assert_eq!(histograms.get(&-2), Some(&306));
    assert_eq!(histograms.get(&-1), Some(&118));
    assert!(histograms.get(&-2) > histograms.get(&-1));
}

#[test]
fn the_pool_the_reference_drew_from_never_has_a_negative_weight() {
    // A negative weight cannot be drawn and would make the accumulation walk backwards, so its absence is the first
    // thing to pin: it says the empty ticket's subtraction stays inside the total it is subtracted from.
    for square in trace() {
        for weight in square.pool {
            assert!(weight >= 0.0, "a negative weight in the pool: {weight}");
        }
    }
}

#[test]
fn the_ticket_drawn_is_one_that_carries_weight() {
    // The pool is walked by accumulated weight, so a draw lands on the first ticket whose running total reaches it -
    // which is never a zero-weight ticket. Every recorded ticket therefore has weight behind it, and a port that
    // produced a different pool would have to explain how the reference drew from a ticket it gave nothing.
    for square in trace() {
        assert!(
            square.pool[square.ticket_index()] > 0.0,
            "ticket {} carries no weight",
            square.ticket
        );
    }
}

impl Square {
    /// The pool slot a ticket names: the four neighbours and this square in order, then the empty ticket.
    fn ticket_index(&self) -> usize {
        match self.ticket {
            -2 => 5,
            -1 => 4,
            other => other as usize,
        }
    }
}

#[test]
fn the_chunk_beside_the_trace_is_dense_woodland() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/maps/reference-forest-pools.json");
    let cdda_json::Value::Object(root) =
        cdda_json::parse(&std::fs::read(&path).expect("the chunk")).expect("valid json")
    else {
        panic!("an object");
    };
    let Some(cdda_json::Value::Str(oter)) = root.get("oter") else {
        panic!("an oter");
    };
    assert_eq!(oter, "forest_thick");
    let perimeter = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/overmap/reference-forest-pools-perimeter.txt"),
    )
    .expect("the perimeter");
    let fields: Vec<&str> = perimeter.split_whitespace().collect();
    assert_eq!(
        &fields[1..4],
        &["60", "257", "0"],
        "the same square as the chunk"
    );
    assert_eq!(fields.len(), 100);
}
