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
    /// What this material resists, by damage type, exactly as the data states it (`material_type::_resistances`).
    ///
    /// A type the data does not name is **absent**, and the reference answers zero for it (`type_resist`, `damage.cpp:719-723`) rather than borrowing another type's figure.
    pub resists: BTreeMap<String, f64>,
    /// Resistance of the item itself to physical damage.
    pub chip_resist: i32,
    /// Its resistance to bash, per millimetre of thickness (`material::resist` on bash).
    pub bash_resist: f64,
    /// Its resistance to cut, per millimetre of thickness.
    pub cut_resist: f64,
    /// How breathable the material is, as the reference reports it.
    pub breathability: i32,
    /// Specific heat as a liquid, in the reference's units.
    pub specific_heat_liquid: f64,
    /// Specific heat as a solid.
    pub specific_heat_solid: f64,
    /// Latent heat of fusion or vaporisation.
    pub latent_heat: f64,
    /// How this material behaves in fire, per intensity (`src/fire.h:45-56`).
    pub burn: Vec<BurnData>,
    /// What burning it leaves behind, as an item id and an efficiency (`material.h:115-116`).
    pub burn_products: Vec<(String, f64)>,
    /// Whether things made of it are soft (`material.h:95`, loaded from `"soft"`). Read by `item::is_soft`, which decides whether an item has a length at all.
    pub soft: bool,
    /// How much of each vitamin one unit of the material carries, as `material_type::vitamin` answers it.
    ///
    /// Reference: `"vitamins"` in the material's own data (`src/material.cpp`), a list of `[vitamin, amount]` pairs whose amounts are **fractional** (`0.5` for
    /// `flesh`'s calcium and iron). It exists for one caller: a comestible that states no vitamins of its own **derives them from its materials**
    /// (`src/item_factory.cpp:762-776`), which is why a food's vitamins are a fact about what it is made of rather than only about what the JSON says.
    pub vitamins: std::collections::BTreeMap<String, f64>,
}

/// One material's behaviour in a fire of a given intensity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurnData {
    /// The intensity this describes.
    pub intensity: i32,
    /// Nothing burns or is destroyed, whatever the fire.
    pub immune: bool,
    /// Volume destroyed per turn, in millilitres; zero means no ceiling.
    pub volume_per_turn: f64,
    /// Fuel contributed to the fire.
    pub fuel: f64,
    /// The energy in one millilitre when this material is burned as fuel, in joules - the reference's `fuel_data::energy` (`material.h`), which the JSON writes as a
    /// string such as `"1000 kJ"`. `None` for a material that is not a fuel. **This is where diesel's energy lives**, not on the item that carries it.
    pub fuel_energy_j: Option<f64>,
    /// Smoke produced.
    pub smoke: f64,
    /// Fraction of the material destroyed per turn.
    pub burn: f64,
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
        resists: BTreeMap::new(),
        bash_resist: 0.0,
        cut_resist: 0.0,
        breathability: 0,
        specific_heat_liquid: 0.0,
        specific_heat_solid: 0.0,
        latent_heat: 0.0,
        burn: Vec::new(),
        burn_products: Vec::new(),
        soft: false,
        vitamins: std::collections::BTreeMap::new(),
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
            let Some(value) = crate::read_json(&path) else {
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

impl Material {
    /// What this material takes off a blow of one damage type, per millimetre.
    ///
    /// Reference: `material_type::resist( dmg_type )` (`material.cpp:213-216`) over `resistances::type_resist` (`damage.cpp:719-723`) - **zero when the data does not
    /// state the type**, which is the reference's own answer and not a fallback to a neighbouring type.
    #[must_use]
    pub fn resist_for(&self, damage_type: &str) -> f64 {
        self.resists.get(damage_type).copied().unwrap_or(0.0)
    }
}

/// The material's resistances, from the `resist` object (`material_type::load`, `material.cpp:85-88`, through `load_resistances_instance`,
/// `damage.cpp:841-847`).
///
/// **Every type the data states is kept, by name**, because that is what the reference keeps: a `resistances` holds a map from damage type to figure, and
/// `type_resist` (`damage.cpp:719-723`) answers **zero for a type the material does not state** - which is why a garment protects nothing against a stab unless
/// its material names one. The port used to keep bash and cut alone, which made the other types unanswerable rather than zero.
fn resistances(material: &mut Material, object: &Object) {
    let Some(Value::Object(resist)) = object.get("resist") else {
        return;
    };
    for (damage_type, value) in resist.members() {
        #[allow(clippy::cast_precision_loss)]
        let figure = match value {
            Value::Int(figure) => Some(*figure as f64),
            Value::Float(figure) => Some(*figure),
            _ => None,
        };
        if let Some(figure) = figure {
            material.resists.insert(damage_type.clone(), figure);
        }
    }
    // The two readers that predate the map keep answering the same way, so nothing that asks `bash_resist` has to change at the same moment.
    material.bash_resist = material.resists.get("bash").copied().unwrap_or(0.0);
    material.cut_resist = material.resists.get("cut").copied().unwrap_or(0.0);
}

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
    resistances(material, object);
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
    let burn_key = if object.get("burn_data").is_some() {
        "burn_data"
    } else {
        "burn"
    };
    if let Some(Value::Array(burns)) = object.get(burn_key) {
        material.burn.clear();
        for entry in burns {
            let Value::Object(fields) = entry else {
                return Err(LoadError {
                    message: format!("material {}: a burn entry is not an object", material.id),
                });
            };
            let intensity = i32::try_from(material.burn.len() + 1).unwrap_or(1);
            let volume = match fields.get("volume_per_turn") {
                Some(Value::Str(text)) => volume_in_ml(text),
                _ => number(fields, "volume_per_turn"),
            };
            material.burn.push(BurnData {
                intensity,
                immune: matches!(fields.get("immune"), Some(Value::Bool(true))),
                volume_per_turn: volume.unwrap_or(0.0),
                fuel: number(fields, "fuel").unwrap_or(0.0),
                fuel_energy_j: match fields.get("fuel_data") {
                    Some(Value::Object(data)) => {
                        string(data, "energy").and_then(|text| fuel_energy_j(&text))
                    }
                    _ => None,
                },
                smoke: number(fields, "smoke").unwrap_or(0.0),
                burn: number(fields, "burn").unwrap_or(0.0),
            });
        }
    }
    if let Some(Value::Array(products)) = object.get("burn_products") {
        material.burn_products.clear();
        for entry in products {
            let Value::Array(pair) = entry else {
                continue;
            };
            let (Some(Value::Str(id)), Some(weight)) = (pair.first(), pair.get(1)) else {
                continue;
            };
            let efficiency = match weight {
                Value::Int(value) => f64::from(i32::try_from(*value).unwrap_or(0)),
                Value::Float(value) => *value,
                _ => 0.0,
            };
            material.burn_products.push((id.clone(), efficiency));
        }
    }
    // Only when the data states it: a copy-from child that says nothing keeps what it inherited, which is what the reference's `optional( jo, was_loaded, ... )` does.
    if let Some(Value::Bool(soft)) = object.get("soft") {
        material.soft = *soft;
    }
    // `"vitamins": [ [ "calcium", 2 ] ]` - pairs, and the amounts are floats often enough that a fraction must survive.
    if let Some(Value::Array(entries)) = object.get("vitamins") {
        material.vitamins.clear();
        for entry in entries {
            let Value::Array(pair) = entry else {
                continue;
            };
            let (Some(Value::Str(id)), Some(amount)) = (pair.first(), pair.get(1)) else {
                continue;
            };
            let amount = match amount {
                Value::Int(value) => f64::from(i32::try_from(*value).unwrap_or(0)),
                Value::Float(value) => *value,
                _ => continue,
            };
            material.vitamins.insert(id.clone(), amount);
        }
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

/// Reads a volume written as the data writes it, e.g. `"200 ml"`.
fn volume_in_ml(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    let (number_text, unit) = trimmed.split_once(' ')?;
    let value: f64 = number_text.parse().ok()?;
    match unit {
        "ml" => Some(value),
        "L" => Some(value * 1000.0),
        _ => None,
    }
}

/// Turns the JSON's `"1000 kJ"` into joules.
///
/// Reference: `fuel_data::energy` is loaded with `assign( jo, "energy", energy )` (`material.cpp`), which reads a `units::energy` - a number and a unit. The units the
/// data uses are `J`, `kJ` and `MJ`; a string with any other unit, or none, is not a number this rule can read and answers `None` rather than a guess.
#[must_use]
pub fn fuel_energy_j(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    let split = trimmed.find(|character: char| character.is_alphabetic())?;
    let (number, unit) = trimmed.split_at(split);
    let value: f64 = number.trim().parse().ok()?;
    let factor = match unit.trim() {
        "J" => 1.0,
        "kJ" => 1_000.0,
        "MJ" => 1_000_000.0,
        _ => return None,
    };
    Some(value * factor)
}
