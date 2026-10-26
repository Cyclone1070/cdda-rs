//! Wearing an effect: what repeated application does to its intensity and its duration.
//!
//! Reference: `Creature::add_effect` (`src/creature.cpp:1731-1856`) and the small mutation rules it leans on — `effect::set_duration` (`src/effect.cpp:1081-1097`),
//! `effect::mod_duration` (`:1098-1101`), `effect::set_intensity` (`:1214-1245`) and `effect::mod_intensity` (`:1247-1250`). The split is worth knowing before reading
//! the code, because it decides the answers:
//!
//! * **A creature that does not have the effect** gets it at the intensity the caller asked for, capped by the type's ceiling, and with the duration capped by the
//!   type's ceiling. A type with a non-zero `int_dur_factor` overrides that: the intensity is *derived* from the duration.
//! * **A creature that already has it** has the duration grown by `dur_add_perc` percent of the new duration — integer arithmetic, so half of three turns is one —
//!   and then its intensity either jumped to the one asked for, or stepped by `int_add_val`, or left alone, in that order of preference.
//! * **Intensity cannot reach zero** unless the type says `int_decay_remove`, so an effect that decays looks stuck at one rather than vanishing.
//!
//! One division truncates and one does not, and confusing them was the first thing the recording caught. `dur * dur_add_perc / 100` is integer arithmetic, so a 50%
//! type grows by less than half — three turns become four, not five. But `ceil( duration / int_dur_factor )` divides **two durations**, and
//! `time_duration / time_duration` answers a `double` (`src/calendar.h:317-319`), so that ceiling is real: 21 minutes against a 10-minute factor is intensity 3, not
//! 2. `units::volume / units::volume` truncates and `time_duration / time_duration` does not — the same-looking operator with opposite behaviour.

use cdda_content::effects::EffectType;

/// One effect a creature is wearing, as far as its numbers go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveEffect {
    /// Which type it is.
    pub id: String,
    /// How strong it is: never below one while it exists.
    pub intensity: i32,
    /// How long it has left, in turns.
    pub duration: i64,
    /// Whether it is paused, which is how "permanent" is spelled.
    pub permanent: bool,
}

impl ActiveEffect {
    /// A freshly applied effect: `effect::effect` plus the tail of `Creature::add_effect`'s "we did not have it" branch (`src/creature.cpp:1824-1847`).
    #[must_use]
    pub fn new(kind: &EffectType, duration: i64, intensity: i32) -> Self {
        let mut effect = Self {
            id: kind.id.clone(),
            intensity,
            duration,
            permanent: false,
        };
        // Capped by the type before anything else reads it, and a duration-based type derives its intensity from the capped figure.
        if effect.duration > kind.max_duration {
            effect.set_duration(kind, kind.max_duration);
        }
        if kind.int_dur_factor != 0 {
            let derived = duration_intensity(effect.duration, kind.int_dur_factor);
            effect.set_intensity(kind, derived.max(1));
        }
        // "Bound new effect intensity by [1, max intensity]."
        if effect.intensity < 1 {
            effect.set_intensity(kind, 1);
        } else if effect.intensity > kind.max_intensity {
            effect.set_intensity(kind, kind.max_intensity);
        }
        effect
    }

    /// Sets the intensity, clamped to the type's ceiling: `effect::set_intensity` (`src/effect.cpp:1214-1245`).
    ///
    /// Returns what the intensity ended up as. A request for zero is a request to be gone, which only a type with `int_decay_remove` honours — for everyone else it
    /// becomes one, so an effect that decays to nothing sits at one until its duration runs out.
    pub fn set_intensity(&mut self, kind: &EffectType, value: i32) -> i32 {
        if self.intensity < 1 {
            self.intensity = 1;
        }
        let mut value = value.clamp(0, kind.max_intensity);
        if value == self.intensity {
            return self.intensity;
        }
        if value == 0 && !kind.int_decay_remove {
            value = 1;
        }
        self.intensity = value;
        self.intensity
    }

    /// Steps the intensity: `effect::mod_intensity` (`src/effect.cpp:1247-1250`).
    pub fn mod_intensity(&mut self, kind: &EffectType, delta: i32) -> i32 {
        self.set_intensity(kind, self.intensity.saturating_add(delta))
    }

    /// Sets the duration, capped by the type: `effect::set_duration` (`src/effect.cpp:1081-1097`).
    ///
    /// A type whose intensity is duration-based has its intensity **recomputed** here, which is why every path that changes a duration goes through this.
    pub fn set_duration(&mut self, kind: &EffectType, duration: i64) {
        self.duration = duration.min(kind.max_duration);
        if kind.int_dur_factor != 0 {
            let derived = duration_intensity(self.duration, kind.int_dur_factor);
            self.set_intensity(kind, derived.max(1));
        }
    }

    /// Adds to the duration: `effect::mod_duration` (`src/effect.cpp:1098-1101`).
    pub fn mod_duration(&mut self, kind: &EffectType, delta: i64) {
        self.set_duration(kind, self.duration.saturating_add(delta));
    }

    /// Applies the same effect again to a creature that already has it: the `found` branch of `Creature::add_effect` (`src/creature.cpp:1782-1821`).
    ///
    /// The duration grows by `dur_add_perc` percent of the new one, then the intensity takes whichever of the three rules the caller set up: an explicit intensity wins,
    /// otherwise the type's own step size, otherwise nothing. "Duration-based intensity overrides all other intensity settings" — that is handled inside
    /// `set_duration`, so the intensity block is skipped entirely for such a type.
    pub fn merge(&mut self, kind: &EffectType, duration: i64, intensity: i32) {
        let grown = duration.saturating_mul(i64::from(kind.dur_add_perc)) / 100;
        self.mod_duration(kind, grown);
        if self.duration > kind.max_duration {
            self.set_duration(kind, kind.max_duration);
        }
        if kind.int_dur_factor > 0 {
            // "Set intensity if value is given" — and nothing to do, because `set_duration` already did it.
        } else if intensity > 0 {
            self.set_intensity(kind, intensity);
        } else if kind.int_add_val != 0 {
            self.mod_intensity(kind, kind.int_add_val);
        }
        // "Bound intensity by [1, max intensity]."
        if self.intensity < 1 {
            self.set_intensity(kind, 1);
        } else if self.intensity > kind.max_intensity {
            self.set_intensity(kind, kind.max_intensity);
        }
    }
}

/// The intensity a duration implies: `ceil( duration / int_dur_factor )` (`src/effect.cpp:1090-1092`).
///
/// Both operands are `time_duration`, whose division is a `double` (`src/calendar.h:317-319`), so this genuinely rounds up: seven minutes against a ten-minute factor
/// is intensity one, and twenty-one minutes is three — where truncation would give two.
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn duration_intensity(duration: i64, factor: i64) -> i32 {
    if factor == 0 {
        return 0;
    }
    let ratio = (duration as f64 / factor as f64).ceil();
    if ratio >= f64::from(i32::MAX) {
        i32::MAX
    } else {
        ratio as i32
    }
}
