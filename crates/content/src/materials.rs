//! Materials: the numeric definitions items and terrain are made of.
//!
//! `material_type` has no symbol or colour, which makes it the cleanest place
//! to pin the pure-number half of loading: inheritance of plain values, the
//! breathability rating table (`material.cpp:299-319`), and the four heat
//! constants that items later inherit through their materials.

use cdda_json::{Object, Value};
use std::collections::BTreeMap;

/// One resolved material.
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    /// Its id.
    pub id: String,
    /// The name a player sees, untranslated.
    pub name: String,
    /// Density relative to powder, which is 1.
    pub density: f64,
    /// Resistance of the item itself to physical damage.
    pub chip_resist: i32,
    /// How breathable the material is, as the reference reports it.
    pub breathability: i32,
    /// Specific heat as a liquid, in the reference's units.
    pub specific_heat_liquid: f64,
    /// Specific heat as a solid.
    pub specific_heat_solid: f64,
    /// Latent heat of fusion or vaporisation.
    pub latent_heat: f64,
}

/// What a material starts as.
///
/// Not the member initialisers in `material.h:84-92`: every one of these fields
/// is read through `optional` without an explicit default argument, and that
/// overload assigns the *template* default — zero — when a fresh object omits
/// the member (`material.cpp:103-105`). Bronze and pseudo fuel therefore have a
/// specific heat of zero in the reference, not water's 4.186.
fn defaults(id: &str) -> Material {
    Material {
        id: id.to_owned(),
        name: String::new(),
        density: 1.0,
        chip_resist: 0,
        breathability: 0,
        specific_heat_liquid: 0.0,
        specific_heat_solid: 0.0,
        latent_heat: 0.0,
    }
}

/// Loads every material in the given directories, in the reference's file
/// order, resolving `copy-from` across files.
///
/// # Errors
///
/// Fails when a file cannot be read, or when an object names a base that is not
/// defined.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<Vec<Material>, LoadError> {
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
        let material = resolve(id, &objects, &mut seen)?;
        out.push(material);
    }
    Ok(out)
}

/// Why a material could not be loaded.
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
            if !matches!(object.get("type"), Some(Value::Str(text)) if text == "material") {
                return;
            }
            // An `abstract` is a base other materials copy from, not a
            // prototype of its own, so it is indexed but never listed.
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
) -> Result<Material, LoadError> {
    if seen.iter().any(|entry| entry == id) {
        return Err(LoadError {
            message: format!("cyclic copy-from through {id}: {seen:?}"),
        });
    }
    let Some(object) = objects.get(id) else {
        return Err(LoadError {
            message: format!("material {id} is not defined"),
        });
    };
    seen.push(id.to_owned());
    let mut material = match object.get("copy-from") {
        Some(Value::Str(base)) => resolve(base, objects, seen)?,
        _ => defaults(id),
    };
    seen.pop();
    id.clone_into(&mut material.id);
    apply(&mut material, object)?;
    Ok(material)
}

fn apply(material: &mut Material, object: &Object) -> Result<(), LoadError> {
    if let Some(name) = name_of(object) {
        material.name = name;
    }
    if let Some(density) = number(object, "density") {
        material.density = density;
    }
    if let Some(chip) = integer(object, "chip_resist") {
        material.chip_resist = chip;
    }
    if let Some(breathability) = string(object, "breathability") {
        material.breathability = breathability_rating(&breathability).ok_or_else(|| LoadError {
            message: format!(
                "material {}: unknown breathability {breathability:?}",
                material.id
            ),
        })?;
    }
    if let Some(heat) = number(object, "specific_heat_liquid") {
        material.specific_heat_liquid = heat;
    }
    if let Some(heat) = number(object, "specific_heat_solid") {
        material.specific_heat_solid = heat;
    }
    if let Some(heat) = number(object, "latent_heat") {
        material.latent_heat = heat;
    }
    Ok(())
}

/// The rating the reference reports for a breathability name
/// (`material.cpp:299-319`).
///
/// The content writes the enum's own spelling, in capitals
/// (`"SECOND_SKIN"`), and the reference's enum reader accepts it as written.
#[must_use]
pub fn breathability_rating(name: &str) -> Option<i32> {
    match name.to_ascii_uppercase().as_str() {
        "IMPERMEABLE" => Some(0),
        "POOR" => Some(30),
        "AVERAGE" => Some(50),
        "GOOD" => Some(80),
        "MOISTURE_WICKING" => Some(110),
        "SECOND_SKIN" => Some(140),
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

fn number(object: &Object, member: &str) -> Option<f64> {
    match object.get(member) {
        // Content integers are small counts and percentages; a material with a
        // density beyond 2^53 is not a case worth carrying.
        #[allow(clippy::cast_precision_loss)]
        Some(Value::Int(value)) => Some(*value as f64),
        Some(Value::Float(value)) => Some(*value),
        _ => None,
    }
}
