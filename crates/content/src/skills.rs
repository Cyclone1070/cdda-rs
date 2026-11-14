//! Every skill's own attack-time line.
//!
//! Reference: `struct time_info_t` (`src/skill.h:23-30`) - `min_time`, `base_time` and `time_reduction_per_level` - which `time_to_attack` (`ranged.cpp:2299-2308`) reads through the gun's
//! own skill. **The struct's defaults (50, 220, 25) are marksmanship's own line**, so a port that assumed them would make every rifle shot seven times as slow as the data says.

use cdda_json::{Object, Value};

/// One skill's line: how long an attack with it takes, and how fast practice shortens it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillTime {
    /// The skill's id, such as `rifle`.
    pub id: String,
    /// The name a player sees: `rifles`.
    pub name: String,
    /// The floor no amount of practice goes below.
    pub min_time: i32,
    /// What a beginner pays.
    pub base_time: i32,
    /// What each level takes off.
    pub time_reduction_per_level: i32,
}

/// Reads one skill's line from a dump row.
///
/// # Errors
///
/// When a member that is always written is missing or the wrong shape.
pub fn read_one(id: &str, object: &Object) -> Result<SkillTime, String> {
    Ok(SkillTime {
        id: id.to_owned(),
        name: match object.get("name") {
            Some(Value::Str(name)) => name.clone(),
            _ => String::new(),
        },
        min_time: number(object, "min_time")?,
        base_time: number(object, "base_time")?,
        time_reduction_per_level: number(object, "time_reduction_per_level")?,
    })
}

/// Reads a whole dump: `{"harness":"skills","version":1,"skills":[...]}`.
///
/// # Errors
///
/// When the dump is not the shape this loader speaks, or a row is malformed.
pub fn load_dump(value: &Value) -> Result<Vec<SkillTime>, String> {
    let Value::Object(root) = value else {
        return Err("the skills dump is not an object".to_owned());
    };
    let Some(Value::Array(rows)) = root.get("skills") else {
        return Err("the skills dump has no `skills` array".to_owned());
    };
    let mut skills = Vec::with_capacity(rows.len());
    for row in rows {
        let Value::Object(object) = row else {
            continue;
        };
        let Some(Value::Str(id)) = object.get("id") else {
            continue;
        };
        skills.push(read_one(id, object)?);
    }
    Ok(skills)
}

/// An integer member, which every row states.
///
/// # Errors
///
/// When the member is missing or not an integer.
fn number(object: &Object, key: &str) -> Result<i32, String> {
    match object.get(key) {
        Some(Value::Int(value)) => {
            i32::try_from(*value).map_err(|_| format!("`{key}` is too large"))
        }
        Some(_) => Err(format!("`{key}` is not an integer")),
        None => Err(format!("`{key}` is missing")),
    }
}
