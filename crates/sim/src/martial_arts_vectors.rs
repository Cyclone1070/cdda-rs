//! What a move aims at.

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
            .filter(|part| part.base_hp - crate::creature::wound_on(world, striker, &part.name) > part.health_limit)
            .count();
        i32::try_from(standing).unwrap_or(i32::MAX) >= need.count
    })
}
