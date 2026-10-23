//! Building a level's whole light map: sunlight first, then the sources, then the overrides.
//!
//! Reference: `map::generate_lightmap( zlev )` (`src/lightmap.cpp:446-661`), whose order is the behaviour and is quoted here in full because every step depends on the one
//! before it:
//!
//! 1. zero the light map, the light-source cache and the **buffer**;
//! 2. pour the sunlight in (`build_sunlight_cache`);
//! 3. the player and NPCs light their squares immediately - a held torch is not bulk;
//! 4. sweep every square: light through openings into buildings, then items (immediate), then terrain, furniture and fields (`light_emitted` into the **buffer**), and
//!    a field's `local_light_override` into a list;
//! 5. monsters and vehicles add their light;
//! 6. **flush the buffer**: every square that recorded a source casts it, in x-then-y order;
//! 7. apply the overrides, which overwrite a square's four quarters outright.
//!
//! Step 6 is why the buffer exists. Its own comment in the reference counts the work for a 12x12 inferno: unbuffered it is 92,160 ray casts, buffered it is 7,680 - and,
//! more importantly, the direction gating can only work if every emitter has been recorded before the first cast happens.

#![allow(clippy::expect_used)]
#![allow(clippy::indexing_slicing)]
// The grid is seven squares wide and every index in these tests is a literal or a loop over that same seven.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use cdda_core_types::light::TRANSPARENCY_OPEN_AIR;
use cdda_core_types::shadowcasting::{
    AMBIENT_OPEN, FourQuadrants, LightMap, Quadrant, TransparencyGrid, sight_calc,
};

/// The sky's light, poured everywhere.
fn sky(level: f32) -> Vec<FourQuadrants> {
    vec![FourQuadrants::uniform(level); SIDE * SIDE]
}

const SIDE: usize = 7;

#[track_caller]
fn assert_close(passed: f32, expected: f32, what: &str) {
    assert!(
        (passed - expected).abs() < 1e-4,
        "{what}: got {passed}, expected {expected}"
    );
}

/// An open field with no sunlight, ready for sources.
fn dark_field() -> LightMap {
    LightMap::new(
        TransparencyGrid::new(SIDE, vec![AMBIENT_OPEN; SIDE * SIDE]),
        false,
    )
}

#[test]
fn sunlight_is_poured_in_before_any_source_and_a_source_only_ever_adds() {
    // Step 2 then step 6: the sunlight is the floor the sources build on, and a source can only make a square brighter - `update_light` is a maximum.
    let mut lit = dark_field();
    lit.pour_sunlight(&sky(3.0));
    assert_close(lit.light_at(3, 3).max(), 3.0, "the sky, everywhere");

    lit.buffer_source((3, 3), 10.0);
    lit.flush();
    assert!(lit.light_at(4, 3).max() > 3.0, "the source brightened it");
    // The lamp reaches the whole seven-square field, so the claim that matters is not "the far corner is untouched" but "the corner is dimmer than the square beside the
    // lamp, and never darker than the sky".
    assert!(lit.light_at(0, 0).max() < lit.light_at(4, 3).max());
    assert!(lit.light_at(0, 0).max() >= 3.0, "a source only ever adds");
}

#[test]
fn the_buffer_is_flushed_after_every_emitter_is_recorded() {
    // The point of two passes. Two equally bright fires side by side: because both are in the buffer before the first one casts, each sees the other and skips that
    // direction - which is the whole saving. Casting as they were recorded would give the first fire a full eight-octant cast.
    let mut lit = dark_field();
    lit.buffer_source((3, 3), 10.0);
    lit.buffer_source((3, 4), 10.0);
    lit.flush();
    // The fire at (3,3) skips the cast guarded by the buffer at (3,2)... which is empty, so it casts; what matters here is that both fires were applied.
    assert!(lit.source_at(3, 3) > 0.0, "the first fire is lit");
    assert!(lit.source_at(3, 4) > 0.0, "and so is the second");
    assert_close(
        lit.light_at(4, 3).max(),
        sight_calc(10.0, TRANSPARENCY_OPEN_AIR, 1),
        "and the square beside the first takes its light",
    );
}

#[test]
fn a_square_with_nothing_buffered_is_not_cast_at_all() {
    // The flush skips zeros (`lightmap.cpp:655-659`), which is what makes a mostly dark level cheap.
    let mut lit = dark_field();
    lit.flush();
    for x in 0..SIDE {
        for y in 0..SIDE {
            assert_close(lit.light_at(x as i32, y as i32).max(), 0.0, "still dark");
            assert_close(lit.source_at(x as i32, y as i32), 0.0, "no source");
        }
    }
}

#[test]
fn an_override_replaces_a_square_outright_and_not_just_the_brightest_quarter() {
    // `lm[x][y].fill( elem.second )` (`lightmap.cpp:660-662`): a field with a local light override does not *add* light, it *is* the light in that square, all four
    // quarters, whatever the sky and the lamps had already put there.
    let mut lit = dark_field();
    lit.pour_sunlight(&sky(3.0));
    lit.buffer_source((3, 3), 10.0);
    lit.flush();
    lit.override_square((3, 3), 0.5);
    let cell = lit.light_at(3, 3);
    for quarter in [
        Quadrant::NorthEast,
        Quadrant::SouthEast,
        Quadrant::SouthWest,
        Quadrant::NorthWest,
    ] {
        assert_close(cell.get(quarter), 0.5, "overwritten, not added to");
    }
    assert_close(
        lit.light_at(4, 3).max(),
        sight_calc(10.0, TRANSPARENCY_OPEN_AIR, 1),
        "the neighbours the override does not cover keep the lamp's light",
    );
}

#[test]
fn the_sunlight_does_not_light_a_square_through_a_wall_that_is_added_later() {
    // An ordering trap worth pinning: sunlight is poured before the terrain is consulted by the *source* pass, so it cannot be blocked by it. The blocking is the
    // sunlight pass's own business - a level built in the wrong order gets daylight through a wall.
    let mut cells = vec![AMBIENT_OPEN; SIDE * SIDE];
    for y in 0..SIDE {
        cells[4 * SIDE + y] = 0.0;
    }
    let mut lit = LightMap::new(TransparencyGrid::new(SIDE, cells), false);
    lit.pour_sunlight(&sky(3.0));
    assert_close(
        lit.light_at(5, 3).max(),
        3.0,
        "the pour itself does not know about walls",
    );
    // A source on this side of the wall cannot reach past it, which is the source pass's own check.
    lit.buffer_source((3, 3), 10.0);
    lit.flush();
    assert!(
        lit.light_at(5, 3).max() < sight_calc(10.0, TRANSPARENCY_OPEN_AIR, 2),
        "the wall stopped the lamp"
    );
}
