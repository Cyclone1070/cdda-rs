//! What each field type does to a ray of light, read from the loaded prototypes.
//!
//! Reference: `field_type`'s intensity levels, whose `translucency` `calc_transp` multiplies into a square's value (`src/lightmap.cpp:158-165`). The number is an
//! **absorption**: a larger one dims more, and zero makes the square solid. The type's `transparent` flag is a separate thing and is not enough on its own - smoke at
//! intensity two is marked opaque yet has a translucency of ten, so it dims a ray rather than stopping it.

use std::collections::BTreeMap;

/// The translucency of each field type at each of its intensities.
#[derive(Debug, Clone, Default, bevy_ecs::prelude::Resource)]
pub struct FieldTable {
    by_type: BTreeMap<u32, Vec<f32>>,
}

impl FieldTable {
    /// An empty table: every field is treated as solid, which is the safe direction for a world that has loaded nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one type's translucency, one number per intensity, lowest first.
    pub fn insert(&mut self, id: u32, translucency: &[f32]) {
        self.by_type.insert(id, translucency.to_vec());
    }

    /// The translucency of a field at an intensity, clamped to the range the data gives.
    ///
    /// An intensity the data does not cover answers zero: a field the world has no numbers for hides what is behind it rather than revealing it.
    #[must_use]
    pub fn translucency(&self, id: u32, intensity: i32) -> f32 {
        let Some(levels) = self.by_type.get(&id) else {
            return 0.0;
        };
        let Ok(index) = usize::try_from(intensity.saturating_sub(1)) else {
            return 0.0;
        };
        levels.get(index).copied().unwrap_or(0.0)
    }

    /// Whether the type is known at all.
    #[must_use]
    pub fn knows(&self, id: u32) -> bool {
        self.by_type.contains_key(&id)
    }
}
