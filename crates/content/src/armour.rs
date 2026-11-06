//! What a worn thing protects, which is a material's resistance times how much of it is in the way.
//!
//! Reference: `item::_resist` (`src/item.cpp:9199-9231`) - for each material of the armour entry covering the part,
//! if the material's coverage beats the hit's roll, add `material.resist( type ) * thickness`; then the sum is
//! scaled by the item's own damage. The port keeps that sum and leaves the scaling for the day an item can be
//! damaged enough for it to matter.

use crate::materials::Material;

/// One material in the way of a blow, as the sum needs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layer {
    /// What the material resists per millimetre, for the damage type in question.
    pub resist: f64,
    /// How many millimetres of it are there.
    pub thickness: f32,
    /// The percentage of the part it covers.
    pub cover: i32,
}

/// What one layer takes off a blow, or nothing when the hit missed it.
///
/// Reference: `item::_resist` tests `internal_roll < m->cover` before counting a material - a garment covering
/// sixty per cent of a torso is not in the way of every blow that reaches the torso.
#[must_use]
pub fn layer_resist(layer: Layer, roll: i32) -> f64 {
    if roll < layer.cover {
        layer.resist * f64::from(layer.thickness)
    } else {
        0.0
    }
}

/// What a stack of layers takes off a blow.
#[must_use]
pub fn resist_of(layers: &[Layer], roll: i32) -> f64 {
    layers
        .iter()
        .map(|layer| layer_resist(*layer, roll))
        .sum()
}

/// What a garment still takes off a blow, once its own damage is accounted for.
///
/// Reference: `item::_resist` sums the materials and then returns `( resist + mod ) * damage_scale` (`item.cpp:9226-9234`), where the scale is the rule below. **The join lives here so that a
/// caller cannot sum a garment's materials and forget that the garment has been worn out.**
#[must_use]
pub fn resist_as_worn(layers: &[Layer], roll: i32, damage_level: i32) -> f64 {
    damage_adjusted_armor_resist(resist_of(layers, roll), damage_level)
}

/// What a garment's protection is worth once it has been damaged.
///
/// Reference: `item::damage_adjusted_armor_resist` (`item.cpp:944-962`) - a garment at damage level one is whole, and **each level past that takes an eighth**, with a floor of zero so a
/// thing past level nine still protects by nothing rather than by a negative figure. The port summed materials and left this out, which is invisible while every garment is whole.
#[must_use]
pub fn damage_adjusted_armor_resist(value: f64, damage_level: i32) -> f64 {
    (value * (1.0 - f64::from((damage_level - 1).max(0)) * 0.125)).max(0.0)
}

/// One step of damage a garment takes, and the point past which it is ruined.
///
/// Reference: `itype::damage_scale` and `itype::damage_max_` (`itype.h:1568-1588`). A garment damaged by a blow takes
/// one whole step, and is destroyed when its damage would pass the maximum rather than when it reaches it - so it
/// survives exactly `damage_max / damage_scale` steps, which is four, and breaks on the fifth.
pub const DAMAGE_SCALE: i32 = 1000;

/// The damage at which a garment is ruined.
pub const DAMAGE_MAX: i32 = 4000;

/// How damaged a garment is, on the reference's five-step scale.
///
/// Reference: `itype::damage_level` (`itype.cpp:117-126`) - zero for an untouched thing, then one plus four steps of
/// damage over the maximum, clamped. The reference answers **five** for a thing counted by charges, which cannot be
/// damaged at all; the port has no charged garments, and a caller with no maximum gets zero rather than a worry.
#[must_use]
pub fn damage_level(damage: i32, maximum: i32) -> i32 {
    if damage == 0 || maximum <= 0 {
        return 0;
    }
    (1 + 4 * damage / maximum).clamp(0, 5)
}

/// How likely a garment is to be damaged by a blow that reaches it.
///
/// Reference: `item::damage_armor_durability` (`item.cpp:9481-9537`). The chance is a logistic in the damage the hit
/// would have done against what the garment itself resists, with three corrections that each exist for a reason:
///
/// * **at most 80%.** No blow destroys a garment outright, however hard it lands.
/// * **non-fragile things count as at least 10 points of armour** (`max( 10.0, resist * 1.3333 )`), so ordinary
///   clothes do not explode into ribbons when somebody punches you.
/// * **fragile things count for less** (`resist * 0.6666`), which is the whole of what the flag means here.
///
/// The chance is then **divided by how many parts the garment covers**: a long coat takes more punishment than a
/// glove before it is ruined, because there is more of it to ruin.
///
/// `sturdy` is the reference's other gate: a sturdy thing takes no damage at all until the blow beats its threshold.
#[must_use]
pub fn damage_chance(
    own_resist: f64,
    premitigated: f64,
    covered_parts: usize,
    fragile: bool,
    sturdy: bool,
) -> f64 {
    if own_resist > 1000.0 {
        // "Some weird type that doesn't damage armors": the reference returns undamaged here.
        return 0.0;
    }
    let threshold = if fragile {
        own_resist * 0.6666
    } else {
        own_resist.mul_add(1.3333, 0.0).max(10.0)
    };
    if sturdy && premitigated < threshold {
        return 0.0;
    }
    let logistic = 0.8 / (1.0 + (-(premitigated - threshold)).exp());
    if covered_parts == 0 {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let parts = covered_parts as f64;
    logistic / parts
}

/// The layers a garment puts in the way of a blow to one part of a body.
///
/// The join between the item layer and the material layer, done in one place so that a garment's protection is
/// worked out the same way wherever it is asked for. A material the item names but the data does not describe
/// contributes nothing, which is what the reference does with a null material id.
#[must_use]
pub fn layers_on(
    item: &crate::items::ItemType,
    materials: &[Material],
    part: &str,
    cutting: bool,
) -> Vec<Layer> {
    item.armor_parts
        .iter()
        .filter(|entry| entry.covers.iter().any(|covered| covered == part))
        .filter_map(|entry| {
            let material = materials.iter().find(|one| one.id == entry.material)?;
            Some(Layer {
                resist: if cutting {
                    material.cut_resist
                } else {
                    material.bash_resist
                },
                thickness: entry.thickness,
                cover: entry.cover,
            })
        })
        .collect()
}
