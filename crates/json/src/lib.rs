//! JSON reader/writer for the port.
//!
//! The reference is Cataclysm-DDA at tag `0.I-1` (commit `7b2efa5`), cited by
//! file and line wherever a rule is encoded. Game data is not parsed by the
//! reference's hand-written reader: every file goes through the vendored
//! flatbuffers strict-JSON parser via
//! `src/flexbuffer_cache.cpp:59-129`, so that parser's lexer, value parser and
//! flexbuffers storage rules are what this crate reproduces — acceptance,
//! rejection, key order, and integer-versus-float typing alike.

#![forbid(unsafe_code)]

pub mod cache;
mod error;
mod parser;
mod value;
mod writer;

pub use error::ParseError;
pub use parser::parse;
pub use value::{Object, Value};
pub use writer::Writer;
