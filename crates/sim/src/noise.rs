//! Simplex noise, exactly as the reference computes it.
//!
//! The single-letter names below are the reference's own (x, y, z, w, i, j, k, l, n0 through n4), kept deliberately: this is a transcription of a
//! published algorithm, and renaming its coordinates would make it harder to check against the source, not easier.
//!
//! Reference: `simplexnoise.cpp`/`simplexnoise.h`. The tables below are copied from that header by a script rather than by hand, because a single
//! wrong number in a 512-entry permutation table is a wrong world that still looks like a world.
//!
//! Everything here is `f32`, in the same order the reference writes it. The noise feeds a temperature the oracle records to four decimal places, and
//! single-precision arithmetic is not associative: reordering the sums would show up as a difference in the last digits rather than as an obviously
//! wrong answer, which is exactly the kind of bug a tolerance-based gate would forgive.

/// The permutation table, repeated twice (`simplexnoise.h`).
const PERM: [i32; 512] = [
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

/// The gradients, the midpoints of the vertices of a hypercube (`simplexnoise.h`).
const GRAD4: [[i32; 4]; 32] = [
    [0, 1, 1, 1],
    [0, 1, 1, -1],
    [0, 1, -1, 1],
    [0, 1, -1, -1],
    [0, -1, 1, 1],
    [0, -1, 1, -1],
    [0, -1, -1, 1],
    [0, -1, -1, -1],
    [1, 0, 1, 1],
    [1, 0, 1, -1],
    [1, 0, -1, 1],
    [1, 0, -1, -1],
    [-1, 0, 1, 1],
    [-1, 0, 1, -1],
    [-1, 0, -1, 1],
    [-1, 0, -1, -1],
    [1, 1, 0, 1],
    [1, 1, 0, -1],
    [1, -1, 0, 1],
    [1, -1, 0, -1],
    [-1, 1, 0, 1],
    [-1, 1, 0, -1],
    [-1, -1, 0, 1],
    [-1, -1, 0, -1],
    [1, 1, 1, 0],
    [1, 1, -1, 0],
    [1, -1, 1, 0],
    [1, -1, -1, 0],
    [-1, 1, 1, 0],
    [-1, 1, -1, 0],
    [-1, -1, 1, 0],
    [-1, -1, -1, 0],
];

/// Which of the twenty-four simplices a point falls in (`simplexnoise.h`).
const SIMPLEX: [[i32; 4]; 64] = [
    [0, 1, 2, 3],
    [0, 1, 3, 2],
    [0, 0, 0, 0],
    [0, 2, 3, 1],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [1, 2, 3, 0],
    [0, 2, 1, 3],
    [0, 0, 0, 0],
    [0, 3, 1, 2],
    [0, 3, 2, 1],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [1, 3, 2, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [1, 2, 0, 3],
    [0, 0, 0, 0],
    [1, 3, 0, 2],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [2, 3, 0, 1],
    [2, 3, 1, 0],
    [1, 0, 2, 3],
    [1, 0, 3, 2],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [2, 0, 3, 1],
    [0, 0, 0, 0],
    [2, 1, 3, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [2, 0, 1, 3],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [3, 0, 1, 2],
    [3, 0, 2, 1],
    [0, 0, 0, 0],
    [3, 1, 2, 0],
    [2, 1, 0, 3],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [0, 0, 0, 0],
    [3, 1, 0, 2],
    [0, 0, 0, 0],
    [3, 2, 0, 1],
    [3, 2, 1, 0],
];

/// Reference: `simplexnoise.cpp`, `fastfloor` - floor toward negative infinity, which a cast alone does not do for negatives.
///
/// The saturating cast cannot be reached by real noise coordinates (they are positions in days and map squares, nowhere near 2^31), and saturating is the
/// defined behaviour where the reference would be undefined.
#[allow(clippy::cast_possible_truncation)]
fn fast_floor(x: f32) -> i32 {
    if x > 0.0 { x as i32 } else { x as i32 - 1 }
}

/// The dot product of one gradient with one point.
///
/// The gradients are the numbers -1, 0 and 1 (`grad4`), so the casts are exact even though `i32` to `f32` is not exact in general.
#[allow(clippy::cast_precision_loss)]
fn dot4(gradient: [i32; 4], x: f32, y: f32, z: f32, w: f32) -> f32 {
    gradient[0] as f32 * x
        + gradient[1] as f32 * y
        + gradient[2] as f32 * z
        + gradient[3] as f32 * w
}

/// Reference: `simplexnoise.cpp`, `raw_noise_4d` - four-dimensional simplex noise in `[-1, 1]`.
#[allow(
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::indexing_slicing
)]
#[must_use]
pub fn raw_noise_4d(x: f32, y: f32, z: f32, w: f32) -> f32 {
    let f4 = (5.0f32.sqrt() - 1.0) / 4.0;
    let g4 = (5.0 - 5.0f32.sqrt()) / 20.0;

    let s = (x + y + z + w) * f4;
    let i = fast_floor(x + s);
    let j = fast_floor(y + s);
    let k = fast_floor(z + s);
    let l = fast_floor(w + s);
    let t = (i + j + k + l) as f32 * g4;
    let x0 = x - (i as f32 - t);
    let y0 = y - (j as f32 - t);
    let z0 = z - (k as f32 - t);
    let w0 = w - (l as f32 - t);

    let c1 = if x0 > y0 { 32 } else { 0 };
    let c2 = if x0 > z0 { 16 } else { 0 };
    let c3 = if y0 > z0 { 8 } else { 0 };
    let c4 = if x0 > w0 { 4 } else { 0 };
    let c5 = if y0 > w0 { 2 } else { 0 };
    let c6 = i32::from(z0 > w0);
    // The six comparisons produce a number below 64 by construction, which is why the reference indexes a 64-entry table with it directly.
    let corner = SIMPLEX[usize::try_from(c1 + c2 + c3 + c4 + c5 + c6).unwrap_or(0)];

    let step = |threshold: i32| -> [i32; 4] {
        [
            i32::from(corner[0] >= threshold),
            i32::from(corner[1] >= threshold),
            i32::from(corner[2] >= threshold),
            i32::from(corner[3] >= threshold),
        ]
    };
    let first = step(3);
    let second = step(2);
    let third = step(1);

    let corner_of = |offsets: [i32; 4], amount: f32| {
        (
            x0 - offsets[0] as f32 + amount * g4,
            y0 - offsets[1] as f32 + amount * g4,
            z0 - offsets[2] as f32 + amount * g4,
            w0 - offsets[3] as f32 + amount * g4,
        )
    };
    let (x1, y1, z1, w1) = corner_of(first, 1.0);
    let (x2, y2, z2, w2) = corner_of(second, 2.0);
    let (x3, y3, z3, w3) = corner_of(third, 3.0);
    let (x4, y4, z4, w4) = (
        x0 - 1.0 + 4.0 * g4,
        y0 - 1.0 + 4.0 * g4,
        z0 - 1.0 + 4.0 * g4,
        w0 - 1.0 + 4.0 * g4,
    );

    let ii = i & 255;
    let jj = j & 255;
    let kk = k & 255;
    let ll = l & 255;
    let at = |index: i32| PERM[usize::try_from(index).unwrap_or(0)];
    let corner_index = |a: i32, b: i32, c: i32, d: i32| -> usize {
        usize::try_from(at(a + at(b + at(c + at(d)))) % 32).unwrap_or(0)
    };
    let gradients = [
        (x0, y0, z0, w0, corner_index(ii, jj, kk, ll)),
        (
            x1,
            y1,
            z1,
            w1,
            corner_index(ii + first[0], jj + first[1], kk + first[2], ll + first[3]),
        ),
        (
            x2,
            y2,
            z2,
            w2,
            corner_index(
                ii + second[0],
                jj + second[1],
                kk + second[2],
                ll + second[3],
            ),
        ),
        (
            x3,
            y3,
            z3,
            w3,
            corner_index(ii + third[0], jj + third[1], kk + third[2], ll + third[3]),
        ),
        (x4, y4, z4, w4, corner_index(ii + 1, jj + 1, kk + 1, ll + 1)),
    ];

    let mut contributions = 0.0f32;
    for (cx, cy, cz, cw, gradient) in gradients {
        let mut falloff = 0.6 - cx * cx - cy * cy - cz * cz - cw * cw;
        if falloff >= 0.0 {
            falloff *= falloff;
            contributions += falloff * falloff * dot4(GRAD4[gradient], cx, cy, cz, cw);
        }
    }

    27.0 * contributions
}
