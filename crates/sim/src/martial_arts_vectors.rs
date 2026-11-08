//! What a move aims at.

use bevy_ecs::prelude::*;

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
