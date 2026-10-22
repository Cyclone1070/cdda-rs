//! How strongly the neighbours pull on a forest square.

#![allow(clippy::panic, clippy::indexing_slicing)]

use cdda_core_types::overmap::{feather_pool, nesw_weights};

const SIDE: i32 = 24;

fn flat(perimeter_depth: i32) -> Vec<i32> {
    vec![perimeter_depth; 96]
}

#[test]
fn a_square_in_the_middle_is_pulled_by_its_two_nearest_sides() {
    // A 24-square chunk has no middle square: the square at (12,12) is 12 from the north and west edges and 11 from
    // the east and south. So the weights come in pairs rather than all four together, which is what the reference's
    // half-open distances do - my first version expected all four equal and the numbers said otherwise.
    let perimeter = flat(5);
    let weights = nesw_weights((12, 12), SIDE, &perimeter, 0.0, [true; 4]);
    assert!(
        (weights[0] - weights[3]).abs() < 1e-6,
        "north and west are equally far"
    );
    assert!(
        (weights[1] - weights[2]).abs() < 1e-6,
        "east and south are equally far"
    );
    assert!(
        weights[1] > weights[0],
        "and the nearer pair pulls harder: {weights:?}"
    );
}

#[test]
fn a_square_near_the_north_edge_is_pulled_most_by_the_north() {
    let perimeter = flat(5);
    let weights = nesw_weights((12, 2), SIDE, &perimeter, 0.0, [true; 4]);
    assert!(
        weights[0] > weights[2],
        "north {} against south {}",
        weights[0],
        weights[2]
    );
    assert!(weights[0] > weights[1], "and against east");
}

#[test]
fn a_neighbour_that_owns_a_biome_is_pulled_by_distance_alone() {
    // The weight is the perimeter depth divided by the distance and then **divided by that same depth**, so the depth
    // cancels and only the distance is left. That surprised me - I expected a deeper neighbour to reach further - and
    // it is worth pinning, because it means the perimeter curve only decides anything for neighbours that have **no**
    // biome of their own.
    let shallow = nesw_weights((12, 6), SIDE, &flat(1), 0.0, [true; 4]);
    let deep = nesw_weights((12, 6), SIDE, &flat(10), 0.0, [true; 4]);
    assert!(
        (deep[0] - shallow[0]).abs() < 1e-6,
        "the depth cancels: {} against {}",
        deep[0],
        shallow[0]
    );
    assert!(
        (deep[0] - 1.0 / 7.0).abs() < 1e-6,
        "and what is left is one over the distance plus one"
    );

    // Without a biome on that side the depth does not cancel, and there it matters.
    let shallow = nesw_weights((12, 6), SIDE, &flat(1), 0.0, [false, true, true, true]);
    let deep = nesw_weights((12, 6), SIDE, &flat(10), 0.0, [false, true, true, true]);
    assert!(
        deep[0] > shallow[0],
        "a deep biome-less neighbour pulls harder: {} against {}",
        deep[0],
        shallow[0]
    );
}

#[test]
fn a_neighbour_without_a_biome_is_not_divided_by_its_depth() {
    let perimeter = flat(4);
    let with = nesw_weights((12, 6), SIDE, &perimeter, 0.0, [true; 4]);
    let without = nesw_weights((12, 6), SIDE, &perimeter, 0.0, [false, true, true, true]);
    assert!(
        without[0] > with[0],
        "the biome-less north still counts, and without the division it counts more: {} against {}",
        without[0],
        with[0]
    );
}

#[test]
fn the_ground_cover_margin_moves_the_edge_only_where_there_is_no_biome() {
    // The margin steps the perimeter back by two squares before the ground cover is chosen. For a neighbour that owns
    // a biome it makes **no difference**, because the depth cancels out of the ratio either way - the second surprise
    // this rule produced, and the same one that made the curve matter only for biome-less neighbours.
    let perimeter = flat(6);
    let neighbour_biomes = [true; 4];
    let feature = nesw_weights((12, 6), SIDE, &perimeter, 0.0, neighbour_biomes);
    let ground = nesw_weights((12, 6), SIDE, &perimeter, -2.0, neighbour_biomes);
    assert!(
        (ground[0] - feature[0]).abs() < 1e-6,
        "the margin cancels for a biome: {} against {}",
        ground[0],
        feature[0]
    );

    let none = [false; 4];
    let feature = nesw_weights((12, 6), SIDE, &perimeter, 0.0, none);
    let ground = nesw_weights((12, 6), SIDE, &perimeter, -2.0, none);
    assert!(
        ground[0] < feature[0],
        "without a biome the margin does move it: {} against {}",
        ground[0],
        feature[0]
    );
}

#[test]
fn the_pool_has_six_tickets_with_this_square_holding_three() {
    let weights = [1.0_f32, 1.0, 1.0, 1.0];
    let pool = feather_pool(weights, 3.0, 4.0, 3.0, 1.0, [1.0; 4]);
    assert_eq!(pool.len(), 6);
    assert_eq!(
        &pool[..4],
        &weights,
        "the four neighbours keep their weights"
    );
    assert!(
        (pool[4] - 3.0).abs() < 1e-6,
        "this square's own biome holds three"
    );
}

#[test]
fn the_empty_ticket_is_what_the_neighbourhood_does_not_claim() {
    // The empty ticket is the ceiling for this neighbourhood minus what its neighbours and its own biome claim. My
    // first reading of it was backwards - I had the ticket falling as the neighbourhood filled up - and the numbers
    // said otherwise: with the ceiling above the per-direction densities, a **denser** neighbourhood leaves a
    // **bigger** empty ticket, because the ceiling grows with the total while the claims grow with the densities.
    let dense = [4.0_f32; 4];
    let thin = [0.5_f32; 4];
    let dense_pool = feather_pool(dense, 3.0, 16.0, 4.0, 3.0, [3.0; 4]);
    let thin_pool = feather_pool(thin, 3.0, 2.0, 4.0, 3.0, [3.0; 4]);
    assert!(
        (dense_pool[5] - 19.0).abs() < 1e-4,
        "dense: {} ",
        dense_pool[5]
    );
    assert!((thin_pool[5] - 5.0).abs() < 1e-4, "thin: {}", thin_pool[5]);
    // What decides plain ground is the ticket **against** the neighbours, and that ratio does fall as the
    // neighbourhood fills: 19 against 16 at the shared ceiling of 16, 5 against 2 at the shared ceiling of 2.
    assert!(
        dense_pool[5] / dense_pool[..4].iter().sum::<f32>()
            < thin_pool[5] / thin_pool[..4].iter().sum::<f32>(),
        "the empty ticket is a smaller share of a dense neighbourhood"
    );
}
