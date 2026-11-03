//! How far a shot can wander, as the numbers the reference adds up.
//!
//! Reference: `dispersion_sources` (`src/dispersion.cpp:8-42`). The reference's own comment says what the three kinds are: **linear** sources are each drawn from zero to
//! their own size, **normal** ones are drawn around zero, and **multipliers** scale the sum - and the data's `spread` is added to the **total** but not to a roll. The port
//! keeps the same three sums because the arithmetic, not the container, is the rule: a `vec` here is a list of numbers, and the store keeps none of them.

/// What a shot's spread is made of.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Dispersion {
    /// Sources each drawn from zero: encumbrance, recoil, the shooter's steadiness.
    pub linear: Vec<f64>,
    /// Sources drawn around zero: the weapon's own wobble.
    pub normal: Vec<f64>,
    /// Sources that scale everything before them.
    pub multipliers: Vec<f64>,
    /// What the data adds on top, which is **not** a source a roll draws from.
    pub spread: f64,
}

impl Dispersion {
    /// The most this shot can spread, which is the sum of every source with the spread counted in.
    ///
    /// Reference: `dispersion_sources::max` (`dispersion.cpp:23-37`): the two source sums, then each multiplier, then the spread.
    #[must_use]
    pub fn max(&self) -> f64 {
        let mut sum: f64 = self.linear.iter().sum::<f64>() + self.normal.iter().sum::<f64>();
        for multiplier in &self.multipliers {
            sum *= multiplier;
        }
        sum + self.spread
    }

    /// Half the maximum, which is what the reference calls the average.
    #[must_use]
    pub fn avg(&self) -> f64 {
        self.max() / 2.0
    }

    /// One shot's spread, drawn from the game's dice.
    ///
    /// Reference: `dispersion_sources::roll` (`dispersion.cpp:8-21`): each linear source is a uniform draw from zero to itself, each normal one a normal draw around zero,
    /// the multipliers scale the result, **the spread is not added**, and the whole thing is capped at a full circle of 3600.
    pub fn roll(&self, dice: &mut cdda_core_types::rng::Rolls) -> f64 {
        let mut total = 0.0;
        for source in &self.linear {
            total += dice.rng_float(0.0, *source);
        }
        for source in &self.normal {
            // `rng_normal( hi )` is `rng_normal( 0, hi )` (`rng.h:82-85`): centred on zero, a quarter of the range either side.
            total += dice.rng_normal(0.0, *source);
        }
        for multiplier in &self.multipliers {
            total *= multiplier;
        }
        total.min(3600.0)
    }
}

/// The highest skill the game has, which `MAX_SKILL` sets to ten.
pub const MAX_SKILL: f64 = 10.0;

/// The dispersion a shooter's own skill adds, which is not a straight line.
///
/// Reference: `dispersion_from_skill` (`src/ranged.cpp:2477-2497`). The reference's comment on the branch is worth keeping: **lack of mastery multiplies the weapon's own
/// dispersion**, so a beginner with a precise weapon still shoots poorly - and the penalty is larger below skill five than above it.
#[must_use]
pub fn from_skill(skill: f64, weapon_dispersion: f64) -> f64 {
    if skill >= MAX_SKILL {
        return 0.0;
    }
    let shortfall = MAX_SKILL - skill;
    let mut penalty = 10.0 * shortfall;
    let threshold = 5.0;
    if skill >= threshold {
        // Above the threshold the weapon's dispersion is scaled by what is left to learn, over the range above the threshold.
        return penalty + (weapon_dispersion * shortfall * 1.25) / (MAX_SKILL - threshold);
    }
    // Below it the shortfall is steeper: ten more thousandths of the weapon's dispersion for every level missing.
    penalty += weapon_dispersion * (1.25 + (threshold - skill) * 10.0 / threshold);
    penalty
}
