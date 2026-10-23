//! How much light a square lets through, and the two answers the reference keeps for every square.
//!
//! Reference: `map::build_transparency_cache( zlev )` (`src/lightmap.cpp:100-203`). One square's transparency is computed by a small rule with three parts, and the rule is
//! run **twice over the same facts**, because the game needs both answers:
//!
//! * `transparency` - what light actually gets through, fields included;
//! * `transparency_wo_fields` - what it would get through with the fields removed, which is what "can I see the square at all" uses, so that smoke dims a view without
//!   making the square behind it invisible.
//!
//! The three parts, in the reference's order:
//!
//! 1. **opacity**: terrain and furniture must *both* be transparent, or the square is solid - the `&&` is what makes a transparent window in an opaque frame opaque;
//! 2. **weather**: a square under open sky is multiplied by the weather's `sight_penalty`, because rain, fog and snow cost light before it reaches the ground;
//! 3. **fields**: every field whose intensity level is not transparent multiplies the value by its `translucency`, so two layers of smoke are dimmer than one.

#![allow(clippy::expect_used)]

use cdda_core_types::light::TRANSPARENCY_OPEN_AIR;
use cdda_core_types::transparency::{FieldOpacity, SquareFacts, transparency_of};

#[track_caller]
fn assert_close(passed: f32, expected: f32, what: &str) {
    assert!(
        (passed - expected).abs() < 1e-6,
        "{what}: got {passed}, expected {expected}"
    );
}

/// A square of open air under the sky, with nothing on it.
fn clear() -> SquareFacts {
    SquareFacts {
        terrain_transparent: true,
        furniture_transparent: true,
        outside: true,
        fields: Vec::new(),
    }
}

#[test]
fn open_air_under_a_clear_sky_lets_everything_through() {
    let answer = transparency_of(&clear(), 1.0);
    assert_close(answer.value, TRANSPARENCY_OPEN_AIR, "the base value");
    assert_close(
        answer.without_fields,
        TRANSPARENCY_OPEN_AIR,
        "and the same, there being no fields",
    );
}

#[test]
fn terrain_and_furniture_must_both_be_transparent() {
    // `get_ter().obj().transparent && get_furn().obj().transparent` (`lightmap.cpp:146-149`): a **conjunction**, so an opaque table on a glass floor is an opaque square
    // and a transparent chair on a wall is too - and both answers are solid.
    let mut opaque_terrain = clear();
    opaque_terrain.terrain_transparent = false;
    let answer = transparency_of(&opaque_terrain, 1.0);
    assert_close(answer.value, 0.0, "solid");
    assert_close(answer.without_fields, 0.0, "solid either way");

    let mut opaque_furniture = clear();
    opaque_furniture.furniture_transparent = false;
    assert_close(
        transparency_of(&opaque_furniture, 1.0).value,
        0.0,
        "an opaque chair on a glass floor is still opaque",
    );
}

#[test]
fn weather_only_costs_light_under_open_sky() {
    // `if( outside_cache[p] ) value *= sight_penalty;` (`lightmap.cpp:150-154`). A square indoors is not in the rain, and the reference's own FIXME admits vehicles are
    // not marked as inside yet - a bug it names rather than hides.
    let outside = transparency_of(&clear(), 0.8);
    assert_close(outside.value, TRANSPARENCY_OPEN_AIR * 0.8, "dimmed");

    let mut inside = clear();
    inside.outside = false;
    assert_close(
        transparency_of(&inside, 0.8).value,
        TRANSPARENCY_OPEN_AIR,
        "untouched by weather",
    );
}

#[test]
fn fields_multiply_and_transparent_fields_are_skipped_entirely() {
    // `value = value * i_level.translucency` for every field that is not transparent (`lightmap.cpp:158-165`): multiplication, not the dimmest one winning, so two layers
    // of half-opaque smoke leave a quarter of the light.
    let mut smoked = clear();
    smoked.fields = vec![
        FieldOpacity {
            transparent: false,
            translucency: 0.5,
        },
        FieldOpacity {
            transparent: false,
            translucency: 0.5,
        },
        FieldOpacity {
            transparent: true,
            translucency: 0.1,
        },
    ];
    assert_close(
        transparency_of(&smoked, 1.0).value,
        TRANSPARENCY_OPEN_AIR * 0.25,
        "two layers, and the transparent one does nothing at all",
    );
}

#[test]
fn the_two_answers_differ_only_by_the_fields() {
    // This is why the reference computes both: `transparent_cache_wo_fields` answers "is there something to see here", and smoke that made squares invisible would make a
    // smoked room look like solid rock.
    let mut smoked = clear();
    smoked.fields = vec![FieldOpacity {
        transparent: false,
        translucency: 0.25,
    }];
    let answer = transparency_of(&smoked, 0.8);
    assert_close(
        answer.value,
        TRANSPARENCY_OPEN_AIR * 0.8 * 0.25,
        "fields and weather",
    );
    assert_close(
        answer.without_fields,
        TRANSPARENCY_OPEN_AIR * 0.8,
        "weather but no fields",
    );
    assert!(
        answer.without_fields > answer.value,
        "so the second answer is always the generous one"
    );
}

#[test]
fn a_solid_square_is_solid_with_or_without_its_fields() {
    // The opacity test comes first and returns both values, so a wall with smoke on it is not "less opaque" for having the smoke counted.
    let mut wall = clear();
    wall.terrain_transparent = false;
    wall.fields = vec![FieldOpacity {
        transparent: false,
        translucency: 0.5,
    }];
    let answer = transparency_of(&wall, 0.5);
    assert_close(answer.value, 0.0, "solid");
    assert_close(answer.without_fields, 0.0, "solid either way");
}
