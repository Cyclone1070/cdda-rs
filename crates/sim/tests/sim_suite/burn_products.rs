//! What a fire leaves behind when something loses weight but survives.
//!
//! Transcribed from `map::create_burnproducts` (`src/map_field.cpp:116-143`).

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_sim::burn_products;

fn wood() -> Vec<(String, f64)> {
    vec![("ash".to_owned(), 0.018)]
}

fn ten_gram_ash(_id: &str) -> i64 {
    10
}

#[test]
fn a_thousand_grams_of_wood_leaves_one_ash() {
    // floor(0.018 * 1000 / 10) = floor(1.8) = 1
    let left = burn_products(&wood(), 1000, 1, ten_gram_ash, "wood");
    assert_eq!(left, vec![("ash".to_owned(), 1)]);
}

#[test]
fn a_little_wood_leaves_nothing_at_all() {
    // floor(0.018 * 100 / 10) = floor(0.18) = 0
    assert!(burn_products(&wood(), 100, 1, ten_gram_ash, "wood").is_empty());
}

#[test]
fn an_item_never_leaves_a_copy_of_itself() {
    let itself = vec![("wood".to_owned(), 0.5)];
    assert!(
        burn_products(&itself, 1000, 1, ten_gram_ash, "wood").is_empty(),
        "the reference skips this to avoid spawning forever"
    );
}

#[test]
fn the_product_is_shared_between_the_materials_of_the_item() {
    // Half the item is wood, half is something else: the wood gets half the burned weight.
    let left = burn_products(&wood(), 1000, 2, ten_gram_ash, "plank");
    assert!(left.is_empty(), "half of 1.8 is 0.9, under one");
}

#[test]
fn a_product_that_weighs_nothing_is_not_spawned() {
    let left = burn_products(&wood(), 1000, 1, |_| 0, "wood");
    assert!(left.is_empty(), "no weight, no numerator");
}
