//! Vitamin types: what one unit of each weighs, and nothing else yet.
//!
//! Reference: `vitamin` (`src/vitamin.h`) and `vitamin::units_from_mass` (`src/vitamin.cpp:136-144`). The only member the comestible slot needs is `weight_per_unit`,
//! because the data may write a vitamin's amount as a **mass** — `["calcium", "25588 mg"]` in `bone` — and the reference converts it to whole units on the vitamin's
//! own scale: `mass / weight_per_unit`, an integer division of one `units::mass` by another.
//!
//! Three of the 53 vitamins carry a weight: `calcium` is `10 mg 420 μg`, `iron` is `188 ug`, `vitC` is `938 mcg`. A vitamin without one cannot be written as a mass at
//! all — the reference complains and answers **1** unit (`src/vitamin.cpp:138-142`) — so the port keeps `None` and refuses the same way rather than inventing a scale.

use cdda_json::{Object, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// What the port needs to know about a vitamin type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vitamin {
    /// Its id.
    pub id: String,
    /// What one unit weighs, in **micrograms**, when the type says.
    pub weight_per_unit: Option<i64>,
    /// What kind of thing it is: `vitamin`, `counter`, `drug` or `toxin` (`vitamin_type`, `src/vitamin.h:20-26`).
    ///
    /// Only a `vitamin` writes its amount in the data as an **RDA percentage**. Everything else is already in units — `meat_allergen` at 1 is one allergen, not one
    /// percent of a day — which is why the conversion below asks first.
    pub vit_type: String,
    /// The lowest level the body can reach: `vitamin::min`, **zero** when the data states none (`src/vitamin.h:126`).
    pub min: i32,
    /// The highest level the body can reach: `vitamin::max`, **zero** when the data states none (`src/vitamin.h:127`).
    ///
    /// A zero ceiling is not a bug in the data: 50 of the 53 vitamins in the content are markers (`meat_allergen`, `mutagen`, `bad_food`) that are read, not stored,
    /// so a level that clamps to zero is the same answer the reference gives.
    pub max: i32,
    /// How long one unit lasts, in turns: the type's `rate`, which every vitamin in the content states as `15 m`.
    ///
    /// This is the scale that turns a food's **RDA percentage** into internal units. `RDA_to_default` is `24h * percent / 100 / rate`, so with a 15-minute rate one
    /// unit is one 96th of a day and a percent is 0.96 units (`src/vitamin.cpp:122-129`).
    pub rate: i64,
}

/// Reads every vitamin prototype under `dirs`.
///
/// # Errors
///
/// If a file cannot be read or parsed.
pub fn load_paths(dirs: &[&Path]) -> Result<BTreeMap<String, Vitamin>, crate::mapdata::LoadError> {
    let mut out = BTreeMap::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| crate::mapdata::LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            let Value::Array(entries) = &*value else {
                continue;
            };
            for entry in entries {
                let Value::Object(object) = entry else {
                    continue;
                };
                if !matches!(object.get("type"), Some(Value::Str(kind)) if kind == "vitamin") {
                    continue;
                }
                let Some(id) = crate::items::string(object, "id") else {
                    continue;
                };
                let number = |key: &str| match object.get(key) {
                    Some(Value::Int(value)) => i32::try_from(*value).unwrap_or(0),
                    _ => 0,
                };
                let min = number("min");
                let max = number("max");
                let rate = match object.get("rate") {
                    Some(Value::Str(text)) => crate::constructions::parse_duration(text),
                    Some(Value::Int(turns)) => *turns,
                    // The class's own default when the data says nothing (`src/vitamin.h:128`: zero).
                    _ => 0,
                };
                let weight_per_unit = match object.get("weight_per_unit") {
                    Some(Value::Str(text)) => {
                        Some(cdda_core_types::units::read_vitamin_mass(text).map_err(|error| {
                            crate::mapdata::LoadError {
                                message: format!("vitamin {id}: weight_per_unit {text:?}: {error:?}"),
                            }
                        })?)
                    }
                    Some(Value::Int(grams)) => Some(*grams),
                    _ => None,
                };
                out.insert(
                    id.clone(),
                    Vitamin {
                        id,
                        vit_type: crate::items::string(object, "vit_type").unwrap_or_else(|| "vitamin".to_owned()),
                        min,
                        max,
                        weight_per_unit,
                        rate,
                    },
                );
            }
        }
    }
    Ok(out)
}

/// Turns a food's **RDA percentage** into internal units: `vitamin::RDA_to_default` (`src/vitamin.cpp:122-129`).
///
/// `24_hours * ( percent / 100.0f ) / rate`, and the reference assigns the `float` it returns to an `int`, so the answer truncates toward zero.
///
/// **Only a `vitamin` is converted.** The reference returns the percent unchanged for anything else (`vitamin.cpp:122-126`), and 50 of the 53 types in the content are
/// not vitamins: `meat_allergen` at 1 is one allergen, and converting it would answer 0 — which the food-summary gate caught on the snake skins.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
#[must_use]
pub fn rda_to_default(vitamin: &Vitamin, percent: i32) -> i32 {
    if vitamin.vit_type != "vitamin" || vitamin.rate <= 0 {
        return percent;
    }
    let seconds = 24_i64 * 60 * 60;
    let units = seconds as f32 * (percent as f32 / 100.0) / vitamin.rate as f32;
    units as i32
}

/// Converts a mass to whole units on the vitamin's scale: `vitamin::units_from_mass` (`src/vitamin.cpp:136-144`).
///
/// Both numbers are in micrograms - `vitamin_units::mass` counts them (`src/stomach.h:22-26`) - so the division needs no scale factor.
///
/// A vitamin with no `weight_per_unit` cannot be converted, and the reference answers **1** with a complaint; the port takes the same answer so a recording compares
/// equal, and the complaint is the declared part.
#[must_use]
pub fn units_from_mass(vitamin: &Vitamin, micrograms: i64) -> i32 {
    match vitamin.weight_per_unit {
        Some(per_unit) if per_unit > 0 => i32::try_from(micrograms / per_unit).unwrap_or(i32::MAX),
        _ => 1,
    }
}

/// The vitamins a comestible gets when it states none of its own, derived from what it is made of.
///
/// Reference: `item_factory.cpp:762-776`. Three details make it more than a lookup:
///
/// * it runs only when the stated map is **empty** *and* `healthy >= 0` — a food with a negative healthy rating gets no derived vitamins at all;
/// * the scale is `max(healthy, 1) * 10`, so a healthy rating of 0 and one of 1 give the same **10**;
/// * each material contributes `material.vitamin(v) * scale / materials.len()` and is added **ceil**ed on its own, so three materials of 0.1 become three additions of
///   1 rather than one addition of 3 — and every vitamin is inserted, which is why an all-zero map is the normal shape for a food made of materials that carry none.
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn derived_from_materials(
    materials: &std::collections::BTreeMap<String, i32>,
    healthy: i32,
    vitamins: &BTreeMap<String, Vitamin>,
    registry: &std::collections::BTreeMap<String, super::materials::Material>,
) -> BTreeMap<String, i32> {
    let mut out = BTreeMap::new();
    if healthy < 0 || materials.is_empty() {
        return out;
    }
    let scale = healthy.max(1) * 10;
    let count = materials.len();
    for id in vitamins.keys() {
        let mut total = 0;
        for material in materials.keys() {
            let Some(known) = registry.get(material) else {
                continue;
            };
            let amount = known.vitamins.get(id).copied().unwrap_or(0.0) * f64::from(scale)
                / count as f64;
            // `std::ceil` on each addition, then an integer add.
            let units = amount.ceil() as i32;
            total += units;
        }
        out.insert(id.clone(), total);
    }
    out
}

/// Parses the `[[vitamin, amount], ...]` list a comestible writes, with amounts as integers or as masses.
///
/// Reference: `vitamins_reader` (`src/item_factory.cpp:3382-3403`) — a two-element array per entry, the second either an integer or a mass string — followed by
/// `nutrients::finalize_vitamins` (`src/stomach.cpp:134-146`), which converts every mass to units on that vitamin's own scale.
///
/// # Errors
///
/// If a vitamin is not registered or a mass does not parse.
#[allow(clippy::cast_possible_truncation)]
pub fn read_list(
    object: &Object,
    vitamins: &BTreeMap<String, Vitamin>,
) -> Result<BTreeMap<String, i32>, crate::mapdata::LoadError> {
    let Some(Value::Array(entries)) = object.get("vitamins") else {
        return Ok(BTreeMap::new());
    };
    let mut out = BTreeMap::new();
    for entry in entries {
        let Value::Array(pair) = entry else {
            continue;
        };
        let (Some(Value::Str(id)), Some(amount)) = (pair.first(), pair.get(1)) else {
            continue;
        };
        let units = match amount {
            Value::Int(units) => i32::try_from(*units).unwrap_or(0),
            Value::Float(units) => *units as i32,
            Value::Str(mass) => {
                let micrograms = cdda_core_types::units::read_vitamin_mass(mass).map_err(|error| {
                    crate::mapdata::LoadError {
                        message: format!("vitamin {id}: {mass:?}: {error:?}"),
                    }
                })?;
                let vitamin = vitamins.get(id).ok_or_else(|| crate::mapdata::LoadError {
                    message: format!("vitamin {id} is not registered"),
                })?;
                units_from_mass(vitamin, micrograms)
            }
            _ => continue,
        };
        out.insert(id.clone(), units);
    }
    Ok(out)
}
