//! Morale: what a creature is feeling, and how it fades.
//!
//! Reference: `player_morale::morale_point` (`morale.cpp:130-240`). A morale point holds a bonus, a cap, a duration and a point partway through
//! that duration at which it starts to fade; stacking two points of the same sign uses the square root of the sum of squares, which is why
//! three small good things never add up to one big one.

use bevy_ecs::prelude::*;

/// One thing a creature is feeling.
#[derive(Debug, Clone, PartialEq)]
pub struct MoralePoint {
    /// The morale type's id, which is what stacks.
    pub kind: String,
    /// The bonus before decay, capped at `max_bonus`.
    pub bonus: i32,
    /// The largest the bonus may reach; zero means uncapped unless `capped`.
    pub max_bonus: i32,
    /// How long it lasts in turns; zero means it never runs out.
    pub duration: i64,
    /// The age at which it starts to fade.
    pub decay_start: i64,
    /// The age it has reached, which the turn system advances.
    pub age: i64,
    /// Whether the cap is applied even when `max_bonus` is zero.
    pub capped: bool,
}

/// Every morale point a creature carries.
#[derive(Debug, Clone, Default, PartialEq, Component)]
pub struct Morale(pub Vec<MoralePoint>);

impl Morale {
    /// What the points add up to right now.
    #[must_use]
    pub fn net_bonus(&self) -> i32 {
        self.0.iter().map(MoralePoint::net_bonus).sum()
    }
}

impl MoralePoint {
    /// Whether this point never runs down.
    #[must_use]
    pub fn is_permanent(&self) -> bool {
        self.duration == 0
    }

    /// Whether this point has run its course.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        (!self.is_permanent() && self.age >= self.duration) || self.bonus == 0
    }

    /// The bonus as it stands now, after the fade.
    ///
    /// Reference: `morale_point::get_net_bonus` (`morale.cpp:136-143`): a permanent or un-faded point gives its whole bonus, and past
    /// `decay_start` the bonus is scaled by a logistic fall from one at `decay_start` to zero at `duration`.
    #[must_use]
    pub fn net_bonus(&self) -> i32 {
        if !self.is_permanent() && self.age > self.decay_start {
            let fell = logarithmic_range(self.decay_start, self.duration, self.age);
            faded(self.bonus, fell)
        } else {
            self.bonus
        }
    }
}

/// A bonus as it stands after falling by a fraction of itself.
#[allow(clippy::cast_possible_truncation)]
fn faded(bonus: i32, fell: f64) -> i32 {
    (f64::from(bonus) * fell).round() as i32
}

/// The logistic curve the reference fades morale along.
///
/// Reference: `logarithmic_range` (`cata_utility.cpp:166-195`): one at the start, zero at the end, and a logistic between them with the ends
/// clamped - so a point that is un-faded gives its whole bonus and one at its end gives none.
#[must_use]
pub fn logarithmic_range(min: i64, max: i64, position: i64) -> f64 {
    const CUTOFF: f64 = 4.0;
    if min >= max || position <= min {
        return 1.0;
    }
    if position >= max {
        return 0.0;
    }
    let logistic = |value: f64| 1.0 / (1.0 + (-value).exp());
    let lowest = logistic(-CUTOFF);
    let highest = logistic(CUTOFF);
    let range = highest - lowest;
    #[allow(clippy::cast_precision_loss)]
    let unit = (position - min) as f64 / (max - min) as f64;
    let scaled = CUTOFF - 2.0 * CUTOFF * unit;
    (logistic(scaled) - lowest) / range
}

/// Adds a morale point to a creature, stacking it with the point of the same kind.
///
/// Reference: `player_morale::morale_point::add` (`morale.cpp:160-204`): a permanent or capped point takes the new bonus whole, an opposing one
/// adds, and two of the same sign stack as the square root of the sum of squares - the reason three snacks do not feel three times as good.
pub fn add(world: &mut World, creature: Entity, mut point: MoralePoint) {
    let mut points = world.get::<Morale>(creature).cloned().unwrap_or_default();
    let existing = points
        .0
        .iter_mut()
        .find(|held| held.kind == point.kind);
    let Some(held) = existing else {
        point.bonus = normalize(point.bonus, point.max_bonus, point.capped);
        points.0.push(point);
        world.entity_mut(creature).insert(points);
        return;
    };
    let same_sign = (held.bonus > 0) == (point.bonus > 0);
    if point.capped || point.duration == 0 {
        held.duration = point.duration;
        held.decay_start = point.decay_start;
    } else {
        held.duration = pick_time(held.duration, held.age, point.duration, same_sign);
        held.decay_start = pick_time(held.decay_start, held.age, point.decay_start, same_sign);
    }
    #[allow(clippy::cast_possible_truncation)]
    let stacked = if point.duration == 0 {
        point.bonus
    } else if point.capped || !same_sign {
        held.net_bonus() + point.bonus
    } else {
        let sum = f64::from(held.net_bonus()).powi(2) + f64::from(point.bonus).powi(2);
        let magnitude = sum.sqrt() * f64::from(held.bonus.signum());
        magnitude.round() as i32
    };
    held.max_bonus = point.max_bonus;
    held.capped = point.capped;
    held.bonus = normalize(stacked, point.max_bonus, point.capped);
    held.age = 0;
    world.entity_mut(creature).insert(points);
}

/// Keeps a bonus inside its cap, when the cap applies.
///
/// Reference: `morale_point::normalize_bonus` (`morale.cpp:232-236`).
#[must_use]
pub fn normalize(bonus: i32, max_bonus: i32, capped: bool) -> i32 {
    if bonus.abs() > max_bonus.abs() && (max_bonus != 0 || capped) {
        max_bonus
    } else {
        bonus
    }
}

/// How much time a stacking point keeps.
///
/// Reference: `morale_point::pick_time` (`morale.cpp:206-211`): the longer of what is left and what is new, but only while the signs agree.
#[must_use]
pub fn pick_time(current: i64, age: i64, new: i64, same_sign: bool) -> i64 {
    let remaining = current - age;
    if remaining <= new && same_sign { new } else { remaining }
}

/// Every morale point a creature carries.
#[must_use]
pub fn points_of(world: &World, creature: Entity) -> Vec<MoralePoint> {
    world
        .get::<Morale>(creature)
        .map_or_else(Vec::new, |morale| morale.0.clone())
}

/// What a creature's morale adds up to.
#[must_use]
pub fn net_bonus_of(world: &World, creature: Entity) -> i32 {
    world.get::<Morale>(creature).map_or(0, Morale::net_bonus)
}

/// Ages every creature's morale by a turn and drops what has run out.
pub fn age_morale(world: &mut World) {
    let mut aged: Vec<(Entity, Morale)> = Vec::new();
    let mut query = world.query::<(Entity, &mut Morale)>();
    for (entity, mut morale) in query.iter_mut(world) {
        for point in &mut morale.0 {
            point.age += 1;
        }
        morale.0.retain(|point| !point.is_expired());
        aged.push((entity, morale.clone()));
    }
    for (entity, morale) in aged {
        world.entity_mut(entity).insert(morale);
    }
}

/// Removes **every** point of one morale type from a creature.
///
/// Reference: `Character::remove_morale` (`morale.cpp`), reached by `f_lose_morale` (`npctalk.cpp:7197-7204`): it erases the points whose type matches and leaves the
/// rest. A type the creature is not feeling is not a failure - there is simply nothing to erase.
pub fn remove_kind(world: &mut World, creature: Entity, kind: &str) -> bool {
    let Some(mut morale) = world.get_mut::<Morale>(creature) else {
        return false;
    };
    let before = morale.0.len();
    morale.0.retain(|point| point.kind != kind);
    morale.0.len() != before
}

/// Whether a character is cheerful enough to read something they have already read.
///
/// Reference: `Character::has_morale_to_read` (`character_morale.cpp:171-174`): `get_morale_level() >= -40`. **The floor is inclusive**, so a character at exactly −40 may still read - which is the
/// kind of boundary a `<` would move without anyone noticing.
#[must_use]
pub const fn has_morale_to_read(morale_level: i32) -> bool {
    morale_level >= -40
}
