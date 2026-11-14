//! Which squares a spell covers: the blast and the cone.
//!
//! Reference: `spell_effect::spell_effect_area` (`magic_spell_effect.cpp:229-296`) and `spell_effect_cone_range_override` (`:253-295`). The shape is not decoration - 15 of
//! the content's 245 spells are cones, and a blast's square reaches ground a cone never covers, so a port that treats them alike over-reaches.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

/// Ground that lets everything through, so a gate can isolate the shape's own arithmetic.
fn open(_square: (i32, i32, i32)) -> bool {
    true
}

/// Ground that is solid from `wall_x` eastwards at `y`.
fn walled(wall_x: i32) -> impl Fn((i32, i32, i32)) -> bool {
    move |square: (i32, i32, i32)| square.0 < wall_x
}

#[test]
fn a_blast_covers_a_square_and_a_cone_does_not() {
    let blast = cdda_sim::spell_area::cover("blast", (0, 0, 0), (5, 0, 0), 0, 5, &open, false);
    assert_eq!(
        blast.len(),
        1,
        "an area of zero is the square aimed at alone"
    );

    let cone = cdda_sim::spell_area::cover("cone", (0, 0, 0), (5, 0, 0), 0, 5, &open, false);
    assert!(
        cone.contains(&(5, 0, 0)),
        "**a cone reaches the square it was aimed at**"
    );
    assert!(
        !cone.contains(&(0, 0, 0)),
        "and never its own caster, which the reference erases by hand"
    );
    assert!(
        !cone.contains(&(5, 5, 0)),
        "**while a square blast of the same radius would have covered it** - which is the over-reach this module exists to remove"
    );
}

/// **A cone stops at a wall**, and a blast does not.
///
/// Reference: `magic_spell_effect.cpp:275-292`: each candidate's line from the source is walked only while the ground is passable, and the walk breaks on the first
/// square that is not. The blast never asks the map at all.
#[test]
fn a_cone_stops_at_a_wall_where_a_blast_carries_on() {
    let stone = walled(3);
    let cone = cdda_sim::spell_area::cover("cone", (0, 0, 0), (5, 0, 0), 0, 5, &stone, false);
    assert!(
        cone.contains(&(2, 0, 0)),
        "the squares before the wall are covered"
    );
    assert!(
        !cone.contains(&(3, 0, 0)) && !cone.contains(&(5, 0, 0)),
        "**and nothing at or past the wall is**"
    );
    let blast = cdda_sim::spell_area::cover("blast", (0, 0, 0), (5, 0, 0), 0, 5, &stone, false);
    assert!(
        blast.contains(&(5, 0, 0)),
        "while a blast of area zero never looks at the ground"
    );
}
