//! What the wind is called and which way it blows.
//!
//! Reference: `get_wind_desc` and `convert_wind_to_coord` (`src/weather.cpp:817-874`). Both are ladders read from the top down - one over a power, one over a compass angle - and both
//! have a first row that behaves differently from the rest, which is exactly the sort of detail a port loses by rewriting the ladder as arithmetic.

/// The Beaufort name for a wind of this power.
///
/// Reference: `get_wind_desc` (`weather.cpp:817-850`), which is a chain of comparisons rather than a table. **The first is `< 1` and every other is `<=`**: exactly one is *Calm* and a
/// wind of one is already *Light Air*. The reference's own comment gives the scale's origin (the Beaufort scale) and says anything above a whole gale has no extra effects - the name still
/// changes to *Hurricane*, which is why the last comparison is `> 72` rather than a ceiling.
#[must_use]
pub fn description(windpower: f64) -> &'static str {
    /// The Beaufort ladder's upper bounds, in the reference's order.
    const GRADES: [(f64, &str); 12] = [
        (3.0, "Light Air"),
        (7.0, "Light Breeze"),
        (12.0, "Gentle Breeze"),
        (18.0, "Moderate Breeze"),
        (24.0, "Fresh Breeze"),
        (31.0, "Strong Breeze"),
        (38.0, "Moderate Gale"),
        (46.0, "Gale"),
        (54.0, "Strong Gale"),
        (63.0, "Whole Gale"),
        (72.0, "Violent Storm"),
        (f64::INFINITY, "Hurricane"),
    ];
    if windpower < 1.0 {
        return "Calm";
    }
    for (ceiling, name) in GRADES {
        if windpower <= ceiling {
            return name;
        }
    }
    "Hurricane"
}

/// The direction the wind blows towards, as a step across the map.
///
/// Reference: `convert_wind_to_coord` (`weather.cpp:852-874`), whose table is **read from the top down and descends**: the first row the angle is at or above wins. The table's own quirk
/// is kept - **330 and 0 both answer north**, so the first row catches every angle from 330 up and the last row catches the rest down to zero - and an angle outside the table's range
/// (which the reference's own callers never produce, since a direction is 0..=359) answers **no wind at all** rather than a made-up direction.
#[must_use]
pub fn direction(angle: i32) -> (i32, i32) {
    /// The table in the reference's order: an angle and the step it means.
    const ROWS: [(i32, (i32, i32)); 9] = [
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
    for (at, step) in ROWS {
        if angle >= at {
            return step;
        }
    }
    (0, 0)
}

/// The ground a wind has to get through before it reaches a creature.
///
/// Reference: the three modifiers and the early return in `get_local_windpower` (`weather.cpp:785-810`). **A struct because the four answers are asked together at every call site**, and
/// because `local_windpower( 20, true, 3, false, false )` is a call nobody can read.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ground {
    /// The creature is underground, in a vehicle or indoors (`sheltered`), which stops the wind outright.
    pub sheltered: bool,
    /// The overmap terrain is forest or forest water.
    pub forest: bool,
    /// How far above the ground the creature stands; zero or below changes nothing.
    pub height: i32,
    /// A wall stands on the upwind square.
    pub blocked: bool,
}

/// The wind's power after the ground and the height have had their say.
///
/// Reference: `get_local_windpower` (`weather.cpp:785-810`), in the reference's own order:
///
/// ```text
/// if( sheltered )                        return 0;
/// if( forest or forest_water )           windpower /= 2;
/// if( location.z() > 0 )                 windpower += z * min( 5, windpower );
/// if( is_wind_blocker( upwind square ) ) windpower /= 10;
/// ```
///
/// **The order matters twice**: the forest halves before the height adds, and the wall divides by ten *after* both - so a gale in a wood on a roof beside a wall is
/// `( g / 2 + z * min( 5, g / 2 ) ) / 10`, which is not the same as dividing the gale by ten first. **A shelter answers zero whatever the sky is doing**, which is why its early return is first.
#[must_use]
pub fn local_windpower(windpower: i32, ground: Ground) -> i32 {
    if ground.sheltered {
        return 0;
    }
    let mut power = windpower;
    if ground.forest {
        power /= 2;
    }
    if ground.height > 0 {
        power += ground.height * power.min(5);
    }
    if ground.blocked {
        power /= 10;
    }
    power
}
