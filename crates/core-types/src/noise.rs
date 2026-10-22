//! Layered noise: the shape the reference builds on top of its simplex field.
//!
//! Reference: `simplexnoise.cpp` - `octave_noise_3d` and `scaled_octave_noise_3d`. The rule is small and exact:
//! each octave asks the raw field at **double the frequency** and adds it at **half the amplitude**, and the sum is
//! divided by the amplitude that was added so the answer stays in the raw field's own range. Then the whole thing is
//! scaled into the bounds the caller wants.
//!
//! The raw simplex field itself comes next; it is passed in here so this arithmetic can be tested on its own, with a
//! field whose answers are known exactly.

/// How the reference layers its noise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Octaves {
    /// How many layers are added together.
    pub octaves: i32,
    /// How much each layer is quieter than the one before.
    pub persistence: f32,
    /// The frequency of the first layer.
    pub scale: f32,
}

impl Octaves {
    /// The reference's forest layer: four octaves, persistence a half, frequency three hundredths.
    pub const FOREST_BROAD: Self = Self {
        octaves: 4,
        persistence: 0.5,
        scale: 0.03,
    };

    /// The reference's detail layer: six octaves, persistence a half, frequency seven hundredths.
    pub const FOREST_DETAIL: Self = Self {
        octaves: 6,
        persistence: 0.5,
        scale: 0.07,
    };
}

/// Adds the octaves together, normalised back into the raw field's range.
///
/// Reference: `octave_noise_3d`. The sum is divided by the total amplitude added, not by the octave count, which is
/// what keeps a field of any octave count in `[-1, 1]`.
#[must_use]
pub fn octave_noise_3d(
    layers: Octaves,
    x: f32,
    y: f32,
    z: f32,
    raw: impl Fn(f32, f32, f32) -> f32,
) -> f32 {
    let mut total = 0.0_f32;
    let mut frequency = layers.scale;
    let mut amplitude = 1.0_f32;
    let mut added = 0.0_f32;
    for _ in 0..layers.octaves {
        total += raw(x * frequency, y * frequency, z * frequency) * amplitude;
        frequency *= 2.0;
        added += amplitude;
        amplitude *= layers.persistence;
    }
    if added == 0.0 {
        return 0.0;
    }
    total / added
}

/// The octaves, stretched into a range of the caller's choosing.
///
/// Reference: `scaled_octave_noise_3d`. With bounds of zero and one this is `value / 2 + 0.5`: the raw field's
/// `[-1, 1]` becomes `[0, 1]`, which is the form the forest threshold is written against.
#[must_use]
pub fn scaled_octave_noise_3d(
    layers: Octaves,
    low: f32,
    high: f32,
    x: f32,
    y: f32,
    z: f32,
    raw: impl Fn(f32, f32, f32) -> f32,
) -> f32 {
    octave_noise_3d(layers, x, y, z, raw) * (high - low) / 2.0 + (high + low) / 2.0
}
