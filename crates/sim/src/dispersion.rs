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

/// A target that moves, which makes a throw harder.
///
/// Reference: `Character::throwing_dispersion`'s `critter` branch (`src/ranged.cpp:1367-1372`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Dodging {
    /// The target's dodge score.
    pub dodge: f64,
    /// How far away it is, in squares.
    pub distance: i32,
    /// The thrower's dexterity, which makes a dodge cheaper to beat.
    pub dexterity: i32,
    /// The dexterity modifier, which is `1.0` unless a mutation or an item changes it.
    pub dex_modifier: f64,
}

/// What is being thrown, how, and at what.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Throw {
    /// What it weighs, in grams.
    pub weight_g: i64,
    /// How much room it takes, in millilitres.
    pub volume_ml: i64,
    /// How many of them go at once: a stack is split, so a handful of pebbles throws like one pebble.
    pub charges: i32,
    /// The thrower's arm strength - **not** its strength, which the reference reads separately.
    pub arm_strength: i32,
    /// The thrower's throwing skill, out of ten.
    pub skill: f64,
    /// The thrower's perception.
    pub perception: i32,
    /// What is in the thrower's eyes, from zero to a hundred.
    pub eye_encumbrance: i32,
    /// Whether the throw is blind, which the reference multiplies by four.
    pub blinded: bool,
    /// The target, when it is a creature that can move out of the way.
    pub target: Option<Dodging>,
}

/// The per-dodge cost of a throw, which falls as the thrower's dexterity climbs.
///
/// Reference: `Character::throw_dispersion_per_dodge` (`src/ranged.cpp:1329-1340`): `100 / max( 1, 2 + dex / 4 )`, truncated. The reference's comment beside it is
/// **stale** and says 200 at zero dexterity where the code gives 50; the port follows the code.
#[must_use]
pub fn per_dodge(dexterity: i32, dex_modifier: f64) -> i32 {
    let effective = (2.0 + f64::from(dexterity) / 4.0) * dex_modifier;
    let divisor = effective.max(1.0);
    // Reference: `static_cast<int>( 100.0f / std::max( 1.0f, effective_dex ) )` - a truncation toward zero, which is what `as` does here.
    #[allow(clippy::cast_possible_truncation)]
    let cost = (100.0 / divisor) as i32;
    cost
}

/// How far a throw can wander, as a plain number of dispersion points.
///
/// Reference: `Character::throwing_dispersion` (`src/ranged.cpp:1342-1384`). The reference's own comment above it claims 1000 at skill zero and 250 at skill ten; the code
/// gives 2500 and 119, because the divisor is `8 * skill + 4`. **The comment is stale, and a port that transcribed it would be wrong in every number while keeping the right
/// shape** - which is the failure this test exists to catch.
#[must_use]
pub fn throwing_dispersion(shot: &Throw) -> i32 {
    let charges = shot.charges.max(1);
    let weight_g = shot.weight_g / i64::from(charges);
    let volume_ml = shot.volume_ml / i64::from(charges);
    let mut difficulty: i64 = 1_000;
    // A litre is free and every millilitre over it costs one.
    difficulty += (volume_ml - 1_000).max(0);
    // Every gram over `arm strength * 100` costs one, so a strong arm throws heavy things straight.
    difficulty += (weight_g - i64::from(shot.arm_strength) * 100).max(0);
    // Reference: `10 * throw_difficulty / ( 8 * throw_skill + 4 )`, truncated - the skill term is a float and the answer is an int.
    let skill = shot.skill.min(MAX_SKILL);
    // The difficulty is a small number of dispersion points; the cast is exact, where the answer's truncation is the rule.
    #[allow(clippy::cast_precision_loss)]
    let difficulty_f = difficulty as f64;
    #[allow(clippy::cast_possible_truncation)]
    let mut spread = (10.0 * difficulty_f / (8.0 * skill + 4.0)) as i32;
    if let Some(target) = &shot.target {
        // A dodge counts **ten times** at point blank and once from two squares away, because a thrower has to adjust more when the target is close.
        let scale = (10 - 5 * target.distance).max(1);
        #[allow(clippy::cast_possible_truncation)]
        let effective_dodge = (target.dodge * f64::from(scale)) as i32;
        spread += per_dodge(target.dexterity, target.dex_modifier) * effective_dodge;
    }
    // Eyes full of water cost ten points of perception each.
    spread += (shot.eye_encumbrance - shot.perception).max(0) * 10;
    if shot.blinded {
        spread *= 4;
    }
    spread.max(0)
}

/// The `Shotmaker` a character and a gun make together: the gun's own figure, the shooter's **effective** hands and eyes, and the two skills the reference averages.
///
/// Reference: `Character::get_weapon_dispersion` (`ranged.cpp:2500-2545`) reads `obj.gun_dispersion()` for the weapon, `get_dex()` and `get_per()` for the body, and then averages
/// `get_skill_level( skill_gun )` with `get_skill_level( obj.gun_skill() )` before asking the curve. The port reads the same three things from the store, which is why this is a function of
/// the world rather than of numbers: **the effective stats are the ones that count** (round 1133), not the base ones.
///
/// The gun skill is the reference's own `"gun"` - the skill of using firearms at all - and the weapon's skill is the one the gun's data names, such as `rifle` or `archery`.
#[must_use]
pub fn shotmaker_for(
    world: &bevy_ecs::prelude::World,
    shooter: bevy_ecs::prelude::Entity,
    gun: &cdda_content::guns::GunType,
    divider: f64,
) -> Shotmaker {
    Shotmaker {
        weapon_dispersion: f64::from(gun.dispersion),
        dexterity: crate::creature::effective_dexterity(world, shooter),
        perception: crate::creature::effective_perception(world, shooter),
        gun_skill: crate::creature::skill_level(world, shooter, "gun"),
        weapon_skill: crate::creature::skill_level(world, shooter, &gun.skill),
        archery: gun.skill == "archery",
        divider,
    }
}

/// The dispersion a shooter's own perception adds, which only ever hurts and hurts faster than dexterity.
///
/// Reference: `Character::ranged_per_mod` (`character.cpp:4369-4373`) - `std::max( ( 20.0 - get_per() ) * 1.2, 0.0 )` - the same floor at twenty, and **1.2 a point** instead of the half
/// point dexterity takes. A character at ten perception is worth twelve units of dispersion, and the reference means it: aiming is perception's job.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn from_perception(perception: i32) -> i32 {
    let penalty: f64 = (20.0 - f64::from(perception)) * 1.2;
    penalty.max(0.0) as i32
}

/// What a shooter's own weapon costs in dispersion before anything is drawn.
///
/// Reference: `Character::get_weapon_dispersion` (`ranged.cpp:2500-2560`), in the reference's order: the weapon's own dispersion, then the shooter's dexterity, then the shooter's
/// perception, then the skill term - which is `dispersion_from_skill` against **300 over `GUN_DISPERSION_DIVIDER`** for anything but archery and **450 over it** for a bow.
///
/// Everything the reference adds from the body, the enchantment cache, vehicles, water and gunmods is left out on purpose: **this is the part of the rule that needs no gun table**, and
/// the whole function is one argument list away from the gun table being filled in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shotmaker {
    /// The weapon's own dispersion, before any modifier: `item::gun_dispersion`'s first term.
    pub weapon_dispersion: f64,
    /// The shooter's **effective** dexterity, not the base.
    pub dexterity: i32,
    /// The shooter's **effective** perception.
    pub perception: i32,
    /// The gun skill, and the weapon's own skill, averaged by the reference before the curve.
    pub gun_skill: i32,
    /// The weapon's own skill level.
    pub weapon_skill: i32,
    /// Whether the weapon's skill is archery, which uses the gentler 450 denominator.
    pub archery: bool,
    /// `GUN_DISPERSION_DIVIDER` from the options, which is **18** in the data.
    pub divider: f64,
}

/// The dispersion a shot starts with, from the weapon, the hands and the skill - before anything is rolled.
///
/// Reference: `Character::get_weapon_dispersion` (`ranged.cpp:2500-2545`), with the terms this port has: the weapon's own figure, `ranged_dex_mod`, `ranged_per_mod` and
/// `dispersion_from_skill` over the averaged gun and weapon skill. The reference clamps the average at `MAX_SKILL` before the curve; so does this.
#[must_use]
pub fn weapon_dispersion(shotmaker: Shotmaker) -> f64 {
    let average = f64::from(shotmaker.gun_skill + shotmaker.weapon_skill) / 2.0;
    let average = average.min(MAX_SKILL);
    let denominator = if shotmaker.archery { 450.0 } else { 300.0 };
    f64::from(from_dexterity(shotmaker.dexterity))
        + f64::from(from_perception(shotmaker.perception))
        + from_skill(average, denominator / shotmaker.divider)
        + shotmaker.weapon_dispersion
}

/// The dispersion a shooter's own dexterity adds, which only ever hurts.
///
/// Reference: `Character::ranged_dex_mod` (`character.cpp:4363-4367`) - `std::max( ( 20.0 - get_dex() ) * 0.5, 0.0 )`, floored at zero and **truncated to an int on the way out**
/// because the function returns `int`. Twenty dexterity is the point where the penalty stops: below it every point costs half a unit of dispersion, and above it nothing improves.
///
/// The reference's own comment calls it a bonus for low dexterity only in the negative, and the shape is worth naming: **the curve is flat at the top**, so a dexterity of twenty-four
/// shoots exactly as well as twenty, and a character with an effect that costs two dexterity pays **one** unit of dispersion for it.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn from_dexterity(dexterity: i32) -> i32 {
    let penalty: f64 = (20.0 - f64::from(dexterity)) * 0.5;
    penalty.max(0.0) as i32
}
