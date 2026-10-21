//! What the port would draw, square by square.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_sim::{ItemTypeId, SquareAppearance, drop_item, render_frame};

const FLOOR: u32 = 1;
const WALL: u32 = 2;
const TABLE: u32 = 3;

fn appearance(terrain: u32) -> SquareAppearance {
    match terrain {
        1 => SquareAppearance {
            glyph: '.',
            furniture: None,
        },
        2 => SquareAppearance {
            glyph: '#',
            furniture: None,
        },
        _ => SquareAppearance {
            glyph: '.',
            furniture: Some('t'),
        },
    }
}

fn level() -> Level {
    let mut level = Level::new();
    for y in 0..5 {
        for x in 0..5 {
            let terrain = if x == 0 || y == 0 { WALL } else { FLOOR };
            level.set_terrain(x, y, terrain, FLOOR);
        }
    }
    level.set_terrain(2, 2, TABLE, FLOOR);
    level
}

#[test]
fn the_frame_covers_every_square_of_the_view() {
    let mut world = World::new();
    let squares = render_frame(&mut world, &level(), (2, 2), 2, appearance, |_, _| 4.0);
    assert_eq!(squares.len(), 25, "a five by five view");
}

#[test]
fn the_glyph_is_the_terrain_hex_or_the_furniture_when_one_stands_there() {
    let mut world = World::new();
    let squares = render_frame(&mut world, &level(), (2, 2), 2, appearance, |_, _| 4.0);
    let at = |x: i32, y: i32| {
        squares
            .iter()
            .find(|square| square.x == x && square.y == y)
            .map(|square| square.glyph)
            .expect("the square is in the frame")
    };
    assert_eq!(at(0, 0), '#', "a wall");
    assert_eq!(at(1, 1), '.', "a floor");
    assert_eq!(at(2, 2), 't', "furniture covers the floor");
}

#[test]
fn something_lying_on_a_square_covers_the_furniture() {
    let mut world = World::new();
    drop_item(&mut world, (2, 2), ItemTypeId(7), 1);
    let squares = render_frame(&mut world, &level(), (2, 2), 2, appearance, |_, _| 4.0);
    let here = squares
        .iter()
        .find(|square| square.x == 2 && square.y == 2)
        .expect("the centre");
    assert_eq!(here.items, 1, "one item, counted");
    assert_eq!(here.glyph, '*', "and it hides the table");
}

#[test]
fn light_is_reported_as_the_lighting_system_gave_it() {
    let mut world = World::new();
    let squares = render_frame(&mut world, &level(), (2, 2), 2, appearance, |x, y| {
        if (x, y) == (2, 2) { 10.0 } else { 0.0 }
    });
    let here = squares
        .iter()
        .find(|square| square.x == 2 && square.y == 2)
        .expect("the centre");
    assert!(
        (here.light - 10.0).abs() < f32::EPSILON,
        "the centre is lit"
    );
    let away = squares
        .iter()
        .find(|square| square.x == 0 && square.y == 0)
        .expect("a corner");
    assert!(away.light.abs() < f32::EPSILON, "the corner is not");
}

#[test]
fn the_squares_come_in_reading_order_so_two_frames_can_be_compared_row_by_row() {
    let mut world = World::new();
    let squares = render_frame(&mut world, &level(), (2, 2), 2, appearance, |_, _| 4.0);
    let first = squares.first().expect("a first square");
    let second = squares.get(1).expect("a second square");
    assert_eq!((first.x, first.y), (0, 0), "top left first");
    assert_eq!(second.x, first.x + 1, "then rightwards");
    let row_start = squares.get(5).expect("the sixth square");
    assert_eq!(
        (row_start.x, row_start.y),
        (0, 1),
        "and the next row starts below"
    );
}
