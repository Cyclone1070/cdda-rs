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
