//! What the reference's **ground** raffle drew, and the answer to the corner.
//!
//! `corpus/overmap/reference-forest-ground-trace.txt` holds one line per square whose feature raffle produced no terrain:
//! `x y ticket`, logged from the ground raffle itself. It is shorter than the feature trace because it only runs where
//! the first raffle left a square without a feature.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

fn trace() -> Vec<(i32, i32, i32)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-forest-ground-trace.txt");
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
fn the_ground_raffle_runs_only_where_the_feature_raffle_left_nothing() {
    let trace = trace();
    // 1,138 calls against 17,856 squares, so it runs on one square in sixteen - the squares the feature raffle left
    // without a feature, across the two chunks of this world that are generated as forest.
    assert_eq!(trace.len(), 1138);
    let mut tickets: Vec<i32> = trace.iter().map(|(_, _, ticket)| *ticket).collect();
    tickets.sort_unstable();
    tickets.dedup();
    assert_eq!(tickets, vec![-2, -1, 0, 1, 2, 3], "the same six choices");
}

#[test]
fn plain_ground_is_the_most_common_answer_and_west_the_least() {
    let trace = trace();
    let count = |wanted: i32| {
        trace
            .iter()
            .filter(|(_, _, ticket)| *ticket == wanted)
            .count()
    };
    assert_eq!(count(-2), 807, "plain regional ground");
    assert_eq!(count(-1), 140, "this square's own biome");
    assert_eq!(count(3), 63, "the west neighbour");
    assert_eq!(count(0) + count(1) + count(2), 128, "and the other three");
}

#[test]
fn the_corner_of_the_measured_chunk_draws_its_west_neighbour() {
    // This is the answer four rounds went looking for. The ground trace holds **two** corners, because two chunks in
    // this world are generated as forest: the first draws plain ground at `(0,0)` and the second draws **west**. West is
    // woodland, so the corner takes the **west neighbour's own ground palette** - the forest palette, which prefers
    // forest floor ten to one - and that is why the reference's chunk has forest floor at the corner while the port,
    // drawing plain ground there instead, has grass.
    let trace = trace();
    let corners: Vec<(usize, i32)> = trace
        .iter()
        .enumerate()
        .filter(|(_, (x, y, _))| *x == 0 && *y == 0)
        .map(|(index, (_, _, ticket))| (index, *ticket))
        .collect();
    assert_eq!(corners.len(), 2, "two forest chunks in this world");
    assert_eq!(
        corners[0],
        (0, -2),
        "the first chunk's corner is plain ground"
    );
    assert_eq!(
        corners[1].1, 3,
        "the measured chunk's corner is its west neighbour"
    );
}
