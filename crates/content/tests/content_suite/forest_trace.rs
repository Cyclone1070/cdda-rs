//! What tickets the reference's forest generator actually drew, square by square.
//!
//! `corpus/overmap/reference-forest-trace.txt` is written by the oracle when `CDDA_MAP_TRACE` names a file: one line per
//! square, `x y ticket`, in the order the generator visited them. **The reference loops x outside and y inside**, so
//! the file is column-major even though a map dump is written row by row - a difference worth knowing before comparing
//! the two.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

fn trace() -> Vec<(i32, i32, i32)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-forest-trace.txt");
    let text = std::fs::read_to_string(&path).expect("the trace");
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut parts = line.split_whitespace();
            let mut next = || {
                parts
                    .next()
                    .expect("a field")
                    .parse::<i32>()
                    .expect("a number")
            };
            (next(), next(), next())
        })
        .collect()
}

#[test]
fn the_trace_is_a_whole_number_of_chunks() {
    let trace = trace();
    assert_eq!(trace.len() % 576, 0, "{} lines", trace.len());
    assert_eq!(trace.len() / 576, 31, "thirty-one chunks in this world");
}

#[test]
fn the_tickets_are_the_reference_s_six_choices() {
    // 0..4 are the four compass neighbours, -1 is this square's own biome and -2 is plain regional ground.
    let trace = trace();
    let mut tickets: Vec<i32> = trace.iter().map(|(_, _, ticket)| *ticket).collect();
    tickets.sort_unstable();
    tickets.dedup();
    assert_eq!(
        tickets,
        vec![-2, -1, 0, 1, 2, 3],
        "every ticket is one of the six"
    );
}

#[test]
fn the_first_chunk_is_mostly_plain_ground_and_a_little_forest() {
    let trace = trace();
    let chunk = &trace[..576];
    let count = |wanted: i32| {
        chunk
            .iter()
            .filter(|(_, _, ticket)| *ticket == wanted)
            .count()
    };
    // The histogram of the first chunk, measured from the file rather than assumed.
    assert_eq!(count(-2), 400, "plain ground, seven tenths of the chunk");
    assert_eq!(count(-1), 83, "this square's own biome");
    assert_eq!(
        count(0) + count(1) + count(2) + count(3),
        93,
        "and the four neighbours together"
    );
}

#[test]
fn the_generator_visits_columns_not_rows() {
    // The first twenty-four lines walk the north edge of one column: all of them share x = 0 and count y up from zero.
    let trace = trace();
    let first: Vec<(i32, i32)> = trace[..24].iter().map(|(x, y, _)| (*x, *y)).collect();
    assert!(first.iter().all(|(x, _)| *x == 0), "one column: {first:?}");
    assert_eq!(
        first.iter().map(|(_, y)| *y).collect::<Vec<i32>>(),
        (0..24).collect::<Vec<i32>>()
    );
}

#[test]
fn the_corner_draws_plain_ground_which_is_what_the_port_says_it_should() {
    // The port's own arithmetic at (0,0) also gives the empty ticket the largest share - three quarters - so the ticket
    // agrees. That is the useful part of this file: it says the disagreement at the corner is **not** in the feature
    // raffle, because both sides draw the same ticket there.
    let trace = trace();
    assert_eq!(trace[0], (0, 0, -2));
}
