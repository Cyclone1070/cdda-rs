//! What the character remembers, as the world holds it.
//!
//! The store itself is [`cdda_core_types::memory::MapMemory`], which knows nothing about the ECS. This wrapper is the world-facing handle, so that a frame can fill it by
//! looking and read it back when it cannot: the draw loop memorizes what it prints (`drawsqu_params().memorize( true )`, `src/map.cpp:7216`) and, for a square it cannot
//! see, prints the memorized symbol in place of the terrain (`draw_background`, `src/map.cpp:6906-6917`).

/// The character's map memory, as a resource.
#[derive(Debug, Clone, Default, PartialEq, Eq, bevy_ecs::prelude::Resource)]
pub struct Memory(pub cdda_core_types::memory::MapMemory);

impl Memory {
    /// An empty memory.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
