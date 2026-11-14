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

use bevy_ecs::prelude::*;
use cdda_content::mapdata::MapData;
use std::collections::BTreeMap;

/// Installs the map data every square rule reads - bashing's, and now the vehicle rules' too.
///
/// The table itself is [`crate::mapsquare::MapDataTable`], because it is not bash's: it is the loader's record of
/// what each tile id says about itself, and one store keeps it once. Reference: `map::bash_ter_furn` reads each
/// square's `ter_t` and `furn_t` prototypes out of the loaded game data.
pub fn set_data(world: &mut World, entries: impl IntoIterator<Item = MapData>) {
    crate::mapsquare::set_data(world, entries);
}

/// Hits one square of the world, reading it and writing back what the hit did.
///
/// This is the join between the rule and the store: [`bash`] decides, and this reads the square's terrain, furniture, the square below and the damage it has kept, then writes
/// the answers back through the same functions every other rule uses. The **roll** the reference draws inside its own strength check is drawn here, from the store's generator.
///
/// Reference: `map::bash( pos, force, silent )` (`src/map.cpp:4251-4261`), which is `bash_ter_furn` plus the writing back. Answering `None` means the world had no map data to
/// bash with, which is a setup fault rather than a rule.
pub fn bash_square(
    world: &mut World,
    at: (i32, i32, i32),
    strength: i32,
    silent: bool,
) -> Option<BashOutcome> {
    let data = crate::mapsquare::table(world)?.0;
    let ground = crate::square::terrain_at(world, at).and_then(|code| crate::terrain_types::name_of(world, code))?;
    let furniture = crate::square::furniture_at(world, at)
        .and_then(|code| crate::furniture_types::name_of(world, code))
        .unwrap_or_else(|| "f_null".to_owned());
    let below = crate::square::terrain_at(world, (at.0, at.1, at.2 - 1))
        .and_then(|code| crate::terrain_types::name_of(world, code))
        .unwrap_or_else(|| "t_open_air".to_owned());
    let target = BashTarget {
        terrain: ground,
        furniture,
        below,
        damage: crate::square::bash_damage_at(world, at),
    };
    let mut rolls = crate::singletons::take_rolls(world)?;
    #[allow(clippy::cast_possible_truncation)]
    let roll = rolls.rng_float(0.0, 1.0) as f32;
    crate::singletons::put_rolls(world, rolls);
    let params = BashParams {
        strength,
        silent,
        destroy: false,
        bash_floor: false,
        roll,
        bashing_from_above: false,
    };
    let outcome = bash(&target, params, &data);
    if !outcome.did_bash {
        return Some(outcome);
    }
    if let Some(code) = crate::terrain_types::code_of(world, &outcome.terrain) {
        crate::square::set_terrain(world, at, code);
    }
    if let Some(code) = crate::furniture_types::code_of(world, &outcome.furniture) {
        crate::square::set_furniture(world, at, code);
    }
    crate::square::set_bash_damage(world, at, outcome.damage);
    Some(outcome)
}

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

    // The roof over the square below, and whether this square **is** that roof (`src/map.cpp:3947-3952`). Both are worked out before anything is bashed, because the bash
    // can change either square.
    let below_roof = data.get(&target.below).and_then(|below| below.roof.clone());
    let roof_of_below_tile = below_roof.as_deref() == Some(target.terrain.as_str());

    // The floor bashing check (`src/map.cpp:3966-3985`). An ordinary hit cannot reach a floor at all. A `destroy` **can**, and it keeps going even when the tile would become
    // the null terrain — unless the tile is already the roof of the square below, which is the reference's guard against a `destroy` that never ends.
    let mut bash = bash;
    if smash_terrain && bash.as_ref().is_some_and(|block| block.bash_below) && !params.bash_floor {
        let null_set = bash
            .as_ref()
            .is_some_and(|block| block.set.is_empty() || block.set == "t_null");
        if !params.destroy || (null_set && roof_of_below_tile) {
            smash_terrain = false;
            bash = None;
        }
    }

    let Some(block) = bash else {
        // Nothing bashable here. The reference still reports a hit if the square cannot be walked through (`src/map.cpp:3992-4006`), which is what makes bashing a chain
        // fence a hit on something solid rather than a swing at nothing.
        if square_cost(terrain, furniture) == 0 {
            outcome.did_bash = true;
            outcome.bashed_solid = true;
        }
        return outcome;
    };
    if block.destroy_only && !params.destroy {
        if square_cost(terrain, furniture) == 0 {
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
        outcome.terrain = destined_terrain(
            target,
            params,
            data,
            &block,
            smash_terrain,
            roof_of_below_tile,
            below_roof.as_deref(),
        );
    }

    outcome.did_bash = true;
    outcome.success |= success;
    outcome.bashed_solid = true;
    outcome
}

/// What a destroyed terrain becomes.
///
/// Reference: the `smash_ter` arm of `map::bash_ter_furn`'s destructuring (`src/map.cpp:4183-4236`), in its own order: a hit from above uses `ter_set_bashed_from_above`, an
/// ordinary hit uses `ter_set` unless this square **is** the roof of the one below, and a tile that has no `ter_set` at all becomes open air and then gets a roof back from the
/// floor underneath it.
fn destined_terrain(
    target: &BashTarget,
    params: BashParams,
    data: &BTreeMap<String, MapData>,
    block: &cdda_content::mapdata::BashInfo,
    smash_terrain: bool,
    roof_of_below_tile: bool,
    below_roof: Option<&str>,
) -> String {
    let from_above = if params.bashing_from_above {
        block.set_from_above.as_deref()
    } else {
        None
    };
    let terrain = match from_above {
        Some(above) => above.to_owned(),
        // `ter_set` is a **string id**: `t_null` is what the data writes when a destroyed tile should become open air rather than a named terrain.
        None if !block.set.is_empty() && block.set != "t_null" && !roof_of_below_tile => {
            block.set.clone()
        }
        None => "t_open_air".to_owned(),
    };
    // The roof comes back (`src/map.cpp:4229-4236`): a square left as open air over a floor that supports a roof gets that floor's roof put back over it, so a destroyed room
    // does not become a hole in the world. `allow_air` is what lets a floor-bash leave a hole on purpose.
    let below = data.get(&target.below);
    if ZLEVELS
        && smash_terrain
        && data.get(&terrain).is_some_and(|now| now.has_flag("EMPTY_SPACE"))
        && below.is_some_and(|below| below.has_flag("SUPPORTS_ROOF"))
    {
        let allow_air = params.bash_floor && below.is_some_and(|below| below.move_cost != 0);
        return get_roof(below_roof, allow_air);
    }
    terrain
}

/// Whether the reference is running with z-levels, which decides a handful of the rules above.
///
/// It is a **world option**, and the harness world has it on — which its own answers prove: a floor bashed to open air comes back as `t_rock_roof` (`src/map.cpp:4229`), and
/// that whole branch is behind `if( zlevels )`. A world without z-levels would answer `t_open_air` there instead, and `bash` has no way to ask.
const ZLEVELS: bool = true;

/// What the square below would become if it were used as a roof (`map::get_roof`, `src/map.cpp:3878-3911`).
///
/// **Declared gaps:** the reference also answers `t_rock_floor` below the bottom of the world (`p.z() <= -OVERMAP_DEPTH`) and complains about a `t_null` roof; neither can
/// happen for a square at ground level, which is the only kind this function is asked about.
fn get_roof(below_roof: Option<&str>, allow_air: bool) -> String {
    below_roof.map_or_else(
        || if allow_air { "t_open_air".to_owned() } else { "t_dirt".to_owned() },
        str::to_owned,
    )
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
/// The rule itself is [`crate::mapsquare::SquareData::move_cost`] - one implementation, asked here with the two records
/// `bash` already holds, so the vehicle rules and bashing can never disagree about what a floor is.
fn square_cost(terrain: &MapData, furniture: Option<&MapData>) -> i32 {
    crate::mapsquare::SquareData {
        terrain: terrain.clone(),
        furniture: furniture.cloned(),
    }
    .move_cost()
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
