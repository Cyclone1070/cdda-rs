//! The arithmetic behind the metabolic rates: how hungry a body is, what it burns, and how much it needs.
//!
//! Reference: four small functions that everything else in the food clock is measured against —
//! `Character::get_starvation` (`src/character.cpp:4657-4670`), `metabolic_rate` (`:770-790`), `base_bmr` (`:6514-6527`), `get_bmr` (`:6538-6542`) — and the
//! `calc_needs_rates` that assembles them (`:5273-5316`). They are small but they are the **inputs** to digestion, so an error here is an error in every meal.
//!
//! Three of the four are interpolations over a table, and the tables are literal:
//!
//! * **starvation** climbs as the calorie store empties — nothing at 95% of the healthy weight's calories, then 100, 300 and 6,000 as it falls;
//! * **metabolic rate** is a multiplier on the base rate that falls as a character gets hungrier, from 1 at an effective hunger of 300 to 0.5 at 8,000 — and the
//!   effective hunger is `( hunger + starvation ) * 100 / speed`, so being **faster** makes the same hunger a smaller share;
//! * **basal metabolic rate** is the Harris-Benedict equation the reference spells out, with the activity level's ceiling multiplied over it.
//!
//! Two conventions decide the numbers and are easy to get wrong: the interpolators are `multi_lerp`'s own (a value below the first point answers the first, above the
//! last answers the last), and every cast in `base_bmr` **truncates** — the height factor is `6.25 * height` assigned to an `int`, so a height of 175 cm contributes
//! 1,093 and not 1,094.

use bevy_ecs::prelude::{Component, Entity, World};

/// One point of a piecewise linear curve.
pub type Point = (f32, f32);

/// The starvation curve: `( kilocalorie percentage, starvation )` (`src/character.cpp:4659-4663`).
pub const STARVATION_THRESHOLDS: [Point; 3] = [(0.0, 6_000.0), (0.8, 300.0), (0.95, 100.0)];

/// The metabolic multiplier curve: `( effective hunger, multiplier )` (`:775-780`).
pub const METABOLIC_THRESHOLDS: [Point; 4] = [(300.0, 1.0), (2_000.0, 0.8), (5_000.0, 0.6), (8_000.0, 0.5)];

/// An integer as a float for the curves, named so the cast is visible at every call.
#[allow(clippy::cast_precision_loss)]
fn as_float(value: i32) -> f32 {
    value as f32
}

/// A float already, kept so the two curves read alike at their call sites.
fn as_float_of(value: f32) -> f32 {
    value
}

/// Piecewise linear interpolation: `multi_lerp` (`src/cata_utility.cpp:267-288`).
///
/// The points are **ascending by their first member** and the answer outside the curve is the nearest end, not an extrapolation. The reference finds the first point
/// strictly above `x` and interpolates between it and the one before, which is why a `x` exactly on a point answers that point's own value.
#[must_use]
pub fn multi_lerp(points: &[Point], x: f32) -> f32 {
    let Some(index) = points.iter().position(|point| point.0 > x) else {
        // At or past the last point: the curve flattens rather than extrapolating.
        return points.last().map_or(0.0, |point| point.1);
    };
    if index == 0 {
        return points.first().map_or(0.0, |point| point.1);
    }
    let (Some(left), Some(right)) = (
        points.get(index.wrapping_sub(1)).copied(),
        points.get(index).copied(),
    ) else {
        return points.last().map_or(0.0, |point| point.1);
    };
    let t = (x - left.0) / (right.0 - left.0);
    t * right.1 + (1.0 - t) * left.1
}

/// How starved a body is, from how full its calorie store is: `Character::get_starvation` (`src/character.cpp:4657-4670`).
///
/// A body at 95% of the calories its healthy weight holds is not starving at all; below that the curve climbs to 6,000 at empty, and the answer is **rounded** to a
/// whole number, because the hunger figure it is added to is one.
#[allow(clippy::cast_possible_truncation)]
#[must_use]
pub fn starvation(kcal_percent: f32) -> i32 {
    if kcal_percent < 0.95 {
        return multi_lerp(&STARVATION_THRESHOLDS, kcal_percent).round() as i32;
    }
    0
}

/// The multiplier on the base metabolic rate: `Character::metabolic_rate` (`src/character.cpp:770-790`).
///
/// "Note: Values do not match hungry/v.hungry/famished/starving, because effective hunger is affected by speed (which drops when hungry)" — the effective hunger is
/// `( hunger + starvation ) * 100 / max( 50, speed )`, so a faster character is *less* affected by the same hunger.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn metabolic_rate(hunger: i32, starvation: i32, speed: i32, base: f32) -> f32 {
    let effective_hunger = (hunger + starvation) as f32 * 100.0 / speed.max(50) as f32;
    multi_lerp(&METABOLIC_THRESHOLDS, effective_hunger) * base
}

/// The Harris-Benedict basal metabolic rate: `Character::base_bmr` (`src/character.cpp:6514-6527`).
///
/// `multiplier * ( weight_g / 100 + 6.25 * height_cm - 5 * age + 5 )`, with every term truncated on the way in and the product truncated on the way out, because the
/// reference stores them in `int`s.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
#[must_use]
pub fn base_bmr(weight_grams: i32, height_cm: i32, age: i32, multiplier: f32) -> i32 {
    let weight_factor = weight_grams / 100;
    let height_factor = (6.25 * f64::from(height_cm)) as i32;
    let age_factor = 5 * age;
    let sum = weight_factor + height_factor - age_factor + 5;
    (multiplier * sum as f32) as i32
}

/// The rate the body burns at, activity included: `Character::get_bmr` (`src/character.cpp:6538-6542`).
///
/// The activity level is **rounded up** to the next whole step and clamped between resting and the character's maximum exertion, so a light walk costs a whole step's
/// worth of calories and not a fraction of one.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
#[must_use]
pub fn bmr(base: i32, average_activity: f32, max_exertion: f32) -> i32 {
    let level = average_activity.clamp(NO_EXERCISE, max_exertion).ceil();
    (base as f32 * level) as i32
}

/// The ceiling hunger can reach: `set_hunger` caps it at **300**, "just below famished" (`src/character.cpp:4650-4651`).
///
/// This matters to the metabolic rate: `metabolic_rate` reads `get_hunger()`, which is the **capped** figure, so a character whose hunger was set to 600 is at an
/// effective hunger of 300 and a multiplier of exactly 1 — the first recorded grid case, and the one that caught a port reading the number it was handed.
pub const MAX_HUNGER: i32 = 300;

/// `NO_EXERCISE` (`src/game_constants.h:142`): the floor the activity level is clamped to, which is **one**, not zero.
pub const NO_EXERCISE: f32 = 1.0;

/// The four rates `update_stomach` runs on: `needs_rates` (`src/character.h:493-499`), of which the port uses these.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NeedsRates {
    /// The body's calorie burn for the day.
    pub kcal: f32,
    /// The metabolic multiplier.
    pub hunger: f32,
    /// How fast thirst climbs, which is the `PLAYER_THIRST_RATE` option and nothing else for an awake character with no traits.
    pub thirst: f32,
    /// How fast sleepiness climbs, the `PLAYER_SLEEPINESS_RATE` option.
    pub sleepiness: f32,
}

/// Assembles the rates: `Character::calc_needs_rates` (`src/character.cpp:5273-5316`) for an **awake** character with no traits that touch it.
///
/// **Declared:** the sleeping path (`calc_sleep_recovery_rate`, which halves both rates and reads every body part's temperature), the `ROOTS2`/`ROOTS3`/`CHLOROMORPH`
/// tree-communion halving, `TRANSPIRATION`'s use of the local temperature, and the `HUNGER` / `THIRST` / `SLEEPINESS` enchantment modifiers.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
#[must_use]
pub fn needs_rates(bmr: i32, metabolic_rate: f32, thirst_option: f32, sleepiness_option: f32) -> NeedsRates {
    NeedsRates {
        kcal: bmr as f32,
        hunger: metabolic_rate,
        thirst: thirst_option,
        sleepiness: sleepiness_option,
    }
}

/// The rates a creature's own body burns at, as a component so a system can read them without asking anybody else.
///
/// Reference: `needs_rates` (`character.h:493-499`) is a value the character holds; the port keeps it on the creature for the same reason - the clock is a question
/// about one body.
#[derive(Debug, Clone, Copy, Default, PartialEq, Component)]
pub struct NeedsRatesOf(pub NeedsRates);

/// How sleepy a creature is, on the reference's scale: 191 is tired, 383 dead tired, 575 exhausted, 1000 barely awake.
///
/// Reference: `Character::sleepiness` (`character.cpp:4698-4712`): an integer the game adds to and clamps, with `MASSIVE_SLEEPINESS` at 1000 and the `update_needs`
/// cap at 1050 - the extra fifty are the overshoot the reference lets a five-minute roll carry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct Sleepiness(pub i32);

/// The level a creature cannot climb past: `character.cpp:5161`, the cap inside `update_needs`.
pub const MAX_SLEEPINESS: i32 = 1050;

/// Moves a creature's sleepiness by an amount, within the range the reference allows.
///
/// Reference: `Character::set_sleepiness` (`character.cpp:4704-4712`): the total is clamped between `-1000` and `MAX_SLEEPINESS`, so a creature that sleeps can go
/// well below zero and stay there for a while.
pub fn mod_sleepiness(world: &mut World, creature: Entity, amount: i32) {
    let mut held = world.get::<Sleepiness>(creature).copied().unwrap_or_default();
    held.0 = (held.0 + amount).clamp(-1000, MAX_SLEEPINESS);
    world.entity_mut(creature).insert(held);
}

/// How sleepy a creature is, **zero** when it has never been tracked.
#[must_use]
pub fn sleepiness_of(world: &World, creature: Entity) -> i32 {
    world.get::<Sleepiness>(creature).map_or(0, |held| held.0)
}

/// How much a creature's sleepiness climbs over a span of five-minute intervals.
///
/// Reference: `Character::update_needs` (`character.cpp:5155-5175`): the climb is `roll_remainder( rates.sleepiness * rate_multiplier )`, it happens only while the
/// creature is **awake** and below the cap, and `DEBUG_LS` (a creature with no needs at all) stops it entirely.
pub fn climb_sleepiness(
    world: &mut World,
    creature: Entity,
    rates: NeedsRates,
    five_mins: i64,
    awake: bool,
    foodless: bool,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    if !awake || foodless {
        return;
    }
    let current = sleepiness_of(world, creature);
    if current >= MAX_SLEEPINESS {
        return;
    }
    #[allow(clippy::cast_precision_loss)]
    let span = five_mins as f32;
    let climb = crate::roll_remainder(f64::from(rates.sleepiness * span), rolls);
    mod_sleepiness(world, creature, climb);
}

/// Whether a creature is asleep, which is the state the sleepiness clock reads in the other direction.
///
/// Reference: `Character::update_needs` (`character.cpp:5145`): `asleep` is `!get_effect( effect_sleep ).is_null()`. The port has no effect named "sleep" on the
/// creature yet, so the state is a marker the sleeping activity will insert - and, until then, a test or a playtest can.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct Asleep;

/// How fast a creature recovers while asleep.
///
/// Reference: `Character::calc_sleep_recovery_rate` (`character.cpp:5321-5345`) with the enchantment at its default of one, no cold body parts and no hibernation: the
/// rate is one, plus an **accelerated** recovery that is faster the deeper the sleep - `1 / ( 24 - intensity + 1 )`, which is a twenty-fourth at no intensity and a
/// twentieth at intensity four.
#[must_use]
pub fn recovery_rate(intensity: i32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let denom = f64::from(24 - intensity.clamp(0, 23) + 1);
    #[allow(clippy::cast_possible_truncation)]
    let bonus = (1.0 / denom) as f32;
    1.0 + bonus
}

/// What a span of sleep did to a creature's sleepiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slept {
    /// It slept on, and this much sleepiness went away.
    Deeper(i32),
    /// It has slept enough: the reference wakes it and takes twenty-five.
    Woke,
}

/// Takes sleepiness away for a span of sleep.
///
/// Reference: `Character::update_needs` (`character.cpp:5181-5193`): `recovered = roll_remainder( rates.recovery * rate_multiplier )`, then either
/// `mod_sleepiness( -recovered )` or, when that would take the creature below **minus twenty**, the sleep is cut short, the creature wakes, and it loses twenty-five.
pub fn sleep_off(
    world: &mut World,
    creature: Entity,
    recovery: f32,
    five_mins: i64,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> Slept {
    #[allow(clippy::cast_precision_loss)]
    let span = five_mins as f32;
    let recovered = crate::roll_remainder(f64::from(recovery * span), rolls);
    if sleepiness_of(world, creature) - recovered < -20 {
        mod_sleepiness(world, creature, -25);
        return Slept::Woke;
    }
    mod_sleepiness(world, creature, -recovered);
    Slept::Deeper(recovered)
}

/// The daily rates a creature actually works at, once its enchantments have had their say.
///
/// Reference: `Character::calc_needs_rates`'s last three lines (`character.cpp:5313-5316`): hunger, sleepiness and thirst each pass through
/// `enchantment_cache->modify_value`, so a mutation that makes a creature sleep less really does slow the clock rather than only describing itself.
///
/// A creature with no mutation table answers its stored rates unchanged - which is what the reference does with an empty cache.
#[must_use]
pub fn rates_with_enchantments(world: &World, creature: bevy_ecs::entity::Entity, rates: NeedsRates) -> NeedsRates {
    let Some(data) = crate::mutation_data::holder(world) else {
        return rates;
    };
    let modify = |stat: &str, value: f32| {
        let (add, multiply) = crate::mutations::enchantment_values(world, creature, &data, stat);
        #[allow(clippy::cast_possible_truncation)]
        let answer = crate::mutations::apply_enchantments(f64::from(value), add, multiply) as f32;
        answer
    };
    NeedsRates {
        kcal: rates.kcal,
        hunger: modify("HUNGER", rates.hunger),
        thirst: modify("THIRST", rates.thirst),
        sleepiness: modify("SLEEPINESS", rates.sleepiness),
    }
}

/// What being thirsty costs a character's speed, along the reference's own thresholds.
///
/// Reference: `Character::thirst_speed_penalty` (`character.cpp:12439-12451`):
///
/// ```text
/// { 40 -> 0, 300 -> -25, 600 -> -50, 1200 -> -75 }   // and 1200 is where a character dies
/// return static_cast<int>( multi_lerp( thirst_thresholds, thirst ) );
/// ```
///
/// `multi_lerp` (`cata_utility.cpp:267-286`) walks past every threshold `<= x`, answers the **first** value below the range and the **last** above it, and interpolates linearly between the two that
/// bracket the value. **The cast truncates toward zero**, so a penalty of −12.5 is **−12** and −37.5 is **−37** - the reference rounds *up* towards zero for penalties, which is the opposite of what
/// "round down" would do to a negative number, and the gate says so with both halves.
#[must_use]
pub fn thirst_speed_penalty(thirst: i32) -> i32 {
    /// The reference's `thirst_thresholds`, in order.
    const THRESHOLDS: [Point; 4] = [(40.0, 0.0), (300.0, -25.0), (600.0, -50.0), (1200.0, -75.0)];
    // `static_cast<int>` truncates toward zero, and Rust's `as` does the same, so the sign is the reference's.
    #[allow(clippy::cast_possible_truncation)]
    let penalty = multi_lerp(&THRESHOLDS, as_float(thirst)) as i32;
    penalty
}


/// What being starved costs a character's speed, by **fat category** rather than by BMI.
///
/// Reference: `Character::kcal_speed_penalty` (`character.cpp:4531-4549`):
///
/// ```text
/// if( !needs_food() ) return 0;
/// { 0 -> -90, emaciated -> -50, underweight -> -25, normal -> 0 }   // fat categories 0, 1, 2, 3
/// if( get_kcal_percent() > 0.95f ) return 0;
/// return std::round( multi_lerp( starv_thresholds, get_bmi_fat() ) );
/// ```
///
/// **The x axis is the fat category, not the BMI**: `get_bmi_fat()` returns a bucket from `character_weight_category` (`game_constants.h:170-179`), so the interpolation happens between buckets and a
/// character sitting between two of them pays the average. **`std::round` takes halves away from zero**, which for a penalty means −12.5 becomes **−13** - the opposite direction from the truncation
/// `thirst_speed_penalty` uses, and the reason both have gates of their own.
#[must_use]
pub fn kcal_speed_penalty(needs_food: bool, kcal_percent: f32, fat_category: f32) -> i32 {
    /// The reference's `starv_thresholds`, in fat-category order.
    const THRESHOLDS: [Point; 4] = [(0.0, -90.0), (1.0, -50.0), (2.0, -25.0), (3.0, 0.0)];
    if !needs_food || kcal_percent > 0.95 {
        return 0;
    }
    // `std::round`: halves away from zero, which is what `f32::round` does.
    #[allow(clippy::cast_possible_truncation)]
    let penalty = multi_lerp(&THRESHOLDS, as_float_of(fat_category)).round() as i32;
    penalty
}
