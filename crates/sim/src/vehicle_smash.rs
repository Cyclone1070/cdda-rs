//! **Breaking a whole vehicle at once.** Reference: `vehicle::smash` (`vehicle.cpp:908-948`), the rule the reference uses when a vehicle is placed in a bad state - a wreck
//! that has been sitting somewhere, a car whose owner did not come back.
//!
//! The rule walks every part and asks two questions in order:
//!
//! 1. **Is this square already over-built?** A square holding more than one structure is destroyed outright - every part on it goes to zero - because the reference says a
//!    square cannot carry two structures and the data got here through damage rather than construction.
//! 2. **Otherwise the part is rolled for**: one in a thousand per thousandth of `percent`, and if it loses, its hit points drop by a random fraction of its own durability
//!    between `hp_percent_loss_min` and `hp_percent_loss_max`, scaled by how far the part is from the damage's origin.
//!
//! The port keeps the reference's own signature and defaults: [`Smash::percent`] is the one argument `init_state` passes (`smash( placed_on, 0.5 )`, `vehicle.cpp:616`), and
//! the rest are the header's defaults (`vehicle.h:986-988`).

use bevy_ecs::prelude::{Entity, World};

use crate::vehicles::{Vehicle, part_spec};

/// One `smash` call's numbers: how hard, how many parts, and where the blow came from.
///
/// Reference: `vehicle::smash`'s parameters (`vehicle.h:986-988`). A rule that only wants "half the parts, hard" writes [`Smash::half`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Smash {
    /// The least a part can lose, as a fraction of its durability - `hp_percent_loss_min`.
    pub hp_min: f64,
    /// The most a part can lose - `hp_percent_loss_max`, where anything over one breaks the part.
    pub hp_max: f64,
    /// The fraction of parts the blow reaches - `percent_of_parts_to_affect`.
    pub percent: f64,
    /// The square the blow came from, as an offset from the vehicle's origin - `damage_origin`.
    pub origin: (i32, i32),
    /// How far the blow reaches, where zero means **every part equally** - `damage_size`.
    pub size: f64,
}

impl Smash {
    /// The header's defaults with every part reached: 10% to 120%, all parts, from nowhere in particular.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            hp_min: 0.1,
            hp_max: 1.2,
            percent: 1.0,
            origin: (0, 0),
            size: 0.0,
        }
    }

    /// **Half the parts, 50% to 120% of their durability** - what `init_state` does to a disabled vehicle (`smash( placed_on, 0.5 )`, `vehicle.cpp:616`).
    #[must_use]
    pub const fn half() -> Self {
        Self {
            hp_min: 0.5,
            hp_max: 1.2,
            percent: 1.0,
            origin: (0, 0),
            size: 0.0,
        }
    }
}

/// **Destroys parts of a vehicle across the board.** Reference: `vehicle::smash` (`vehicle.cpp:908-948`).
///
/// The two branches are the reference's own, in its order: a square with **more than one structure** is emptied regardless of the dice, and every other part is rolled for.
///
/// **Named gaps, both fields the reference has and the port does not**: `ammo_unset` (a port part's fuel is two fields, and clearing them is what the port's other damage rules do
/// instead of calling it), and `part.precalc` - the reference measures the distance to the part's **precalculated square**, and the port measures to the part's mount, which is
/// the same square before rotation. `damage_size` is zero everywhere the port calls this, where the distance is not read at all.
pub fn smash(
    world: &mut World,
    vehicle: Entity,
    smash: &Smash,
    dice: &mut cdda_core_types::rng::Rolls,
) {
    let Some(record) = world.get::<Vehicle>(vehicle).cloned() else {
        return;
    };
    // **How many structures each square holds**, which is the first question the reference asks. A part the store does not know is not a structure.
    let mut structures: std::collections::BTreeMap<(i32, i32), usize> =
        std::collections::BTreeMap::new();
    for (index, part) in record.parts.iter().enumerate() {
        let is_structure =
            part_spec(world, vehicle, index).is_some_and(|spec| spec.location == "STRUCTURE");
        if is_structure {
            *structures.entry((part.x, part.y)).or_default() += 1;
        }
    }
    let percent_in_thousandths = {
        #[allow(clippy::cast_possible_truncation)]
        let thousandths = (smash.percent * 1000.0) as i32;
        thousandths
    };
    let mut parts = record.parts;
    for (index, part) in parts.iter_mut().enumerate() {
        // Reference: `if( part.is_broken() || part.removed ) { continue; }`. The port has no `removed`: a removed part is not a part.
        if part.broken {
            continue;
        }
        // **An over-built square is emptied, whatever the dice say** - and the reference then moves to the next part without rolling.
        if structures.get(&(part.x, part.y)).copied().unwrap_or(0) > 1 {
            part.hp = 0;
            part.broken = true;
            part.charges = 0;
            part.fuel.clear();
            continue;
        }
        let roll = dice.dice(1, 1000);
        if roll >= percent_in_thousandths {
            continue;
        }
        // Reference: the distance falls from one at the origin to zero at `damage_size`, and a size of zero means "every part equally".
        let distance = if smash.size.abs() < f64::EPSILON {
            1.0
        } else {
            let dx = f64::from(smash.origin.0 - part.x);
            let dy = f64::from(smash.origin.1 - part.y);
            let away = (dx * dx + dy * dy).sqrt();
            (1.0 - away / smash.size).clamp(0.0, 1.0)
        };
        let durability = part_spec(world, vehicle, index).map_or(0, |spec| spec.durability);
        let loss = dice.rng_float(smash.hp_min * distance, smash.hp_max * distance);
        #[allow(clippy::cast_possible_truncation)]
        let damage = (f64::from(durability) * loss) as i32;
        part.hp = part.hp.saturating_sub(damage);
        // Reference: `if( mod_hp( part, -durability * roll ) ) { part.ammo_unset(); }` - a part that broke is emptied.
        if part.hp <= 0 {
            part.hp = 0;
            part.broken = true;
            part.charges = 0;
            part.fuel.clear();
        }
    }
    if let Some(mut record) = world.get_mut::<Vehicle>(vehicle) {
        record.parts = parts;
    }
}
