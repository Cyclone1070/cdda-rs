//! What a spell does when it lands.
//!
//! Reference: `magic_spell_effect.cpp`, one function per `effect_name`, dispatched by `spell_effect::make_unique`. The
//! port keeps the same shape - a name, a table entry, a function - and grows one effect at a time, because a spell whose
//! effect does nothing is worse than a spell that refuses: it spends the caster's time and looks like it worked.

use bevy_ecs::prelude::*;
use cdda_content::spells::Spell;

/// What came of casting a spell's effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effective {
    /// It did what the effect does.
    Done,
    /// Nothing was in reach of it.
    NothingAffected,
    /// The port has no rule for this effect yet, so nothing happened and nothing was spent.
    NotPorted,
}

/// What an effect does, once the spell has been cast.
type Apply = fn(&mut World, Entity, &Spell, i32, (i32, i32, i32)) -> Effective;

/// The effects the port can perform, by the `effect_name` the data uses.
///
/// Reference: the `spell_effect` subclasses (`magic_spell_effect.cpp`). A table rather than a match for the same reason
/// the activity finishers are one: the answer to "what can a spell do in this port?" is a list, and a list can be
/// counted.
/// Whether the port has a rule for a named effect.
///
/// The table is the port's dispatch, and a name that is not in it is refused rather than answered weakly - which is what
/// makes the spell census meaningful: "the port runs four of the data's 32 effect kinds" is a sentence a test checks.
#[must_use]
pub fn is_ported(name: &str) -> bool {
    EFFECTS.iter().any(|(known, _)| *known == name)
}

/// Every effect kind the port can run, by the name the data gives it.
pub const EFFECTS: &[(&str, Apply)] = &[
    ("attack", attack),
    ("mod_moves", mod_moves),
    ("remove_effect", remove_effect),
    ("remove_field", remove_field),
    ("summon", summon),
];

/// An `attack` spell damages everything its area covers.
///
/// Reference: `spell_effect::attack` (`magic_spell_effect.cpp:530-545`), which asks `damage_targets` for each creature in
/// the area. Each target draws its own body part from the same generator the cast used, so a seed replays the whole blast - and
/// the area decides who is hit, not the range, which is why this calls [`crate::spellcasting::damage_one`] rather than the
/// range-checked `cast_attack`.
fn attack(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    // The only effect so far that draws: each target's body part comes from the same generator the cast used. Taking it here
    // rather than in the dispatch keeps every effect that does not draw free of the store's randomness.
    let Some(mut rolls) = crate::singletons::take_rolls(world) else {
        return Effective::NotPorted;
    };
    let targets = creatures_in_reach(world, caster, spell, level, centre);
    let mut hit = 0;
    for target in targets {
        // Reference: `Creature::get_random_body_part()` draws over the parts' hit sizes, which `lands_on` replays from a unit
        // draw - the same phrasing `cast_at` uses, so a spell cast both ways draws alike.
        #[allow(clippy::cast_precision_loss)]
        let fraction = f64::from(rolls.rng_bits()) / f64::from(u32::MAX);
        if matches!(
            crate::spellcasting::damage_one(world, spell, level, target, fraction),
            crate::spellcasting::Cast::Hit { .. }
        ) {
            hit += 1;
        }
    }
    crate::singletons::put_rolls(world, rolls);
    if hit == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// Reference: `spell_effect::remove_field` (`magic_spell_effect.cpp:1010-1028`) - the field the spell names in
/// `effect_str` (`fd_fatigue` for the one spell that uses it) comes off every square in the blast, and each square keeps
/// whatever else was on it.
fn remove_field(
    world: &mut World,
    _caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let Some(field) = crate::field_registry::id_of(world, &spell.effect_str) else {
        return Effective::NothingAffected;
    };
    let area = area_of(spell, level);
    let mut took = 0;
    for x in (centre.0 - area)..=(centre.0 + area) {
        for y in (centre.1 - area)..=(centre.1 + area) {
            if crate::remove_field(world, (x, y, centre.2), field) {
                took += 1;
            }
        }
    }
    if took == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// How far a spell reaches from its centre, in squares.
fn area_of(spell: &Spell, level: i32) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    let area = spell.area.at(level).unwrap_or(0.0).max(0.0) as i32;
    area
}

/// Reference: `spell_effect::remove_effect` (`magic_spell_effect.cpp:1712-1721`) - every creature in the spell's area
/// loses the effect the spell names in `effect_str`, and nothing else about it changes.
fn remove_effect(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let affected = creatures_in_reach(world, caster, spell, level, centre);
    let mut took = 0;
    for creature in &affected {
        if crate::applied_effects::remove(world, *creature, &spell.effect_str) {
            took += 1;
        }
    }
    if took == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// Reference: `spell_effect::summon` (`magic_spell_effect.cpp`), which spawns the monster the spell names, as many times as
/// the spell's damage at that level, on free squares near the target. The port spawns them on the target square itself
/// when it is free and on the nearest free square otherwise, because it has no path-finding to place a crowd yet.
fn summon(
    world: &mut World,
    _caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let Some(how_many) = spell.damage.at(level) else {
        return Effective::NothingAffected;
    };
    #[allow(clippy::cast_possible_truncation)]
    let how_many = how_many.max(0.0) as i32;
    let Some(kind) = crate::monster_registry::id_of(world, &spell.effect_str) else {
        return Effective::NothingAffected;
    };
    let hp = crate::monster_registry::hit_points(world, kind);
    let mut made = 0;
    for step in 0..how_many {
        // A ring of squares around the centre: the first is the centre itself, so a single summon lands where aimed.
        let offset = ring_offset(step);
        let square = (centre.0 + offset.0, centre.1 + offset.1);
        if !crate::creature::creature_at(world, (square.0, square.1, centre.2)).is_empty() {
            continue;
        }
        crate::spawn_monster(world, square, kind, hp);
        made += 1;
    }
    if made == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// The square to try when placing the `step`th summon: the centre, then its neighbours, then the next ring out.
#[must_use]
fn ring_offset(step: i32) -> (i32, i32) {
    let mut left = step;
    let mut radius = 0;
    loop {
        let side = radius * 2 + 1;
        let in_ring = side * side;
        if left < in_ring {
            let along = left;
            let row = along / side - radius;
            let column = along % side - radius;
            return if radius == 0 { (0, 0) } else { (row, column) };
        }
        left -= in_ring;
        radius += 1;
    }
}

/// Reference: `spell_effect::mod_moves` (`magic_spell_effect.cpp:1475-1490`) - every creature in the spell's area has
/// its moves changed by `sp.damage( caster )`, which may be negative for a spell that slows a creature down.
fn mod_moves(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let Some(change) = spell.damage.at(level) else {
        return Effective::NothingAffected;
    };
    #[allow(clippy::cast_possible_truncation)]
    let change = change as i32;
    let _ = &change;
    let affected = creatures_in_reach(world, caster, spell, level, centre);
    for creature in &affected {
        crate::creature::give_moves(world, *creature, change);
    }
    if affected.is_empty() {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// Every creature the spell reaches: the centre square and everything within the spell's area of it.
///
/// Reference: `spell_effect_area` (`magic_spell_effect.cpp`), reduced to the shapes the port's spells use - a blast is
/// everything within `area` of the centre, and a spell with no area is its own square.
#[must_use]
pub fn creatures_in_reach(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Vec<Entity> {
    #[allow(clippy::cast_possible_truncation)]
    let area = spell.area.at(level).unwrap_or(0.0).max(0.0) as i32;
    let mut query = world.query::<(Entity, &crate::Position, &crate::creature::Creature)>();
    query
        .iter(world)
        .filter(|(entity, position, _)| {
            *entity != caster
                && (position.x - centre.0).abs() <= area
                && (position.y - centre.1).abs() <= area
                && position.z == centre.2
        })
        .map(|(entity, _, _)| entity)
        .collect()
}

/// Performs a spell's effect, if the port knows it.
///
/// A spell whose effect the port has not ported reports [`Effective::NotPorted`] and spends nothing - the same choice
/// [`crate::spellcasting`] makes for a cast it cannot resolve.
pub fn apply(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let Some((_, effect)) = EFFECTS.iter().find(|(name, _)| *name == spell.effect_name) else {
        return Effective::NotPorted;
    };
    effect(world, caster, spell, level, centre)
}

/// Whether the port can perform a spell's effect.
#[must_use]
pub fn known(effect_name: &str) -> bool {
    EFFECTS.iter().any(|(name, _)| *name == effect_name)
}
