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
    ("effect_on_condition", effect_on_condition),
    ("short_range_teleport", short_range_teleport),
    ("timed_event", timed_event),
    ("morale", morale),
    ("targeted_polymorph", targeted_polymorph),
    ("dash", dash),
    ("recover_energy", recover_energy),
    ("explosion", explosion),
    ("map", map),
    ("none", none),
    ("charm_monster", charm_monster),
    ("ter_transform", ter_transform),
    ("flashbang", flashbang),
    ("add_trap", add_trap),
    ("guilt", guilt),
    ("bash", bash),
    ("vomit", vomit),
    ("revive", revive),
    ("revive_dormant", revive_dormant),
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
    // Reference: `damage_targets` (`magic_spell_effect.cpp:529-533`) skips a square whose creature the spell may not affect **before** the sound and the damage - so a spell that
    // only names the ground passes over a monster standing in its blast, and one that names `mon_dog` passes over a zombie.
    let from = world
        .get::<crate::Position>(caster)
        .map(|at| (at.x, at.y, at.z));
    let targets: Vec<Entity> = creatures_in_reach(world, caster, spell, level, centre)
        .into_iter()
        .filter(|target| {
            let at = world.get::<crate::Position>(*target).map(|seen| (seen.x, seen.y, seen.z));
            match (from, at) {
                (Some(from), Some(at)) => crate::spell_targets::allows(world, from, spell, at),
                _ => true,
            }
        })
        .collect();
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
    caster: Entity,
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
    let squares = squares_in_area(world, caster, spell, level, centre);
    let mut placed = 0;
    for square in squares {
        for emission in &emissions {
            // Reference: `map::emit_field` per square (`map_field.cpp:2212-2224`), which is the same rule an `u_emit` effect asks for one square.
            if crate::emit_at::emit_one(world, square, emission, 1.0, &mut rolls) {
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
    // Reference: `spell_effect::noise` passes `sp.damage( caster )` straight through, and that is the **truncated** integer rather than the rounded one.
    let Some(volume) = crate::spellcasting::damage_of(spell, level) else {
        return Effective::NothingAffected;
    };
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

/// Every square a spell covers, which is what the area effects walk.
///
/// Reference: `spell_effect_area` (`magic_spell_effect.cpp:229-296`): the shape decides. A **blast** is a square of half-width `area` on both axes; a **cone** runs from
/// the caster towards where the spell landed, `area / 2` degrees to either side and `range` squares long, stopping at walls. The dispatch lives in
/// [`crate::spell_area::cover`] so that "which squares" is one readable question rather than a table of shapes inside an effect.
///
/// It is public because it **is** a rule rather than plumbing: the wall the blast stops at, and the shape it covers, are answers a caller can hold up to the reference.
pub fn squares_in_area(
    world: &World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Vec<(i32, i32, i32)> {
    let Some(source) = world.get::<crate::Position>(caster).map(|at| (at.x, at.y, at.z)) else {
        return crate::spell_area::blast(centre, area_of(spell, level));
    };
    #[allow(clippy::cast_possible_truncation)]
    let range = spell.range.at(level).unwrap_or(0.0).max(0.0) as i32;
    let passable = |square: (i32, i32, i32)| crate::routing::square_cost(world, square).is_some();
    // Reference: only a blast is stopped short by a wall (`sp.shape() == spell_shape::blast`), and only when the spell does not carry `NO_PROJECTILE`.
    let centre = if spell.shape == "blast" && !spell.has_flag("NO_PROJECTILE") {
        crate::spell_area::epicentre(source, centre, true, &passable)
    } else {
        centre
    };
    let squares = crate::spell_area::cover(
        &spell.shape,
        source,
        centre,
        area_of(spell, level),
        range,
        &passable,
        spell.has_flag("IGNORE_WALLS"),
    );
    // Reference: `calculate_spell_effect_area` erases every square the spell may not affect before any effect sees it (`magic_spell_effect.cpp:461-476`), which is why a blast
    // that names only hostile creatures leaves an ally standing in it untouched. **The shortcut above it is the exception**: an area smaller than one square returns the bare
    // epicentre without asking, so a summon aimed at a square is not refused by its own target list.
    if area_of(spell, level) < 1 {
        return squares;
    }
    squares
        .into_iter()
        .filter(|square| crate::spell_targets::allows(world, source, spell, *square))
        .collect()
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
/// A `short_range_teleport` spell moves the caster somewhere between its range and its range plus its area.
///
/// Reference: `spell_effect::short_range_teleport` (`magic_spell_effect.cpp:159-182`): `min_distance` is the spell's range, `max_distance` that range plus its area, and
/// safety is the **absence** of the `UNSAFE_TELEPORT` flag. The port does not load spell flags yet, so the caller states safety - and with nothing stated the teleport is
/// safe, which is the reference's default for every spell that does not opt out.
fn short_range_teleport(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    _centre: (i32, i32, i32),
) -> Effective {
    let Some(range) = spell.range.at(level) else {
        return Effective::NotPorted;
    };
    #[allow(clippy::cast_possible_truncation)]
    let min_distance = range.max(0.0) as i32;
    #[allow(clippy::cast_possible_truncation)]
    let area = spell.area.at(level).unwrap_or(0.0).max(0.0) as i32;
    // Reference: `const bool safe = !sp.has_flag( spell_flag::UNSAFE_TELEPORT );` (`magic_spell_effect.cpp:162`) - **one spell in this content** carries the flag, and an unsafe
    // jump lands wherever the draw says even if that square kills.
    let safe = !spell.has_flag("UNSAFE_TELEPORT");
    match crate::teleport::creature(world, caster, min_distance, min_distance + area, safe) {
        crate::teleport::Teleport::Moved { .. } => Effective::Done,
        crate::teleport::Teleport::Unchanged
        | crate::teleport::Teleport::Refused
        | crate::teleport::Teleport::Died => Effective::NothingAffected,
    }
}

/// A `revive` spell stands up the first corpse of the species it names, on each square of its blast.
///
/// Reference: `spell_effect::revive` (`magic_spell_effect.cpp:1551-1568`): the spell's `effect_str` names a species, and each square of the area gives up one body that
/// `REVIVES`, is **not** `DORMANT`, is not `NO_NECRO`, and belongs to that species. The raised monster is the corpse's own kind, and the corpse is gone.
fn revive(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    stand_up(world, caster, spell, level, centre, false)
}

/// A `revive_dormant` spell is the same rule with the dormant clause the other way round.
///
/// Reference: `spell_effect::revive_dormant` (`magic_spell_effect.cpp:1570-1596`), whose own comment says it is "identical to above, but checks for REVIVES && DORMANT flag.
/// Ignores NO_NECRO" - the relaxed revive, which is why a body that was dug up can be woken but not raised.
fn revive_dormant(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    stand_up(world, caster, spell, level, centre, true)
}

/// The rule both revive spells share, with the dormant clause chosen by the caller.
fn stand_up(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
    dormant: bool,
) -> Effective {
    let mut raised = 0;
    for square in squares_in_area(world, caster, spell, level, centre) {
        // Reference: one body per square, and a square it cannot use is simply skipped.
        if crate::revive::revive(world, square, &spell.effect_str, dormant).is_ok() {
            raised += 1;
        }
    }
    if raised == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// A `vomit` spell makes every character its blast covers throw up.
///
/// Reference: `spell_effect::vomit` (`magic_spell_effect.cpp:1440-1455`): for each square, a **character** there is made to vomit - and the reference's own comment notes that
/// ``Character::vomit`` does the rest, so this effect is a loop and a call. A monster in the blast is untouched: the rule is a character's stomach.
fn vomit(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let mut retched = 0;
    for square in squares_in_area(world, caster, spell, level, centre) {
        for held in crate::creature::creature_at(world, square) {
            if world.get::<crate::Player>(held).is_none() && !crate::npcs::is_npc(world, held) {
                continue;
            }
            if crate::vomit::vomit(world, held).is_some() {
                retched += 1;
            }
        }
    }
    if retched == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// A `bash` spell hits every square of its blast with the spell's damage as the force.
///
/// Reference: `spell_effect::bash` (`magic_spell_effect.cpp:1770-1781`): for each square the spell's area covers, `here.bash( potential_target, sp.damage( caster ), silent )` -
/// the damage **is** the strength, and the reference's own comment says the bash makes its own noise.
///
/// `SILENT` is the spell flag that says the hit is not heard, and the rule reads it from the data now that the dump carries flags.
fn bash(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let Some(force) = spell.damage.at(level) else {
        return Effective::NotPorted;
    };
    #[allow(clippy::cast_possible_truncation)]
    let force = force as i32;
    let mut hits = 0;
    for square in squares_in_area(world, caster, spell, level, centre) {
        let Some(outcome) = crate::bash::bash_square(world, square, force, spell.has_flag("SILENT")) else {
            return Effective::NotPorted;
        };
        if outcome.did_bash {
            hits += 1;
        }
    }
    if hits == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// A `guilt` spell makes a character feel bad about the monsters it has killed - and only a **monster** can cast it.
///
/// Reference: `spell_effect::guilt` (`magic_spell_effect.cpp:1617-1711`). The shape is a rising scale rather than a fixed penalty:
///
/// * the caster must be a monster, and **its kind** is what the tally is counted against (`guilt_kill_count`);
/// * `max_kills` is the spell's damage - the count at which a character stops caring - and `guilt_mult` its level;
/// * the malus is `-5 * guilt_mult * (1 - killRatio)`, capped at `-250 * (1 - killRatio)`, lasting `duration * (1 - killRatio)` with a decay delay of three minutes scaled
///   the same way;
/// * killing has less bite for a psychopath or a `PRED3`/`PRED4` (who feel nothing), five times the bite for a `PACIFIST`, and a quarter or a fifth for `PRED1`/`PRED2`;
/// * and a character killing **its own species** feels it twice over.
///
/// The reference's threshold **messages** are interface, and the port records the morale point and leaves them to a caller that draws text.
fn guilt(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let Some(kind) = world.get::<crate::creature::CreatureKind>(caster).map(|held| held.0) else {
        // Reference: `if( !caster.is_monster() ) { return; }` - a character casting this feels nothing.
        return Effective::NothingAffected;
    };
    let Some(max_kills) = spell.damage.at(level) else {
        return Effective::NotPorted;
    };
    let Some(seconds) = spell.duration.at(level) else {
        return Effective::NotPorted;
    };
    #[allow(clippy::cast_possible_truncation)]
    let max_kills = max_kills as i32;
    if max_kills <= 0 {
        return Effective::NothingAffected;
    }
    let kill_count = crate::kills::count_of(world, kind);
    let mut felt = 0;
    for square in squares_in_area(world, caster, spell, level, centre) {
        for held in crate::creature::creature_at(world, square) {
            let person = world.get::<crate::Player>(held).is_some() || crate::npcs::is_npc(world, held);
            if !person {
                continue;
            }
            // Reference: a psychopath, or a `PRED3`/`PRED4`, has no conscience to move.
            if crate::traits::has_trait(world, held, "PSYCHOPATH")
                || crate::creature_flags::has_flag(world, held, "PRED3")
                || crate::creature_flags::has_flag(world, held, "PRED4")
            {
                continue;
            }
            // Reference: `if( kill_count >= max_kills ) { return; }` - a character who has killed that many no longer cares.
            if kill_count >= max_kills {
                continue;
            }
            #[allow(clippy::cast_precision_loss)]
            let ratio = f64::from(kill_count) / f64::from(max_kills);
            let remaining = 1.0 - ratio;
            #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
            let mut malus = -5.0 * level as f64 * remaining;
            #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
            let cap = -250.0 * remaining;
            if crate::traits::has_trait(world, held, "PACIFIST") {
                malus *= 5.0;
            } else if crate::creature_flags::has_flag(world, held, "PRED1") {
                malus /= 4.0;
            } else if crate::creature_flags::has_flag(world, held, "PRED2") {
                malus /= 5.0;
            }
            // Reference: killing your own kind hurts more, asked either of the victim's species or of the species the spell names.
            let shared = crate::creature_flags::has_flag(world, caster, &format!("SPECIES_{}", spell.effect_str))
                || spell.effect_str.is_empty();
            if shared {
                malus *= 2.0;
            }
            #[allow(clippy::cast_possible_truncation)]
            let bonus = malus.round() as i32;
            #[allow(clippy::cast_possible_truncation)]
            let max_bonus = cap.round() as i32;
            // Reference: `duration * ( 1 - killRatio )`, and a decay delay of three minutes scaled the same way.
            #[allow(clippy::cast_possible_truncation)]
            let duration = (seconds * remaining) as i64;
            #[allow(clippy::cast_possible_truncation)]
            let decay = (180.0 * remaining) as i64;
            crate::morale::add(
                world,
                held,
                crate::morale::MoralePoint {
                    kind: "morale_killed_monster".to_owned(),
                    bonus,
                    max_bonus,
                    duration,
                    decay_start: decay,
                    age: 0,
                    capped: true,
                },
            );
            felt += 1;
        }
    }
    if felt == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// An `add_trap` spell sets a trap on the square it landed on.
///
/// Reference: `spell_effect::add_trap` (`magic_spell_effect.cpp:1598-1605`): the spell's `effect_str` names the trap, and it is set **only where no trap is already set** -
/// the reference's `if( here.tr_at( target ) == tr_null )`, which is `map::trap_set`'s own guard as well. Setting one over another is not an error; it simply does nothing.
fn add_trap(
    world: &mut World,
    _caster: Entity,
    spell: &Spell,
    _level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    if spell.effect_str.is_empty() {
        return Effective::NothingAffected;
    }
    if crate::square::set_trap(world, centre, &spell.effect_str) {
        Effective::Done
    } else {
        Effective::NothingAffected
    }
}

/// A `flashbang` spell is a bang and a flash at the square it landed on.
///
/// Reference: `spell_effect::flashbang` (`magic_spell_effect.cpp:1469-1473`): one call into `explosion_handler::flashbang`, with `player_immune` set when the caster is the
/// avatar **and** the spell does not count the caster as a valid target - a caster who could hit itself does not get the immunity.
fn flashbang(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    _level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    // Reference: `caster.is_avatar() && !sp.is_valid_target( spell_target::self )`. The spell's targets are a property of its flags, which the port does not load, so the
    // port takes the avatar half and treats "can hit itself" as the exception it is: a self-targeting flashbang is the only case that would clear the immunity.
    let immune = world.get::<crate::Player>(caster).is_some();
    let outcome = crate::flashbang::flashbang(world, centre, immune);
    if outcome.deafened.is_empty() && outcome.blinded.is_empty() && outcome.stunned.is_empty() {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// A `ter_transform` spell changes the ground its blast covers, one square at a time and only sometimes.
///
/// Reference: `spell_effect::transform_blast` (`magic_spell_effect.cpp:1423-1434`): the spell's `effect_str` names a `ter_furn_transform`, the blast's squares are walked, and
/// **each square draws `one_in( sp.damage( caster ) )`** before it is transformed - so the damage is the odds, not a quantity.
///
/// `one_in`'s own rule is the surprise here: `chance <= 1` is **always true** (`rng.cpp:196-199`), so all three of the content's terrain spells - whose damage is zero - change
/// every square their blast covers, and a later round that "fixed" the zero would change what the content does. The gate states that.
fn ter_transform(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let Some(chance) = spell.damage.at(level) else {
        return Effective::NotPorted;
    };
    #[allow(clippy::cast_possible_truncation)]
    let chance = chance as i32;
    let Some(mut rolls) = crate::singletons::take_rolls(world) else {
        return Effective::NotPorted;
    };
    let mut changed = 0;
    for square in squares_in_area(world, caster, spell, level, centre) {
        // Reference: `if( one_in( sp.damage( caster ) ) ) { transform->transform( ... ) }`.
        if !rolls.one_in(chance) {
            continue;
        }
        if crate::terrain_transform::apply_at(world, &spell.effect_str, square) {
            changed += 1;
        }
    }
    crate::singletons::put_rolls(world, rolls);
    if changed == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// A `charm_monster` spell wins a monster over, if it is already hurt enough.
///
/// Reference: `spell_effect::charm_monster` (`magic_spell_effect.cpp:1530-1549`): for every monster the spell's area covers, **nothing happens unless its hit points are at
/// or below the spell's damage**, and a monster that is already friendly is left alone unless the spell carries `RECHARM` (a flag the port does not load yet). The charm
/// itself is an **addition** to how long it stays friendly - the spell's duration over a hundred - so charming twice lasts twice as long.
///
/// `AEA_ENTRANCE` - the content's one such spell - states a damage of **zero**, so it can only charm a monster with no hit points left. That is the data's own arithmetic and
/// the gate states it rather than quietly treating zero as "always".
fn charm_monster(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let Some(threshold) = spell.damage.at(level) else {
        return Effective::NotPorted;
    };
    let Some(seconds) = spell.duration.at(level) else {
        return Effective::NotPorted;
    };
    #[allow(clippy::cast_possible_truncation)]
    let threshold = threshold as i32;
    // Reference: `mon->friendly += sp.duration( caster ) / 100` - whole turns of a hundredth of the duration.
    #[allow(clippy::cast_possible_truncation)]
    let turns = (seconds / 100.0) as i32;
    let mut charmed = 0;
    for square in squares_in_area(world, caster, spell, level, centre) {
        for held in crate::creature::creature_at(world, square) {
            if world.get::<crate::creature::CreatureKind>(held).is_none() {
                continue;
            }
            let hurt = crate::creature::health_of(world, held).unwrap_or(0);
            if hurt > threshold {
                continue;
            }
            if crate::friendly::is_friendly(world, held)
                && crate::friendly::turns_of(world, held) > 0
                && !spell.has_flag("RECHARM")
            {
                // Reference: `mon->friendly == 0 || ( mon->friendly != 0 && sp.has_flag( RECHARM ) )` - a monster that is already friendly is only re-charmed by a spell
                // that says it may be, and then the charm **adds** to the count it already has.
                continue;
            }
            crate::friendly::charm(world, held, turns);
            charmed += 1;
        }
    }
    if charmed == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// A `map` spell shows the avatar the overmap around it.
///
/// Reference: `spell_effect::map` (`magic_spell_effect.cpp:1492-1501`): only the avatar has a map to reveal, the centre is the avatar's own overmap tile, and the radius is the
/// spell's area **in overmap tiles** - which is why `AEA_MAP`'s twenty is a window forty-one tiles across and not forty-one squares.
fn map(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    _centre: (i32, i32, i32),
) -> Effective {
    if world.get::<crate::Player>(caster).is_none() {
        // Reference: `if( !you ) { return; }` - a monster's map spell reveals nothing, because a monster has no map.
        return Effective::NothingAffected;
    }
    let Some(at) = world.get::<crate::Position>(caster).map(|held| (held.x, held.y)) else {
        return Effective::NotPorted;
    };
    let Some(area) = spell.area.at(level) else {
        return Effective::NotPorted;
    };
    #[allow(clippy::cast_possible_truncation)]
    let radius = area.max(0.0) as i32;
    let tile = crate::overmap::tile_of(at.0, at.1);
    crate::overmap::reveal(world, tile, radius);
    Effective::Done
}

/// An effect kind the data names but which does nothing at all.
///
/// Reference: `spell_effect::none` (`magic_spell_effect.cpp:1418-1421`): the reference reports the spell as having an invalid effect and returns. `AEA_BUGS` is the content's one
/// such spell, and the port answers the same way it answers every other effect that decides nothing: the world is unchanged and the cast is not an error.
fn none(
    _world: &mut World,
    _caster: Entity,
    _spell: &Spell,
    _level: i32,
    _centre: (i32, i32, i32),
) -> Effective {
    Effective::NothingAffected
}

/// An `explosion` spell is a blast at the square it landed on.
///
/// Reference: `spell_effect::explosion` (`magic_spell_effect.cpp:1463-1467`): one call -
/// `explosion_handler::explosion( &caster, target, sp.damage( caster ), sp.aoe( caster ) / 10.0, true )`. The damage is the blast's force at its centre, the area divided by ten
/// is what the force is multiplied by per square of distance, and **the blast always sets things on fire**.
fn explosion(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let Some(power) = spell.damage.at(level) else {
        return Effective::NotPorted;
    };
    let Some(area) = spell.area.at(level) else {
        return Effective::NotPorted;
    };
    #[allow(clippy::cast_possible_truncation)]
    let power = power as f32;
    #[allow(clippy::cast_possible_truncation)]
    let factor = (area / 10.0) as f32;
    let outcome = crate::explosion::explode(world, Some(caster), centre, power, factor, true);
    if outcome.struck.is_empty() {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// A `recover_energy` spell moves one of a character's pools, up or down, by the spell's damage.
///
/// Reference: `spell_effect::recover_energy` (`magic_spell_effect.cpp:1241-1279`). The pool is named by `effect_str` and **a negative amount spends rather than restores**,
/// which is not a mistake to tidy: four of the content's six spells are drains, and `STAMINA` of -1500 is a spell that winds its victim. Two of the reference's branches carry
/// a comment saying the sign is backwards (`PAIN`, `SLEEPINESS`), and `BIONIC` spends *stamina* when the amount is negative - a quirk kept whole.
///
/// Only a character has these pools: a monster standing on the square is not affected, which the reference says by asking for a `Character`.
fn recover_energy(
    world: &mut World,
    _caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let Some(healing) = spell.damage.at(level) else {
        return Effective::NotPorted;
    };
    #[allow(clippy::cast_possible_truncation)]
    let healing = healing as i32;
    let Some(person) = crate::creature::creature_at(world, centre)
        .into_iter()
        .find(|held| {
            world.get::<crate::Player>(*held).is_some() || crate::npcs::is_npc(world, *held)
        })
    else {
        return Effective::NothingAffected;
    };
    match spell.effect_str.as_str() {
        "MANA" => crate::creature::mod_mana(world, person, healing),
        "STAMINA" => crate::creature::spend_stamina(world, person, healing),
        // Reference: `PAIN` is backwards - `mod_pain( -healing )` - so a spell that "recovers" pain with a negative amount **adds** it.
        "PAIN" => crate::creature::mod_pain(world, person, -healing),
        // Reference: a positive amount charges the bionic pool in kilojoules, and a negative one is spent as **stamina** instead (`:1260-1265`).
        "BIONIC" => {
            if healing > 0 {
                crate::creature::mod_power(world, person, i64::from(healing) * 1_000);
            } else {
                crate::creature::spend_stamina(world, person, healing);
            }
        }
        _ => return Effective::NothingAffected,
    }
    Effective::Done
}

/// A `dash` spell leaps the caster along a line and hits what stands in the cone it leaves behind.
///
/// Reference: `spell_effect::dash` (`magic_spell_effect.cpp:1783-1833`). The reference's own shape is unusual and is kept whole:
///
/// * the walk is `line_to( source, target )`, and a step is refused when a creature stands there or the move is not valid - so a dash **stops short** rather than pushing
///   through;
/// * **only an avatar walks at all** (`caster_you` is null for a monster, whose loop body does nothing) - a monster's dash leaves it where it started;
/// * the moves spent on the walk are **given back** afterwards (`caster.set_moves( cur_moves )`), so a dash costs the caster nothing but the turns its own casting already
///   took;
/// * the damage cone starts at the source, points at `calc_ray_end( angle, aoe, where_it_stopped )` and is `aoe` wide and `aoe` long.
///
/// All four of the content's dash spells state an area of **zero**, which makes that cone empty and the leap the whole effect - the code is here for the spells that will
/// state otherwise, and a gate states the zero case rather than leaving it to be discovered.
fn dash(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let Some(source) = world.get::<crate::Position>(caster).map(|at| (at.x, at.y, at.z)) else {
        return Effective::NotPorted;
    };
    let Some(area) = spell.area.at(level) else {
        return Effective::NotPorted;
    };
    #[allow(clippy::cast_possible_truncation)]
    let area = area.max(0.0) as i32;
    let mut trajectory = crate::line::between((source.0, source.1), (centre.0, centre.1));
    // Reference: `if( here.get_bub( *walk_point ) == source ) { ++walk_point; }` - the line normally starts *after* the caster, and only a dash aimed at its own square
    // hands back the square it is standing on.
    if trajectory.first() == Some(&(source.0, source.1)) {
        trajectory.remove(0);
    }
    let saved = world
        .get::<crate::creature::Moves>(caster)
        .map_or(0, |moves| moves.left);
    let mut reached = source;
    // Reference: `if( caster_you != nullptr )` - a monster's dash does not walk.
    if world.get::<crate::Player>(caster).is_some() {
        for square in &trajectory {
            let at = (square.0, square.1, source.2);
            if !crate::creature::creature_at(world, at).is_empty() {
                break;
            }
            let Some(cost_here) = crate::routing::square_cost(world, reached) else {
                break;
            };
            let Some(cost_next) = crate::routing::square_cost(world, at) else {
                break;
            };
            let axes = usize::from(at.0 != reached.0) + usize::from(at.1 != reached.1);
            let cost = crate::creature::combined_move_cost(cost_here, cost_next, 0, axes);
            if crate::creature::try_step(world, caster, at, cost)
                != crate::creature::StepOutcome::Moved
            {
                break;
            }
            reached = at;
        }
    }
    if let Some(mut moves) = world.get_mut::<crate::creature::Moves>(caster) {
        moves.left = saved;
    }
    // Reference: `calc_ray_end( coord_to_angle( source, target ), sp.aoe( caster ), *walk_point, far_target )`, then a cone of `params.range = sp.aoe( caster )`.
    let far = crate::spell_area::ray_end(crate::spell_area::angle_of(source, centre), area, reached);
    let hit = crate::spell_area::cone(source, far, area, area, false, &|square| {
        crate::routing::square_cost(world, square).is_some()
    });
    let mut struck = 0;
    for square in hit {
        for victim in crate::creature::creature_at(world, square) {
            if victim == caster {
                continue;
            }
            crate::spellcasting::damage_one(world, spell, level, victim, 1.0);
            struck += 1;
        }
    }
    if reached == source && struck == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// A `targeted_polymorph` spell turns the monster it hits into another kind, if the monster is hurt enough.
///
/// Reference: `spell_effect::targeted_polymorph` (`magic_spell_effect.cpp:742-754`): only a monster is a candidate, and only when its hit points are **below** the spell's
/// damage - a whole victim resists. The new kind is the spell's `effect_str`, and the change itself is `monster::poly`.
///
/// **The empty case is refused, not invented.** The reference fills an empty `effect_str` with a random monster of nearby difficulty, drawn from the whole monster list in
/// load order; the port's registry has no difficulty field and no load-ordered list, so `activate_yrax` - whose `effect_str` is empty - would need a number nobody dumped.
/// It does nothing instead, and `docs/parity.md` names it.
fn targeted_polymorph(
    world: &mut World,
    _caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    if spell.effect_str.is_empty() {
        return Effective::NothingAffected;
    }
    let Some(threshold) = spell.damage.at(level) else {
        return Effective::NotPorted;
    };
    let Some(kind) = crate::monster_registry::id_of(world, &spell.effect_str) else {
        return Effective::NothingAffected;
    };
    let mut changed = 0;
    for held in crate::creature::creature_at(world, centre) {
        if world.get::<crate::creature::CreatureKind>(held).is_none() {
            continue;
        }
        let hurt = crate::creature::health_of(world, held).unwrap_or(0);
        #[allow(clippy::cast_possible_truncation)]
        let needed = threshold as i32;
        if hurt >= needed {
            continue;
        }
        crate::creature::poly(world, held, kind);
        changed += 1;
    }
    if changed == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// A `morale` spell makes the people its area covers feel something.
///
/// Reference: `spell_effect::morale` (`magic_spell_effect.cpp:1503-1527`): the spell's `effect_str` names a **morale type**, its damage is the bonus, its duration is how
/// long the feeling lasts, and a tenth of the duration is when it starts to fade. Only creatures of the character kind are affected - a monster has no morale to move -
/// and a spell whose type the data does not define is refused whole rather than applied to nobody in particular.
fn morale(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    if spell.effect_str.is_empty() || !crate::morale::is_known(world, &spell.effect_str) {
        return Effective::NothingAffected;
    }
    let Some(bonus) = spell.damage.at(level) else {
        return Effective::NotPorted;
    };
    let Some(seconds) = spell.duration.at(level) else {
        return Effective::NotPorted;
    };
    #[allow(clippy::cast_possible_truncation)]
    let bonus = bonus as i32;
    #[allow(clippy::cast_possible_truncation)]
    let turns = seconds.max(0.0) as i64;
    let mut felt = 0;
    for square in squares_in_area(world, caster, spell, level, centre) {
        for held in crate::creature::creature_at(world, square) {
            // Reference: `creatures.creature_at<Character>` - a monster has no morale.
            let person = world.get::<crate::Player>(held).is_some()
                || crate::npcs::is_npc(world, held);
            if !person {
                continue;
            }
            crate::morale::add(
                world,
                held,
                crate::morale::MoralePoint {
                    kind: spell.effect_str.clone(),
                    bonus,
                    max_bonus: 0,
                    duration: turns,
                    decay_start: turns / 10,
                    age: 0,
                    capped: false,
                },
            );
            felt += 1;
        }
    }
    if felt == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// The event names a `timed_event` spell may schedule, exactly as the reference spells them.
///
/// Reference: the map at `magic_spell_effect.cpp:1283-1293`. **It is matched case-sensitively**, and the data does not always agree with it: `death_triffid_heart` names
/// `ROOTS_DIE`, which the reference's `find` misses, so it schedules its `NONE` sentinel instead - an event nothing can observe. The port schedules nothing in that case and says
/// so rather than inventing a name the reference never matches.
const TIMED_EVENT_NAMES: [&str; 9] = [
    "help",
    "spawn_wyrms",
    "amigara",
    "roots_die",
    "temple_open",
    "temple_flood",
    "temple_spawn",
    "dim",
    "artifact_light",
];

/// A `timed_event` spell books an event for a time in the future.
///
/// Reference: `spell_effect::timed_event` (`magic_spell_effect.cpp:1281-1304`): the spell's `effect_str` names the event, the spell's duration says how far ahead, and the
/// event is added at `calendar::turn + sp.duration_turns( caster )`.
fn timed_event(
    world: &mut World,
    _caster: Entity,
    spell: &Spell,
    level: i32,
    _centre: (i32, i32, i32),
) -> Effective {
    if !TIMED_EVENT_NAMES.contains(&spell.effect_str.as_str()) {
        return Effective::NothingAffected;
    }
    let Some(seconds) = spell.duration.at(level) else {
        return Effective::NotPorted;
    };
    // Reference: `duration_turns` is the duration divided by one turn, and a turn **is** a second - so the number the dump states is the number of turns.
    #[allow(clippy::cast_possible_truncation)]
    let turns = seconds.max(0.0) as i64;
    crate::timed_events::schedule(world, &spell.effect_str, turns);
    Effective::Done
}

/// An `effect_on_condition` spell runs the program it names, once for each square of its area that holds a valid target.
///
/// Reference: `spell_effect::effect_on_condition` (`magic_spell_effect.cpp:1910-1943`). The program is run with a **context variable** set: `spell_location` is the
/// absolute square the program is running for, which is how a program written for a spell knows where it happened. The speaker is the victim when there is one and the
/// caster is the listener, exactly as the reference builds it; a square with nothing on it runs with no speaker at all.
///
/// **Declared**: the reference refuses a spell whose program is not an `ACTIVATION` program (it `debugmsg`s rather than running it), and the port has no
/// `eoc_type` on its programs yet, so it runs what it finds. Nothing in the content's 63 such spells states a recurrence, which is what that check exists for.
fn effect_on_condition(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    if spell.effect_str.is_empty() {
        return Effective::NothingAffected;
    }
    let mut ran = 0;
    for square in squares_in_area(world, caster, spell, level, centre) {
        // Reference: the speaker is the creature standing on the square when there is one, and the caster is always the listener.
        let held = crate::creature::creature_at(world, square);
        let Some(&victim) = held.first() else {
            continue;
        };
        crate::dialogue_values::set(
            world,
            victim,
            "spell_location",
            crate::dialogue_values::Value::Position {
                x: square.0,
                y: square.1,
                z: square.2,
            },
        );
        crate::eoc_registry::run_program(world, victim, Some(caster), &spell.effect_str, 0);
        ran += 1;
    }
    if ran == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

fn summon(
    world: &mut World,
    caster: Entity,
    spell: &Spell,
    level: i32,
    centre: (i32, i32, i32),
) -> Effective {
    let Some(how_many) = spell.damage.at(level) else {
        return Effective::NothingAffected;
    };
    #[allow(clippy::cast_possible_truncation)]
    let how_many = how_many.max(0.0) as i32;
    // Reference: `add_summoned_mon` (`magic_spell_effect.cpp:1322-1332`) reads `effect_str` as a monster's id **or**, when the spell carries `SPAWN_GROUP`, as a group's id and
    // draws the monster from it - one draw per summon, so a group with two entries can put two different monsters on the ground.
    let from_group = spell.has_flag("SPAWN_GROUP");
    // Reference: `spawn_summoned_monster` walks the spell's **own area** and draws a square from it for each summon, erasing the square whether or not it was used
    // (`magic_spell_effect.cpp:1345-1362`). The port had a ring order of its own, which is a second source of truth for "where inside the blast".
    let mut area = squares_in_area(world, caster, spell, level, centre);
    let mut made = 0;
    while made < how_many && !area.is_empty() {
        let Some(mut rolls) = crate::singletons::take_rolls(world) else {
            break;
        };
        #[allow(clippy::cast_possible_truncation)]
        let pick = rolls.rng(0, area.len() as i32 - 1).max(0) as usize;
        crate::singletons::put_rolls(world, rolls);
        let Some(square) = area.get(pick).copied() else {
            break;
        };
        area.swap_remove(pick);
        if !crate::creature::creature_at(world, square).is_empty() {
            continue;
        }
        let Some(name) = monster_to_summon(world, spell, from_group) else {
            break;
        };
        let Some(kind) = crate::monster_registry::id_of(world, &name) else {
            continue;
        };
        let hp = crate::monster_registry::hit_points(world, kind);
        let spawned = crate::spawn_monster(world, (square.0, square.1), kind, hp);
        // Reference: `spawned_mon.no_extra_death_drops = !sp.has_flag( spell_flag::SPAWN_WITH_DEATH_DROPS );` (`:1343`) - a summoned body leaves its corpse and none of the loot its
        // type names, unless the spell says otherwise. **Nine spells in this content keep the loot.**
        if !spell.has_flag("SPAWN_WITH_DEATH_DROPS") {
            world.entity_mut(spawned).insert(crate::creature::NoExtraDeathDrops);
        }
        // Reference: `spawned_mon.no_corpse_quiet = sp.has_flag( spell_flag::NO_CORPSE_QUIET );` (`:1344`) - the other half of the same block, and a **different** promise: this one
        // leaves no corpse and still leaves the loot. No spell in this content carries it, so the sum path is gated by a hand-built spell and the data is named as silent.
        if spell.has_flag("NO_CORPSE_QUIET") {
            world.entity_mut(spawned).insert(crate::creature::NoCorpseQuiet);
        }
        // Reference: `is_summon_friendly` (`:1306-1314`) - a summon is on the caster's side unless the spell says `HOSTILE_SUMMON`, and `HOSTILE_50` halves that with a draw.
        if friendly_summon(world, spell) {
            crate::friendly::make_friendly(world, spawned, i32::MAX);
        } else {
            crate::friendly::make_friendly(world, spawned, 0);
        }
        // Reference: `if( !sp.has_flag( spell_flag::PERMANENT ) ) { mon->set_summon_time( sp.duration_turns( caster ) ); }` - a permanent summon simply has no deadline, which
        // is the reference's own way of saying "for good".
        if !spell.has_flag("PERMANENT")
            && let Some(seconds) = spell.duration.at(level)
        {
            #[allow(clippy::cast_possible_truncation)]
            crate::summoned::set_summon_time(world, spawned, seconds.max(0.0) as i64);
        }
        made += 1;
    }
    if made == 0 {
        Effective::NothingAffected
    } else {
        Effective::Done
    }
}

/// Which monster a summon brings: a draw from the group the spell names, or the monster the spell names.
///
/// Reference: `std::string monster_id = sp.effect_data(); if( sp.has_flag( spell_flag::SPAWN_GROUP ) ) { monster_id = MonsterGroupManager::GetRandomMonsterFromGroup( ... ); }`
/// (`magic_spell_effect.cpp:1322-1330`). **Three spells in this content name a group**, and a world that never installed the groups answers nothing rather than inventing a monster.
fn monster_to_summon(world: &mut World, spell: &Spell, from_group: bool) -> Option<String> {
    if !from_group {
        return Some(spell.effect_str.clone());
    }
    let mut rolls = crate::singletons::take_rolls(world)?;
    let picked = crate::monster_groups::pick(world, &spell.effect_str, &mut rolls);
    crate::singletons::put_rolls(world, rolls);
    picked
}

/// Whether a summoned monster arrives on the caster's side.
///
/// Reference: `is_summon_friendly` (`magic_spell_effect.cpp:1306-1314`): not hostile unless `HOSTILE_SUMMON`, and `HOSTILE_50` makes that a coin flip-
/// `rng( 0, 1000 ) < 500`. **Thirty spells in this content are hostile summons and exactly one is the half-hostile one.**
fn friendly_summon(world: &mut World, spell: &Spell) -> bool {
    let mut friendly = !spell.has_flag("HOSTILE_SUMMON");
    if spell.has_flag("HOSTILE_50") {
        let Some(mut rolls) = crate::singletons::take_rolls(world) else {
            return friendly;
        };
        let flip = rolls.rng(0, 1000) < 500;
        crate::singletons::put_rolls(world, rolls);
        friendly = friendly && flip;
    }
    friendly
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
