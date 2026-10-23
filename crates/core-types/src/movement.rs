//! Whether a step from one level to the next is allowed: `map::valid_move`'s vertical half.
//!
//! Reference: `map::valid_move` (`src/map.cpp:2490-2585`). The whole function is about creatures and vehicles; the part a *field* needs is the part that decides
//! whether there is a hole to fall through, and it is written out here on its own so that a gas can ask it without a map, a vehicle layer or a creature in the
//! room.
//!
//! The three checks, in the reference's order, are the surprising part:
//!
//! 1. the **upper** square must be passable at all (`move_cost != 0`);
//! 2. a square below that is impassable refuses the step, unless the upper square is a ledge - open air is allowed to have rock under it, because a drop with
//!    nowhere to land is still a drop as far as this rule is concerned;
//! 3. a square with a **floor** on it cannot be fallen through, unless it is a ledge or something says `GOES_DOWN`.
//!
//! After those, the reference returns true because the gas calls it with `bash = true`. The vehicle and furniture halves (`veh_at_internal`, `movecost >= 0`)
//! are not reachable on that path and are left to the phases that have vehicles and furniture movement.

/// What one square says about a vertical step: the facts `valid_move` reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StepFacts {
    /// The terrain's move cost. Zero means impassable, and it is the reference's proxy for "is there anything here at all".
    pub move_cost: i32,
    /// `NO_FLOOR`: nothing to stand on, which the reference also calls a ledge.
    pub open_air: bool,
    /// `NO_FLOOR_WATER`: no floor, but not open air either. It counts for `has_floor` and not for the ledge.
    pub no_floor_water: bool,
    /// `GOES_DOWN`: a stair, a ramp, a hole with trim around it.
    pub goes_down: bool,
}

impl StepFacts {
    /// Whether there is a floor to stand on, as `map::has_floor` decides it.
    ///
    /// Reference: `map.cpp:2639-2650`. Out of bounds and the extremes of the z range answer **true**: the map pretends there is a floor where it has no data,
    /// which is why a fall at the bottom of the world is refused rather than endless.
    #[must_use]
    pub fn has_floor(&self) -> bool {
        !self.open_air && !self.no_floor_water
    }
}

/// Whether a gas may pass between two squares of the same column, one level apart.
///
/// Reference: `map::valid_move( from, to, bash = true, flying = true, via_ramp = false )`. The caller has already established that the two squares differ by one
/// level and are otherwise in the same column; a horizontal move between levels is the reference's two-step case and belongs to the movement code.
///
/// The arguments are named **up** and **down** rather than from and to because the reference's own first act is to sort them that way, and a caller that has its
/// arguments the wrong way round would otherwise get a plausible answer to the wrong question.
#[must_use]
pub fn can_step_vertically(upper: StepFacts, lower: StepFacts) -> bool {
    if upper.move_cost == 0 {
        return false;
    }
    let upper_is_ledge = upper.open_air;
    if !upper_is_ledge && lower.move_cost == 0 {
        return false;
    }
    if upper.has_floor() && !upper.goes_down && !upper_is_ledge {
        return false;
    }
    true
}
