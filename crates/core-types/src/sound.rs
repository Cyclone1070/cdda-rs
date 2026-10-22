//! Sound: how far a noise carries and who is close enough to hear it.
//!
//! Reference: `sound_distance` and the hearing filter in `sounds::process_sounds`
//! (`src/sounds.cpp:253-275` and `:484-489`). A sound carries `volume * 2` squares: a noise of volume
//! ten reaches nineteen squares and not twenty-one. Distance is measured the way the reference
//! measures reach - a straight line in steps where a diagonal is one step - and the reference adds a
//! vertical attenuation for sounds heard across z-levels.
//!
//! The port keeps the flat-map rule. The vertical part, the weather attenuation and the clustering of
//! simultaneous sounds are declared gaps in the ledger.

/// The straight-line distance in the reference's own steps, where a diagonal counts as one.
#[must_use]
pub fn sound_distance(source: (i32, i32), sink: (i32, i32)) -> i32 {
    (source.0 - sink.0).abs().max((source.1 - sink.1).abs())
}

/// Whether a listener at `distance` can hear a sound of `volume` at all.
///
/// This is the reference's own pre-filter, `vol * 2 > dist`, which exists so that the expensive
/// per-creature hearing test only runs for creatures close enough to matter.
#[must_use]
pub fn heard(volume: i32, distance: i32) -> bool {
    volume * 2 > distance
}

/// One noise, at a square, at a volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundEvent {
    /// Where it happened.
    pub source: (i32, i32),
    /// How loud it was.
    pub volume: i32,
}

impl SoundEvent {
    /// Whether a listener on a square hears it, and how loud it is from there.
    ///
    /// The reported volume falls off with distance, which is why a gunshot across a street is loud
    /// and the same shot a block away is barely there. The rule is the reference's own: the volume
    /// less half the distance, never below zero.
    #[must_use]
    pub fn heard_from(&self, listener: (i32, i32)) -> Option<i32> {
        let distance = sound_distance(self.source, listener);
        if !heard(self.volume, distance) {
            return None;
        }
        Some((self.volume - distance / 2).max(0))
    }
}

/// How far a noise has to travel between two **levels**, in the reference's steps.
///
/// Reference: `sound_distance` (`src/sounds.cpp:253-269`). The horizontal part is the ordinary step distance; the vertical part is scaled
/// **five times**, and below ground it is worse: the first level down adds four *before* that scaling and each further level adds twenty
/// more. The reference's own comment calls these "5x" and "100x", which is not what its arithmetic does - the port follows the
/// arithmetic, because that is what a monster's hearing is built from.
#[must_use]
pub fn sound_distance_3d(source: (i32, i32, i32), sink: (i32, i32, i32)) -> i32 {
    let lower_z = source.2.min(sink.2);
    let upper_z = source.2.max(sink.2);
    let displacement = upper_z - lower_z;
    let mut attenuation = displacement;
    if lower_z < 0 && displacement > 0 {
        // The first level down is worth four extra before scaling; deeper than that costs twenty per level, counted from the ground.
        attenuation += 4;
        let underground = lower_z.saturating_neg().min(displacement);
        attenuation += (underground - 1) * 20;
    }
    attenuation *= 5;
    crate::sound::sound_distance((source.0, source.1), (sink.0, sink.1)) + attenuation
}

/// What of a noise survives the weather.
///
/// Reference: `sounds::process_sounds` (`src/sounds.cpp:470`), which subtracts `weather_id->sound_attn` from every cluster's volume
/// before any monster hears it. The subtraction is **unguarded**, so a quiet noise in a storm has a negative volume here - and a port
/// that clamped at zero would report a different number to anything that shows the player what they heard, even where the rules
/// downstream behave the same.
#[must_use]
pub fn heard_volume(volume: i32, weather_attn: i32) -> i32 {
    volume - weather_attn
}

/// The signal a noise sends to hordes on the overmap.
///
/// Reference: `get_signal_for_hordes` (`src/sounds.cpp:432-458`). The weather comes off the volume first; below ground the result is
/// divided by **twice** the depth, so a cellar halves it and a deep shaft silences it. A result of sixty or less sends nothing at all;
/// past that the signal is the volume over `SEEX`, floored at eight and capped at twenty-six, so a distant horde is either not called
/// or called with a usable number - never with a whisper.
#[must_use]
pub fn horde_signal(volume: i32, weather_attn: i32, z: i32) -> i32 {
    /// Hordes cannot hear a volume below this.
    const MIN_VOLUME_CAP: i32 = 60;
    /// Underground, the volume is divided by twice the depth.
    const UNDERGROUND_DIV: i32 = 2;
    /// The reference's `SEEX`, which is the divisor from tiles to submaps.
    const HORDES_SIGNAL_DIV: i32 = 12;
    /// The quietest signal that still counts as one.
    const MIN_SIGNAL_CAP: i32 = 8;
    /// The loudest.
    const MAX_SIGNAL_CAP: i32 = 26;

    let heard = heard_volume(volume, weather_attn);
    let underground = if z < 0 {
        heard / (UNDERGROUND_DIV * z.abs())
    } else {
        heard
    };
    if underground <= MIN_VOLUME_CAP {
        return 0;
    }
    // `std::ceil` of a float, which for a positive integer division means rounding up.
    let signal = (underground + HORDES_SIGNAL_DIV - 1) / HORDES_SIGNAL_DIV;
    signal.clamp(MIN_SIGNAL_CAP, MAX_SIGNAL_CAP)
}
