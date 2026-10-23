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
        // **A fresh deadline every turn.** `CDDA_DECAY_TRACE` on the reference shows `decay_time` at zero on the entry of *every* `do_decay`, so
        // `initialize_decay` runs each time and the value it computed last turn is never reused. Caching it instead kept a field alive that the
        // reference buries: with the harness's calendar standing still, a deadline once set ahead can never be reached.
        let decay_time = self.decay_start(now, rolls);
        self.decay_time = Some(decay_time);
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
        // **The reference's own draw, when a recording is attached.** `initialize_decay` draws its exponential straight from the engine
        // (`src/field.cpp:86-88`, recorded as `drawn decay.turns`), so a replay that computed its own would decay a field on a different schedule
        // and leave a corpse where the reference has a living field - or the other way round.
        if let Some(drawn) = rolls.recorded_engine_draw("decay.turns") {
            return now - self.age + drawn;
        }
        // Otherwise the same rule: rate `1 / (log2(e) * half_life)`, truncated to whole turns because `from_turns` takes whole turns.
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

/// What one intensity level of a field does on its turn.
///
/// The numbers are the field's own: its type carries them per level, and the port hands them over as a value rather than letting a
/// spawner keep a second copy. Only the processors the port models are here - monster spawning, and the intensity upgrade that
/// competes with it for the same turn. Reference: the processor list `field_type` registers at load time
/// (`src/field_type.cpp:350`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldTurn {
    /// The chance in a hundred that this level spawns a group.
    pub spawn_chance: i32,
    /// How many spawns to attempt.
    pub spawn_count: i32,
    /// The group to draw from.
    pub spawn_group: String,
    /// How far from the field the spawns appear.
    pub spawn_radius: i32,
    /// The chance in a hundred that the level upgrades instead.
    pub upgrade_chance: i32,
    /// How often the upgrade is rolled, in turns; zero disables it.
    pub upgrade_duration: i64,
}
