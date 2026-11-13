//! **`transform_line` transforms every square on the line between two points.**
//!
//! Reference: `f_transform_line` (`npctalk.cpp:4781-4796`) reads two positions from variables and calls `map::transform_line` (`map.cpp:4802-4811`), which walks
//! `line_to( first, second )` and applies the transform to each square it passes through - the same `ter_furn_transform` table `u_transform_radius` uses.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

#[test]
fn the_line_between_two_points_is_what_changes() {
    let mut world = World::new();
    cdda_sim::terrain_types::set_types(
        &mut world,
        [("t_dirt".to_owned(), 1), ("t_pit".to_owned(), 2)],
    );
    cdda_sim::terrain_transform::set_transforms(
        &mut world,
        [(
            "transform_pit".to_owned(),
            std::collections::BTreeMap::from([("t_dirt".to_owned(), "t_pit".to_owned())]),
        )],
    );
    for x in 0..6 {
        for y in 0..6 {
            cdda_sim::square::set_terrain(&mut world, (x, y, 0), 1);
        }
    }
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    // The two ends come from variables, which is how the data writes them.
    let holder = cdda_sim::dialogue_values::holder_for(&mut world, speaker, true);
    cdda_sim::dialogue_values::set(
        &mut world,
        holder,
        "line_start",
        cdda_sim::dialogue_values::Value::Position { x: 0, y: 0, z: 0 },
    );
    cdda_sim::dialogue_values::set(
        &mut world,
        holder,
        "line_end",
        cdda_sim::dialogue_values::Value::Position { x: 4, y: 0, z: 0 },
    );

    let text = r#"{ "transform_line": "transform_pit",
                    "first": { "global_val": "line_start" },
                    "second": { "global_val": "line_end" } }"#;
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let effect = cdda_content::dialogue::read_effect(&value).expect("**the port reads `transform_line`**");
    let applied = cdda_sim::dialogue_effects::run_effects(&mut world, speaker, None, std::slice::from_ref(&effect));
    assert_eq!(applied, cdda_sim::dialogue_effects::Applied::Done, "the line runs");

    // The reference's `line_to` **excludes** the first square and includes the last, so the walk is 1..=4 along x.
    for x in 1..=4 {
        assert_eq!(
            cdda_sim::square::terrain_at(&world, (x, 0, 0)),
            Some(2),
            "**the square at x = {x} on the line is a pit now**"
        );
    }
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 0, 0)),
        Some(1),
        "the first square is where the line starts, not one of the squares it passes through"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (5, 0, 0)),
        Some(1),
        "and the squares past the second end are untouched"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (2, 1, 0)),
        Some(1),
        "as is the dirt one square off the line"
    );
}
