//! What each martial art asks of the hands that hold it.
//!
//! Reference: `martialart` and `ma_buff` (`martialarts.h:247-441`), loaded from `data/json/martialarts.json`. The port keeps the **requirements** - what a style or a buff wants in hand - and
//! leaves the buffs' own numbers to the round that reads them: a requirement the port guessed at would silently allow a style the data forbids.

use cdda_json::{Object, Value};

/// One static buff's requirement, as the data states it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffRequirement {
    /// Its id, which is what the data calls the buff.
    pub id: String,
    /// Whether it works with an empty hand.
    pub unarmed_allowed: bool,
    /// Whether it works with a melee weapon.
    pub melee_allowed: bool,
    /// Whether the hand must be truly empty.
    pub strictly_unarmed: bool,
    /// Whether it only works with a wall beside the striker.
    pub wall_adjacent: bool,
    /// The weapon categories it allows. Empty allows any.
    pub weapon_categories: Vec<String>,
    /// The skills it needs, with the level each must reach.
    pub min_skill: Vec<(String, i32)>,
}

/// One martial art.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Style {
    /// Which style.
    pub id: String,
    /// What it is called in the data.
    pub name: String,
    /// The chance to block with an arm, in percent.
    pub arm_block: i32,
    /// The chance to block with a leg, in percent.
    pub leg_block: i32,
    /// Whether a book may teach it.
    pub teachable: bool,
    /// Whether it may only be used with a weapon.
    pub strictly_melee: bool,
    /// Whether it may only be used with an empty hand.
    pub strictly_unarmed: bool,
    /// Whether any weapon is allowed with it.
    pub allow_all_weapons: bool,
    /// Whether it refuses every weapon.
    pub force_unarmed: bool,
    /// The weapon categories it allows.
    pub weapon_categories: Vec<String>,
    /// Its static buffs, in the data's order.
    pub static_buffs: Vec<BuffRequirement>,
}

/// Every style the data defines, in the reference's own order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Styles {
    /// The styles, in order.
    pub all: Vec<Style>,
}

fn text(object: &Object, key: &str) -> Option<String> {
    match object.get(key) {
        Some(Value::Str(found)) => Some(found.clone()),
        _ => None,
    }
}

fn number(object: &Object, key: &str, fallback: i32) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    match object.get(key) {
        Some(Value::Int(found)) => i32::try_from(*found).unwrap_or(fallback),
        Some(Value::Float(found)) => *found as i32,
        _ => fallback,
    }
}

fn flag(object: &Object, key: &str) -> bool {
    matches!(object.get(key), Some(Value::Bool(true)))
}

fn words(object: &Object, key: &str) -> Vec<String> {
    match object.get(key) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::Str(said) => Some(said.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Reads one style, or nothing when the row names no id.
#[must_use]
pub fn read_style(object: &Object) -> Option<Style> {
    let id = text(object, "id")?;
    let mut static_buffs = Vec::new();
    if let Some(Value::Array(entries)) = object.get("static_buffs") {
        for entry in entries {
            let Value::Object(buff) = entry else {
                continue;
            };
            let Some(buff_id) = text(buff, "id") else {
                continue;
            };
            let mut min_skill = Vec::new();
            if let Some(Value::Array(needs)) = buff.get("min_skill") {
                for need in needs {
                    let Value::Object(requirement) = need else {
                        continue;
                    };
                    if let Some(skill) = text(requirement, "skill") {
                        min_skill.push((skill, number(requirement, "level", 0)));
                    }
                }
            }
            static_buffs.push(BuffRequirement {
                id: buff_id,
                unarmed_allowed: flag(buff, "unarmed_allowed"),
                melee_allowed: flag(buff, "melee_allowed"),
                strictly_unarmed: flag(buff, "strictly_unarmed"),
                wall_adjacent: flag(buff, "wall_adjacent"),
                weapon_categories: words(buff, "weapon_categories"),
                min_skill,
            });
        }
    }
    Some(Style {
        name: text(object, "name").unwrap_or_default(),
        arm_block: number(object, "arm_block", 0),
        leg_block: number(object, "leg_block", 0),
        teachable: flag(object, "teachable"),
        strictly_melee: flag(object, "strictly_melee"),
        strictly_unarmed: flag(object, "strictly_unarmed"),
        allow_all_weapons: flag(object, "allow_all_weapons"),
        force_unarmed: flag(object, "force_unarmed"),
        weapon_categories: words(object, "weapon_categories"),
        static_buffs,
        id,
    })
}
