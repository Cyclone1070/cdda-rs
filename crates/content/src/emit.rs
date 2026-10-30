//! Emit definitions: what a named emission puts on the ground.
//!
//! Reference: `emit.cpp` / `emit.h` - an `emit` entry names a **field**, an intensity and a chance, and `map::emit_field`
//! then asks the dice once per square. A spell's `effect_str` on an `emit` effect is one of these ids (`emit_toxic_blast`),
//! not a field id, which is why the port needs the table before it can run a single one of them.

use cdda_json::Value;

/// One thing an emission puts down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Emission {
    /// The field type's id, e.g. `fd_toxic_gas`.
    pub field: String,
    /// How thick it is placed.
    pub intensity: i32,
    /// The chance in a hundred that it is placed at all.
    pub chance: i32,
}

/// Reads one `emit` entry, or nothing when the port cannot use it.
///
/// Reference: `emit::load` (`emit.cpp`), which requires `field` and takes `intensity` and `chance` with defaults of one and
/// a hundred. An entry with a `qty` is an emission of things rather than of a field; the port refuses those rather than
/// dropping the quantity and placing one.
#[must_use]
pub fn read_emit(value: &Value) -> Option<Emission> {
    let Value::Object(object) = value else {
        return None;
    };
    let Some(Value::Str(field)) = object.get("field") else {
        return None;
    };
    if object.get("qty").is_some() {
        return None;
    }
    Some(Emission {
        field: field.clone(),
        intensity: read_whole(object.get("intensity")).unwrap_or(1),
        chance: read_whole(object.get("chance")).unwrap_or(100),
    })
}

/// A whole number the data states.
fn read_whole(value: Option<&Value>) -> Option<i32> {
    match value? {
        Value::Int(number) => i32::try_from(*number).ok(),
        _ => None,
    }
}
