//! What an NPC thinks of someone: the four numbers a conversation asks about.
//!
//! Reference: `npc::op_of_u` (`npc.h`), the `npc_opinion` struct of `value`, `anger`, `trust` and `fear`, read by
//! `npc_value_eval`/`npc_anger_eval`/`npc_trust_eval`/`npc_fear_eval` (`math_parser_diag.cpp:1408-1440`) and written by the matching `_ass` functions. Those
//! are the numbers the dialogue data checks most often of anything: 166 of the reference's refused expressions ask for one.
//!
//! The port keeps them as a component on the creature that holds them, so "how does this NPC feel" is a query rather than a parallel table.

use bevy_ecs::prelude::*;

/// One creature's opinion of another, as the four numbers the data names.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Opinion {
    /// How much the NPC values the other: the reference's `value`, which is positive for a friend.
    pub value: i32,
    /// How angry it is with them.
    pub anger: i32,
    /// How far it trusts them.
    pub trust: i32,
    /// How afraid of them it is.
    pub fear: i32,
}

/// Which of the four numbers a rule asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feeling {
    /// `npc_value`.
    Value,
    /// `npc_anger`.
    Anger,
    /// `npc_trust`.
    Trust,
    /// `npc_fear`.
    Fear,
}

/// Sets one creature's opinion of another.
pub fn set_opinion(world: &mut World, npc: Entity, opinion: Opinion) {
    world.entity_mut(npc).insert(opinion);
}

/// One of the four numbers, answering **zero** when the creature has no opinion.
///
/// Reference: `npc_value_eval` (`math_parser_diag.cpp:1430-1440`) answers `0` when there is no such actor rather than failing, which is what lets a program ask
/// an opinion of a listener that does not exist.
#[must_use]
pub fn feeling_of(world: &World, npc: Entity, feeling: Feeling) -> i32 {
    world.get::<Opinion>(npc).copied().map_or(0, |opinion| match feeling {
        Feeling::Value => opinion.value,
        Feeling::Anger => opinion.anger,
        Feeling::Trust => opinion.trust,
        Feeling::Fear => opinion.fear,
    })
}
