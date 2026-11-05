//! What standing in a fire does to the creature standing there.
//!
//! Reference: `map_field.cpp:1626-1702`, the avatar's half of the fire field's per-turn work. A fire does not damage a creature once - it **burns a set of body parts, each for its own
//! draw**, and the set grows with the fire's intensity: feet and legs at intensity one, the torso at two, hands and arms at three. A creature **lying down** burns everywhere at once,
//! which is why falling over in a fire is worse than standing in it.
//!
//! The damage draws are `rng( burn_min, burn_max )` per part, with `burn_min` the intensity and `burn_max` **three times the intensity plus three** - so the span widens faster than the
//! floor rises, and the difference matters at intensity three where one part can take twelve points.

/// The damage one burning body part takes, as the ends of the range to draw between.
///
/// Reference: `const int burn_min = adjusted_intensity; const int burn_max = 3 * adjusted_intensity + 3;` (`map_field.cpp:1663-1664`). **The three-times-and-three is the rule**: a fire of
/// intensity one burns for one to six, and one of intensity three for three to twelve.
#[must_use]
pub fn burn_span(intensity: i32) -> (i32, i32) {
    (intensity, 3 * intensity + 3)
}

/// Which body parts a fire burns, in the reference's own order.
///
/// Reference: the `switch( adjusted_intensity )` with its fallthroughs (`map_field.cpp:1668-1691`), which builds the list **from the feet up** and stops at the intensity - and which burns
/// **every part at once** for a creature that is on the ground. The names are the reference's own body-part ids.
#[must_use]
pub fn parts_burned(intensity: i32, on_the_ground: bool, all_parts: &[&'static str]) -> Vec<&'static str> {
    if on_the_ground {
        return all_parts.to_vec();
    }
    let mut parts = Vec::new();
    if intensity >= 3 {
        parts.extend(["hand_l", "hand_r", "arm_l", "arm_r"]);
    }
    if intensity >= 2 {
        parts.push("torso");
    }
    if intensity >= 1 {
        parts.extend(["foot_l", "foot_r", "leg_l", "leg_r"]);
    }
    parts
}

/// What one turn in a fire takes from a creature: a draw per burning body part.
///
/// Reference: the loop over `parts_burned` at `map_field.cpp:1696-1701`, each `deal_damage( part, damage_instance( damage_heat, rng( burn_min, burn_max ) ) )`. **One draw per part, in the
/// reference's order** - so the answer is a list rather than a total, because the parts are what the reference damages and a port that summed them would lose which limb caught it.
#[must_use]
pub fn fire_burn(
    intensity: i32,
    on_the_ground: bool,
    all_parts: &[&'static str],
    dice: &mut cdda_core_types::rng::Rolls,
) -> Vec<(&'static str, i32)> {
    if intensity < 1 {
        return Vec::new();
    }
    let (min, max) = burn_span(intensity);
    parts_burned(intensity, on_the_ground, all_parts)
        .into_iter()
        .map(|part| (part, dice.rng(min, max)))
        .collect()
}
