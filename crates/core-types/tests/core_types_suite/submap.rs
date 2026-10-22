//! A submap's tiles, and the trick that keeps most of them out of memory.
//!
//! Reference: `src/submap.h`. A submap is twelve by twelve squares and starts **uniform**: all 144 squares hold one terrain, and the array of per-square data has
//! not been allocated at all (`m` is a null `unique_ptr`, and `is_uniform()` is exactly `!m`).
//!
//! Two consequences, and the port had both of them wrong until these tests existed:
//!
//! * **writing anything allocates.** Every setter calls `ensure_nonuniform()` first, *before* it looks at the value - so writing the uniform value onto a square
//!   that already holds it still allocates all 144 squares. Uniformity is not "all the squares are alike"; it is "nothing has ever been written". The port
//!   skipped the allocation when the value matched, which is a nicer data structure and a different one.
//! * **the dense order is `x` outer, `y` inner.** `maptile_soa` holds `mdarray<T, point_sm_ms>`, and `mdarray_impl_2d` is `std::array<column_type, DimX>` with
//!   `column_type = std::array<T, DimY>` (`mdarray.h:45,64`) - so the flat index is `x * SEEY + y`, not the row-major `y * SIDEX + x`. The port stored the
//!   transpose, which nothing noticed because every read and write went through the same wrong index.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_core_types::submap::Submap;
use cdda_core_types::tiles::TilePage;

/// A terrain id to fill submaps with. The numbers are placeholders: what is being tested is the shape of the store, not the vocabulary of terrain.
const ROCK: u32 = 1;

#[test]
fn a_new_submap_is_a_solid_block_of_one_terrain_and_holds_no_array() {
    let submap = Submap::new(ROCK);
    assert!(submap.is_uniform(), "nothing has been written");
    for x in 0..12 {
        for y in 0..12 {
            assert_eq!(submap.terrain(x, y), ROCK, "({x},{y})");
        }
    }
}

#[test]
fn writing_the_value_it_already_had_still_allocates_the_array() {
    // The subtle one: `set_ter` calls `ensure_nonuniform()` before it compares anything, so this is a real state change even though the data did not change.
    let mut submap = Submap::new(ROCK);
    submap.set_terrain(3, 4, ROCK);
    assert!(
        !submap.is_uniform(),
        "the array exists now, whatever the value"
    );
    assert_eq!(submap.terrain(3, 4), ROCK);
}

#[test]
fn one_square_can_differ_from_the_other_143() {
    let mut submap = Submap::new(ROCK);
    submap.set_terrain(1, 2, ROCK + 1);
    assert_eq!(submap.terrain(1, 2), ROCK + 1);
    assert_eq!(submap.terrain(1, 3), ROCK, "the square beside it is not");
    assert_eq!(submap.terrain(2, 2), ROCK, "nor the one below");
}

#[allow(clippy::indexing_slicing)]
#[test]
fn a_pages_dense_array_is_stored_x_first_then_y() {
    // Transcribed from `mdarray_impl_2d`: the outer array is indexed by x. Setting (1,0) therefore lands twelve slots in, and setting (0,1) lands one slot in.
    let mut page = TilePage::new(ROCK);
    page.set(1, 0, ROCK + 1);
    let values: Vec<u32> = page.iter().collect();
    assert_eq!(values[12], ROCK + 1, "(1,0) is a whole column in");
    assert_eq!(values[1], ROCK, "(0,1) is not where (1,0) went");

    let mut other = TilePage::new(ROCK);
    other.set(0, 1, ROCK + 2);
    let values: Vec<u32> = other.iter().collect();
    assert_eq!(values[1], ROCK + 2, "(0,1) is one slot in");
}
