//! Reading the reference's recordings and replaying them in the port.
//!
//! The corpus is the specification: a fixture is a dump the instrumented reference wrote while it ran
//! a scene. This crate turns one into a world the simulation can be run against, so a gate only has to
//! say what to compare.
//!
//! It is a library rather than a test suite helper because two things need it: the tests, and the QA
//! table (`cdda-cli qa`), which reports on the same replays. A comparison only one of them can run is
//! a comparison the other will eventually disagree with.

pub mod session;

pub use session::Session;
