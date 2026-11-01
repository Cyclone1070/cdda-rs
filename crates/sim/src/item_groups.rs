//! The item groups the data defines, so a spawn can name a group instead of an item.
//!
//! Reference: `item_group` (`item_group.cpp`) is a registry of groups, each a **collection** (every entry rolled separately) or a **distribution** (one entry picked
//! by probability), and `item_group::item_from` draws from it. The port keeps the definitions in one component on one entity, and the drawing rule itself lives in
//! `cdda_content::item_group_draw`, where the reader already put it.

use bevy_ecs::prelude::{Component, Entity, With, World};

/// Every group definition the data loaded, by id.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct ItemGroups(pub std::collections::BTreeMap<String, cdda_content::item_group_draw::Definition>);

/// Marks the one entity that holds the world's item groups.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheItemGroups;

/// Installs the definitions the data loaded, replacing any that were there.
pub fn set_groups(
    world: &mut World,
    groups: std::collections::BTreeMap<String, cdda_content::item_group_draw::Definition>,
) {
    let entity = crate::singletons::singleton::<ItemGroups, TheItemGroups>(world);
    if let Some(mut held) = world.get_mut::<ItemGroups>(entity) {
        held.0 = groups;
    }
}

/// The definitions, when the world has loaded any.
#[must_use]
pub fn holder(world: &World) -> Option<Entity> {
    let mut query = world.try_query_filtered::<Entity, With<TheItemGroups>>()?;
    query.iter(world).next()
}

/// Draws what a group produces, using the world's own dice.
///
/// Answers `None` when the world holds no groups or the dice are missing - both are states a caller can check rather than a silent empty draw.
pub fn draw(world: &mut World, id: &str) -> Option<Vec<(String, i32)>> {
    let entity = holder(world)?;
    let definitions = world.get::<ItemGroups>(entity)?.0.clone();
    let mut dice = crate::singletons::take_rolls(world);
    let drawn = dice
        .as_mut()
        .map(|rolls| cdda_content::item_group_draw::draw(&definitions, id, rolls));
    if let Some(dice) = dice.take() {
        crate::singletons::put_rolls(world, dice);
    }
    drawn
}
