//! **A camp is a place somebody owns.**
//!
//! Reference: `basecamp` (`basecamp.h`), one per overmap tile, with an owner faction; `character::camps` (`character.h:2844`) is the set of tiles the character knows about,
//! and `overmap_buffer.find_camp( tile )` answers the camp on one. `f_u_has_camp` (`condition.cpp:1985-2001`) joins the two questions: it walks the player's tiles and answers
//! true for the first camp whose owner is the player's faction.
//!
//! The port has **one store**, so a camp is an entity carrying [`Camp`] rather than a list beside the world - the same decision every other feature here makes. Ownership is
//! the reference's faction id, kept as the id it is: factions themselves are not ported, and the player's own is the constant the reference returns.

use bevy_ecs::prelude::*;

/// The faction every player belongs to.
///
/// Reference: `avatar::get_faction` returns `faction_your_followers` (`avatar.cpp:102`, `:1182-1185`), the constant `"your_followers"`. It is a constant of the rule rather than
/// something the data states, which is why it lives here as one.
pub const PLAYERS_FACTION: &str = "your_followers";

/// A camp on one overmap tile, owned by one faction.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Camp {
    /// The overmap tile it stands on.
    pub tile: (i32, i32),
    /// The faction that owns it, which is what `f_u_has_camp` compares.
    pub owner: String,
}

/// Makes a camp on a tile, owned by a faction.
pub fn found(world: &mut World, tile: (i32, i32), owner: &str) -> Entity {
    world
        .spawn(Camp {
            tile,
            owner: owner.to_owned(),
        })
        .id()
}

/// How many camps the store holds.
#[must_use]
pub fn count(world: &World) -> usize {
    let Some(mut query) = world.try_query::<&Camp>() else {
        return 0;
    };
    query.iter(world).count()
}

/// Whether a faction owns a camp anywhere.
///
/// Reference: `f_u_has_camp` (`condition.cpp:1985-2001`) - the question is answered by the **owner**, not by the tile, so a camp the player stands in but does not own is not
/// their camp.
#[must_use]
pub fn owns_any(world: &World, faction: &str) -> bool {
    let Some(mut query) = world.try_query::<&Camp>() else {
        return false;
    };
    query.iter(world).any(|camp| camp.owner == faction)
}

/// The camp on a tile, when the store knows one.
#[must_use]
pub fn at(world: &World, tile: (i32, i32)) -> Option<(Entity, String)> {
    let mut query = world.try_query::<(Entity, &Camp)>()?;
    query
        .iter(world)
        .find(|(_, camp)| camp.tile == tile)
        .map(|(entity, camp)| (entity, camp.owner.clone()))
}
