//! A field on the map - fire, smoke, blood - and how it fades.
//!
//! Reference: `field_entry` (`src/field.h:20-70`, `src/field.cpp:58-113`). A field has
//! an intensity, an age, and a half-life from its type; every turn the map asks it to
//! decay, and when its age passes its decay time it loses a level and starts again.

use crate::rng::Rolls;

/// Where a field of one type stands on the map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldEntry {
    /// How strong it is. Never drops below one, even when the field is dead, which is
    /// the reference's quirk rather than an oversight - see `set_intensity`.
    intensity: i32,
    /// How long it has been at this intensity.
    age: i64,
    /// Whether the last intensity change left it alive.
    is_alive: bool,
    /// When this intensity should drop, once a decay has been started.
    decay_time: Option<i64>,
    /// The type's limits, copied in so decay needs no registry.
    max_intensity: i32,
    half_life: i64,
    linear_half_life: bool,
    /// Whether this is fire, which decays by its own rule.
    is_fire: bool,
}

impl FieldEntry {
    /// A field at full strength.
    #[must_use]
    pub fn new(max_intensity: i32, half_life: i64, linear_half_life: bool, is_fire: bool) -> Self {
        Self {
            intensity: 1,
            age: 0,
            is_alive: true,
            decay_time: None,
            max_intensity,
            half_life,
            linear_half_life,
            is_fire,
        }
    }

    /// How strong the field is.
    #[must_use]
    pub fn intensity(&self) -> i32 {
        self.intensity
    }

    /// Whether the field still exists as far as the rest of the game is concerned.
    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.is_alive
    }

    /// How long it has been at this intensity.
    #[must_use]
    pub fn age(&self) -> i64 {
        self.age
    }

    /// Sets the strength, clamped to the type's maximum.
    ///
    /// Two rules from the reference (`src/field.cpp:58-63`) that look like bugs and are
    /// not: the intensity never goes below one even when the field is dead, and death
    /// is recorded separately. A port that clamped to zero would lose the dead-but-strong
    /// state the removal code looks at.
    pub fn set_intensity(&mut self, intensity: i32) {
        self.is_alive = intensity > 0;
        self.intensity = intensity.min(self.max_intensity).max(1);
    }

    /// Sets how long the field has been at this intensity, clearing any decay in
    /// progress - the reference's `set_field_age`.
    pub fn set_age(&mut self, age: i64) {
        self.age = age;
        self.decay_time = None;
    }

    /// Ages the field by one turn and drops its intensity when its time is up.
    ///
    /// Fire is not handled here: the reference branches on it before anything else
    /// (`src/field.cpp:98-104`), and that branch needs the dice the map rolls.
    pub fn do_decay(&mut self, now: i64, rolls: &mut Rolls) {
        self.age += 1;
        if self.half_life <= 0 || self.age <= 0 {
            return;
        }
        if self.is_fire {
            // `if half_life < dice(2, age)` - the longer a fire has burned, the likelier
            // it is to lose a level, which is why fires gutter out quickly and slowly.
            let roll = rolls.dice(2, i32::try_from(self.age).unwrap_or(i32::MAX));
            if self.half_life < i64::from(roll) {
                self.age = 0;
                self.set_intensity(self.intensity - 1);
            }
            return;
        }
        let decay_time = if let Some(time) = self.decay_time {
            time
        } else {
            let start = self.decay_start(now, rolls);
            self.decay_time = Some(start);
            start
        };
        if decay_time <= now {
            self.age = 0;
            // The reference resets the age through `set_field_age`, which clears the
            // decay time, so the next half-life is drawn afresh rather than reusing the
            // one that just expired.
            self.decay_time = None;
            self.set_intensity(self.intensity - 1);
        }
    }

    /// When this intensity's decay should fire.
    ///
    /// Linear half-lives decay exactly; the rest draw from an exponential distribution
    /// with rate `1 / (log2(e) * half_life)`, so a field's lifetime is not the same
    /// every time (`src/field.cpp:81-90`).
    fn decay_start(&mut self, now: i64, rolls: &mut Rolls) -> i64 {
        if self.linear_half_life {
            return now - self.age + self.half_life;
        }
        // The reference draws with rate `1 / (log2(e) * half_life)` and adds the result,
        // in turns, to the moment this intensity began.
        #[allow(clippy::cast_precision_loss)]
        let half_life = self.half_life as f64;
        let drawn = rolls.exponential_roll(1.0 / (std::f64::consts::LOG2_E * half_life));
        #[allow(clippy::cast_possible_truncation)]
        let drawn_turns = drawn as i64;
        now - self.age + drawn_turns
    }
}

/// A field type's name, as the registry knows it.
///
/// Zero is the reference's null id, which `add_field` refuses to store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldId(pub u32);

impl FieldId {
    /// The null field type.
    pub const NULL: Self = Self(0);
}

/// The fields sitting on one map tile.
///
/// Reference: `field` (`src/field.cpp:158-200`). A tile holds at most one entry per type,
/// and at most one of them is the type the map draws - the one with the highest
/// priority, with ties going to the later arrival.
#[derive(Debug, Clone, Default)]
pub struct FieldLayer {
    fields: std::collections::BTreeMap<FieldId, FieldEntry>,
    displayed: Option<FieldId>,
    priorities: std::collections::BTreeMap<FieldId, i32>,
}

impl FieldLayer {
    /// Builds an empty tile.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a field, or merges it into the one already there.
    ///
    /// Returns whether a new type was stored: merging an existing type returns false,
    /// which the reference's callers use to tell "this tile just caught fire" from "the
    /// fire got stronger".
    ///
    /// Merging sums the intensities, and a *dead* entry is treated as zero first, so
    /// reviving a spent field gives it the new strength rather than adding to the one it
    /// died with.
    pub fn add(&mut self, id: FieldId, mut entry: FieldEntry, priority: i32) -> bool {
        if id == FieldId::NULL {
            return false;
        }
        if let Some(existing) = self.fields.get_mut(&id) {
            let mut previous = existing.intensity();
            if !existing.is_alive() {
                existing.set_age(entry.age());
                previous = 0;
            }
            existing.set_intensity(previous + entry.intensity());
            return false;
        }
        if self
            .displayed
            .is_none_or(|shown| priority >= self.priority_of(shown))
        {
            self.displayed = Some(id);
        }
        entry.set_age(entry.age());
        self.priorities.insert(id, priority);
        self.fields.insert(id, entry);
        true
    }

    /// How many field types the tile holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Whether the tile holds no fields.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// One field entry.
    #[must_use]
    pub fn get(&self, id: FieldId) -> Option<&FieldEntry> {
        self.fields.get(&id)
    }

    /// One field entry, for changing it in place.
    ///
    /// The reference's `find_field` hands out an iterator for the same purpose: decay
    /// and spread work on the entry where it sits rather than taking it out and putting
    /// it back.
    pub fn get_mut(&mut self, id: FieldId) -> Option<&mut FieldEntry> {
        self.fields.get_mut(&id)
    }

    /// The type the map should draw.
    #[must_use]
    pub fn displayed(&self) -> Option<FieldId> {
        self.displayed
    }

    /// The priority of a stored type.
    ///
    /// The layer does not carry the registry, so priorities are kept beside the entries
    /// rather than looked up. Zero for a type that is not there, which cannot happen
    /// because only stored types are asked about.
    fn priority_of(&self, id: FieldId) -> i32 {
        self.priorities.get(&id).copied().unwrap_or(0)
    }
}
