//! What a move aims at.

use crate::martial_arts::Vector;

/// One limb of the striker, as far as a move's needs are concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimbState {
    /// What kind of limb it is.
    pub kind: String,
    /// Whether it is still above its health limit.
    pub healthy: bool,
}

/// A move's need for a number of limbs of one kind.
///
/// Reference: `attack_vector::limb_req` (`martialarts.h:82`): a list of `( body part type, count )` pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorNeed {
    /// What kind of limb it needs.
    pub kind: String,
    /// How many of them it needs.
    pub count: i32,
}

/// Whether the striker has enough healthy limbs of a kind.
///
/// Reference: `character_martial_arts::choose_attack_vector` (`martialarts.cpp:1495-1515`): the count is of parts of that type whose hit points are **above their health limit**, and the vector is
/// discarded when the count falls short (`if( count < req.second )`).
#[must_use]
pub fn limbs_enough(limbs: &[LimbState], need: &VectorNeed) -> bool {
    let healthy = limbs
        .iter()
        .filter(|limb| limb.kind == need.kind && limb.healthy)
        .count();
    i32::try_from(healthy).unwrap_or(i32::MAX) >= need.count
}

/// One aim and the weight it carries into the pick.
#[derive(Debug, Clone, PartialEq)]
pub struct WeightedAim {
    /// The vector's id.
    pub id: String,
    /// Its weight: the damage the strike would do through that aim.
    pub weight: f64,
}

/// Which aim a striker takes, given their weights and one fraction of the total.
///
/// Reference: `weighted_float_list::pick_ent` (`weighted_list.h:284-295`) and `pick` (`:137-143`):
///
/// - the draw is **a fraction of the total weight**, `randi / UINT_MAX * total_weight`;
/// - the entries are walked accumulating weight, and the walk stops at the first whose accumulated weight is **`>=` the draw** - so a draw on a boundary stays with the **earlier** entry;
/// - a list whose total weight is zero aims at nothing, which the reference spells as a null pointer rather than the first entry.
#[must_use]
pub fn weighted_pick(aims: &[WeightedAim], fraction: f64) -> Option<String> {
    let total: f64 = aims.iter().map(|aim| aim.weight).sum();
    if total <= 0.0 {
        return None;
    }
    let picked = fraction.clamp(0.0, 1.0) * total;
    let mut accumulated = 0.0;
    let mut chosen = aims.last();
    for aim in aims {
        accumulated += aim.weight;
        if accumulated >= picked {
            chosen = Some(aim);
            break;
        }
    }
    chosen.map(|aim| aim.id.clone())
}

/// Whether the striker's body can meet every limb need of an aim.
///
/// Reference: `choose_attack_vector` (`martialarts.cpp:1495-1515`): each `limb_req` counts the user's parts of that kind whose hit points are above the part's health limit, and the aim is discarded
/// when the count falls short. The port reads a part's kind from its `primary_limb_type` and its health as **the hit points a wound has left**.
///
/// The limit is the part's own `health_limit` (`bodypart.h:272`), which loads as one when the data says nothing (`bodypart.cpp:414`) - **the dump states it**, so the port compares against the
/// reference's figure rather than a number it wrote down.
#[must_use]
pub fn aims_available(
    world: &bevy_ecs::prelude::World,
    striker: bevy_ecs::prelude::Entity,
    needs: &[VectorNeed],
) -> bool {
    let Some(body) = world.get::<crate::creature::Body>(striker) else {
        // A body the port has not built has no limbs to count, which is the honest answer: a creature with no anatomy cannot claim to have two arms.
        return needs.is_empty();
    };
    needs.iter().all(|need| {
        let standing = body
            .parts
            .iter()
            .filter(|part| part.primary_limb_type.as_deref() == Some(need.kind.as_str()))
            // **A limb counts while its hit points are above its own health limit** (`martialarts.cpp:1500-1510`), which the dump now states rather than the port assuming.
            .filter(|part| {
                part.base_hp - crate::creature::wound_on(world, striker, &part.name)
                    > part.health_limit
            })
            .count();
        i32::try_from(standing).unwrap_or(i32::MAX) >= need.count
    })
}

/// Whether a limb's share of its own hit points is above an aim's limit.
///
/// Reference: `choose_attack_vector` (`martialarts.cpp:1559-1560`): `( 100 * bp_hp_cur / bp_hp_max ) > vec->bp_hp_limit`. **The division is integer and truncating**, which is why a limb at six of
/// sixty is a percentage of ten and not ten-something, and a limit of ten refuses it. A limit of a hundred is satisfied only by a limb at full health.
#[must_use]
pub fn health_above_limit(current: i32, maximum: i32, limit: i32) -> bool {
    if maximum <= 0 {
        return false;
    }
    (100 * current / maximum) > limit
}

/// Which aim the striker takes for a move, given the move's aims and one draw.
///
/// Reference: `choose_attack_vector` (`martialarts.cpp:1475-1605`): each aim is checked against the striker's **limbs** - the count of each kind it needs (`:1495-1515`) and, for the limbs it may
/// strike, their share of their own health against the aim's limit (`:1559-1560`) - and what survives is drawn by weight.
///
/// **The weight is one for every aim here**, and that is the port's own limit rather than the reference's rule: an aim is weighted by the damage it would do, and the port does not model unarmed
/// limb damage yet, so the draw is the reference's own floor for an aim that would do none (`:1568-1571`). Named in `docs/parity.md`.
#[must_use]
pub fn aim_for(
    world: &bevy_ecs::prelude::World,
    striker: bevy_ecs::prelude::Entity,
    vectors: &[Vector],
    roll: f64,
) -> Option<String> {
    let eligible: Vec<WeightedAim> = vectors
        .iter()
        .filter(|vector| aims_available(world, striker, &vector.needs))
        .filter(|vector| limbs_healthy_enough(world, striker, vector))
        .map(|vector| WeightedAim {
            id: vector.id.clone(),
            weight: 1.0,
        })
        .collect();
    weighted_pick(&eligible, roll)
}

/// Whether the limbs a move may strike are each above the move's health limit.
///
/// Reference: `choose_attack_vector` (`martialarts.cpp:1553-1580`): a limb is only taken for an aim when its share of its own health is above the aim's `bp_hp_limit`. An aim that names no limb is
/// not filtered by this at all, which is what the reference's loop over `vec->limbs` means when the list is empty.
#[must_use]
fn limbs_healthy_enough(
    world: &bevy_ecs::prelude::World,
    striker: bevy_ecs::prelude::Entity,
    vector: &Vector,
) -> bool {
    if vector.limbs.is_empty() {
        return true;
    }
    let Some(body) = world.get::<crate::creature::Body>(striker) else {
        return false;
    };
    let mut any = false;
    for name in &vector.limbs {
        let Some(part) = body.parts.iter().find(|part| &part.name == name) else {
            continue;
        };
        let current = part.base_hp - crate::creature::wound_on(world, striker, &part.name);
        if health_above_limit(current, part.base_hp, vector.health_limit) {
            any = true;
        }
    }
    any
}

/// The aim a striker took on its last blow, or nothing when it took none.
///
/// Reference: the aim travels with the blow in the reference's `attack` struct (`melee.cpp:754-760`). The port leaves it on the striker for the same reason it leaves the move there: the blow is a
/// function, so the answer needs somewhere a turn, a test or a UI can read it.
#[derive(Debug, Clone, Default, PartialEq, Eq, bevy_ecs::component::Component)]
pub struct LastAim(pub Option<String>);

/// The aim this striker takes for these aims, drawing from the store's own dice **only when something qualified**.
///
/// Reference: `choose_attack_vector` (`martialarts.cpp:1600-1605`) calls `pick` only when the list is not empty, so a striker with nothing to aim at consumes no draw. The port keeps that: the
/// eligible aims are counted first and the dice are untouched when there are none - the same rule that keeps a technique-less swing from moving every later roll.
#[must_use]
pub fn aim_with_roll(
    world: &mut bevy_ecs::prelude::World,
    striker: bevy_ecs::prelude::Entity,
    vectors: &[Vector],
) -> Option<String> {
    let eligible: Vec<WeightedAim> = vectors
        .iter()
        .filter(|vector| aims_available(world, striker, &vector.needs))
        .filter(|vector| limbs_healthy_enough(world, striker, vector))
        .map(|vector| WeightedAim {
            id: vector.id.clone(),
            weight: 1.0,
        })
        .collect();
    if eligible.is_empty() {
        return None;
    }
    let roll = match crate::singletons::take_rolls(world) {
        Some(mut dice) => {
            let roll = dice.rng_float(0.0, 1.0);
            crate::singletons::put_rolls(world, dice);
            roll
        }
        None => 0.0,
    };
    weighted_pick(&eligible, roll)
}
