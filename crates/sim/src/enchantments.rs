//! What an enchantment does to a number.
//!
//! Reference: `enchantment` and `enchant_cache` (`src/magic_enchantment.h`, `src/magic_enchantment.cpp`). An enchantment is a list of values, each with an **add** and a **multiply**, and applying
//! one is two lines of arithmetic: `value += add; value *= 1.0 + multiply` (`magic_enchantment.cpp:1396-1400`).
//!
//! Two things about it are easy to get wrong and both are the reference's own behaviour:
//!
//! - **the multiply is a fraction of the number, not of the result** - a multiply of `0.5` is `+50%`, so the arithmetic is `1.0 + multiply` and not `multiply`;
//! - **a value named twice keeps its first row**, because the loader fills a `std::map` with `emplace`, which does not overwrite (`:204-232`).
//!
//! What an enchantment *carries besides* numbers - the hit and dodge effects, the mutations, the intermittent activation, the conditions - is named in `docs/parity.md` rather than invented here.

use std::collections::BTreeMap;

/// One enchantment's numbers: what it adds to a value and what it multiplies it by.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Enchantment {
    /// What it adds, by value name.
    adds: BTreeMap<String, f64>,
    /// What it multiplies by, by value name. **A fraction of the number**: `0.5` is half again.
    multiplies: BTreeMap<String, f64>,
}

impl Enchantment {
    /// States what this enchantment adds to a value.
    pub fn add(&mut self, value: &str, amount: f64) {
        self.adds.insert(value.to_owned(), amount);
    }

    /// States what this enchantment multiplies a value by, as a fraction: `0.5` is half again.
    pub fn multiply(&mut self, value: &str, fraction: f64) {
        self.multiplies.insert(value.to_owned(), fraction);
    }

    /// What this enchantment adds to a value, and nothing when it says nothing about it.
    #[must_use]
    pub fn add_of(&self, value: &str) -> f64 {
        self.adds.get(value).copied().unwrap_or(0.0)
    }

    /// What this enchantment multiplies a value by, and nothing when it says nothing about it.
    #[must_use]
    pub fn multiply_of(&self, value: &str) -> f64 {
        self.multiplies.get(value).copied().unwrap_or(0.0)
    }

    /// The number after the enchantment has had it: **added to, then multiplied by one plus the fraction**.
    ///
    /// Reference: `enchant_cache::modify_value( enchant_vals::mod, double )` (`magic_enchantment.cpp:1396-1400`), whose two lines are the whole rule and whose order matters.
    #[must_use]
    pub fn modify(&self, value: f64, named: &str) -> f64 {
        let added = value + self.add_of(named);
        added * (1.0 + self.multiply_of(named))
    }
}

/// Adds one row of an enchantment's data to its numbers, **keeping the first row for a value**.
///
/// Reference: `load_add_and_multiply` (`magic_enchantment.cpp:204-232`): a row with an `add` and no `multiply` fills only the add map, and `emplace` drops a second row for a value the map already
/// has. The port passes `None` for a member the row does not carry, so the two maps stay as separate as the reference keeps them.
pub fn merge(enchantment: &mut Enchantment, value: &str, add: Option<f64>, multiply: Option<f64>) {
    if let Some(amount) = add {
        enchantment.adds.entry(value.to_owned()).or_insert(amount);
    }
    if let Some(fraction) = multiply {
        enchantment
            .multiplies
            .entry(value.to_owned())
            .or_insert(fraction);
    }
}
