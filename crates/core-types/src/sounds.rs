//! The noises of one turn: what was heard, where, and whether it should stir anything.
//!
//! Reference: `sounds::sound` (`src/sounds.cpp:317-336`) and `sounds::process_sounds` (`:460`). The reference keeps two lists - one for
//! monsters, one for the player - and both are cleared each turn. The port keeps the monster-facing one, which is what a spawn or an AI
//! reads: a square, a volume, and whether the noise provokes.

/// What kind of noise it was.
///
/// Reference: `sounds::sound_t` (`src/sounds.h:23-38`), in the reference's order, because the order is what a save file and a soundpack
/// rely on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SoundCategory {
    /// Weather and the world's own noise.
    Background,
    /// Rain, wind, thunder.
    Weather,
    /// Sonar and the like.
    Sensory,
    /// Music.
    Music,
    /// Footsteps and the like.
    Movement,
    /// Talking, and the player's own voice.
    Speech,
    /// A robot speaking, a radio.
    ElectronicSpeech,
    /// Something the player is doing.
    Activity,
    /// Breaking things.
    DestructiveActivity,
    /// A siren.
    Alarm,
    /// A fight.
    Combat,
    /// Something trying to be heard.
    Alert,
    /// Something that expects to be obeyed.
    Order,
}

impl SoundCategory {
    /// Every category, in the reference's order.
    pub const ALL: [Self; 13] = [
        Self::Background,
        Self::Weather,
        Self::Sensory,
        Self::Music,
        Self::Movement,
        Self::Speech,
        Self::ElectronicSpeech,
        Self::Activity,
        Self::DestructiveActivity,
        Self::Alarm,
        Self::Combat,
        Self::Alert,
        Self::Order,
    ];

    /// Whether this noise should stir a monster.
    ///
    /// Reference: `is_provocative` (`src/sounds.cpp:287-313`). Four categories answer yes - speech, electronic speech, alert and order -
    /// and everything else, a fight included, does not. That is worth stating plainly: **a fight is not what makes a monster come**,
    /// a shout is.
    #[must_use]
    pub fn is_provocative(self) -> bool {
        matches!(
            self,
            Self::Speech | Self::ElectronicSpeech | Self::Alert | Self::Order
        )
    }
}

/// One noise, as a monster hears it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonsterNoise {
    /// Where it came from.
    pub square: (i32, i32),
    /// How loud it was.
    pub volume: i32,
    /// Whether it should stir something.
    pub provokes: bool,
    /// What kind of noise it was.
    pub category: SoundCategory,
}

/// The noises of the current turn.
///
/// Reference: `recent_sounds` (`src/sounds.cpp:332`), a file-scope list that `process_sounds` reads and the turn clears. The port makes
/// the transience explicit: what is recorded is read once by [`SoundQueue::drain`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SoundQueue {
    noises: Vec<MonsterNoise>,
}

impl SoundQueue {
    /// Records a noise, and says whether it was recorded at all.
    ///
    /// Reference: `sounds::sound` (`src/sounds.cpp:317-336`). A **negative** volume records nothing - the reference bails after a debug
    /// message - while a volume of zero is a legitimate noise. Both facts are pinned by tests, because a port that refused zero would
    /// silently swallow silence and one that accepted negatives would keep a noise nobody can hear.
    pub fn record(&mut self, square: (i32, i32), volume: i32, category: SoundCategory) -> bool {
        if volume < 0 {
            return false;
        }
        self.noises.push(MonsterNoise {
            square,
            volume,
            provokes: category.is_provocative(),
            category,
        });
        true
    }

    /// Takes everything this turn recorded, in the order it happened.
    pub fn drain(&mut self) -> Vec<MonsterNoise> {
        std::mem::take(&mut self.noises)
    }

    /// How many noises are waiting to be read.
    #[must_use]
    pub fn len(&self) -> usize {
        self.noises.len()
    }

    /// Whether nothing has been recorded this turn.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.noises.is_empty()
    }
}
