//! Standing a corpse back up.
//!
//! Reference: `game::revive_corpse` (`game.cpp:5713-5760`) and the two spell effects that call it, `spell_effect::revive` and `spell_effect::revive_dormant`
//! (`magic_spell_effect.cpp:1551-1596`).
//!
//! The **filter** is the rule, and the two spells differ in exactly one clause:
//!
//! | question | `revive` | `revive_dormant` |
//! |---|---|---|
//! | the corpse can revive at all (`REVIVES`) | yes | yes |
//! | the kind is `DORMANT` | **no** | **yes, required** |
//! | the kind is `NO_NECRO` | **refused** | ignored |
//! | the kind is of the species the spell names | yes | yes |
//! | the corpse was butchered | refused | refused |
//!
//! **Named, not invented**: the reference also asks the corpse's `active` flag and, for a body that was not a zombie, reads its `zombie_form`; it restores the monster's hit
//! points from the corpse's damage and its upgrades through `monster::init_from_item`. The port's corpses carry their kind, whether it revives and the volume taken up, but not a
//! damage figure or an upgrade list, so a revived monster comes back at its type's full hit points - stated here rather than guessed at.

use bevy_ecs::prelude::*;

/// Why a corpse was not brought back, so a caller can tell "nothing there" from "refused".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// There was no corpse on the square.
    NoCorpse,
    /// The corpse is not one that can revive, or the kind is of the wrong sort for this spell.
    Cannot,
    /// The corpse was butchered, which the reference refuses in `revive_corpse` before any flag is read.
    Butchered,
}

/// Brings back the first corpse on a square that this spell's filter accepts.
///
/// Reference: `spell_effect::revive` walks its area, and for each square walks the items on it, **reviving the first corpse that passes and breaking out of the square**. The
/// monster is created from the corpse and the corpse is removed; a square with several acceptable bodies gives up one of them per cast.
///
/// # Errors
///
/// Returns a [`Refusal`] naming why nothing rose: the square holds no corpse, the corpse is not one that revives, or it was butchered.
pub fn revive(
    world: &mut World,
    square: (i32, i32, i32),
    species: &str,
    dormant: bool,
) -> Result<Entity, Refusal> {
    let corpses = crate::items_at(world, (square.0, square.1));
    for corpse in &corpses {
        let Some(dead) = world.get::<crate::CorpseOf>(*corpse).copied() else {
            continue;
        };
        if butchered(world, *corpse) {
            return Err(Refusal::Butchered);
        }
        if !dead.revives {
            continue;
        }
        let is_dormant = crate::monster_registry::kind_has_flag(world, dead.kind, "DORMANT");
        if is_dormant != dormant {
            continue;
        }
        // Reference: `revive` refuses a `NO_NECRO` kind outright, and `revive_dormant` ignores the flag - it is the *relaxed* revive, which is why the two spells exist.
        if !dormant && crate::monster_registry::kind_has_flag(world, dead.kind, "NO_NECRO") {
            continue;
        }
        if !species.is_empty()
            && !crate::monster_registry::kind_in_species(world, dead.kind, species)
        {
            continue;
        }
        let hit_points = crate::monster_registry::hit_points(world, dead.kind);
        let raised = crate::spawn_monster(world, (square.0, square.1), dead.kind, hit_points);
        world.entity_mut(*corpse).despawn();
        crate::PositionIndex::rebuild(world);
        return Ok(raised);
    }
    Err(Refusal::Cannot)
}

/// Whether a corpse has been butchered, which the reference spells `FIELD_DRESS` and `FIELD_DRESS_FAILED`.
fn butchered(world: &World, corpse: Entity) -> bool {
    world.get::<crate::ItemFlags>(corpse).is_some_and(|flags| {
        flags.0.iter().any(|flag| {
            matches!(
                flag.as_str(),
                "FIELD_DRESS" | "FIELD_DRESS_FAILED" | "QUARTERED"
            )
        })
    })
}
