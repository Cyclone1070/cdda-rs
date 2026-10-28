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

use cdda_content::effects::{EffectType, ModAction};
use cdda_core_types::rng::Rolls;

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

impl ActiveEffect {
    /// How likely the effect's per-turn something is to happen, as a percentage: `effect::get_percentage` (`src/effect.cpp:1324-1356`).
    ///
    /// Four rules stack here and none of them reads the way it looks:
    ///
    /// * **A value of zero with no stated chance answers zero**, rather than "always": the chance members are optional, and an effect that states none cannot fire.
    /// * **The bottom of the range is optional, and its absence changes the formula's shape** — with a bottom the chance is `100 * top / bottom`, without one it is
    ///   `100 / top`, so a stated bottom of zero and a stated top of zero mean opposite things.
    /// * **A tick of zero means one**, because most effects fire every turn and the member exists for the ones that do not — but a tick of two halves the chance, and a
    ///   *negative* tick means the thing cannot happen at all, which is a different answer from zero chance.
    /// * The intensity used is the **raw** one, unlike the amount and range readers, which use the effective intensity.
    ///
    /// Integer truncation happens where the reference truncates: the three members are read as `int` before the arithmetic.
    #[must_use]
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    pub fn percentage(&self, kind: &EffectType, argument: &str, value: i32, reduced: bool) -> f64 {
        let level = u8::from(reduced);
        let top = kind.mod_value(argument, ModAction::ChanceTop, level, self.intensity) as i32;
        if value == 0 && top <= 0 {
            return 0.0;
        }
        let bottom = kind.mod_value(argument, ModAction::ChanceBot, level, self.intensity) as i32;
        let mut tick = kind.mod_value(argument, ModAction::Tick, level, self.intensity) as i32;
        if tick == 0 {
            tick = 1;
        }
        let mut percentage = 100.0;
        if bottom != 0 {
            percentage *= f64::from(top) / f64::from(bottom);
        } else {
            percentage /= f64::from(top);
        }
        if tick > 1 {
            percentage /= f64::from(tick);
        }
        percentage
    }

    /// Whether the effect's per-turn something fires this turn: `effect::activated` (`src/effect.cpp:1360-1394`).
    ///
    /// The percentage is only half of it. This adds the **clock** — the thing can only happen on turns the tick divides, and a negative tick means it can never happen —
    /// and the **dice**: one uniform float from zero to one, compared against the chance. A missing bottom means the odds are one in `top`; a stated bottom means `top`
    /// in `bot`, and the caller's `modifier` scales the numerator.
    #[allow(clippy::too_many_arguments, clippy::cast_possible_truncation)]
    pub fn activated(
        &self,
        kind: &EffectType,
        rolls: &mut Rolls,
        turn: i64,
        argument: &str,
        value: i32,
        reduced: bool,
        modifier: f64,
    ) -> bool {
        let level = u8::from(reduced);
        let top = kind.mod_value(argument, ModAction::ChanceTop, level, self.intensity) as i32;
        if value == 0 && top <= 0 {
            return false;
        }
        let mut tick = kind.mod_value(argument, ModAction::Tick, level, self.intensity) as i32;
        if tick == 0 {
            tick = 1;
        } else if tick < 0 {
            return false;
        }
        let bottom = kind.mod_value(argument, ModAction::ChanceBot, level, self.intensity) as i32;
        // The clock: only the turns the tick divides. Turn zero is the first, so a tick of three fires on turns zero, three, six.
        if turn % i64::from(tick) != 0 {
            return false;
        }
        if bottom == 0 {
            rolls.x_in_y(modifier, f64::from(top))
        } else {
            rolls.x_in_y(f64::from(top) * modifier, f64::from(bottom))
        }
    }

    /// Wears the effect down by one turn: `effect::decay` (`src/effect.cpp:1042-1068`).
    ///
    /// Returns whether this turn was the one the effect should be taken off. Two guards decide the intensity half and both are easy to miss: it only steps while the
    /// effect is **short of its ceiling** — an effect at `max_duration` is freshly applied and does not lose intensity yet — and a **positive** step is skipped when
    /// another effect blocks this one (a negative one always applies, so a blocker cannot keep something from getting worse).
    ///
    /// `absolute_turn` is the game's turn count rather than a count of how long this effect has lasted: the tick divides the former.
    pub fn decay(&mut self, kind: &EffectType, absolute_turn: i64, blocked: bool) -> bool {
        let mut removed = false;
        if self.intensity > 0
            && kind.int_decay_tick != 0
            && absolute_turn % i64::from(kind.int_decay_tick) == 0
            && kind.max_duration > self.duration
        {
            if kind.int_decay_step <= 0 || !blocked {
                self.set_intensity(kind, self.intensity.saturating_add(kind.int_decay_step));
            }
            if self.intensity <= 0 {
                removed = true;
            }
        }
        if self.duration <= 0 {
            removed = true;
        } else if !self.permanent {
            // One turn off, which for a duration-sized type recomputes the intensity on the way through `set_duration`.
            self.mod_duration(kind, -1);
        }
        removed
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
