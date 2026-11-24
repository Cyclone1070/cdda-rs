//! The vehicle groups a mapgen's placement draws from.
//!
//! Reference: `vehicle_group` (`src/veh_type.h:500-520`) and `VehicleGroup::pick` (`vehicle_group.cpp`), the list a mapgen's `place_vehicles` entry names: a parking lot does not
//! name a car, it names `parkinglotbasic` and lets the draw decide. 111 mapgens do it that way, so a port that only understood a prototype id would leave those lots empty.

use crate::monsters::LoadError;
use cdda_core_types::rng::Rolls;
use cdda_core_types::weighted::WeightedList;
use cdda_json::Value;

/// A group of vehicles a placement may draw from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleGroup {
    /// Its id.
    pub id: String,
    /// Its members, each with its weight, in the order the data gives them.
    pub vehicles: Vec<(String, i32)>,
}

/// Loads every vehicle group under `dirs`.
///
/// # Errors
///
/// Reports a directory that cannot be read.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<Vec<VehicleGroup>, LoadError> {
    let mut by_id: std::collections::BTreeMap<String, Vec<(String, i32)>> =
        std::collections::BTreeMap::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            collect_into(value.as_ref(), &mut by_id);
        }
    }
    Ok(by_id
        .into_iter()
        .map(|(id, vehicles)| VehicleGroup { id, vehicles })
        .collect())
}

/// Reads groups out of in-memory JSON, which is how a test hands them over without a file.
#[must_use]
pub fn load_objects(prototypes: &[Value]) -> Vec<VehicleGroup> {
    let mut by_id: std::collections::BTreeMap<String, Vec<(String, i32)>> =
        std::collections::BTreeMap::new();
    for prototype in prototypes {
        collect_into(prototype, &mut by_id);
    }
    by_id
        .into_iter()
        .map(|(id, vehicles)| VehicleGroup { id, vehicles })
        .collect()
}

/// Picks a vehicle out of a group by weight.
#[must_use]
pub fn pick(group_id: &str, groups: &[VehicleGroup], rolls: &mut Rolls) -> Option<String> {
    let group = groups.iter().find(|group| group.id == group_id)?;
    let list = WeightedList::from_entries(group.vehicles.iter().cloned());
    list.pick(rolls).cloned()
}

/// Whether an id names a group rather than a prototype.
#[must_use]
pub fn is_group(group_id: &str, groups: &[VehicleGroup]) -> bool {
    groups.iter().any(|group| group.id == group_id)
}

/// Reads the `vehicle_group` prototypes out of one JSON value.
fn collect_into(value: &Value, by_id: &mut std::collections::BTreeMap<String, Vec<(String, i32)>>) {
    let objects: Vec<&Value> = match value {
        Value::Array(objects) => objects.iter().collect(),
        other => vec![other],
    };
    for object in objects {
        let Value::Object(fields) = object else {
            continue;
        };
        match fields.get("type") {
            Some(Value::Str(kind)) if kind == "vehicle_group" => {}
            _ => continue,
        }
        let id = match fields.get("id").or_else(|| fields.get("name")) {
            Some(Value::Str(id)) => id.clone(),
            _ => continue,
        };
        let Some(Value::Array(list)) = fields.get("vehicles") else {
            continue;
        };
        let entries = by_id.entry(id).or_default();
        for item in list {
            // The data writes each member as `[ id, weight ]` (`veh_type.h:500-520`).
            let Value::Array(pair) = item else {
                continue;
            };
            let Some(Value::Str(vehicle)) = pair.first() else {
                continue;
            };
            let weight = match pair.get(1) {
                Some(Value::Int(weight)) => i32::try_from(*weight).unwrap_or(1),
                _ => 1,
            };
            entries.push((vehicle.clone(), weight));
        }
    }
}
