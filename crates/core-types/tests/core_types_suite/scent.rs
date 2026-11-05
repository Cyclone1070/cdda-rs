//! Scent spreading and fading.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::scent::{ScentCell, decay, diffuse_step};

fn grid(side: usize) -> Vec<ScentCell> {
    vec![ScentCell::default(); side * side]
}

#[test]
fn a_smelly_square_hands_scent_to_its_neighbours_and_keeps_most_of_its_own() {
    let side = 5;
    let mut cells = grid(side);
    if let Some(centre) = cells.get_mut(2 * side + 2) {
        centre.value = 1000;
    }
    diffuse_step(&mut cells, side);
    let centre = cells.get(2 * side + 2).map_or(0, |c| c.value);
    let neighbour = cells.get(2 * side + 3).map_or(0, |c| c.value);
    assert!(centre > 0, "the source keeps scent");
    assert!(neighbour > 0, "and passes some on");
    assert!(
        centre > neighbour,
        "but keeps more than it gives: {centre} against {neighbour}"
    );
}

#[test]
fn a_square_that_blocks_scent_is_emptied_and_passes_nothing_on() {
    let side = 5;
    let mut cells = grid(side);
    if let Some(centre) = cells.get_mut(2 * side + 2) {
        centre.value = 1000;
    }
    if let Some(wall) = cells.get_mut(2 * side + 3) {
        wall.blocks = true;
        wall.value = 500;
    }
    diffuse_step(&mut cells, side);
    assert_eq!(cells.get(2 * side + 3).map(|c| c.value), Some(0), "emptied");
    assert_eq!(
        cells.get(2 * side + 4).map(|c| c.value),
        Some(0),
        "and the square behind it got nothing"
    );
}

#[test]
fn decay_takes_one_point_and_stops_at_zero() {
    let mut cells = vec![
        ScentCell {
            value: 10,
            ..ScentCell::default()
        },
        ScentCell {
            value: 0,
            ..ScentCell::default()
        },
    ];
    decay(&mut cells);
    assert_eq!(cells.first().map(|c| c.value), Some(9));
    assert_eq!(cells.get(1).map(|c| c.value), Some(0), "never negative");
}
