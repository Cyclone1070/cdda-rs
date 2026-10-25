//! The stomach emptying itself: what leaves it, how fast, and where it goes.
//!
//! Reference: `stomach_contents` (`src/stomach.h:132-260`) and its two rules — `digest` (`src/stomach.cpp:400-437`), which takes material out, and `get_digest_rates`
//! (`:440-476`), which decides how much of each thing may leave per interval.
//!
//! There are two compartments and they are **different machines wearing the same class**. The **stomach** passes material on to the guts in half-hour steps at a sixth
//! of its capacity; the **guts** absorb into the body, and their rates are not a share of a capacity at all but the character's metabolism — 5% of the daily intake per
//! half hour, with a floor of `kcal / 24 * hunger * 1000` thousandths of a calorie. Everything the stomach releases goes **into** the guts, and only what the guts
//! release reaches the body (`src/character_body.cpp:948-975`), which is why eating a big meal does not feed you immediately.
//!
//! Three details are easy to get wrong and are all recorded:
//!
//! * **water leaves in five-minute steps** and solids in half-hour ones, so the caller passes both counts and a zero half-hour count digests water alone;
//! * a food that carries **negative** water — anything dry or salty — has that water taken back out of the stomach, and `min( water, rate )` hands the negative figure
//!   straight through;
//! * a **drug** vitamin skips the trickle: the stomach dumps it whole in one step so a dose is a dose, while the guts treat it like any other vitamin.

use crate::food_summary::FoodSummary;
use cdda_content::vitamins::Vitamin;
use std::collections::BTreeMap;

/// The character's metabolic pressure, as the rate rule sees it.
///
/// Reference: `needs_rates` (`src/character.h:493-499`), of which four members reach the digestion rates: the daily `kcal`, and the `hunger`, `thirst` and (not used
/// here) `sleepiness` pressures. The recording fixes them at a maintenance 2,500 kcal and a hunger of 1, so the port's rates can be compared against the reference's
/// without dragging an avatar's whole day into the comparison.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MetabolicRates {
    /// The daily calorie intake the body is working at.
    pub kcal: f32,
    /// How hard the body is working, as a multiplier on its absorption.
    pub hunger: f32,
    /// How thirsty it is, which the digestion rates do not consult.
    pub thirst: f32,
    /// How sleepy it is, which the digestion rates do not consult.
    pub sleepiness: f32,
}

/// How much of each thing may leave a compartment per interval.
///
/// Reference: `stomach_digest_rates` (`src/stomach.h:60-80`). Solids and water are in millilitres per interval; the calories are in **thousandths** of a kilocalorie,
/// which is the unit the stomach does all its arithmetic in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DigestRates {
    /// Solid bulk per half hour, in millilitres.
    pub solids_ml: i64,
    /// Water per five minutes, in millilitres.
    pub water_ml: i64,
    /// The fewest units of a vitamin that leave in one step.
    pub min_vitamin: i32,
    /// The share of a vitamin that leaves in one step.
    pub percent_vitamin: f32,
    /// The fewest thousandths of a kilocalorie that leave in one half hour.
    pub min_calories: i64,
    /// The share of the calories that leaves in one half hour.
    pub percent_kcal: f32,
}

/// The rates one compartment works at: `stomach_contents::get_digest_rates` (`src/stomach.cpp:440-476`).
///
/// The stomach's solid rate is a sixth of its **capacity**, so a bigger stomach empties at the same pace in hours rather than the same pace in millilitres. The guts'
/// rate is the metabolism's, and its minimum calorie release is the daily intake spread over the day and multiplied by the hunger pressure — `floor`ed, because the
/// reference casts it to an integer and that floors anyway.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
#[must_use]
pub fn digest_rates(metabolic: &MetabolicRates, is_stomach: bool, capacity_ml: i64) -> DigestRates {
    if is_stomach {
        DigestRates {
            // "3 hours to empty in 30 minute increments."
            solids_ml: capacity_ml / 6,
            water_ml: 250,
            min_vitamin: 1,
            percent_vitamin: 1.0 / 6.0,
            min_calories: 5_000,
            percent_kcal: 1.0 / 6.0,
        }
    } else {
        DigestRates {
            // A flat 250 mL: the guts are not a container, so the rate only has to be big enough to avoid overflow.
            solids_ml: 250,
            water_ml: 250,
            min_calories: f64::from(metabolic.kcal / 24.0 * metabolic.hunger * 1000.0).floor() as i64,
            percent_kcal: 0.05 * metabolic.hunger,
            min_vitamin: (100.0 / 24.0 * f64::from(metabolic.hunger)).round() as i32,
            percent_vitamin: 0.05 * metabolic.hunger,
        }
    }
}

/// What one compartment holds, and how fast it lets go.
///
/// Reference: `stomach_contents` (`src/stomach.h:132-260`). The `is_stomach` flag is not a label: it selects one of two completely different rate rules, and a
/// compartment with the flag **set** is the one that dumps drug vitamins whole.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StomachContents {
    /// The solid bulk it holds, in millilitres.
    pub contents_ml: i64,
    /// The water it holds, in millilitres. **Negative** after a drying food until it is absorbed.
    pub water_ml: i64,
    /// Nutrition held, in thousandths of a kilocalorie.
    pub calories: i64,
    /// Vitamins held, in internal units.
    pub vitamins: BTreeMap<String, i32>,
    /// Its base capacity, in millilitres.
    pub max_volume_ml: i64,
    /// Whether it is a stomach rather than a set of guts.
    pub is_stomach: bool,
}

impl StomachContents {
    /// A compartment with a base capacity, of one kind or the other.
    #[must_use]
    pub fn new(max_volume_ml: i64, is_stomach: bool) -> Self {
        Self {
            max_volume_ml,
            is_stomach,
            ..Self::default()
        }
    }

    /// Its capacity: `stomach_contents::capacity` (`src/stomach.cpp:362-366`), which floors at a quarter of a litre and is what an enchantment would scale.
    ///
    /// **Declared:** the `STOMACH_SIZE_MULTIPLIER` enchantment modifier, which needs an enchantment cache the port does not have yet.
    #[must_use]
    pub fn capacity_ml(&self) -> i64 {
        self.max_volume_ml.max(250)
    }

    /// Everything it holds, solids and water: `stomach_contents::contains` (`src/stomach.cpp:387-390`).
    #[must_use]
    pub fn contains_ml(&self) -> i64 {
        self.contents_ml + self.water_ml
    }

    /// The calories it holds, in whole kilocalories: `stomach_contents::get_calories`.
    #[must_use]
    pub fn kcal(&self) -> i64 {
        self.calories / 1_000
    }

    /// Empties it: `stomach_contents::empty`.
    pub fn empty(&mut self) {
        self.contents_ml = 0;
        self.water_ml = 0;
        self.calories = 0;
        self.vitamins.clear();
    }

    /// Puts a bite in: `stomach_contents::ingest` (`src/stomach.cpp:392-398`), which is a plain addition of the four numbers.
    pub fn ingest(&mut self, ingested: &FoodSummary) {
        self.contents_ml += ingested.solids_ml;
        self.water_ml += ingested.water_ml;
        self.calories += ingested.calories;
        for (id, units) in &ingested.vitamins {
            *self.vitamins.entry(id.clone()).or_insert(0) += units;
        }
    }

    /// Lets material go for `five_mins` five-minute intervals and `half_hours` half-hour ones: `stomach_contents::digest` (`src/stomach.cpp:400-437`).
    ///
    /// The answer is what left, in the same shape as what went in — which is exactly what the caller feeds to the next compartment, or to the body.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    pub fn digest(
        &mut self,
        rates: &DigestRates,
        five_mins: i64,
        half_hours: i64,
        vitamins: &BTreeMap<String, Vitamin>,
    ) -> FoodSummary {
        // Water goes first and in five-minute steps, and never more than there is - a negative figure passes straight through.
        let water_ml = self.water_ml.min(rates.water_ml * five_mins);
        self.water_ml -= water_ml;
        let mut digested = FoodSummary {
            water_ml,
            ..FoodSummary::default()
        };

        // With no half hour gone by, only water has moved.
        if half_hours == 0 {
            return digested;
        }

        digested.solids_ml = self.contents_ml.min(rates.solids_ml * half_hours);
        self.contents_ml -= digested.solids_ml;

        // "Use min_kcal by default, but no more than what's in stomach, and no less than percentage_kcal of what's in stomach." The order of the two clamps matters
        // when the minimum is larger than what is left: the reference's `std::clamp` answers the upper bound, so the whole remainder leaves.
        let fraction = (self.kcal() as f32 * rates.percent_kcal).round() as i64;
        digested.calories = half_hours * rates.min_calories.max(fraction * 1_000).min(self.calories);

        for (id, units) in &self.vitamins {
            let kind = vitamins.get(id).map_or("vitamin", |vitamin| vitamin.vit_type.as_str());
            if kind != "drug" {
                let fraction = (*units as f32 * rates.percent_vitamin).round() as i32;
                digested
                    .vitamins
                    .insert(id.clone(), (half_hours as i32) * rates.min_vitamin.max(fraction).min(*units));
            } else if self.is_stomach {
                // "Drug vitamins are absorbed to the blood instantly after the first stomach step."
                digested.vitamins.insert(id.clone(), *units);
            }
        }

        self.calories -= digested.calories;
        for (id, units) in &digested.vitamins {
            if let Some(held) = self.vitamins.get_mut(id) {
                *held -= units;
                if *held == 0 {
                    self.vitamins.remove(id);
                }
            }
        }
        digested
    }
}
