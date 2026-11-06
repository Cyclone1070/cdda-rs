//! How fast a monster mends, and when the mending arrives.
//!
//! Reference: `monster::regenerate` (`src/monster.cpp:4094-4103`). A monster's hourly rate is the data's own `regenerates` when a type states one, and otherwise one of two
//! fallbacks: four times the flesh rate for a body that `REVIVES` and is not `DORMANT`, and the flesh rate itself for anything made of flesh or vegetable matter. The
//! reference then heals `roll_remainder( regen * dt )` - a **fractional** hourly rate turned into whole hit points by a draw, so a slow mending lands as a point every so
//! often instead of never.

use bevy_ecs::prelude::*;

/// How many hit points a creature mends in an hour, as a component on the creature.
///
/// Reference: `monster::regenerate` reads `type->regenerates`, the type's hit points, its flags and its materials **every turn**. The port works the answer out once, when
/// the creature is placed, and keeps it: the type cannot change under a monster, and a rule that has already been decided does not need deciding again.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Mending {
    /// The hit points it mends in an hour.
    pub per_hour: f64,
    /// The speed its type gives it, which is what mending restores it towards.
    pub type_speed: i32,
    /// The hit points its type gives it, which is the ceiling both mending and speed read.
    pub type_hp: i32,
    /// Whether its type mends **by** the dark (`mtype::regenerates_in_dark`, `mtype.h:541`).
    pub in_dark: bool,
}

/// How many turns an hour is.
///
/// Reference: `to_turns<int>( 1_hours )` - the game's turn is one second, so an hour is three thousand six hundred of them.
pub const TURNS_PER_HOUR: i32 = 3_600;

/// The hit points a monster mends in an hour.
///
/// Reference: `monster::regenerate` (`monster.cpp:4094-4103`), in the reference's own order: a stated rate first, then the reviving rate, then the flesh rate. `materials`
/// are the type's own, and `dormant` is the `DORMANT` flag that keeps a reviving body still.
#[must_use]
pub fn mending_per_hour(
    hit_points: i32,
    revives: bool,
    dormant: bool,
    materials: &[&str],
    stated: f64,
) -> f64 {
    if stated > 0.0 {
        return stated;
    }
    #[allow(clippy::cast_precision_loss)]
    let hp = f64::from(hit_points);
    if revives && !dormant {
        return 0.02 * hp;
    }
    if materials
        .iter()
        .any(|material| matches!(*material, "flesh" | "iflesh" | "veggy"))
    {
        return 0.005 * hp;
    }
    0.0
}

/// What one stretch of time mends, as whole hit points.
///
/// Reference: `roll_remainder( regen * to_turns<int>( dt ) / to_turns<int>( 1_hours ) )` - the rate is per hour, so the fraction of an hour is what the draw is scaled by,
/// and `roll_remainder` turns that fraction into a whole point with the fraction as its chance. A world with no dice mends **nothing**, which is the port's own convention:
/// a rule that needs a draw does not guess one.
pub fn mend_over(world: &mut World, per_hour: f64, turns: i32) -> i32 {
    if per_hour <= 0.0 || turns <= 0 {
        return 0;
    }
    let expected = per_hour * f64::from(turns) / f64::from(TURNS_PER_HOUR);
    let Some(mut rolls) = crate::singletons::take_rolls(world) else {
        return 0;
    };
    let whole = rolls.roll_remainder(expected);
    crate::singletons::put_rolls(world, rolls);
    whole
}

/// Mends every creature that has a rate, by however much this turn earns it.
///
/// Reference: `monster::process_turn` calls `regenerate( 1_turns )` (`monster.cpp:4094`), so the mending is a per-turn act on every monster rather than a bookkeeping
/// number somewhere else. A creature with no [`Mending`] does not mend, which is what a type that states no rate and is not flesh means.
///
/// Returns how many hit points were given back in total, which is what a test can count.
pub fn mend_everyone(world: &mut World) -> i32 {
    let rates: Vec<(Entity, Mending)> = world
        .query::<(Entity, &Mending)>()
        .iter(world)
        .map(|(entity, rate)| (entity, *rate))
        .collect();
    let mut healed = 0;
    for (entity, rate) in rates {
        // Reference: `monster::process_effects` heals the stated rate, then, for a type that mends **by** the dark, adds `50 * e^(-light^2/10000)` when the square is out of the sun
        // (`monster.cpp:3460-3494`). The two are separate heals in the reference and are summed here into one, which is the same number and one fewer place to mend the body twice.
        let mut amount = mend_over(world, rate.per_hour, 1);
        if rate.in_dark
            && let Some((x, y, z)) = world.get::<crate::Position>(entity).map(|at| (at.x, at.y, at.z))
        {
            let light = crate::singletons::light(world)
                .at(x, y)
                .map_or(0.0, |quarters| quarters.max());
            amount += crate::creature::dark_regeneration(light);
        }
        if amount == 0 {
            continue;
        }
        crate::creature::heal(world, entity, amount);
        healed += amount;
        // Reference: `monster::process_effects` (`monster.cpp:3473-3486`) says one of three lines on `one_in( 2 )`, and the grade is the amount **actually healed**. The message is a
        // warning (`m_warning`): a mending enemy is bad news.
        if let Some(mut rolls) = crate::singletons::take_rolls(world) {
            let speaks = rolls.one_in(2);
            crate::singletons::put_rolls(world, rolls);
            if speaks
                && let Some(line) = crate::creature::regeneration_message(amount)
            {
                let mut name = world
                    .get::<crate::creature::Creature>(entity)
                    .map(|creature| creature.name.clone())
                    .filter(|name| !name.is_empty());
                if name.is_none()
                    && let Some(kind) = world.get::<crate::creature::CreatureKind>(entity)
                {
                    name = crate::monster_registry::name_of(world, kind.0);
                }
                let name = name.unwrap_or_else(|| "monster".to_owned());
                crate::messages::push_kind(world, &line.replacen("%s", &name, 1), crate::messages::MessageKind::Warning);
            }
        }
        // Reference: the same function puts speed back as the body mends (`monster.cpp:4104-4115`), so a monster that was slowed by its wounds walks faster as it heals.
        let now = crate::creature::health_of(world, entity).unwrap_or(0);
        let base = world.get::<crate::creature::Speed>(entity).map(|speed| speed.base);
        if let Some(base) = base
            && let Some(faster) = speed_back(base, rate.type_speed, amount, rate.type_hp, now)
            && let Some(mut speed) = world.get_mut::<crate::creature::Speed>(entity)
        {
            speed.base = faster;
        }
    }
    healed
}

/// What a monster's base speed becomes after mending, or nothing when it is already as fast as its type.
///
/// Reference: `monster::regenerate` (`monster.cpp:4104-4115`). The reference only touches the speed while it is below the type's own, then either puts it all back when the hit
/// points are (`hp >= type->hp`) or gives `max( healed * type_speed / type_hp, 1 )`. The **floor of one** is the reference's, and it is what stops a monster that mends a
/// sliver from standing still for ever.
#[must_use]
pub fn speed_back(base: i32, type_speed: i32, healed: i32, type_hp: i32, hp: i32) -> Option<i32> {
    if base >= type_speed {
        return None;
    }
    if hp >= type_hp {
        return Some(type_speed);
    }
    if type_hp <= 0 {
        return None;
    }
    let delta = (healed * type_speed / type_hp).max(1);
    Some((base + delta).min(type_speed))
}

/// Whether a corpse is ready to get up again.
///
/// Reference: `item::ready_to_revive` (`item.cpp:8877-8912`). The age comes in whole hours and is worked out from the corpse's own clock; damage makes it **younger**
/// (`age_in_hours /= damage_level + 1`), and `rez_factor` is `48 - age_in_hours`. A corpse older than six hours is ready when the factor has run out, or on a one-in-`rez_factor`
/// chance. Everything the reference does with vehicles and the player's distance is left out on purpose: this store has no vehicle and no distance rule for a corpse.
#[must_use]
pub fn ready_to_revive(age_hours: i32, damage_level: i32, rolls: &mut cdda_core_types::rng::Rolls) -> bool {
    let mut age = age_hours;
    if damage_level > 0 {
        age /= damage_level + 1;
    }
    if age <= 6 {
        return false;
    }
    let rez_factor = 48 - age;
    rez_factor <= 0 || rolls.one_in(rez_factor)
}

/// How much wind a character gets back in a turn at rest: `PLAYER_BASE_STAMINA_REGEN_RATE` (`data/core/external_options.json`).
pub const BASE_WIND_BACK_PER_TURN: f64 = 20.0;

/// How much wind comes back in a turn.
///
/// Reference: `Character::update_stamina` (`character.cpp:7022-7082`). The multiplier is one normally and **a tenth when winded**, floored at a tenth
/// (`std::max<float>( 0.1f, base_multiplier )`), and the rate itself is floored at one - `std::max( 1.0f, effective_regen_rate ... )` - so there is always some recovery.
#[must_use]
pub fn wind_back_per_turn(base_rate_per_turn: f64, winded: bool) -> f64 {
    let multiplier: f64 = if winded { 0.1 } else { 1.0 };
    let multiplier = multiplier.max(0.1);
    multiplier * base_rate_per_turn.max(1.0)
}

/// Gives a character its wind back over some turns, clamped to what it can hold.
///
/// Reference: `Character::update_stamina`'s tail (`character.cpp:7079-7082`): `ceil( stamina_recovery * turns )`, added, then `std::min( std::max( get_stamina(), 0 ), max_stam )`.
pub fn recover_wind(world: &mut World, entity: Entity, base_rate_per_turn: f64, winded: bool, turns: i64) -> i32 {
    let per_turn = wind_back_per_turn(base_rate_per_turn, winded);
    #[allow(clippy::cast_precision_loss)]
    let total = (per_turn * turns as f64).ceil();
    #[allow(clippy::cast_possible_truncation)]
    let added = total as i32;
    if let Some(mut stamina) = world.get_mut::<crate::creature::Stamina>(entity) {
        stamina.cur = (stamina.cur + f64::from(added)).clamp(0.0, stamina.max);
        return added;
    }
    0
}
