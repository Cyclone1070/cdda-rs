//! Putting one of the data's emissions down on a square.
//!
//! Reference: `map::emit_field( pos, emit_id, chance_mult )` (`map_field.cpp:2212-2224`), which asks every emission's own chance and then either pours the field out with
//! `propagate_field` or thickens the square. Two callers reach it - a spell's `emit` effect (`magic_spell_effect.cpp:1723-1729`) and the dialogue `u_emit` effect
//! (`npctalk.cpp:7730-7750`) - so the rule lives here once rather than twice.

/// Puts an emission down at a square, scaled by a chance multiplier. Returns whether the emission was defined at all.
///
/// A multiplier of zero places nothing, which is the reference's answer for `chance_mult: 0` rather than a special case.
pub fn emit_field(
    world: &mut bevy_ecs::prelude::World,
    square: (i32, i32, i32),
    emit: &str,
    chance_mult: f64,
) -> Option<()> {
    let emissions = crate::emit_registry::emissions_of(world, emit)?;
    let mut rolls = crate::singletons::take_rolls(world)?;
    for emission in &emissions {
        emit_one(world, square, emission, chance_mult, &mut rolls);
    }
    crate::singletons::put_rolls(world, rolls);
    Some(())
}

/// Puts **one** emission down at a square, asking its chance once. Returns whether anything was placed.
///
/// The split exists because a spell asks each square of its area in turn and counts how many squares it covered, while an effect asks one square - and both must ask the
/// emission's chance the same way (`rng( 1, 100 ) <= chance` after `chance_mult`).
pub fn emit_one(
    world: &mut bevy_ecs::prelude::World,
    square: (i32, i32, i32),
    emission: &cdda_content::emit::Emission,
    chance_mult: f64,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> bool {
    #[allow(clippy::cast_possible_truncation)]
    let chance = (f64::from(emission.chance) * chance_mult).round() as i32;
    if chance <= 0 || rolls.rng(1, 100) > chance {
        return false;
    }
    let Some(field) = crate::field_registry::id_of(world, &emission.field) else {
        return false;
    };
    // Reference: `propagate_field` for an emission that states a quantity, and a plain thickening when it does not (`map_field.cpp:2216-2224`).
    if emission.qty > 0 {
        let blocked = crate::field_passage::blocked_within(world, square, emission.qty);
        let poured = crate::field_spread::propagate(
            world,
            square,
            field,
            emission.qty,
            emission.intensity,
            rolls,
            |x, y, z| !blocked.contains(&(x, y, z)),
        );
        if poured > 0 {
            return true;
        }
        crate::add_field_strength(world, square, field, emission.intensity);
        return true;
    }
    crate::add_field_strength(world, square, field, emission.intensity);
    true
}
