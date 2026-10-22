//! Clustering a turn's noises before any monster reads them.
//!
//! Reference: `cluster_sounds` (`src/sounds.cpp:365-431`). The reference clusters because a big fight with many monsters and many noises
//! would otherwise cost the square of the two, and it seeds the clusters **randomly**: it draws a seed sound, removes it by swapping the
//! last into its place, and repeats. Both halves matter - the dice make the result reproducible only if the port draws in the same order,
//! and the swap makes the removal order part of the answer.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss
)]

use cdda_core_types::rng::Rolls;
use cdda_core_types::sounds::{SoundCategory, cluster_sounds};

fn noise(square: (i32, i32), volume: i32, category: SoundCategory) -> ((i32, i32), i32, bool) {
    (square, volume, category.is_provocative())
}

#[test]
fn a_handful_of_noises_is_never_clustered() {
    // `num_seed_clusters` is `max( min( n, 10 ), log( n ) )` and every seeded cluster is one noise: up to **ten** noises, nothing is
    // merged at all, and each comes out exactly where it was made. A port that clustered every list would pass a smoke test and give
    // monsters subtly different information on every ordinary turn.
    let mut rolls = Rolls::new(11);
    let noises = [
        noise((3, 4), 10, SoundCategory::Combat),
        noise((9, 9), 20, SoundCategory::Speech),
    ];
    let clusters = cluster_sounds(&mut rolls, &noises);
    assert_eq!(clusters.len(), 2, "two noises, two clusters");
    // A single-noise cluster is its noise exactly, so these are the values that went in - but the port keeps the reference's `float`
    // arithmetic, so the comparison is written with a margin rather than as bit equality.
    let near = |cluster: &cdda_core_types::sounds::Centroid, x: f32, y: f32, volume: f32| {
        (cluster.x - x).abs() < f32::EPSILON
            && (cluster.y - y).abs() < f32::EPSILON
            && (cluster.volume - volume).abs() < f32::EPSILON
    };
    assert!(clusters.iter().any(|cluster| near(cluster, 3.0, 4.0, 10.0)));
    assert!(clusters.iter().any(|cluster| near(cluster, 9.0, 9.0, 20.0)));
}

#[test]
fn more_than_ten_noises_are_clustered_down_to_ten() {
    // Twelve noises give ten seeds, so two of them are folded into the nearest centroid. That is the only case the reference has to
    // think about, and the count is what a monster's hearing is built from.
    let mut rolls = Rolls::new(5);
    let noises: Vec<((i32, i32), i32, bool)> = (0..12)
        .map(|index| noise((index, 0), 10, SoundCategory::Combat))
        .collect();
    let clusters = cluster_sounds(&mut rolls, &noises);
    assert_eq!(clusters.len(), 10, "ten seeds, two folded in");
    let total_weight: f32 = clusters.iter().map(|cluster| cluster.weight).sum();
    assert!(
        (total_weight - 120.0).abs() < 0.01,
        "the weight is conserved: {total_weight}"
    );
}

#[test]
fn a_folded_noise_keeps_the_louder_volume_and_provokes_if_either_did() {
    // A centroid takes the **larger** volume rather than the sum - a monster hears the loudest thing in the cluster - while its weight is
    // the sum, which is what the position is averaged by. Provocation is kept if **any** noise in it provoked.
    let mut rolls = Rolls::new(3);
    let mut noises: Vec<((i32, i32), i32, bool)> = (0..10)
        .map(|index| noise((index * 10, 0), 5, SoundCategory::Combat))
        .collect();
    noises.push(noise((0, 0), 40, SoundCategory::Order));
    noises.push(noise((0, 0), 7, SoundCategory::Combat));
    let clusters = cluster_sounds(&mut rolls, &noises);
    assert!(
        clusters.iter().any(|cluster| cluster.volume >= 40.0),
        "the loud one survives"
    );
    // Ten noises of five, one of forty and one of seven: 97. My first version of this test said 96 and the code said 97 - the code was
    // right, I had mis-added a fixture I had just written.
    let combined: f32 = clusters.iter().map(|cluster| cluster.weight).sum();
    assert!((combined - 97.0).abs() < 0.01, "weight {combined}");
    assert!(
        clusters.iter().any(|cluster| cluster.provokes),
        "an order in the mix provokes"
    );
}

#[test]
fn the_same_seed_clusters_the_same_way() {
    let noises: Vec<((i32, i32), i32, bool)> = (0..15)
        .map(|index| noise((index * 3, index), 6, SoundCategory::Combat))
        .collect();
    let mut first = Rolls::new(2024);
    let mut second = Rolls::new(2024);
    assert_eq!(
        cluster_sounds(&mut first, &noises),
        cluster_sounds(&mut second, &noises)
    );
}

#[test]
fn nothing_recorded_is_nothing_to_cluster() {
    let mut rolls = Rolls::new(1);
    assert!(cluster_sounds(&mut rolls, &[]).is_empty());
}
