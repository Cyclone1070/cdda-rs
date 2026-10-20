//! A page of tile values that stays cheap while it is all one value.
//!
//! Reference: `submap` (`src/submap.h:74-130`). A submap holds a pointer that is
//! null while every cell is the same, and allocates the dense array on the first
//! write that makes it uniform no longer (`ensure_nonuniform`). That is the point
//! of it: a generated map is mostly copies of a handful of tiles, and paying for
//! `12 x 12` values per submap before anything has been placed is waste.

use cdda_core_types::tiles::TilePage;

const TILES: usize = 12;

#[test]
fn a_uniform_page_pays_for_nothing_until_it_is_written() {
    // A page of one value answers every coordinate and holds no dense storage.
    let page = TilePage::new(7);
    assert!(page.is_uniform());
    assert_eq!(page.dense_len(), 0, "no array is allocated yet");
    for y in 0..TILES {
        for x in 0..TILES {
            assert_eq!(page.get(x, y), 7);
        }
    }
}

#[test]
fn the_first_different_write_materialises_the_page() {
    let mut page = TilePage::new(7);
    page.set(3, 4, 9);
    assert!(
        !page.is_uniform(),
        "a different value ends the uniform state"
    );
    assert_eq!(page.dense_len(), TILES * TILES);
    assert_eq!(page.get(3, 4), 9);
    // Everything else kept the value it had, which is the part a naive
    // implementation gets wrong by materialising zeros.
    for y in 0..TILES {
        for x in 0..TILES {
            if (x, y) != (3, 4) {
                assert_eq!(page.get(x, y), 7, "cell {x},{y} should still be uniform");
            }
        }
    }
}

#[test]
fn writing_the_uniform_value_back_does_not_make_it_uniform_again() {
    // `ensure_nonuniform` is one-way in the reference: the array stays allocated
    // and the flag stays clear. Collapsing again would be a different structure
    // with different worst-case behaviour, so the port does not do it.
    let mut page = TilePage::new(7);
    page.set(0, 0, 9);
    page.set(0, 0, 7);
    assert!(!page.is_uniform());
    assert_eq!(page.dense_len(), TILES * TILES);
    assert_eq!(page.get(0, 0), 7);
}

#[allow(clippy::indexing_slicing)]
#[test]
fn cells_are_stored_row_by_row() {
    // The dense array is `ter[y][x]` in the reference, so walking it in order is
    // walking the rows; anything that iterates a submap relies on that.
    let mut page = TilePage::new(0);
    page.set(1, 0, 11);
    page.set(0, 1, 22);
    page.set(11, 11, 33);
    let walked: Vec<i32> = page.iter().collect();
    assert_eq!(walked.len(), TILES * TILES);
    assert_eq!(walked[1], 11);
    assert_eq!(walked[TILES], 22);
    assert_eq!(walked[TILES * TILES - 1], 33);
}
