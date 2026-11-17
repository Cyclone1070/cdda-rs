//! **What a vehicle weighs.**
//!
//! Reference: `vehicle::calc_mass_center` (`vehicle.cpp:8484-8547`), which sums one mass term for every part and caches it; `vehicle::total_mass` (`vehicle.cpp:3787-3797`)
//! answers the cache. Movement needs the number before it can decide anything: acceleration, braking and collision all read it.
//!
//! A part's term is its **base item's weight** plus the weight of the items it carries, with the part's `cargo_weight_modifier` scaling the carried part. That is the whole
//! rule, and it is the first one in the port that reads what the last rounds built - the parts, the cargo items and their kinds.

use bevy_ecs::prelude::*;

use crate::GroundItem;
use crate::vehicle_cargo::items_in_part;
use crate::vehicles::{Vehicle, part_spec};

/// What one part adds to the vehicle, in grams.
///
/// Reference: the `m_part` term of `calc_mass_center` (`vehicle.cpp:8495-8512`): `base.weight()` plus every carried item's `weight()`, the carried sum scaled by
/// `cargo_weight_modifier` when it is not 100. The port scales with integer arithmetic, so a modifier that does not divide the contents truncates where the reference's
/// floating-point multiply would round - the current data has no such part.
///
/// **Named, not guessed:** the reference also adds the listed tools' weight, a tied-down piece of furniture's mass, and, for a `BOARDABLE` part, its passenger's and
/// monster's weight. The port has none of those inputs yet.
#[must_use]
pub fn part_mass(world: &World, vehicle: Entity, part: usize) -> i64 {
    let Some(spec) = part_spec(world, vehicle, part) else {
        return 0;
    };
    let base = crate::item_registry::id_in(world, &spec.item)
        .map_or(0, |kind| crate::item_registry::weight_in(world, kind));
    let items: i64 = items_in_part(world, vehicle, part)
        .into_iter()
        .filter_map(|item| world.get::<GroundItem>(item))
        .map(|item| {
            crate::item_registry::weight_in(world, item.kind) * i64::from(item.charges.max(1))
        })
        .sum();
    base + items * spec.cargo_weight_modifier / 100
}

/// Where the vehicle's weight sits, in mount coordinates - the reference's `local_center_of_mass`.
///
/// Reference: `vehicle::calc_mass_center` (`vehicle.cpp:8484-8547`) with `use_precalc` false: each part contributes its mass at its mount, the sum is divided by the total, and the
/// answer is **rounded** to a whole mount. A vehicle whose parts weigh nothing answers the origin rather than dividing by zero, which is the reference's own guard.
#[must_use]
pub fn center_of_mass(world: &World, vehicle: Entity) -> (i32, i32) {
    let count = world
        .get::<Vehicle>(vehicle)
        .map_or(0, |built| built.parts.len());
    let mut weighted_x = 0.0;
    let mut weighted_y = 0.0;
    let mut total = 0_i64;
    for part in 0..count {
        let Some(entry) = world
            .get::<Vehicle>(vehicle)
            .and_then(|built| built.parts.get(part))
        else {
            continue;
        };
        let mass = part_mass(world, vehicle, part);
        if mass == 0 {
            continue;
        }
        #[allow(clippy::cast_precision_loss)]
        let mass_f = mass as f64;
        weighted_x += f64::from(entry.x) * mass_f;
        weighted_y += f64::from(entry.y) * mass_f;
        total += mass;
    }
    if total == 0 {
        return (0, 0);
    }
    #[allow(clippy::cast_precision_loss)]
    let total_f = total as f64;
    #[allow(clippy::cast_possible_truncation)]
    let x = (weighted_x / total_f).round() as i32;
    #[allow(clippy::cast_possible_truncation)]
    let y = (weighted_y / total_f).round() as i32;
    (x, y)
}

/// What the whole vehicle weighs, in grams - the reference's `total_mass` cache.
///
/// **Named:** the reference skips a part that is `removed` or a fake part, and its cache can be modified by enchantments. The port has neither removed parts, fake parts nor
/// enchantments on a vehicle yet, so every part counts.
#[must_use]
pub fn mass(world: &World, vehicle: Entity) -> i64 {
    let count = world
        .get::<Vehicle>(vehicle)
        .map_or(0, |built| built.parts.len());
    (0..count).map(|part| part_mass(world, vehicle, part)).sum()
}
