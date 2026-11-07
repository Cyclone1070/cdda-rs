//! How likely a critical swing is, as the reference itself answers it.
//!
//! Reference: `Character::crit_chance` (`src/melee.cpp:1116-1193`), dumped by `--dump-crit-chance` over a sweep of the inputs the rule reads. The rule has **three chances that multiply** and
//! a **pair** term for a roll past one and a half times the target's dodge, and a port can get the second wrong in several ways that all look plausible - which is why the numbers come from
//! the reference rather than from reading its source twice.
//!
//! One row is one call: the weapon it swung, the character's dexterity and perception, the weapon's skill and the melee skill, the roll, and the chance the reference answered.

use cdda_json::{Object, Value};

/// One recorded call to `crit_chance`.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The weapon's id, which decides its accuracy and whether the hand is empty.
    pub weapon: String,
    /// The character's dexterity.
    pub dexterity: i32,
    /// The character's perception.
    pub perception: i32,
    /// The level set on the weapon's own skill (and on `unarmed`) for this row.
    pub weapon_skill: i32,
    /// What `get_skill_level( weap.melee_skill() )` answered - **zero for a null weapon**, whose skill id is null.
    pub weapon_skill_level: i32,
    /// The general melee skill.
    pub melee_skill: i32,
    /// The hit roll the chance is asked about.
    pub roll: f32,
    /// The chance the reference answered.
    pub chance: f64,
}

/// Reads the dump.
///
/// # Errors
///
/// Returns a sentence naming the row that is not shaped like a row.
pub fn from_dump(value: &Value) -> Result<Vec<Row>, String> {
    let Value::Object(root) = value else {
        return Err("the crit chance dump is not an object".to_owned());
    };
    let Some(Value::Array(rows)) = root.get("rows") else {
        return Err("the crit chance dump has no rows".to_owned());
    };
    let mut out = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let Value::Object(row) = row else {
            return Err(format!("row {index} is not an object"));
        };
        out.push(read_row(row).ok_or_else(|| format!("row {index} is missing a field"))?);
    }
    Ok(out)
}

fn read_row(row: &Object) -> Option<Row> {
    Some(Row {
        weapon: match row.get("weapon")? {
            Value::Str(text) => text.clone(),
            _ => return None,
        },
        dexterity: whole(row.get("dex")?)?,
        perception: whole(row.get("per")?)?,
        weapon_skill: whole(row.get("weapon_skill")?)?,
        weapon_skill_level: whole(row.get("weapon_skill_level")?)?,
        melee_skill: whole(row.get("melee_skill")?)?,
        roll: real(row.get("roll")?)?,
        chance: f64::from(real(row.get("chance")?)?),
    })
}

fn whole(value: &Value) -> Option<i32> {
    match value {
        Value::Int(number) => i32::try_from(*number).ok(),
        _ => None,
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn real(value: &Value) -> Option<f32> {
    match value {
        Value::Float(number) => Some(*number as f32),
        Value::Int(number) => Some(*number as f32),
        _ => None,
    }
}
