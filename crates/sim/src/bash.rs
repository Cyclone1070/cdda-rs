//! Bashing: what one hit does to a square.
//!
//! Reference: `map::bash_ter_furn` (`src/map.cpp:3932-4245`), the terrain-and-furniture half of `map::bash`. The other halves — items, fields, vehicles, sounds, explosions,
//! collapses and tents — are named in [`bash`]'s gaps rather than guessed at.
//!
//! Three rules the code below is built around, all of them easy to get wrong:
//!
//! 1. **Furniture is tried first.** If the square has furniture with a bash block, that block is used and the terrain underneath is not touched, even if the furniture is only
//!    a chair on a concrete floor.
//! 2. **A failed hit that exceeded `str_min` leaves damage behind**, at least one point and more the harder it hit, and that damage is added to the *next* hit's strength. So
//!    a square can take several hits and the state between them is the square's, not the caller's.
//! 3. **`success` is not "something broke".** Under `destroy` the strength check is skipped entirely and `success` stays false, while the square is destroyed anyway — the
//!    reference's `params.success |= success` (`src/map.cpp:4241`) is what carries that quirk out.

use cdda_content::mapdata::MapData;
use std::collections::BTreeMap;

/// The knob the reference's own strength is handed in through (`bash_params`, `src/map.h:139-176`).
// The struct **is** the reference's `bash_params` (`src/map.h:139-176`), which is a bag of six knobs by design; splitting it to please a count would put a translation
// between the port and the thing it ports.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BashParams {
    /// How hard the hit is.
    pub strength: i32,
    /// Whether the hit acts, rather than being reported.
    pub silent: bool,
    /// A `destroy` hit ignores the strength thresholds entirely and always breaks the square.
    pub destroy: bool,
    /// Whether a floor with `bash_below` may be the tile that is bashed.
    pub bash_floor: bool,
    /// The roll between `str_min` and `str_max`, from zero to one, which the reference draws itself.
    pub roll: f32,
    /// Whether the hit comes from the level above, which decides what a destroyed terrain becomes.
    pub bashing_from_above: bool,
}

/// The square being hit, as the reference reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BashTarget {
    /// The terrain's id.
    pub terrain: String,
    /// The furniture's id, `f_null` for none.
    pub furniture: String,
    /// The terrain of the square **below**, which a floor's strength depends on.
    pub below: String,
    /// The damage the square has already taken.
    pub damage: i32,
}

/// What the hit did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BashOutcome {
    /// The terrain after the hit.
    pub terrain: String,
    /// The furniture after the hit.
    pub furniture: String,
    /// The damage the square keeps.
    pub damage: i32,
    /// Whether the hit touched anything.
    pub did_bash: bool,
    /// Whether the strength check succeeded. **Not** whether something broke.
    pub success: bool,
    /// Whether something solid was hit.
    pub bashed_solid: bool,
}

/// Hits a square once.
///
/// **Declared gaps**, all of them outside this function's data: items and fields on the square, vehicles, the sounds a hit makes, the messages it prints, `explosive` blocks,
/// the fungal spore and tent-collapse paths, signage, `furn_is_supported` (the data defines no `str_min_blocked`), and the roof the reference regenerates over a floor it has
/// destroyed — that last one is why a destroyed floor answers `t_open_air` here.
#[must_use]
pub fn bash(
    target: &BashTarget,
    params: BashParams,
    data: &BTreeMap<String, MapData>,
) -> BashOutcome {
    let Some(terrain) = data.get(&target.terrain) else {
        return unchanged(target);
    };
    let furniture = data.get(&target.furniture);
    let has_furniture = furniture.is_some_and(|furniture| furniture.id != "f_null");
    let mut outcome = BashOutcome {
        terrain: target.terrain.clone(),
        furniture: target.furniture.clone(),
        damage: target.damage,
        did_bash: false,
        success: false,
        bashed_solid: false,
    };

    let (bash, smash_furniture) = choose_block(terrain, furniture, has_furniture);
    let mut smash_terrain = !smash_furniture && bash.is_some();

    // The floor check (`src/map.cpp:3966-3985`): a terrain that hands its bash down to the tile below is not the tile being bashed unless the caller said so, and a
    // `destroy` still gets through unless the terrain it would become is the terrain it already is.
    let mut bash = bash;
    if smash_terrain
        && bash.as_ref().is_some_and(|block| block.bash_below)
        && !params.bash_floor
    {
        // An ordinary hit cannot reach a floor at all; a `destroy` gets through unless the floor would become the terrain it already is (`src/map.cpp:3969-3985`), which is
        // the reference's own guard against a `destroy` that never ends.
        let reached = params.destroy
            && !bash.as_ref().is_some_and(|block| {
                block.set.is_empty() || block.set == "t_null" || block.set == target.terrain
            });
        if !reached {
            smash_terrain = false;
            bash = None;
        }
    }

    let Some(block) = bash else {
        // Nothing bashable here. The reference still reports a hit if the square cannot be walked through (`src/map.cpp:3992-4006`), which is what makes bashing a chain
        // fence a hit on something solid rather than a swing at nothing.
        if move_cost(terrain, furniture) == 0 {
            outcome.did_bash = true;
            outcome.bashed_solid = true;
        }
        return outcome;
    };
    if block.destroy_only && !params.destroy {
        if move_cost(terrain, furniture) == 0 {
            outcome.did_bash = true;
            outcome.bashed_solid = true;
        }
        return outcome;
    }

    let mut minimum = block.str_min;
    let mut maximum = block.str_max;
    let mut success = false;
    if !params.destroy {
        // `str_min_supported`/`str_max_supported` apply when the square below supports a roof (`src/map.cpp:4021-4030`); `str_min_blocked` needs neighbouring furniture and
        // the data defines none, so that branch is a declared gap.
        if block.str_min_supported != -1 || block.str_max_supported != -1 {
            let below_supports = data
                .get(&target.below)
                .is_some_and(|below| below.has_flag("SUPPORTS_ROOF"));
            if below_supports {
                if block.str_min_supported != -1 {
                    minimum = block.str_min_supported;
                }
                if block.str_max_supported != -1 {
                    maximum = block.str_max_supported;
                }
            }
        }
        let mut damage = outcome.damage;
        if params.strength + damage >= maximum {
            damage = 0;
            success = true;
        } else if params.strength >= minimum {
            #[allow(clippy::cast_possible_truncation)]
            #[allow(clippy::cast_precision_loss)]
            let earned = ((params.strength - minimum) as f32 * params.roll).max(1.0);
            #[allow(clippy::cast_possible_truncation)]
            {
                damage += earned as i32;
            }
        }
        outcome.damage = damage;
    }

    if !params.destroy && !success {
        // A failed hit is still a hit, and the damage it left behind is what makes the next one easier.
        outcome.did_bash = true;
        return outcome;
    }

    if smash_furniture {
        outcome.furniture.clone_from(&block.set);
    } else if smash_terrain {
        let from_above = if params.bashing_from_above {
            block.set_from_above.as_deref()
        } else {
            None
        };
        match from_above {
            Some(above) => above.clone_into(&mut outcome.terrain),
            // `ter_set` is a **string id**: `t_null` is what the data writes when a destroyed tile should become open air rather than a named terrain.
            None if !block.set.is_empty() && block.set != "t_null" => {
                outcome.terrain.clone_from(&block.set);
            }
            None => "t_open_air".clone_into(&mut outcome.terrain),
        }
    }

    outcome.did_bash = true;
    outcome.success |= success;
    outcome.bashed_solid = true;
    outcome
}

/// Which block a hit uses: the furniture's when there is furniture with one, otherwise the terrain's.
///
/// Reference: `map::bash_ter_furn` (`src/map.cpp:3956-3964`), where the choice is one `if`/`else if` — furniture with a block **hides** the terrain underneath, even when the
/// furniture is a chair and the terrain a concrete floor.
fn choose_block(
    terrain: &MapData,
    furniture: Option<&MapData>,
    has_furniture: bool,
) -> (Option<cdda_content::mapdata::BashInfo>, bool) {
    match furniture {
        Some(furniture) if has_furniture && furniture.bash.is_some() => (furniture.bash.clone(), true),
        _ => (terrain.bash.clone(), false),
    }
}

/// What a square costs to walk through, which is how the reference decides whether a hit struck something solid.
///
/// Reference: `map::move_cost_internal` (`src/map.cpp:2331-2353`): a terrain that cannot be walked through makes the square impassable whatever stands on it, furniture with a
/// negative cost does the same, and otherwise the costs add.
fn move_cost(terrain: &MapData, furniture: Option<&MapData>) -> i32 {
    if terrain.move_cost == 0 {
        return 0;
    }
    if furniture.is_some_and(|furniture| furniture.id != "f_null" && furniture.move_cost < 0) {
        return 0;
    }
    let mut cost = terrain.move_cost.max(0);
    if let Some(furniture) = furniture.filter(|furniture| furniture.id != "f_null") {
        cost += furniture.move_cost.max(0);
    }
    cost
}

/// The answer for a square whose terrain the world does not have: nothing was hit.
fn unchanged(target: &BashTarget) -> BashOutcome {
    BashOutcome {
        terrain: target.terrain.clone(),
        furniture: target.furniture.clone(),
        damage: target.damage,
        did_bash: false,
        success: false,
        bashed_solid: false,
    }
}
