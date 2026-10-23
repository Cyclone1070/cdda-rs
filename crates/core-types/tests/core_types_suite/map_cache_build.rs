//! The four caches a level needs, built in the order they depend on each other.
//!
//! Reference: `map::build_map_cache( zlev, skip_lightmap )` (`src/map.cpp:9549-9569`), whose loop is short and whose order is not arbitrary:
//!
//! ```text
//! for every level:      outside cache
//!                       transparency cache      <- reads the outside cache for the weather
//!                       floor cache             <- its dirty flag feeds the seen cache
//! for every level:      vehicle caching         <- a separate pass, because it writes the floor of the level above
//! for every level:      vision transparency     <- copies the transparency cache, so it must be last of the three
//! ```
//!
//! The reference says why each of these sits where it does. The vehicle pass is separate because "it changes the caches on neighbour z-levels (e.g. `floor_cache`);
//! otherwise such changes might be overwritten by main cache-building logic", and the vision cache copies the transparency cache, so anything that writes transparency
//! after it would be invisible to vision.
//!
//! Getting the order wrong does not crash. It produces a level that is dimmed by weather it never sees, or a closed car door that blocks light and not sight - which is why
//! the order is pinned by tests rather than by a comment.

#![allow(clippy::expect_used)]
#![allow(clippy::indexing_slicing)]

use cdda_core_types::light::TRANSPARENCY_OPEN_AIR;
use cdda_core_types::map_cache_build::{LevelCaches, LevelFacts, VehiclePartCaching, build_level};
use cdda_core_types::outside_cache::OutsideFacts;
use cdda_core_types::transparency::{FieldOpacity, SquareFacts};

const SIDE: usize = 5;

/// A level of open air under the sky, with no vehicles and nothing translucent.
fn open_level(sight_penalty: f32) -> LevelFacts {
    LevelFacts {
        side: SIDE,
        sight_penalty,
        outside: OutsideFacts {
            side: SIDE,
            indoors: vec![false; SIDE * SIDE],
        },
        squares: vec![
            SquareFacts {
                terrain_transparent: true,
                furniture_transparent: true,
                outside: true,
                fields: Vec::new(),
            };
            SIDE * SIDE
        ],
        no_floor: vec![false; SIDE * SIDE],
        sun_roof_below: vec![false; SIDE * SIDE],
        translucent: vec![false; SIDE * SIDE],
        coverage: vec![0; SIDE * SIDE],
        player: None,
        hiding: false,
        vehicles: Vec::new(),
    }
}

#[track_caller]
fn assert_close(passed: f32, expected: f32, what: &str) {
    assert!(
        (passed - expected).abs() < 1e-6,
        "{what}: got {passed}, expected {expected}"
    );
}

#[test]
fn a_rainy_open_square_is_dimmed_and_a_dry_indoor_one_is_not() {
    // The outside cache is built **first** so that the transparency cache can read it (`map.cpp:9556-9557`), which is how rain costs light before it reaches the floor.
    let mut facts = open_level(0.8);
    // One indoor square, stamped by the outside cache's own three-square rule.
    facts.outside.indoors[2 * SIDE + 2] = true;
    facts.squares[2 * SIDE + 2].outside = false;
    facts.squares[SIDE + 1].outside = false; // within the stamp, so it is indoors too
    let caches: LevelCaches = build_level(0, &facts, &mut [true; SIDE * SIDE]);

    assert_close(
        caches.transparency[0].value,
        TRANSPARENCY_OPEN_AIR * 0.8,
        "the open square is rained on",
    );
    assert_close(
        caches.transparency[2 * SIDE + 2].value,
        TRANSPARENCY_OPEN_AIR,
        "the indoor square is not",
    );
}

#[test]
fn below_ground_every_square_is_dry_because_every_square_is_inside() {
    // `build_outside_cache` fills `false` below ground (`map.cpp:9276-9280`), so the weather never reaches a basement - even though the caller's square facts may still claim
    // the sky is up there.
    let mut facts = open_level(0.8);
    for square in &mut facts.squares {
        square.outside = false;
    }
    let caches = build_level(-1, &facts, &mut [true; SIDE * SIDE]);
    assert!(caches.outside.iter().all(|sky| !*sky), "all indoors");
    assert_close(
        caches.transparency[0].value,
        TRANSPARENCY_OPEN_AIR,
        "so no weather penalty anywhere",
    );
}

#[test]
fn a_boardable_car_part_gives_a_floor_here_and_a_roof_above() {
    // The vehicle pass runs **after** the per-level loop because it writes the level above's floor cache (`vehicle_caching_internal_above`, `map.cpp:9516-9527`). A port that
    // ran it inside the loop would see the roof overwritten by the next level's own floor build.
    let mut facts = open_level(1.0);
    facts.no_floor[2 * SIDE + 2] = true;
    facts.vehicles.push(VehiclePartCaching {
        at: (2, 2),
        opaque: false,
        openable_open: false,
        boardable: true,
        inside: false,
        roof_or_opaque_above: true,
    });
    let mut above_floor = [true; SIDE * SIDE];
    let caches = build_level(0, &facts, &mut above_floor);

    assert!(caches.floor.cells[2 * SIDE + 2], "the deck is a floor");
    assert!(
        above_floor[2 * SIDE + 2],
        "and the roof over it is a floor for the level above"
    );
}

#[test]
fn a_closed_car_door_blocks_light_and_sight_alike() {
    // The vision cache is built **last** because it copies the transparency cache (`map.cpp:222-223`). A vehicle part that blocks light must therefore also be opaque to
    // vision - otherwise a closed car door would be see-through.
    let mut facts = open_level(1.0);
    facts.vehicles.push(VehiclePartCaching {
        at: (3, 3),
        opaque: true,
        openable_open: false,
        boardable: false,
        inside: true,
        roof_or_opaque_above: false,
    });
    let caches = build_level(0, &facts, &mut [true; SIDE * SIDE]);

    assert_close(
        caches.transparency[3 * SIDE + 3].value,
        0.0,
        "solid to light",
    );
    assert_close(caches.vision[3 * SIDE + 3], 0.0, "and solid to sight");
    assert_close(
        caches.transparency[0].value,
        TRANSPARENCY_OPEN_AIR,
        "the rest of the level is untouched",
    );
}

#[test]
fn an_open_car_door_is_opaque_to_neither() {
    // `if( dpart < 0 || !part( dpart ).open )` (`map.cpp:9499-9504`): an openable part with its door open is not opaque, and the vehicle stops counting as an obstacle for
    // both caches.
    let mut facts = open_level(1.0);
    facts.vehicles.push(VehiclePartCaching {
        at: (3, 3),
        opaque: true,
        openable_open: true,
        boardable: false,
        inside: false,
        roof_or_opaque_above: false,
    });
    let caches = build_level(0, &facts, &mut [true; SIDE * SIDE]);
    assert_close(
        caches.transparency[3 * SIDE + 3].value,
        TRANSPARENCY_OPEN_AIR,
        "light gets in through the open door",
    );
    assert_close(
        caches.vision[3 * SIDE + 3],
        TRANSPARENCY_OPEN_AIR,
        "and so does sight",
    );
}

#[test]
fn a_translucent_tile_stays_solid_to_sight_even_when_a_vehicle_changes_its_light() {
    // The two caches are not the same map, and the vehicle pass only touches the light one. Vision is built from the final light transparency and then applies its own rule,
    // so a frosted window in a car is still frosted.
    let mut facts = open_level(1.0);
    facts.translucent[SIDE + 1] = true;
    facts.vehicles.push(VehiclePartCaching {
        at: (1, 1),
        opaque: true,
        openable_open: false,
        boardable: false,
        inside: true,
        roof_or_opaque_above: false,
    });
    // The window's light is written after the transparency pass but before vision: the vehicle makes the square solid, and the translucent flag is applied on top.
    facts.squares[SIDE + 1].terrain_transparent = true;
    let caches = build_level(0, &facts, &mut [true; SIDE * SIDE]);
    assert_close(caches.vision[SIDE + 1], 0.0, "solid to sight");
}

#[test]
fn smoke_dims_the_vision_cache_too_but_not_the_second_answer() {
    // Worth pinning because it is counter-intuitive: the vision cache copies the **with-fields** transparency (`map.cpp:222-223`), so smoke does dim sight. What smoke does
    // *not* do is make the square count as invisible - that is the without-fields answer, which the seen cache uses and which this join leaves untouched.
    let mut facts = open_level(1.0);
    facts.squares[2 * SIDE + 2].fields.push(FieldOpacity {
        transparent: false,
        translucency: 0.25,
    });
    let caches = build_level(0, &facts, &mut [true; SIDE * SIDE]);
    let smoked = caches.transparency[2 * SIDE + 2];
    assert_close(smoked.value, TRANSPARENCY_OPEN_AIR * 0.25, "dim");
    assert_close(
        smoked.without_fields,
        TRANSPARENCY_OPEN_AIR,
        "still visible",
    );
    assert_close(
        caches.vision[2 * SIDE + 2],
        TRANSPARENCY_OPEN_AIR * 0.25,
        "and sight is dimmed by the smoke, because it copies the with-fields value",
    );
}
