//! Weighted choices: the list the reference picks its city buildings from.
//!
//! Reference: `weighted_list.h`. An entry's weight is how many tickets it holds in a raffle, and a pick is one ticket
//! drawn by **bits** rather than by a range roll - `pick()` calls `rng_bits()`, so a faithful port must draw the same
//! way or the same seed produces a different town.

/// A list where each entry holds a number of tickets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightedList<T> {
    entries: Vec<(T, i32)>,
    total: i32,
}

impl<T: Clone> WeightedList<T> {
    /// An empty list.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            total: 0,
        }
    }

    /// Builds a list from entries, skipping the ones with no weight.
    ///
    /// The reference stores the weight-zero entries but they can never be picked; dropping them here makes the same
    /// promise with less to go wrong.
    #[must_use]
    pub fn from_entries(entries: impl IntoIterator<Item = (T, i32)>) -> Self {
        let mut list = Self::new();
        for (value, weight) in entries {
            list.add(value, weight);
        }
        list
    }

    /// Adds an entry with a weight. A weight of zero adds nothing.
    pub fn add(&mut self, value: T, weight: i32) {
        if weight > 0 {
            self.total += weight;
            self.entries.push((value, weight));
        }
    }

    /// How many tickets the list holds.
    #[must_use]
    pub fn total_weight(&self) -> i32 {
        self.total
    }

    /// How many entries can be picked.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing can be picked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Picks by a raw ticket number, which is how the reference's `pick( randi )` works.
    ///
    /// The rule is a walk, not a division: entries are visited in order and the ticket number subtracted from each
    /// one's weight until it runs out. The reference wraps the number into the list's range first.
    #[must_use]
    pub fn pick_by_bits(&self, bits: u32) -> Option<&T> {
        if self.total <= 0 {
            return None;
        }
        let mut ticket = i64::from(bits % self.total.unsigned_abs());
        for (value, weight) in &self.entries {
            ticket -= i64::from(*weight);
            if ticket < 0 {
                return Some(value);
            }
        }
        self.entries.last().map(|(value, _)| value)
    }

    /// Picks with the world's dice.
    ///
    /// Reference: `pick()` draws `rng_bits()`, so this draws the same and hands the number to the walk above.
    pub fn pick(&self, rolls: &mut crate::rng::Rolls) -> Option<&T> {
        self.pick_by_bits(rolls.rng_bits())
    }
}

impl<T: Clone> Default for WeightedList<T> {
    fn default() -> Self {
        Self::new()
    }
}
