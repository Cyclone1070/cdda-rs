//! Field types: the one loader whose inheritance runs *inside* a list.
//!
//! A field type is a list of intensity levels, and each level inherits from the
//! level before it rather than from another prototype (`field_type.cpp:37-90`):
//! `intensity_level` starts as a copy of the previous level, then the new one's
//! members are applied over it. The first level inherits from the C++ defaults
//! instead. Effects, and everything else, follow the same pattern - so a fire
//! field's third level is its second level plus whatever it says.

use cdda_core_types::colors::{NcColor, color_from_string, string_from_color};
use cdda_json::{Object, Value};
use std::collections::BTreeMap;

/// One intensity level of a field.
#[derive(Debug, Clone, PartialEq)]
pub struct IntensityLevel {
    /// The name a player sees, untranslated.
    pub name: String,
    /// The glyph, as a Unicode code point.
    pub symbol: u32,
    /// The colour.
    pub color: NcColor,
    /// Whether standing in it hurts.
    pub dangerous: bool,
    /// Whether it can be seen through.
    pub transparent: bool,
    /// What it costs to walk through.
    pub move_cost: i32,
    /// How much of the field fits in one tile.
    pub concentration: i32,
    /// Light it emits.
    pub light: f64,
}

impl IntensityLevel {
    /// The name the reference prints for this level's colour.
    #[must_use]
    pub fn color_name(&self) -> &'static str {
        string_from_color(self.color)
    }
}

/// One resolved field type.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldType {
    /// Its id.
    pub id: String,
    /// Draw order; higher draws later.
    pub priority: i32,
    /// Its levels, weakest first. Always at least one.
    pub levels: Vec<IntensityLevel>,
}

/// The level a field starts from before any file is read.
///
/// `field_intensity_level`'s member initialisers (`field_type.h:99-123`), which
/// *do* survive here: the level loader passes the previous level explicitly as
/// the fallback for every field, unlike the material loader.
fn default_level() -> IntensityLevel {
    IntensityLevel {
        name: String::new(),
        symbol: u32::from(b'%'),
        color: color_from_string("white").unwrap_or_default(),
        dangerous: false,
        transparent: true,
        move_cost: 0,
        concentration: 0,
        light: 0.0,
    }
}

/// Loads every field type in the given directories, in the reference's order.
///
/// # Errors
///
/// Fails when a file cannot be read, a field type has no levels, or a level
/// names a colour the table does not know.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<Vec<FieldType>, LoadError> {
    let mut objects: BTreeMap<String, Object> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let bytes = std::fs::read(&path).map_err(|error| LoadError {
                message: format!("{}: {error}", path.display()),
            })?;
            let Ok(value) = cdda_json::parse(&bytes) else {
                continue;
            };
            collect(&value, &mut objects, &mut order);
        }
    }
    let mut out = Vec::new();
    for id in &order {
        let mut seen = Vec::new();
        out.push(resolve(id, &objects, &mut seen)?);
    }
    Ok(out)
}

/// Why a field type could not be loaded.
#[derive(Debug)]
pub struct LoadError {
    /// The problem.
    pub message: String,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LoadError {}

fn collect(value: &Value, out: &mut BTreeMap<String, Object>, order: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, out, order);
            }
        }
        Value::Object(object) => {
            if !matches!(object.get("type"), Some(Value::Str(text)) if text == "field_type") {
                return;
            }
            let concrete = string(object, "id");
            let template = concrete.is_none() && object.get("abstract").is_some();
            let id = concrete.or_else(|| string(object, "abstract"));
            if let Some(id) = id {
                if !template && !out.contains_key(&id) {
                    order.push(id.clone());
                }
                out.insert(id, object.clone());
            }
        }
        _ => {}
    }
}

fn resolve(
    id: &str,
    objects: &BTreeMap<String, Object>,
    seen: &mut Vec<String>,
) -> Result<FieldType, LoadError> {
    if seen.iter().any(|entry| entry == id) {
        return Err(LoadError {
            message: format!("cyclic copy-from through {id}: {seen:?}"),
        });
    }
    let Some(object) = objects.get(id) else {
        return Err(LoadError {
            message: format!("field type {id} is not defined"),
        });
    };
    seen.push(id.to_owned());
    let inherited = object.get("copy-from").is_some();
    let mut field = match object.get("copy-from") {
        Some(Value::Str(base)) => {
            let mut base_field = resolve(base, objects, seen)?;
            id.clone_into(&mut base_field.id);
            base_field
        }
        _ => FieldType {
            id: id.to_owned(),
            priority: 0,
            levels: Vec::new(),
        },
    };
    seen.pop();
    id.clone_into(&mut field.id);
    apply(&mut field, object, inherited)?;
    if field.levels.is_empty() {
        return Err(LoadError {
            message: format!("field type {id} has no intensity levels"),
        });
    }
    Ok(field)
}

fn apply(field: &mut FieldType, object: &Object, inherited: bool) -> Result<(), LoadError> {
    if let Some(priority) = integer(object, "priority") {
        field.priority = priority;
    }
    let Some(Value::Array(entries)) = object.get("intensity_levels") else {
        if inherited {
            return Ok(());
        }
        return Err(LoadError {
            message: format!("field type {}: missing intensity_levels", field.id),
        });
    };
    // A loaded object's levels replace the inherited ones; the fallback chain
    // starts again from the first new level (`field_type.cpp:37-40`).
    let mut levels: Vec<IntensityLevel> = Vec::new();
    for entry in entries {
        let Value::Object(level) = entry else {
            continue;
        };
        let mut next = levels.last().cloned().unwrap_or_else(default_level);
        if let Some(name) = name_of(level) {
            next.name = name;
        }
        if let Some(symbol) = string(level, "sym") {
            next.symbol = one_codepoint(&symbol).ok_or_else(|| LoadError {
                message: format!(
                    "field type {}: sym {symbol:?} is not one character",
                    field.id
                ),
            })?;
        }
        if let Some(text) = string(level, "color") {
            next.color = color_from_string(&text).map_err(|error| LoadError {
                message: format!("field type {}: color {text:?}: {error:?}", field.id),
            })?;
        }
        if let Some(dangerous) = boolean(level, "dangerous") {
            next.dangerous = dangerous;
        }
        if let Some(transparent) = boolean(level, "transparent") {
            next.transparent = transparent;
        }
        if let Some(cost) = integer(level, "move_cost") {
            next.move_cost = cost;
        }
        // The one field with an explicit default of its own: a level that says
        // nothing is at concentration 1, not at the previous level's count.
        // Fog writes 0.5, and the reference's integer reader truncates it.
        next.concentration = truncated_integer(level, "concentration").unwrap_or(1);
        if let Some(light) = number(level, "light_emitted") {
            next.light = light;
        }
        levels.push(next);
    }
    field.levels = levels;
    Ok(())
}

/// One code point, or nothing if the string is not exactly one character.
fn one_codepoint(text: &str) -> Option<u32> {
    let mut characters = text.chars();
    match (characters.next(), characters.next()) {
        (Some(character), None) => Some(character as u32),
        _ => None,
    }
}

fn name_of(object: &Object) -> Option<String> {
    match object.get("name") {
        Some(Value::Str(text)) => Some(text.clone()),
        Some(Value::Object(inner)) => string(inner, "str"),
        _ => None,
    }
}

fn string(object: &Object, member: &str) -> Option<String> {
    match object.get(member) {
        Some(Value::Str(text)) => Some(text.clone()),
        _ => None,
    }
}

fn integer(object: &Object, member: &str) -> Option<i32> {
    match object.get(member) {
        Some(Value::Int(value)) => i32::try_from(*value).ok(),
        _ => None,
    }
}

/// An integer member, truncating a float toward zero as the reference's reader
/// does - fog's first level states a concentration of 0.5 and loads as 0.
fn truncated_integer(object: &Object, member: &str) -> Option<i32> {
    match object.get(member) {
        Some(Value::Int(value)) => i32::try_from(*value).ok(),
        #[allow(clippy::cast_possible_truncation)]
        Some(Value::Float(value)) => Some(*value as i32),
        _ => None,
    }
}

fn boolean(object: &Object, member: &str) -> Option<bool> {
    match object.get(member) {
        Some(Value::Bool(value)) => Some(*value),
        _ => None,
    }
}

fn number(object: &Object, member: &str) -> Option<f64> {
    match object.get(member) {
        Some(Value::Int(value)) =>
        {
            #[allow(clippy::cast_precision_loss)]
            Some(*value as f64)
        }
        Some(Value::Float(value)) => Some(*value),
        _ => None,
    }
}
