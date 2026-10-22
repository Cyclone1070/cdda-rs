//! The corner square's arithmetic, worked out by hand.
//!
//! At the chunk's corner the square is **zero** squares from the north and west edges, so both of those neighbours are
//! divided by one while the other two are divided by twenty-four. Working the numbers through gives a lopsided raffle,
//! and knowing its shape matters because the port and the reference disagree about this exact square.

#![allow(clippy::panic)]

use cdda_core_types::overmap::{feather_pool, nesw_weights};

const SIDE: i32 = 24;
const MARGIN: f32 = -2.0;
const FACTOR: f32 = 3.0;
const MAX_FACTOR: f32 = 4.0;
const SELF_WEIGHT: f32 = 3.0;

fn corner_pool(depth_offset: f32) -> [f32; 6] {
    // A flat perimeter at depth five, and the neighbours this square actually has: a road to the north and open
    // ground to the south, neither of which owns a biome, and forest to the east and west, both of which do.
    let perimeter = vec![5_i32; 96];
    let owns_biome = [false, true, false, true];
    let neighbour_factors = [0.0, 3.0, 0.0, 3.0];
    let weights = nesw_weights((0, 0), SIDE, &perimeter, depth_offset, owns_biome)
        .map(|weight| weight * FACTOR);
    let net: f32 = weights.iter().sum();
    feather_pool(
        weights,
        SELF_WEIGHT,
        net,
        MAX_FACTOR,
        FACTOR,
        neighbour_factors,
    )
}

#[test]
fn the_neighbour_over_the_edge_outweighs_its_neighbours_but_not_the_empty_ticket() {
    let pool = corner_pool(0.0);
    // North is at distance zero, so its weight is the perimeter depth itself: five times the scaling factor.
    assert!((pool[0] - 15.0).abs() < 1e-4, "north {}", pool[0]);
    // The forests to the east and west are divided by their own depth, as every biome-owning neighbour is, which leaves
    // them small beside the road that owns no biome at all.
    assert!(pool[1] < 0.2, "east {}", pool[1]);
    assert!(pool[3] < 3.5, "west {}", pool[3]);
    // And the empty ticket - plain regional ground - is the largest of the six, at more than two thirds of the total.
    let total: f32 = pool.iter().map(|weight| weight.max(0.0)).sum();
    assert!(pool[5] / total > 0.6, "empty is {:.2}", pool[5] / total);
}

#[test]
fn the_ground_raffle_is_lopsided_the_same_way() {
    let pool = corner_pool(MARGIN);
    let total: f32 = pool.iter().map(|weight| weight.max(0.0)).sum();
    assert!(pool[5] / total > 0.6, "empty is {:.2}", pool[5] / total);
    assert!(
        pool[0] / total > 0.1,
        "the road still counts for {:.2}",
        pool[0] / total
    );
}

#[test]
fn the_corner_should_therefore_be_mostly_plain_ground() {
    // Which is what the port produces - grass - and what the reference's chunk does **not** at the square the port
    // believes is the corner. Rather than bending the arithmetic to fit, this test records that the arithmetic says
    // plain ground here, so the disagreement is more likely about **which square is the corner** than about the rule.
    let pool = corner_pool(0.0);
    let total: f32 = pool.iter().map(|weight| weight.max(0.0)).sum();
    let self_share = pool[4] / total;
    assert!(
        self_share < 0.05,
        "the forest itself is only {self_share:.3} of the corner's raffle"
    );
}
