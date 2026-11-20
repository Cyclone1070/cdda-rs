//! **Turning an overmap tile into a map.**
//!
//! Reference: `mapgen_factory`'s `generate` (`mapgen.cpp:1070-1090`). A tile's terrain names the mapgens that may build it, each with a weight; the game draws one at
//! random **by weight** - `weights_.pick()` - and that mapgen writes the chunk. A terrain with no mapgen draws nothing, which is the answer a caller needs in order to
//! fall back to whatever the overmap says instead of leaving a blank.
//!
//! Two rules live here and nowhere else: which mapgen is picked, and that the pick uses the store's own generator. A rule that reached for a generator of its own would
//! make a replay disagree with itself, which is the whole reason the port passes `Rolls` around.

use bevy_ecs::prelude::*;

use crate::mapgen_update::{Mapgens, TheMapgens};

/// Draws the map for one overmap tile, and answers how many squares it wrote - or nothing when no mapgen is filed under that terrain.
///
/// Reference: `mapgen_factory::generate` (`mapgen.cpp:1078-1088`): `weights_.pick()` draws one mapgen in proportion to its weight, and `generate` then writes the chunk. The
/// draw is `rng( 1, total )` walking the weights in order, which is what `weighted_list::pick` does (`weighted_list.h`), so the port does the same rather than scaling a
/// float - the two agree on every seed, which is what makes an oracle replay of this rule possible at all.
pub fn generate(world: &mut World, terrain: &str, origin: (i32, i32, i32)) -> Option<usize> {
    generate_below(world, terrain, origin, 0)
}

/// How deep a chain of predecessors may go before the port stops following it.
///
/// The reference has no such bound, because a predecessor is a terrain whose own mapgen names a predecessor in turn only where the data says so, and the data is finite. A cycle in
/// the data would make the reference recurse until the stack runs out; the port answers the same cycle with a **limit** rather than a crash, because a crash takes the whole
/// session with it and this is the one place a data mistake could reach the recursion.
const DEEPEST_PREDECESSOR: usize = 8;

/// [`generate`], with how deep in a predecessor chain this draw already is.
fn generate_below(
    world: &mut World,
    terrain: &str,
    origin: (i32, i32, i32),
    depth: usize,
) -> Option<usize> {
    let choices: Vec<cdda_content::mapgen::MapgenObject> =
        crate::singletons::holder::<TheMapgens>(world)
            .and_then(|holder| world.get::<Mapgens>(holder))
            .and_then(|mapgens| mapgens.0.get(terrain).cloned())?;
    // **A mapgen with no weight is not a candidate**: the reference files one at a weight of at least one (`if( weight >= 1 ) weights_.add_or_replace(...)`,
    // `mapgen.cpp:1070-1074`), and a weight of zero would make the list's total zero and the pick impossible.
    let weights: Vec<i32> = choices.iter().map(|mapgen| mapgen.weight.max(0)).collect();
    let total: i32 = weights.iter().sum();
    if total <= 0 {
        return None;
    }
    let mut rolls = crate::singletons::take_rolls(world)?;
    let roll = rolls.rng(1, total);
    crate::singletons::put_rolls(world, rolls);
    let mut running = 0;
    let mut picked = choices.last();
    for (mapgen, weight) in choices.iter().zip(&weights) {
        running += weight;
        if roll <= running {
            picked = Some(mapgen);
            break;
        }
    }
    let picked = picked?;
    // **The predecessor is drawn first** (`mapgen.cpp:6047-6062`): the mapgen under this one writes its whole chunk, and this one's rows then draw on top of it. A roof keeps
    // the walls of the house below exactly because of this order, and reversing it would leave a roof on bare ground.
    if let Some(predecessor) = picked.predecessor_mapgen.as_deref()
        && depth < DEEPEST_PREDECESSOR
    {
        generate_below(world, predecessor, origin, depth + 1);
    }
    // The chosen mapgen is written by the **one** writer a drawn mapgen has, so a terrain's chunk and an update's chunk cannot disagree about what a symbol means.
    Some(crate::mapgen_update::write(world, picked, origin))
}
