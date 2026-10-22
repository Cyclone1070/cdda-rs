//! Buildings beside streets: which square, and which way they face.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::overmap::{
    BUILDING_CHANCE, Direction, Overmap, building_site, plan_buildings,
};
use cdda_core_types::rng::Rolls;

#[test]
fn a_building_sits_one_square_off_the_street_and_faces_it() {
    let overmap = Overmap::filled("field");
    let (site, facing) =
        building_site(&overmap, (10, 10), Direction::North).expect("a free square");
    assert_eq!(site, (10, 9), "one square north of the street");
    assert_eq!(
        facing,
        Direction::South,
        "and it faces back towards the street"
    );
}

#[test]
fn the_facing_is_the_opposite_of_the_side_it_stands_on() {
    let overmap = Overmap::filled("field");
    for side in Direction::ALL {
        let (_, facing) = building_site(&overmap, (20, 20), side).expect("a free square");
        assert_eq!(facing, side.opposite(), "{side:?}");
    }
}

#[test]
fn a_square_that_is_not_free_gets_no_building() {
    let mut overmap = Overmap::filled("field");
    overmap.set_terrain(10, 9, "road");
    assert_eq!(building_site(&overmap, (10, 10), Direction::North), None);
    overmap.set_terrain(10, 9, "lake_surface");
    assert_eq!(building_site(&overmap, (10, 10), Direction::North), None);
}

#[test]
fn each_side_is_rolled_separately_and_one_in_four_goes_unbuilt() {
    let overmap = Overmap::filled("field");
    let streets = [(10, 10); 1];
    let mut rolls = Rolls::new(1);
    let mut rolls_again = Rolls::new(1);
    let first = plan_buildings(&overmap, &streets, Direction::East, &mut rolls);
    let second = plan_buildings(&overmap, &streets, Direction::East, &mut rolls_again);
    assert_eq!(first, second, "the same dice give the same buildings");
    assert_eq!(BUILDING_CHANCE, 4, "a quarter of the sides go unbuilt");
}

#[test]
fn both_sides_of_a_street_can_be_built_on() {
    let overmap = Overmap::filled("field");
    let streets: Vec<(i32, i32)> = (0..40).map(|x| (x + 5, 20)).collect();
    let mut rolls = Rolls::new(31337);
    let sites = plan_buildings(&overmap, &streets, Direction::East, &mut rolls);
    assert!(
        sites.len() > streets.len(),
        "a street has buildings on both sides: {}",
        sites.len()
    );
    let north = sites
        .iter()
        .filter(|(_, facing)| *facing == Direction::South)
        .count();
    let south = sites
        .iter()
        .filter(|(_, facing)| *facing == Direction::North)
        .count();
    assert!(
        north > 0 && south > 0,
        "{north} facing south, {south} facing north"
    );
}

#[test]
fn a_street_with_buildings_overwrites_neither_the_road_nor_its_neighbours() {
    let mut overmap = Overmap::filled("field");
    let streets: Vec<(i32, i32)> = (0..20).map(|x| (x + 5, 20)).collect();
    for (x, y) in &streets {
        overmap.set_terrain(*x, *y, "road_ew");
    }
    let mut rolls = Rolls::new(7);
    let sites = plan_buildings(&overmap, &streets, Direction::East, &mut rolls);
    for ((x, y), _) in &sites {
        assert!(
            !streets.contains(&(*x, *y)),
            "a building never stands on the road"
        );
    }
}
