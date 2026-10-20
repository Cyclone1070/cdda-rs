//! A level of the map: world coordinates in, submaps out.
//!
//! The reference divides this the same way, submaps holding tiles and the map holding
//! submaps. The arithmetic that matters is negative coordinates, where truncating
//! division and Euclidean division disagree.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use cdda_core_types::level::Level;
use cdda_core_types::submap::SUBMAP_SIDE;

const FLOOR: u32 = 1;
const WALL: u32 = 2;

fn blocks_light(terrain: u32) -> bool {
    terrain == WALL
}

#[test]
fn negative_coordinates_land_in_the_submap_west_and_north_of_them() {
    // Tile -1 is in submap -1, not submap 0: `-1 / 12` truncates to 0 in C and in a
    // naive port, which would put the world's north-west edge inside its middle.
    let mut level = Level::new();
    level.ensure(-1, -1, FLOOR);
    level.ensure(0, 0, FLOOR);
    let (west, cell_x, cell_y) = level.submap_at(-1, -1).expect("the submap exists");
    let (centre, _, _) = level.submap_at(0, 0).expect("the submap exists");
    assert_ne!(
        west, centre,
        "tile -1 belongs to a different submap than tile 0"
    );
    assert_eq!((cell_x, cell_y), (SUBMAP_SIDE - 1, SUBMAP_SIDE - 1));
}

#[test]
fn a_tile_can_be_written_and_read_back_across_a_boundary() {
    let mut level = Level::new();
    level.set_terrain(11, 0, WALL, FLOOR);
    level.set_terrain(12, 0, WALL, FLOOR);
    assert_eq!(level.terrain_at(11, 0), Some(WALL));
    assert_eq!(level.terrain_at(12, 0), Some(WALL));
    // The two walls are in different submaps, and the tiles between them are not walls.
    assert_eq!(level.terrain_at(10, 0), Some(FLOOR));
}

#[test]
fn an_unmade_coordinate_has_no_terrain() {
    let level = Level::new();
    assert_eq!(level.terrain_at(0, 0), None, "nothing has been generated");
}

#[test]
fn a_wall_in_the_next_submap_still_casts_a_shadow() {
    // Eight tiles east is still inside the second submap; the shadow has to cross the
    // boundary between them, which is the whole reason the caster works on world tiles
    // rather than on one submap at a time.
    let mut level = Level::new();
    // A wall at x = 12, which is in the second submap, with the light coming from
    // x = 10, which is in the first.
    for y in 0..12 {
        level.set_terrain(12, y, WALL, FLOOR);
    }
    let radius = 5;
    let origin = (10, 5);
    let rows = level.visibility(origin, radius, blocks_light);
    let at = |dx: i32, dy: i32| rows[(dy + radius) as usize][(dx + radius) as usize];
    // The origin does not light itself: the reference's walk starts one step out.
    assert!(!at(0, 0), "a tile does not see itself");
    assert!(at(0, 1), "the next tile over is visible");
    assert!(at(1, 0), "in front of the wall");
    assert!(!at(3, 0), "behind the wall, one submap over");
}

#[test]
fn light_does_not_pass_through_a_place_the_world_has_not_made() {
    let mut level = Level::new();
    level.ensure(0, 0, FLOOR);
    let radius = 3;
    let origin = (10, 5);
    let rows = level.visibility(origin, radius, blocks_light);
    let at = |dx: i32, dy: i32| rows[(dy + radius) as usize][(dx + radius) as usize];
    // The made submap ends at x = 11. Tile 12 was never made, so it is solid - and a
    // solid tile is still visible, the way a wall is. What is behind it is not.
    assert!(at(1, 0), "tile 11 is the last one the world has");
    assert!(at(2, 0), "the edge of the world is itself something to see");
    assert!(!at(3, 0), "nothing exists behind it to light");
}
