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
    ("emit", emit),
    ("mod_moves", mod_moves),
    ("remove_effect", remove_effect),
    ("noise", noise),
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

/// An `emit` spell puts a field on every square of its area.
///
/// Reference: `spell_effect::emit` (`magic_spell_effect.cpp:1723-1729`) walks the area and calls `map::emit_field` on each
/// square with the emit id the spell names. The emission's chance is asked **once per square**, so a thin emission covers part
/// of the blast rather than all or none of it.
fn emit(
    world: &mut World,
    _caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let Some(emissions) = crate::emit_registry::emissions_of(world, &spell.effect_str) else {
        return Effective::NothingAffected;
    };
    let Some(mut rolls) = crate::singletons::take_rolls(world) else {
        return Effective::NotPorted;
    };
    let squares = squares_in_area(spell, level, centre);
    let mut placed = 0;
    for square in squares {
        for emission in &emissions {
            // Reference: `rng( 1, 100 ) <= chance`, drawn per square per emission.
            if rolls.rng(1, 100) > emission.chance {
                continue;
            }
            let Some(field) = crate::field_registry::id_of(world, &emission.field) else {
                continue;
            };
            // Reference: `map::emit_field` on a square (`map_field.cpp:2212-2224`) - the chance decides whether anything is
            // placed, and the quantity is then **poured out** from that square with `propagate_field`, so a gas spreads over
            // the squares it can reach rather than thickening only the one it was emitted on.
            if let Some(qty) = emission_qty(emission) {
                // Reference: `map::propagate_field` stops at an impassable square unless it is `PERMEABLE`
                // (`map_field.cpp:2264-2270`). The blocked squares are collected first, because placing the field is a write.
                let blocked = crate::field_passage::blocked_within(world, square, qty);
                let poured = crate::field_spread::propagate(
                    world,
                    square,
                    field,
                    qty,
                    emission.intensity,
                    &mut rolls,
                    |x, y, z| !blocked.contains(&(x, y, z)),
                );
                if poured > 0 {
                    placed += 1;
                }
            } else {
                crate::add_field_strength(world, square, field, emission.intensity);
                placed += 1;
            }
        }
    }
    crate::singletons::put_rolls(world, rolls);
    if placed == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// How much of an emission to pour out, or nothing when it places a single unit.
///
/// A quantity of one is the reference's default and is placed directly; anything larger is a spread, which is why the
/// distinction is here rather than at the call site.
fn emission_qty(emission: &cdda_content::emit::Emission) -> Option<i32> {
    (emission.qty > 1).then_some(emission.qty)
}

/// A `noise` spell makes a sound as loud as the damage it would have dealt.
///
/// Reference: `spell_effect::noise` (`magic_spell_effect.cpp:1435-1438`) - one line, `sp.make_sound( target, sp.damage(
/// caster ) )`. The port reads the volume from the spell's damage at the level it was cast at, and refuses the spell when the
/// damage is not a number it can read, because a sound of unknown volume is a sound of any volume.
fn noise(
    world: &mut World,
    _caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let Some(damage) = spell.damage.at(level) else {
        return Effective::NothingAffected;
    };
    #[allow(clippy::cast_possible_truncation)]
    let volume = damage.round() as i32;
    if crate::sounds::make_sound(world, centre, volume).is_some() {
        Effective::Done
    } else {
        Effective::NothingAffected
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

/// Every square a spell's area covers, which is what the area effects walk.
///
/// Reference: `spell_effect_area` (`magic_spell_effect.cpp`): a square is in the area when it is within the radius on both
/// axes and on the same level - the same rule `creatures_in_reach` uses to pick creatures out of that area.
fn squares_in_area(spell: &Spell, level: i32, centre: (i32, i32, i32)) -> Vec<(i32, i32, i32)> {
    let area = area_of(spell, level);
    let mut squares = Vec::new();
    for dx in -area..=area {
        for dy in -area..=area {
            squares.push((centre.0 + dx, centre.1 + dy, centre.2));
        }
    }
    squares
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
