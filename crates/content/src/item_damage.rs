//! **How much damage a thing can take before it is ruined.**
//!
//! Reference: `itype::damage_max` (`item.cpp`), reported by `--dump-damage` for all 9,912 types together with the damage level at six probe values. Only the maximum is read here,
//! because it is the only number a rule needs before the port has a damage model: `item::mod_damage` decides destruction with `damage + qty > max_damage()`.
//!
//! A maximum of **zero** is not a mistake: a rock cannot be damaged, so any damage destroys it outright, and the rule reading this has to say so rather than treat zero as "unknown".

use cdda_json::Value;

/// The damage each type takes before it is ruined, by id.
///
/// Types the recording does not name are absent rather than zero: "the recording never mentioned it" and "it cannot be damaged" are different answers, and a caller that confuses
/// them will destroy things the reference would keep.
#[must_use]
pub fn from_dump(value: &Value) -> std::collections::BTreeMap<String, i32> {
    let mut limits = std::collections::BTreeMap::new();
    let Value::Object(root) = value else {
        return limits;
    };
    let Some(Value::Array(types)) = root.get("types") else {
        return limits;
    };
    for entry in types {
        let Value::Object(row) = entry else {
            continue;
        };
        let (Some(Value::Str(id)), Some(Value::Int(max))) = (row.get("id"), row.get("max_damage"))
        else {
            continue;
        };
        limits.insert(id.clone(), i32::try_from(*max).unwrap_or(i32::MAX));
    }
    limits
}
