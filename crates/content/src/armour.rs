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
