//! Light falling off as it travels, and meeting other light without adding.
//!
//! Source: `src/shadowcasting.h:92-116` - `sight_calc`, `sight_check`,
//! `accumulate_transparency`, `update_light_quadrants`.

#![allow(clippy::panic, clippy::float_cmp, clippy::indexing_slicing)]

use cdda_core_types::shadowcasting::{
    FourQuadrants, Quadrant, accumulate_transparency, sight_calc, sight_check,
    update_light_quadrants,
};

#[test]
fn light_falls_off_as_the_exponential_of_the_distance() {
    // 1 / e^(t * d). Half the distance, twice the divisor's exponent - and no light to
    // speak of at the far end of a long corridor of dense air.
    let near = sight_calc(1.0, 0.031_944, 5);
    let far = sight_calc(1.0, 0.031_944, 10);
    assert!((near - 0.852_382_4).abs() < 1e-6, "near: {near}");
    assert!((far - 0.726_555_8).abs() < 1e-6, "far: {far}");
    assert!(near > far, "further is dimmer");
    // Clear air, and the falloff is barely visible over one tile.
    assert!((sight_calc(1.0, 0.031_944, 1) - 0.968_560_8).abs() < 1e-6);
}

#[test]
fn a_fresh_source_starts_at_its_full_strength() {
    // The numerator is the light arriving; the exponential is what the air takes. This is
    // the reference's own arithmetic, not an approximation of it.
    let full = sight_calc(10.0, 0.0, 1);
    assert!((full - 10.0).abs() < 1e-6, "no transparency, no loss");
}

#[test]
fn only_solid_stops_light_outright() {
    assert!(!sight_check(0.0), "solid");
    assert!(sight_check(0.001), "almost solid still lets light through");
    assert!(sight_check(1.0));
}

#[test]
fn a_ray_carries_the_average_transparency_it_has_crossed() {
    // The first cell is its own average; each further cell pulls the average toward
    // itself without replacing it, which is what keeps a wall of smoke from being as clear
    // as its clearest cell.
    assert_eq!(accumulate_transparency(0.0, 2.0, 1), 2.0);
    assert_eq!(accumulate_transparency(2.0, 0.0, 2), 1.0);
    assert_eq!(accumulate_transparency(1.0, 1.0, 5), 1.0);
    // Crossing one clear cell and one dense one averages to half, in either order.
    let mut average = accumulate_transparency(0.0, 2.0, 1);
    average = accumulate_transparency(average, 0.0, 2);
    assert_eq!(average, 1.0);
}

#[test]
fn the_brighter_ray_wins_a_corner_and_both_are_lit() {
    let mut light = FourQuadrants::uniform(0.0);
    update_light_quadrants(&mut light, 5.0, Quadrant::NorthEast);
    update_light_quadrants(&mut light, 3.0, Quadrant::NorthEast);
    assert_eq!(
        light.get(Quadrant::NorthEast),
        5.0,
        "not summed, not replaced by 3"
    );
    update_light_quadrants(&mut light, 7.0, Quadrant::SouthWest);
    assert_eq!(
        light.get(Quadrant::NorthEast),
        5.0,
        "the other corner is untouched"
    );
    assert_eq!(light.get(Quadrant::SouthWest), 7.0);
    assert_eq!(light.max(), 7.0);
}

#[test]
fn smoke_dims_what_is_behind_it() {
    // A band of half-transparent air between a lamp and a wall. The reference averages the
    // transparency along each ray, so the wall behind the smoke is dimmer than the same wall in
    // clear air - which is the whole reason the accumulation exists.
    use cdda_core_types::shadowcasting::{TransparencyGrid, cast_light_values};

    // Open air is 2.3/72 by the reference's own note, and smoke is 0.38 - *higher*, because the
    // value is attenuation per tile, not clarity. Solid is zero and special-cased as blocking,
    // which is why a lower number is not automatically darker.
    const OPEN_AIR: f32 = 2.3 / 72.0;
    const SMOKE: f32 = 0.38;
    let side = 11;
    let clear = TransparencyGrid::new(side, vec![OPEN_AIR; side * side]);
    let mut smoked_cells = vec![OPEN_AIR; side * side];
    for y in 0..side {
        smoked_cells[7 * side + y] = SMOKE;
    }
    let smoked = TransparencyGrid::new(side, smoked_cells);

    let clear_values = cast_light_values(&clear, (5, 5), 5, 10.0);
    let smoked_values = cast_light_values(&smoked, (5, 5), 5, 10.0);
    let behind = 9 * side + 5;
    assert!(clear_values[behind] > 0.0, "the wall is lit in clear air");
    assert!(
        smoked_values[behind] < clear_values[behind],
        "and dimmer through smoke: {} against {}",
        smoked_values[behind],
        clear_values[behind]
    );
    // The smoke does not make the tile in front of it darker, only what is behind.
    let in_front = 6 * side + 5;
    assert!((smoked_values[in_front] - clear_values[in_front]).abs() < 1e-6);
}
