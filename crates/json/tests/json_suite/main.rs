//! Every json test, in one binary.
//!
//! Cargo gives each file in `tests/` its own binary, and every binary pays the same process startup
//! before a single test runs - measured at about 1.7 seconds each on this machine, which is the
//! dominant cost of the whole suite once there are a hundred and fifty of them. The files are
//! unchanged; this one lists them so the suite runs as a handful of binaries instead of one per file.
//!
//! A case is still named after its file: `frame_diff::the_same_frame_diffs_to_nothing`.

mod bad_strings;
mod bytes;
mod cache;
mod cache_corpus;
mod comments;
mod containers;
mod data_files;
mod delimiters;
mod depth;
mod escapes;
mod numbers;
mod objects;
mod oddities;
mod parsed;
mod reference_parity;
mod scalars;
mod toplevel;
mod utf8;
mod writer;
mod writer_parity;
