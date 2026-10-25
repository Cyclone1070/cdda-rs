//! What a bite puts in the stomach: the water, the dry bulk and the nutrition, after everything the character's own body does to it.
//!
//! Reference: `Character::consume_effects` (`src/consumption.cpp:1689-1714`) as far as the call to `stomach.ingest`, which adds the summary to the stomach unchanged
//! (`src/stomach.cpp:392-398`). Four calculations stand between the item's data and that call, and none of them is visible in the item:
//!
//! * **the calories actually absorbed** — `compute_default_effective_kcal` (`:223-258`): a quarter off anything `RAW` that has not been cooked, half off `CARNIVORE_OK`
//!   food for a carnivore, and rot scaling the rest down to nothing at halfway-rotten;
//! * **the vitamins**, converted from the data's **RDA percentages** to internal units on each vitamin's own scale (`:262-302`);
//! * **the water**, which is `quench * 5 mL` — a negative quench is a food that takes water *out* of you;
//! * **the room it takes up**, which is not its volume: the dry bulk is crunched to its dry weight when it is less dense than water, and then multiplied by the food's
//!   calories per gram (square-rooted above three) because a dense food sits heavier in the stomach than its volume says.

use cdda_content::vitamins::{self, Vitamin};
use std::collections::{BTreeMap, BTreeSet};

/// What a bite puts in the stomach, which is exactly what `stomach::ingest` adds.
///
/// Reference: `food_summary` (`src/stomach.h`). The nutrition is kept in **thousandths of a kilocalorie** (`nutrients::calories`) with `kcal()` as the integer
/// division by a thousand, which is why both are here: the stomach's bookkeeping is in the thousandths and every displayed number is in the whole.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FoodSummary {
    /// The water it carries, in millilitres. **Negative** for a food that dries you out.
    pub water_ml: i64,
    /// The dry bulk it takes up in the stomach, in millilitres: `dry volume * volume ratio`.
    pub solids_ml: i64,
    /// Nutrition in thousandths of a kilocalorie, which is `nutrients::calories`.
    pub calories: i64,
    /// Vitamins in internal units, each on its own scale.
    pub vitamins: BTreeMap<String, i32>,
}

impl FoodSummary {
    /// The nutrition in whole kilocalories: `nutrients::kcal` (`src/stomach.h`), an **integer** division.
    #[must_use]
    pub fn kcal(&self) -> i64 {
        self.calories / 1_000
    }
}

/// What the eater's body does to the food on the way in.
///
/// Only the two traits the reference applies **before** ingestion live here. Everything else about an eater — whether they are sick, a gourmand, what they think of
/// the taste — belongs to [`crate::fun_for`] and to digestion, not to this.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Diet {
    /// `CARNIVORE_DIET`: a carnivore gets half the nutrition out of `CARNIVORE_OK` food that also has plant vitamins in it.
    pub carnivore: bool,
    /// `SAPROPHAGE`: rotten food is full nutrition for them, and nothing is lost.
    pub saprophage: bool,
}

/// The calories a bite is actually worth: `compute_default_effective_kcal` (`src/consumption.cpp:223-258`).
///
/// `stated_kcal` is the comestible's own `kcal` — the stored thousandths divided by a thousand, an integer division the slot has already done. The reference works in
/// `float` and truncates once at the end, so this does too: `RAW` uncooked food loses exactly a quarter and the rot factor is the same `clamp(2 * relative_rot - 2,
/// 0.1, 1)` the taste rule uses.
// The reference multiplies a `float` and truncates to `int`; both casts are that arithmetic.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
#[must_use]
pub fn effective_kcal(
    stated_kcal: i64,
    flags: &BTreeSet<String>,
    relative_rot: f32,
    diet: Diet,
) -> i32 {
    let mut kcal = stated_kcal as f32;

    // "Many raw foods give less calories, as your body expends more energy digesting them."
    if flags.contains("RAW") && !flags.contains("COOKED") {
        kcal *= 0.75;
    }

    // A carnivore gets half out of plant-tainted meat. The vitamin blacklist that decides "tainted" is `carnivore_blacklist` in the reference.
    if diet.carnivore && flags.contains("CARNIVORE_OK") && flags.contains("CARNIVORE_BLACKLISTED") {
        kcal *= 0.5;
    }

    // Rot: full value for a saprophage, and a linear slide to nothing at halfway-rotten for everyone else.
    if relative_rot > 1.0 && !diet.saprophage {
        let rottedness = (2.0 * relative_rot - 2.0).clamp(0.1, 1.0);
        kcal *= 1.0 - rottedness;
    }

    (kcal.max(0.0)) as i32
}

/// Turns the slot's stated vitamins into the units a body receives: `compute_default_effective_vitamins` (`src/consumption.cpp:262-302`).
///
/// The data writes a vitamin as an **RDA percentage**, so `["vitC", 32]` is 32% of a day's vitamin C and not 32 units; `RDA_to_default` is what converts it, on each
/// vitamin's own rate. A vitamin the registry does not know is dropped rather than guessed at, which is what the reference's `obj()` would abort on.
#[must_use]
pub fn effective_vitamins(
    stated: &BTreeMap<String, i32>,
    registry: &BTreeMap<String, Vitamin>,
) -> BTreeMap<String, i32> {
    let mut out = BTreeMap::new();
    for (id, percent) in stated {
        let Some(vitamin) = registry.get(id) else {
            continue;
        };
        out.insert(id.clone(), vitamins::rda_to_default(vitamin, *percent));
    }
    out
}

/// What one item of a stack takes up, which is what the summary divides: `item::volume` for the charge-counted case (`src/item.cpp:7546-7562`).
///
/// A charge-counted item shares its type's volume between `stack_size` charges — a bottle of bleach is a quarter of the type's volume per charge — and the division
/// **rounds up a millilitre**, so nothing can ever be zero-sized. Everything else takes up its stated volume whatever its charge count.
#[must_use]
pub fn bite_volume(volume_ml: i64, charges: i64, stack_size: i64, count_by_charges: bool) -> i64 {
    if !count_by_charges {
        return volume_ml;
    }
    let stack = stack_size.max(1);
    let total = volume_ml.saturating_mul(charges.max(1));
    (total + stack - 1) / stack
}

/// The water and the dry bulk of a bite: `Character::masticated_volume` (`src/consumption.cpp:1524-1543`).
///
/// The water is `quench * 5 mL` in both directions — a food with a negative quench carries **negative** water, which the caller turns into thirst. What is left is the
/// dry volume, and if that is less dense than water — its dry weight in grams is smaller than its dry volume in millilitres — it is crunched down to its dry weight.
/// With 1 g = 1 mL the comparison needs no scale factor.
#[must_use]
pub fn masticated_volume(quench: i32, weight_g: i64, volume_ml: i64, count: i64) -> (i64, i64) {
    let count = count.max(1);
    let water_ml = i64::from(quench) * 5;
    let dry_weight_g = weight_g / count - water_ml;
    let dry_volume_ml = volume_ml / count - water_ml;
    if dry_volume_ml <= 0 {
        return (water_ml, 0);
    }
    if dry_weight_g > 0 && dry_weight_g < dry_volume_ml {
        return (water_ml, dry_weight_g);
    }
    (water_ml, dry_volume_ml)
}

/// How much room a food takes up for the nutrition in it: `Character::compute_effective_food_volume_ratio` (`src/consumption.cpp:1506-1519`).
///
/// Calories per gram, floored at one, and **square-rooted above three** so that a very dense food takes up more room than its volume says without taking up an absurd
/// amount. A food of no weight ratios at one.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn food_volume_ratio(kcal: i32, weight_g: i64) -> f64 {
    if weight_g == 0 {
        return 1.0;
    }
    let ratio = (f64::from(kcal) / weight_g as f64).max(1.0);
    if ratio > 3.0 {
        return (3.0 * ratio).sqrt();
    }
    ratio
}

/// The few numbers about an item that the summary needs, which the item model supplies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemFacts<'a> {
    /// What it weighs, in grams.
    pub weight_g: i64,
    /// What it takes up, in millilitres.
    pub volume_ml: i64,
    /// How many of it make one bite, which the reference divides the weight and volume by.
    pub count: i64,
    /// How much water it carries, in five-millilitre units — the slot's `quench`, in both directions.
    pub quench: i32,
    /// The **stated** nutrition, in kilocalories, before the body does anything to it.
    pub stated_kcal: i64,
    /// The **stated** vitamins, as RDA percentages.
    pub stated_vitamins: &'a BTreeMap<String, i32>,
}

/// The whole summary for one bite of one item.
///
/// The dry bulk is scaled by the volume ratio **after** the crunch, in millilitres, and truncated: `units::volume * double` keeps whole millilitres.
// `units::volume * double` is `double` arithmetic truncated to whole millilitres; both casts are that rule.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
#[must_use]
pub fn food_summary(
    facts: &ItemFacts<'_>,
    flags: &BTreeSet<String>,
    relative_rot: f32,
    diet: Diet,
    registry: &BTreeMap<String, Vitamin>,
) -> FoodSummary {
    let (water_ml, dry_ml) =
        masticated_volume(facts.quench, facts.weight_g, facts.volume_ml, facts.count);
    let kcal = effective_kcal(facts.stated_kcal, flags, relative_rot, diet);
    let ratio = food_volume_ratio(kcal, facts.weight_g / facts.count.max(1));
    FoodSummary {
        water_ml,
        solids_ml: (dry_ml as f64 * ratio) as i64,
        calories: i64::from(kcal) * 1_000,
        vitamins: effective_vitamins(facts.stated_vitamins, registry),
    }
}
