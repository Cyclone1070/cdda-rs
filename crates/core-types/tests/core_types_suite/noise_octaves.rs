//! Layering noise: octaves, persistence and the stretch into a range.

#![allow(clippy::panic)]

use cdda_core_types::noise::{Octaves, octave_noise_3d, scaled_octave_noise_3d};

#[test]
fn one_octave_is_the_raw_field_at_the_scale() {
    // A field that reports its own x lets the frequency be read straight off the answer.
    let raw = |x: f32, _y: f32, _z: f32| x;
    let layers = Octaves {
        octaves: 1,
        persistence: 0.5,
        scale: 0.03,
    };
    let value = octave_noise_3d(layers, 100.0, 0.0, 0.0, raw);
    assert!((value - 3.0).abs() < 1e-5, "100 scaled by 0.03: {value}");
}

#[test]
fn each_octave_doubles_the_frequency_and_halves_the_amplitude() {
    // Doubling the frequency and halving the amplitude cancel exactly for a field that reports its own x, so every
    // octave contributes the same 3.0 - my first expectation forgot that and said 3.0, 1.5, 0.75. Three of them are
    // added and divided by the amplitudes added (1 + 0.5 + 0.25).
    let raw = |x: f32, _y: f32, _z: f32| x;
    let layers = Octaves {
        octaves: 3,
        persistence: 0.5,
        scale: 0.03,
    };
    let value = octave_noise_3d(layers, 100.0, 0.0, 0.0, raw);
    let expected = (3.0 + 3.0 + 3.0) / 1.75;
    assert!(
        (value - expected).abs() < 1e-4,
        "{value} against {expected}"
    );
}

#[test]
fn the_sum_is_divided_by_the_amplitude_added_not_by_the_octave_count() {
    // With persistence 1 the amplitudes added are the octave count, so both readings agree - which is why the
    // distinguishing case needs a persistence below one.
    let raw = |_x: f32, _y: f32, _z: f32| 1.0;
    let layers = Octaves {
        octaves: 4,
        persistence: 0.5,
        scale: 1.0,
    };
    let value = octave_noise_3d(layers, 0.0, 0.0, 0.0, raw);
    // 1 + 0.5 + 0.25 + 0.125 added, and 1.875 added as amplitudes: exactly one.
    assert!(
        (value - 1.0).abs() < 1e-5,
        "a constant field comes back constant: {value}"
    );
}

#[test]
fn the_stretch_maps_the_raw_range_onto_the_bounds() {
    let high = |_x: f32, _y: f32, _z: f32| 1.0;
    let low = |_x: f32, _y: f32, _z: f32| -1.0;
    let layers = Octaves {
        octaves: 1,
        persistence: 0.5,
        scale: 1.0,
    };
    assert!((scaled_octave_noise_3d(layers, 0.0, 1.0, 0.0, 0.0, 1.0, high) - 1.0).abs() < 1e-5);
    assert!((scaled_octave_noise_3d(layers, 0.0, 1.0, 0.0, 0.0, 1.0, low) - 0.0).abs() < 1e-5);
    // And a middle value lands in the middle.
    let middle = |_x: f32, _y: f32, _z: f32| 0.0;
    assert!((scaled_octave_noise_3d(layers, 0.0, 1.0, 0.0, 0.0, 1.0, middle) - 0.5).abs() < 1e-5);
}

#[test]
fn no_octaves_means_no_noise_rather_than_a_division_by_zero() {
    let layers = Octaves {
        octaves: 0,
        persistence: 0.5,
        scale: 0.03,
    };
    assert_eq!(
        octave_noise_3d(layers, 1.0, 2.0, 3.0, |_x, _y, _z| 1.0),
        0.0
    );
}

#[test]
fn the_forest_layers_are_the_frequencies_the_reference_uses() {
    assert_eq!(Octaves::FOREST_BROAD.octaves, 4);
    assert!((Octaves::FOREST_BROAD.scale - 0.03).abs() < 1e-7);
    assert_eq!(Octaves::FOREST_DETAIL.octaves, 6);
    assert!((Octaves::FOREST_DETAIL.scale - 0.07).abs() < 1e-7);
    assert!((Octaves::FOREST_BROAD.persistence - 0.5).abs() < 1e-7);
}
