//! Which things a spell may be aimed at.
//!
//! Reference: `spell::is_valid_target( caster, p )` (`magic.cpp:1697-1719`), the gate every effect walks before it touches a square: a creature on that square is hostile, an ally or
//! the caster itself; a bare square is `ground`; and on top of that the spell may name the monster ids it wants, the species it wants, and the species it leaves alone
//! (`target_by_monster_id`, `target_by_species_id`, `ignore_by_species_id`, `:1721-1760`).
//!
//! **A spell that names no species accepts every species**, which is what the reference says with `if( targeted_species_ids.empty() ) return true;` - and the difference between
//! "no filter" and "filtered to nothing" is the difference between a spell that hits everything and one that hits nothing.

use bevy_ecs::prelude::*;

use cdda_content::spells::Spell;

/// Whether a spell may affect the creature or square it was aimed at, cast from `caster_square`.
///
/// Reference: `spell::is_valid_target( const Creature &caster, const tripoint &p )`. The three creature cases are tried in the reference's order and any one of them may validate the
/// square; the filters then **narrow** the answer rather than widening it, which is why they are applied with `&&` after the class test.
///
/// **The caster is its square, not its entity.** Everything the reference asks a caster here is `p != caster.pos()` and `p == caster.pos()`; a square is therefore the whole
/// question, and taking it that way keeps this rule callable from a cast that was handed a square - which `spellcasting::cast_attack` is.
#[must_use]
pub fn allows(
    world: &World,
    caster_square: (i32, i32, i32),
    spell: &Spell,
    square: (i32, i32, i32),
) -> bool {
    let creature = crate::creature::creature_at(world, square).first().copied();
    let Some(creature) = creature else {
        // Reference: a square with no creature and no vehicle is `ground`, and a vehicle square is either `vehicle` or `ground` - the port has no vehicles, so both answers are the
        // ground answer until one exists.
        return spell.valid_targets.contains("ground");
    };
    let friendly = crate::friendly::is_friendly(world, creature);
    let mut valid = false;
    if !friendly && spell.valid_targets.contains("hostile") {
        valid = true;
    }
    if friendly && spell.valid_targets.contains("ally") && square != caster_square {
        valid = true;
    }
    if spell.valid_targets.contains("self") && square == caster_square {
        valid = true;
    }
    valid
        && by_monster_id(world, spell, creature)
        && by_species(world, spell, creature)
        && not_ignored_species(world, spell, creature)
}

/// Whether the creature's kind is one the spell names, or the spell names none.
///
/// Reference: `spell::target_by_monster_id` (`magic.cpp:1721-1735`).
fn by_monster_id(world: &World, spell: &Spell, creature: Entity) -> bool {
    if spell.targeted_monster_ids.is_empty() {
        return true;
    }
    let Some(kind) = world
        .get::<crate::creature::CreatureKind>(creature)
        .map(|held| held.0)
    else {
        return false;
    };
    crate::monster_registry::name_of(world, kind)
        .is_some_and(|name| spell.targeted_monster_ids.contains(&name))
}

/// Whether the creature's species is one the spell names, or the spell names none.
///
/// Reference: `spell::target_by_species_id` (`magic.cpp:1737-1750`). A creature with no kind at all - a character - is not in any species, so a spell that names species leaves it
/// alone.
fn by_species(world: &World, spell: &Spell, creature: Entity) -> bool {
    if spell.targeted_species_ids.is_empty() {
        return true;
    }
    let Some(kind) = world
        .get::<crate::creature::CreatureKind>(creature)
        .map(|held| held.0)
    else {
        return false;
    };
    spell
        .targeted_species_ids
        .iter()
        .any(|species| crate::monster_registry::kind_in_species(world, kind, species))
}

/// Whether the creature's species is one the spell leaves alone.
///
/// Reference: `spell::ignore_by_species_id` (`magic.cpp:1752-1764`) - the same walk as `by_species`, answering the opposite, and true when the spell names none.
fn not_ignored_species(world: &World, spell: &Spell, creature: Entity) -> bool {
    if spell.ignored_species_ids.is_empty() {
        return true;
    }
    let Some(kind) = world
        .get::<crate::creature::CreatureKind>(creature)
        .map(|held| held.0)
    else {
        return true;
    };
    !spell
        .ignored_species_ids
        .iter()
        .any(|species| crate::monster_registry::kind_in_species(world, kind, species))
}
