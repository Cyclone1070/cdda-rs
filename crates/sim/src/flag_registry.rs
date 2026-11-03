//! Every flag id the data declares, which is what a `foreach` over flags walks.
//!
//! Reference: `json_flag::get_all()` (`flag.h:406`), the registry `f_foreach` reads when the data says `{"foreach": "ids", "target": "flag"}`
//! (`npctalk.cpp:7041-7058`). A flag is a name the data gives a thing - `SEALED`, `FIREPROOF`, `BELTED` - and the list is **the data's own**, so the port keeps
//! it as a table it is handed rather than a list someone typed.

use bevy_ecs::prelude::*;

/// Every flag id the loaded data declares.
#[derive(Debug, Clone, Default, Component)]
pub struct FlagTypes(pub Vec<String>);

/// The entity that holds [`FlagTypes`].
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheFlagTypes;

/// Installs the flag names.
pub fn install(world: &mut World, flags: Vec<String>) {
    let holder = crate::singletons::singleton::<FlagTypes, TheFlagTypes>(world);
    if let Some(mut table) = world.get_mut::<FlagTypes>(holder) {
        table.0 = flags;
    }
}

/// Every flag id the data declares, in the order the reference keeps them.
#[must_use]
pub fn flags_of(world: &World) -> Vec<String> {
    crate::singletons::holder::<TheFlagTypes>(world)
        .and_then(|holder| world.get::<FlagTypes>(holder).map(|table| table.0.clone()))
        .unwrap_or_default()
}

/// Whether the data declares a flag.
#[must_use]
pub fn has(world: &World, flag: &str) -> bool {
    flags_of(world).iter().any(|known| known == flag)
}
