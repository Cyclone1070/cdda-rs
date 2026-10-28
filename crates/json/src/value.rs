//! The value model.
//!
//! What this preserves, and why (the reference is Cataclysm-DDA `0.I-1`,
//! `7b2efa5`; every claim below is cited to a line of that checkout):
//!
//! * **Integers stay integers, floats stay floats.** The reference parses
//!   `kTokenIntegerConstant` with `builder->Int( ... )` and
//!   `kTokenFloatConstant` with `builder->Double( ... )`
//!   (`src/third-party/flatbuffers/idl_parser.cpp:3050-3060`), so `1` and `1.0`
//!   reach game code as different types.
//! * **String escapes are decoded while parsing** and the decoded bytes are
//!   stored (`idl_parser.cpp:369-479`), so a round trip through the writer is
//!   allowed to re-escape them differently.
//! * **Object member order is the reference's order, which is not document
//!   order**: flexbuffers sorts each map's keys with `strcmp` when the map is
//!   closed (`src/third-party/flatbuffers/flexbuffers.h:1127-1151`). Sorting is
//!   therefore part of parsing here too, not a writer-only concern.
//! * **Duplicate keys are a hard error** (`flexbuffers.h:1143-1149` records the
//!   collision, `idl_parser.cpp:3033-3034` turns it into
//!   `"FlexBuffers map has duplicate keys"`).

use std::fmt;

/// A JSON value as the reference loader materialises it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// `null`.
    Null,
    /// `true` / `false`.
    Bool(bool),
    /// An integer literal, e.g. `1` or `-2`.
    Int(i64),
    /// A number that carried `.` or an exponent, e.g. `1.0` or `1e3`.
    Float(f64),
    /// A string with its escapes already decoded.
    Str(String),
    /// An array, in document order.
    Array(Vec<Value>),
    /// An object; see [`Object`] for the member order guarantee.
    Object(Object),
}

/// An object's members, kept in the reference's key order.
///
/// The order is `strcmp` order over the keys, exactly as flexbuffers stores
/// them (`flexbuffers.h:1127-1151`), and no two members can share a key: a
/// duplicate is rejected by the parser before an `Object` is built. Because
/// flexbuffers compares keys as NUL-terminated C strings, a key is ordered and
/// compared up to its first NUL byte; keys that agree up to that point count as
/// duplicates (verified against the reference: `{"a\u0000b":1,"a\u0000c":2}` is
/// rejected).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Object {
    members: Vec<(String, Value)>,
}

impl Object {
    /// An object with no members.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            members: Vec::new(),
        }
    }

    /// The members, in the reference's key order.
    #[must_use]
    pub fn members(&self) -> &[(String, Value)] {
        &self.members
    }

    /// Looks a member up by key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.members
            .binary_search_by(|(k, _)| compare_keys(k, key))
            .ok()
            .and_then(|i| self.members.get(i))
            .map(|(_, v)| v)
    }

    /// The number of members.
    #[must_use]
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Whether the object has no members.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Adds a member in the reference's key order. Returns `false`, and keeps
    /// the member already stored, when the key is present.
    ///
    /// The comparison is the one flexbuffers performs while sorting a finished
    /// map, so keys that share a prefix up to a NUL byte collide here too.
    pub fn insert(&mut self, key: String, value: Value) -> bool {
        match self
            .members
            .binary_search_by(|(k, _)| compare_keys(k, &key))
        {
            Ok(_) => false,
            Err(at) => {
                self.members.insert(at, (key, value));
                true
            }
        }
    }
}

/// Compares two keys the way the reference does: as NUL-terminated C strings.
pub(crate) fn compare_keys(a: &str, b: &str) -> std::cmp::Ordering {
    c_str_bytes(a).cmp(c_str_bytes(b))
}

/// The bytes `strcmp` would see, i.e. everything before the first NUL.
fn c_str_bytes(key: &str) -> &[u8] {
    let bytes = key.as_bytes();
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    bytes.get(..end).unwrap_or(bytes)
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => f.write_str("null"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(i) => write!(f, "{i}"),
            Self::Float(x) => write!(f, "{x}"),
            Self::Str(s) => write!(f, "{s:?}"),
            Self::Array(a) => {
                f.write_str("[")?;
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{v}")?;
                }
                f.write_str("]")
            }
            Self::Object(o) => {
                f.write_str("{")?;
                for (i, (k, v)) in o.members().iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{k:?}:{v}")?;
                }
                f.write_str("}")
            }
        }
    }
}
