//! Lists whose entries carry a weight, and picking from them.
//!
//! Reference: `src/weighted_list.h`. Two lists live there - one weighted by whole
//! numbers, one by fractions - and this is the whole-number one the port needs
//! first. The rules worth keeping in mind are all about what a *rejected* or
//! *empty* case does, because the game rolls those constantly.

use crate::rng::Rolls;

/// One entry and the weight it was given.
#[derive(Debug, Clone)]
struct Entry<T> {
    value: T,
    weight: i32,
}

/// A list of values weighted by whole numbers.
///
/// Reference: `weighted_int_list<T>` in `src/weighted_list.h:228-275`.
#[derive(Debug, Clone)]
pub struct WeightedIntList<T> {
    entries: Vec<Entry<T>>,
    total_weight: i32,
}

impl<T> Default for WeightedIntList<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            total_weight: 0,
        }
    }
}

impl<T: PartialEq> WeightedIntList<T> {
    /// Builds an empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an entry, returning whether it was stored.
    ///
    /// A weight of zero or less is refused rather than stored as a dead entry
    /// (`weighted_list.h:41-49`), so an empty list and a list of zero weights are
    /// the same thing.
    pub fn add(&mut self, value: T, weight: i32) -> bool {
        if weight <= 0 {
            return false;
        }
        self.entries.push(Entry { value, weight });
        self.total_weight += weight;
        true
    }

    /// Adds an entry, or changes the weight of an entry already present.
    ///
    /// A weight of zero removes the entry - that is what the reference does, and
    /// it is the only way a loader can say "and this one is gone now"
    /// (`weighted_list.h:73-77`). A negative weight is refused.
    pub fn add_or_replace(&mut self, value: T, weight: i32) -> bool {
        if weight == 0 {
            self.remove(&value);
            return false;
        }
        if weight < 0 {
            return false;
        }
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.value == value) {
            self.total_weight += weight - entry.weight;
            entry.weight = weight;
            return true;
        }
        self.add(value, weight)
    }

    /// Removes every entry equal to `value`, subtracting their weight.
    pub fn remove(&mut self, value: &T) {
        self.entries.retain(|entry| {
            if entry.value == *value {
                self.total_weight -= entry.weight;
                false
            } else {
                true
            }
        });
    }

    /// Empties the list.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.total_weight = 0;
    }

    /// The weight of everything in the list.
    #[must_use]
    pub fn total_weight(&self) -> i32 {
        self.total_weight
    }

    /// The weight given to `value`, or zero if it is not present.
    #[must_use]
    pub fn specific_weight(&self, value: &T) -> i32 {
        self.entries
            .iter()
            .find(|entry| entry.value == *value)
            .map_or(0, |entry| entry.weight)
    }

    /// How many entries the list holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the list holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Picks an entry using a given draw, which is what the reference's
    /// `pick(unsigned)` overload is for (`weighted_list.h:137-143`).
    ///
    /// The rule: `picked = (draw % total) + 1`, then the first entry whose running
    /// weight reaches `picked`. A single entry short-circuits to index zero, and
    /// so does any list whose total is reached by its first entry.
    #[must_use]
    pub fn pick_with(&self, draw: u32) -> Option<&T> {
        if self.total_weight <= 0 {
            return None;
        }
        if self.entries.len() == 1 {
            return self.entries.first().map(|entry| &entry.value);
        }
        let picked = i32::try_from(draw % u32::try_from(self.total_weight).ok()?).ok()? + 1;
        let mut accumulated = 0;
        for entry in &self.entries {
            accumulated += entry.weight;
            if accumulated >= picked {
                return Some(&entry.value);
            }
        }
        self.entries.last().map(|entry| &entry.value)
    }

    /// Picks an entry, drawing from the stream.
    ///
    /// The draw happens whether or not there is anything to pick: the reference
    /// evaluates `rng_bits()` before it looks at the total
    /// (`weighted_list.h:144-146`), so asking an empty list costs one draw. A port
    /// that returned early would put every later roll out of step.
    pub fn pick(&mut self, rolls: &mut Rolls) -> Option<&T> {
        let draw = rolls.rng_bits();
        self.pick_with(draw)
    }
}

/// A list whose entries are weighted by fractions.
///
/// Reference: `weighted_float_list<T>` in `src/weighted_list.h:277-296`. It shares
/// the storage and the refusal rules with the whole-number list and differs only
/// in how a draw becomes a position: the draw is scaled across the whole 32-bit
/// range instead of being taken modulo the total.
#[derive(Debug, Clone)]
pub struct WeightedFloatList<T> {
    entries: Vec<EntryFloat<T>>,
    total_weight: f64,
}

#[derive(Debug, Clone)]
struct EntryFloat<T> {
    value: T,
    weight: f64,
}

impl<T> Default for WeightedFloatList<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            total_weight: 0.0,
        }
    }
}

impl<T: PartialEq> WeightedFloatList<T> {
    /// Builds an empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an entry, returning whether it was stored. A weight of zero or less
    /// is refused, as in the whole-number list.
    pub fn add(&mut self, value: T, weight: f64) -> bool {
        if weight <= 0.0 {
            return false;
        }
        self.entries.push(EntryFloat { value, weight });
        self.total_weight += weight;
        true
    }

    /// Empties the list.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.total_weight = 0.0;
    }

    /// The weight of everything in the list.
    #[must_use]
    pub fn total_weight(&self) -> f64 {
        self.total_weight
    }

    /// How many entries the list holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the list holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Picks an entry using a given draw.
    ///
    /// Reference: `weighted_list.h:281-292`. The draw is divided by the maximum
    /// word and multiplied by the total, so the range of draws maps onto the run
    /// of weights without any modulo wraparound. Note there is no one-entry
    /// shortcut here: the reference's float list does not have one, and adding a
    /// shortcut would change which floating comparison decides the answer.
    #[must_use]
    pub fn pick_with(&self, draw: u32) -> Option<&T> {
        if self.entries.is_empty() || self.total_weight <= 0.0 {
            return None;
        }
        let picked = (f64::from(draw) / f64::from(u32::MAX)) * self.total_weight;
        let mut accumulated = 0.0;
        for entry in &self.entries {
            accumulated += entry.weight;
            if accumulated >= picked {
                return Some(&entry.value);
            }
        }
        self.entries.last().map(|entry| &entry.value)
    }

    /// Picks an entry, drawing from the stream.
    ///
    /// As with the whole-number list, the draw happens even when there is nothing
    /// to pick (`weighted_list.h:144-146`).
    pub fn pick(&mut self, rolls: &mut Rolls) -> Option<&T> {
        let draw = rolls.rng_bits();
        self.pick_with(draw)
    }
}
