//! What a swing costs in moves and in stamina, as the reference itself works it out.
//!
//! Reference: `Character::attack_speed` (`src/melee.cpp:2668-2712`) and `Character::get_total_melee_stamina_cost` (`src/melee.cpp:970-996`), dumped by `--dump-swing-cost`. Both rules were
//! transcribed from the source and neither had been compared against the reference's own answer - the class of defect the crit-chance dump found twice, so this is the same treatment for the
//! same kind of rule.
//!
//! One row is one character in one stance with one weapon: the weapon, the melee skill, the dexterity, the stamina ratio it had, and the two costs the reference answered.

use cdda_json::{Object, Value};

/// One recorded pair of costs.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The weapon's id, or `null` for an empty hand.
    pub weapon: String,
    /// How the character was standing: `standing`, `downed` or `crouching`.
    pub stance: String,
    /// The melee skill.
    pub melee_skill: i32,
    /// The character's dexterity.
    pub dexterity: i32,
    /// The share of its maximum stamina the character had, as the reference measured it.
    pub stamina_ratio: f32,
    /// The moves `attack_speed` answered.
    pub attack_speed: i32,
    /// The stamina `get_total_melee_stamina_cost` answered, which is **negative**.
    pub stamina_cost: i32,
    /// The weapon's own `attack_time`, which is the input that never changes with the character.
    pub weapon_time: i32,
}

/// Reads the dump.
///
/// # Errors
///
/// Returns a sentence naming the row that is not shaped like a row.
pub fn from_dump(value: &Value) -> Result<Vec<Row>, String> {
    let Value::Object(root) = value else {
        return Err("the swing cost dump is not an object".to_owned());
    };
    let Some(Value::Array(rows)) = root.get("rows") else {
        return Err("the swing cost dump has no rows".to_owned());
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
        stance: match row.get("stance")? {
            Value::Str(text) => text.clone(),
            _ => return None,
        },
        melee_skill: whole(row.get("melee_skill")?)?,
        dexterity: whole(row.get("dexterity")?)?,
        stamina_ratio: real(row.get("stamina_ratio")?)?,
        attack_speed: whole(row.get("attack_speed")?)?,
        stamina_cost: whole(row.get("stamina_cost")?)?,
        weapon_time: whole(row.get("weapon_time")?)?,
    })
}

fn whole(value: &Value) -> Option<i32> {
    match value {
        Value::Int(number) => i32::try_from(*number).ok(),
        _ => None,
    }
}

#[allow(clippy::cast_possible_truncation)]
fn real(value: &Value) -> Option<f32> {
    match value {
        Value::Float(number) => Some(*number as f32),
        Value::Int(number) => Some(*number as f32),
        _ => None,
    }
}
