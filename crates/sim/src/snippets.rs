//! Snippets: the flavour text the game picks from a category.
//!
//! Reference: `snippet_library` (`text_snippets.cpp`). The data groups lines under a `category`, each line carries a `weight` (default 100), and
//! a pick is uniform over the **total** weight - the same rule a weighted list of programs follows.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

/// One line the game can say.
#[derive(Debug, Clone, PartialEq)]
pub struct SnippetLine {
    /// The line's own id, which is what `same_snippet` remembers.
    pub id: String,
    /// The text to say.
    pub text: String,
    /// How likely it is against its neighbours; the data's default is 100.
    pub weight: i32,
}

/// Every line, grouped by the category the data gives it.
#[derive(Debug, Clone, Default, PartialEq, Component)]
pub struct Snippets(pub BTreeMap<String, Vec<SnippetLine>>);

/// The marker that says a single entity in the world holds the snippets.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheSnippets;

/// Installs the snippets, replacing whatever the world held.
pub fn set_snippets(world: &mut World, lines: impl IntoIterator<Item = (String, SnippetLine)>) {
    let mut by_category: BTreeMap<String, Vec<SnippetLine>> = BTreeMap::new();
    for (category, line) in lines {
        by_category.entry(category).or_default().push(line);
    }
    let holder = crate::singletons::singleton::<Snippets, TheSnippets>(world);
    world.entity_mut(holder).insert(Snippets(by_category));
}

/// Every line in a category, or nothing when the port does not know it.
#[must_use]
pub fn category(world: &World, name: &str) -> Vec<SnippetLine> {
    crate::singletons::holder::<TheSnippets>(world)
        .and_then(|holder| world.get::<Snippets>(holder))
        .and_then(|snippets| snippets.0.get(name).cloned())
        .unwrap_or_default()
}

/// Picks one line from a category, uniformly over the total weight.
///
/// Reference: `snippet_library::random_id_from_category` (`text_snippets.cpp`), which draws from a `weighted_int_list` - so a line of weight zero
/// is never picked and the pick is uniform over the sum.
pub fn pick(world: &mut World, name: &str, dice: Option<&mut crate::Rolls>) -> Option<SnippetLine> {
    let lines = category(world, name);
    let total: i32 = lines.iter().map(|line| line.weight.max(0)).sum();
    if total <= 0 {
        return None;
    }
    let Some(dice) = dice else {
        return lines.first().cloned();
    };
    let roll = dice.rng(0, total - 1);
    let mut running = 0;
    for line in lines {
        running += line.weight.max(0);
        if roll < running {
            return Some(line);
        }
    }
    None
}

/// How many categories the world holds, and how many lines they carry.
#[must_use]
pub fn census(world: &World) -> (usize, usize) {
    crate::singletons::holder::<TheSnippets>(world)
        .and_then(|holder| world.get::<Snippets>(holder))
        .map_or((0, 0), |snippets| {
            (
                snippets.0.len(),
                snippets.0.values().map(Vec::len).sum::<usize>(),
            )
        })
}
