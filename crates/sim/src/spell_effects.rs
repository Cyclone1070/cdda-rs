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
pub const EFFECTS: &[(&str, Apply)] = &[("mod_moves", mod_moves), ("summon", summon)];

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
