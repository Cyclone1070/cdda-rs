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
