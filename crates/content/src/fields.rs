//! Field prototypes: what each kind of field looks like at each intensity.
//!
//! Reference: `field_type` (`src/field_type.h`), reached through `--dump-fields`. A field's
//! number of levels is also its maximum intensity, which is what the runtime clamps to - so this
//! loader is what makes "intensity 3" mean something.

use cdda_json::{Object, Value};

/// One intensity of a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLevel {
    /// The displayed name.
    pub name: String,
    /// The ASCII glyph drawn for this intensity.
    pub sym: u32,
    /// The colour id drawn for this intensity.
    pub color: String,
    /// Whether standing in it is hazardous.
    pub dangerous: bool,
    /// Whether light passes through this intensity.
    pub transparent: bool,
    /// Extra movement cost to enter.
    pub move_cost: i32,
}

/// One kind of field, with its levels in order (index 0 is intensity 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldType {
    /// The field's id, e.g. `fd_smoke`.
    pub id: String,
    /// Which field draws when several share a tile; higher wins.
    pub priority: i32,
    /// The intensities, weakest first.
    pub levels: Vec<FieldLevel>,
}

impl FieldType {
    /// The highest intensity this field can reach.
    #[must_use]
    pub fn max_intensity(&self) -> u8 {
        u8::try_from(self.levels.len()).unwrap_or(u8::MAX)
    }
}

fn int_of(object: &Object, key: &str) -> Option<i64> {
    match object.get(key)? {
        Value::Int(value) => Some(*value),
        _ => None,
    }
}

fn bool_of(object: &Object, key: &str) -> Option<bool> {
    match object.get(key)? {
        Value::Bool(value) => Some(*value),
        _ => None,
    }
}

fn str_of(object: &Object, key: &str) -> Option<String> {
    match object.get(key)? {
        Value::Str(value) => Some(value.clone()),
        _ => None,
    }
}

/// Reads the field dump.
///
/// # Errors
///
/// Returns the reason the dump could not be read: a shape it does not understand, a field with no
/// levels, or a level missing a piece the runtime needs.
pub fn from_dump(value: &Value) -> Result<Vec<FieldType>, String> {
    let Value::Object(root) = value else {
        return Err("the dump's root is not an object".to_string());
    };
    let Some(Value::Array(entries)) = root.get("prototypes") else {
        return Err("the dump has no prototypes array".to_string());
    };
    let mut fields = Vec::with_capacity(entries.len());
    for entry in entries {
        let Value::Object(object) = entry else {
            return Err("a field is not an object".to_string());
        };
        let Some(id) = str_of(object, "id") else {
            return Err("a field has no id".to_string());
        };
        let priority = int_of(object, "priority").unwrap_or(0);
        let Some(Value::Array(levels)) = object.get("levels") else {
            return Err(format!("{id} has no levels"));
        };
        if levels.is_empty() {
            return Err(format!("{id} has no levels at all"));
        }
        let mut parsed = Vec::with_capacity(levels.len());
        for level in levels {
            let Value::Object(level) = level else {
                return Err(format!("a level of {id} is not an object"));
            };
            let sym = level.get("sym").and_then(|v| match v {
                Value::Int(code) => u32::try_from(*code).ok(),
                _ => None,
            });
            let Some(sym) = sym else {
                return Err(format!("a level of {id} has no glyph"));
            };
            parsed.push(FieldLevel {
                name: str_of(level, "name").unwrap_or_default(),
                sym,
                color: str_of(level, "color").unwrap_or_default(),
                dangerous: bool_of(level, "dangerous").unwrap_or(false),
                transparent: bool_of(level, "transparent").unwrap_or(true),
                move_cost: i32::try_from(int_of(level, "move_cost").unwrap_or(0)).unwrap_or(0),
            });
        }
        fields.push(FieldType {
            id,
            priority: i32::try_from(priority).unwrap_or(0),
            levels: parsed,
        });
    }
    Ok(fields)
}

/// Finds a field by id.
#[must_use]
pub fn find<'a>(fields: &'a [FieldType], id: &str) -> Option<&'a FieldType> {
    fields.iter().find(|field| field.id == id)
}

/// The field ids the runtime uses, built from the dump.
///
/// An id is the field's position in the dump plus one, so zero stays free for the null field that
/// `add_field` refuses. The numbering is the port's own: the reference keys fields by string, and
/// a number is what lets a tile store one in a byte.
#[derive(Debug, Clone, Default)]
pub struct FieldRegistry {
    ids: std::collections::HashMap<String, u32>,
}

impl FieldRegistry {
    /// Builds the registry from a loaded field list.
    #[must_use]
    pub fn new(fields: &[FieldType]) -> Self {
        let mut ids = std::collections::HashMap::with_capacity(fields.len());
        for (index, field) in fields.iter().enumerate() {
            let Ok(id) = u32::try_from(index + 1) else {
                continue;
            };
            ids.insert(field.id.clone(), id);
        }
        Self { ids }
    }

    /// The id for a field name, if the reference defines it.
    #[must_use]
    pub fn id_of(&self, name: &str) -> Option<cdda_core_types::field::FieldId> {
        self.ids
            .get(name)
            .copied()
            .map(cdda_core_types::field::FieldId)
    }
}
