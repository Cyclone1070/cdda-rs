//! Foundation types for the port: coordinates, units, time, identifiers and
//! randomness.
//!
//! Everything in this crate is a value type with no game state and no I/O, which
//! is why it is ported first: every later system inherits its mistakes.
//!
//! The reference implementation for every rule here is Cataclysm-DDA at tag
//! `0.I-1` (commit `7b2efa5`), cited by file and line in the module where the
//! rule comes from.

#![forbid(unsafe_code)]

pub mod astronomy;
pub mod calendar;
pub mod colors;
pub mod containers;
pub mod coords;
pub mod diagnostics;
pub mod field;
pub mod fire;
pub mod grid;
pub mod ids;
pub mod level;
pub mod light;
pub mod line;
pub mod map_cache;
pub mod memory;
pub mod noise;
pub mod options;
pub mod outside_cache;
pub mod overmap;
pub mod paths;
pub mod rng;
pub mod scent;
pub mod shadowcasting;
pub mod sound;
pub mod sounds;
pub mod strings;
pub mod submap;
pub mod sunlight_cache;
pub mod text;
pub mod tiles;
pub mod units;
pub mod vision;
pub mod weather;
pub mod weighted;

// Modules are added one at a time, test first. See docs/10-task-breakdown.md
// (P0-T4 onward) for the order.
pub mod transparency;
