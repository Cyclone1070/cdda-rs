#![allow(
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
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
    octave_noise_3d(layers, x, y, z, raw) * (high - low) / 2.0 + f32::midpoint(high, low)
}

// The 3D simplex field the reference layers its noise on.
//
// The names here are the reference's - x, y, z, i, j, k, gi0 to gi3 - and they are kept deliberately: a transcription
// that renames its variables cannot be checked against the code it came from.
//
// Reference: `simplexnoise.cpp` - `raw_noise_3d`, with the tables from `simplexnoise.h`. This is a transcription
// rather than a design: the skew, the unskew, the corner ordering and the four gradient dots are all the
// reference's, in its order, because a field that is merely similar produces a different world from the same seed.
//
// The one thing worth knowing before reading it: the "z" coordinate is the **seed** in the forest's case, so the
// field is two-dimensional in practice and the third axis is what makes two worlds different.

/// The twelve gradient vectors' midpoints, in the reference's order.
const GRAD3: [[f32; 3]; 12] = [
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0],
    [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0],
    [0.0, -1.0, 1.0],
    [0.0, 1.0, -1.0],
    [0.0, -1.0, -1.0],
];

/// The permutation table, twice over so a lookup can add up to 255 without wrapping.
const PERM: [usize; 512] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30, 69,
    142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219,
    203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
    74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122, 60, 211, 133, 230,
    220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25, 63, 161, 1, 216, 80, 73, 209, 76,
    132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173,
    186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212, 207, 206,
    59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213, 119, 248, 152, 2, 44, 154, 163,
    70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232,
    178, 185, 112, 104, 218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162,
    241, 81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157, 184, 84, 204,
    176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141,
    128, 195, 78, 66, 215, 61, 156, 180, 151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194,
    233, 7, 225, 140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234,
    75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174,
    20, 125, 136, 171, 168, 68, 175, 74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83,
    111, 229, 122, 60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25,
    63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188,
    159, 86, 164, 100, 109, 198, 173, 186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147,
    118, 126, 255, 82, 85, 212, 207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170,
    213, 119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39, 253,
    19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104, 218, 246, 97, 228, 251, 34, 242, 193,
    238, 210, 144, 12, 191, 179, 162, 241, 81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31,
    181, 199, 106, 157, 184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93,
    222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
];

/// The reference's floor, which truncates towards zero rather than towards negative infinity.
fn fast_floor(value: f32) -> i32 {
    let truncated = value as i32;
    if value < truncated as f32 {
        truncated - 1
    } else {
        truncated
    }
}

fn dot(gradient: [f32; 3], x: f32, y: f32, z: f32) -> f32 {
    gradient[0] * x + gradient[1] * y + gradient[2] * z
}

/// The raw 3D simplex field, in `[-1, 1]`.
///
/// Reference: `raw_noise_3d`. The answer is scaled by thirty-two at the end, which is the reference's own factor for
/// keeping the sum inside the range rather than a value derived here.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::indexing_slicing
)]
pub fn raw_noise_3d(x: f32, y: f32, z: f32) -> f32 {
    const F3: f32 = 1.0 / 3.0;
    const G3: f32 = 1.0 / 6.0;

    // Skew the input space to work out which simplex cell the point is in.
    let skew = (x + y + z) * F3;
    let i = fast_floor(x + skew);
    let j = fast_floor(y + skew);
    let k = fast_floor(z + skew);

    // Unskew the cell origin back into the input space.
    // Summed in floating point: the seed arrives on the third axis and is large enough that an integer sum of the
    // three cell coordinates would overflow. The reference's own arithmetic is the same product either way.
    let unskew = (i as f32 + j as f32 + k as f32) * G3;
    let x0 = x - (i as f32 - unskew);
    let y0 = y - (j as f32 - unskew);
    let z0 = z - (k as f32 - unskew);

    // Which of the six tetrahedra of the cell the point is in.
    let (i1, j1, k1, i2, j2, k2) = if x0 >= y0 {
        if y0 >= z0 {
            (1, 0, 0, 1, 1, 0)
        } else if x0 >= z0 {
            (1, 0, 0, 1, 0, 1)
        } else {
            (0, 0, 1, 1, 0, 1)
        }
    } else if y0 < z0 {
        (0, 0, 1, 0, 1, 1)
    } else if x0 < z0 {
        (0, 1, 0, 0, 1, 1)
    } else {
        (0, 1, 0, 1, 1, 0)
    };

    let x1 = x0 - i1 as f32 + G3;
    let y1 = y0 - j1 as f32 + G3;
    let z1 = z0 - k1 as f32 + G3;
    let x2 = x0 - i2 as f32 + 2.0 * G3;
    let y2 = y0 - j2 as f32 + 2.0 * G3;
    let z2 = z0 - k2 as f32 + 2.0 * G3;
    let x3 = x0 - 1.0 + 3.0 * G3;
    let y3 = y0 - 1.0 + 3.0 * G3;
    let z3 = z0 - 1.0 + 3.0 * G3;

    let ii = (i & 255) as usize;
    let jj = (j & 255) as usize;
    let kk = (k & 255) as usize;
    let gi0 = PERM[ii + PERM[jj + PERM[kk]]] % 12;
    let gi1 = PERM[ii + i1 as usize + PERM[jj + j1 as usize + PERM[kk + k1 as usize]]] % 12;
    let gi2 = PERM[ii + i2 as usize + PERM[jj + j2 as usize + PERM[kk + k2 as usize]]] % 12;
    let gi3 = PERM[ii + 1 + PERM[jj + 1 + PERM[kk + 1]]] % 12;

    let corner = |gradient: usize, cx: f32, cy: f32, cz: f32| {
        let t = 0.6 - cx * cx - cy * cy - cz * cz;
        if t < 0.0 {
            0.0
        } else {
            let t2 = t * t;
            t2 * t2 * dot(GRAD3[gradient], cx, cy, cz)
        }
    };

    let n0 = corner(gi0, x0, y0, z0);
    let n1 = corner(gi1, x1, y1, z1);
    let n2 = corner(gi2, x2, y2, z2);
    let n3 = corner(gi3, x3, y3, z3);

    32.0 * (n0 + n1 + n2 + n3)
}

/// The forest's own noise value at a global overmap position.
///
/// Reference: `om_noise_layer_forest::noise_at` - the broad layer squared, minus half the detail layer cubed, and
/// never below zero. The seed goes in as the **third axis**, which is how the reference separates two worlds.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn forest_noise(x: i32, y: i32, seed: u32) -> f32 {
    // The reference narrows the seed before it becomes the third axis: `seed % SIMPLEX_NOISE_RANDOM_SEED_LIMIT`,
    // a limit of 32,768. Skipping this makes the third axis a different number and therefore a different field.
    let z = (seed % 32_768) as f32;
    let broad = scaled_octave_noise_3d(
        Octaves::FOREST_BROAD,
        0.0,
        1.0,
        x as f32,
        y as f32,
        z,
        raw_noise_3d,
    );
    let detail = scaled_octave_noise_3d(
        Octaves::FOREST_DETAIL,
        0.0,
        1.0,
        x as f32,
        y as f32,
        z,
        raw_noise_3d,
    );
    let broad = broad * broad;
    let detail = detail * detail * detail;
    (broad - detail * 0.5).max(0.0)
}
