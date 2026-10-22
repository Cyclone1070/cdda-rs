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

/// A group of noises close enough together to be read as one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Centroid {
    /// Where it is, as the average of its noises weighted by volume.
    pub x: f32,
    /// See [`Centroid::x`].
    pub y: f32,
    /// The loudest noise in it - **not** the sum, because a monster hears the loudest thing.
    pub volume: f32,
    /// The sum of its noises' volumes, which is what the position is averaged by.
    pub weight: f32,
    /// Whether any noise in it should stir a monster.
    pub provokes: bool,
}

/// Groups a turn's noises into at most ten, the way the reference does before any monster reads them.
///
/// Reference: `cluster_sounds` (`src/sounds.cpp:365-431`). Three things are worth knowing before reading this:
///
/// * **A list of at most ten noises is never clustered**: `num_seed_clusters` is `max( min( n, 10 ), log( n ) )` and every seeded
///   cluster is a single noise, so small lists come out exactly as they went in.
/// * **Seeding draws from the dice** and removes the chosen noise by swapping the last one into its place, so the draw order *and*
///   the swap are both part of the answer, and the port has to reproduce both to be the same world.
/// * The position is a **volume-weighted average**, the volume is the **maximum** rather than the sum, and provocation is kept if
///   **any** noise in the group provoked.
// The reference works in `float` throughout, so the port's casts are its own arithmetic.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
#[must_use]
pub fn cluster_sounds(
    rolls: &mut crate::rng::Rolls,
    noises: &[((i32, i32), i32, bool)],
) -> Vec<Centroid> {
    if noises.is_empty() {
        return Vec::new();
    }
    let count = noises.len();
    // `max( min( n, 10 ), log( n ) )`, the reference's `std::log`: the natural log, truncated the way an integer conversion truncates it.
    // For any list a turn can plausibly hold the minimum dominates, so the log only matters for lists past twenty thousand noises - but
    // it is written the way the reference writes it rather than simplified into something that is only equal in practice.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let logged = f64::from(u32::try_from(count).unwrap_or(u32::MAX)).ln() as usize;
    let seeds = usize::min(count, 10).max(logged);
    let stopping_point = count.saturating_sub(seeds);
    let mut remaining: Vec<((i32, i32), i32, bool)> = noises.to_vec();
    let mut clusters: Vec<Centroid> = Vec::new();
    let mut taken = count;
    while taken > stopping_point {
        let index = rolls.rng(0, i32::try_from(taken).unwrap_or(1) - 1);
        let Ok(index) = usize::try_from(index) else {
            break;
        };
        let Some(noise) = remaining.get(index).copied() else {
            break;
        };
        clusters.push(Centroid {
            x: i32_to_f32(noise.0.0),
            y: i32_to_f32(noise.0.1),
            volume: i32_to_f32(noise.1),
            weight: i32_to_f32(noise.1),
            provokes: noise.2,
        });
        // `vector_quick_remove`: the last element takes the hole, which is why the order the rest are read in is not the order they
        // were recorded in.
        let last = taken - 1;
        if index != last {
            remaining.swap(index, last);
        }
        remaining.truncate(last);
        taken -= 1;
    }
    for noise in remaining {
        let Some(nearest) = clusters.iter_mut().min_by_key(|cluster| {
            #[allow(clippy::cast_possible_truncation)]
            let position = (cluster.x as i32, cluster.y as i32);
            let distance = crate::sound::sound_distance(noise.0, position);
            distance * distance
        }) else {
            continue;
        };
        let volume = i32_to_f32(noise.1);
        let weight_sum = volume + nearest.weight;
        if weight_sum != 0.0 {
            nearest.x =
                (f64::from(noise.0.0) as f32 * volume + nearest.x * nearest.weight) / weight_sum;
            nearest.y =
                (f64::from(noise.0.1) as f32 * volume + nearest.y * nearest.weight) / weight_sum;
        }
        nearest.volume = nearest.volume.max(volume);
        nearest.weight = weight_sum;
        nearest.provokes |= noise.2;
    }
    clusters
}

#[allow(clippy::cast_precision_loss)]
fn i32_to_f32(value: i32) -> f32 {
    value as f32
}
