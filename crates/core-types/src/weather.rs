//! Wind: a speed the weather reports, and the number a field processor actually sees.
//!
//! Reference: `src/weather.cpp`. The port keeps the arithmetic and drops the global lookups:
//! whether a square is under forest, how high it is and whether a wall blocks it are asked as
//! arguments, so the rule can be tested without a map.

/// The direction the wind blows from, as one of the reference's eight vectors.
///
/// Reference: `convert_wind_to_coord` (`src/weather.cpp:852-872`). The table is ordered by
/// descending threshold and the first row whose threshold the angle reaches wins, which is why
/// 330 degrees and 0 degrees both give the same vector.
#[must_use]
pub fn wind_vector(angle: i32) -> (i32, i32) {
    const TABLE: [(i32, (i32, i32)); 9] = [
        (330, (0, -1)),
        (301, (-1, -1)),
        (240, (-1, 0)),
        (211, (-1, 1)),
        (150, (0, 1)),
        (121, (1, 1)),
        (60, (1, 0)),
        (31, (1, -1)),
        (0, (0, -1)),
    ];
    TABLE
        .iter()
        .find(|(threshold, _)| angle >= *threshold)
        .map_or((0, 0), |(_, vector)| *vector)
}

/// The wind a square feels, after terrain, height and shelter.
///
/// Reference: `get_local_windpower` (`src/weather.cpp:785-810`). The order matters and is the
/// reference's: shelter short-circuits; forest halves; height adds `z * min(5, windpower)`; a
/// wall upwind cuts what is left to a tenth.
///
/// `forest` folds the reference's two forest overmap terrains into one question, because the
/// distinction between them does not reach this arithmetic.
#[must_use]
pub fn local_wind_power(
    windpower: i32,
    forest: bool,
    z: i32,
    sheltered: bool,
    wind_blocked: bool,
) -> i32 {
    if sheltered {
        return 0;
    }
    let mut power = windpower;
    if forest {
        power /= 2;
    }
    if z > 0 {
        power += z * power.min(5);
    }
    if wind_blocked {
        power /= 10;
    }
    power
}
