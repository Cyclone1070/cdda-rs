//! What each activity is, and what its moves are counted in.
//!
//! Reference: `activity_type::based_on` (`activity_type.h:44`) and the `activity_type_all` registry (`activity_type.cpp:127-136`). The port keeps the definitions the data
//! loaded in one place, because two rules ask them: the schedule has to know whether an activity spends a creature's moves or its own, and the dialogue has to be able to say what
//! a person is doing.

use bevy_ecs::prelude::*;

/// Every activity the data loaded.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivityTypes(pub Vec<cdda_content::activity_types::ActivityType>);

/// Marks the one entity that holds the world's activities.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheActivityTypes;

/// Installs the activities the data loaded, replacing any that were there.
pub fn set_activity_types(
    world: &mut World,
    activities: Vec<cdda_content::activity_types::ActivityType>,
) {
    let entity = crate::singletons::singleton::<ActivityTypes, TheActivityTypes>(world);
    if let Some(mut held) = world.get_mut::<ActivityTypes>(entity) {
        held.0 = activities;
    }
}

/// The entity that holds the activities, or `None` when the data has not been loaded.
#[must_use]
pub fn holder(world: &World) -> Option<Entity> {
    crate::singletons::holder::<TheActivityTypes>(world)
}

/// One activity, by the id a `Doing` carries.
#[must_use]
pub fn activity_named(
    world: &World,
    activity: &str,
) -> Option<cdda_content::activity_types::ActivityType> {
    let holder = holder(world)?;
    world
        .get::<ActivityTypes>(holder)
        .and_then(|types| types.0.iter().find(|kind| kind.id == activity).cloned())
}

/// What an activity's moves are counted in, and `None` when the data does not define it.
///
/// Reference: `activity_type::based_on` (`activity_type.h:44-46`), whose three answers are the port's [`crate::creature::Basis`]: `speed` is `Speed`, `time` is `Time`, and
/// `neither` is `Neither`. **An unknown activity has no answer** rather than a default: a `Doing` naming something the data never defined is a broken save or a broken rule, and
/// silently counting its moves in speed's way would hide it.
#[must_use]
pub fn basis_of(world: &World, activity: &str) -> Option<crate::creature::Basis> {
    match activity_named(world, activity)?.based_on.as_str() {
        "speed" => Some(crate::creature::Basis::Speed),
        "time" => Some(crate::creature::Basis::Time),
        "neither" => Some(crate::creature::Basis::Neither),
        _ => None,
    }
}
