//! Light falling off as it travels, and meeting other light without adding.
//!
//! Source: `src/shadowcasting.h:92-116` - `sight_calc`, `sight_check`,
//! `accumulate_transparency`, `update_light_quadrants`.

#![allow(clippy::panic, clippy::float_cmp)]

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
