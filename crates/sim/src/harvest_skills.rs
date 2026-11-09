//! Which skills each harvest drop type is judged on, which is data rather than a rule.
//!
//! Reference: `harvest_drop_type.json`, read by `butchery_dissect_skill_level` (`butchery.cpp:550-564`). The five ordinary
//! tissues name no skills, so every ordinary corpse in the game is cut up on the blade alone; the strange drops name their
//! own, and a mutagen's are chemistry and first aid.
//!
//! The table is a component on a singleton like every other registry: one store, no globals, and a world that installed
//! none answers "no skills" for everything - which is the reference's answer for a type with no `harvest_skills` too.

use bevy_ecs::prelude::{Component, World};
use std::collections::BTreeMap;

/// Every drop type's skill list, by the reference's id.
#[derive(Component, Debug, Clone, Default)]
pub struct HarvestDropSkills {
    /// The skills a dissection of this drop type is judged on, in the data's order.
    by_type: BTreeMap<String, Vec<String>>,
}

/// The marker that says which entity holds [`HarvestDropSkills`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheHarvestDropSkills;

/// Installs the drop types' skill lists, replacing whatever the world held.
pub fn set(world: &mut World, by_type: &BTreeMap<String, Vec<String>>) {
    let entity = crate::singletons::singleton::<HarvestDropSkills, TheHarvestDropSkills>(world);
    let Some(mut table) = world.get_mut::<HarvestDropSkills>(entity) else {
        return;
    };
    table.by_type = by_type.clone();
}

/// The skills one drop type is judged on; empty for one that names none, and for one the world never installed.
#[must_use]
pub fn skills_for(world: &World, drop_type: &str) -> Vec<String> {
    world
        .iter_entities()
        .find_map(|entity| entity.get::<HarvestDropSkills>())
        .and_then(|table| table.by_type.get(drop_type).cloned())
        .unwrap_or_default()
}
