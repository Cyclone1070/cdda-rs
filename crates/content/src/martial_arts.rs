//! What each martial art asks of the hands that hold it.
//!
//! Reference: `martialart` and `ma_buff` (`martialarts.h:247-441`), loaded from `data/json/martialarts.json`. The port keeps the **requirements** - what a style or a buff wants in hand - and
//! leaves the buffs' own numbers to the round that reads them: a requirement the port guessed at would silently allow a style the data forbids.

use cdda_json::{Object, Value};

/// One static buff's requirement, as the data states it.
#[allow(clippy::struct_excessive_bools)]
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
#[allow(clippy::struct_excessive_bools)]
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
    /// The moves it offers, each with the requirement it states.
    pub techniques: Vec<Technique>,
}

/// One effect a move may leave on the body it lands on.
///
/// Reference: `tech_effect_data` (`martialarts.h:147-160`): an id, how long it lasts, whether it is permanent, whether it needs the blow to draw blood, and a **chance out of a hundred**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveEffect {
    /// Which effect.
    pub id: String,
    /// How long it lasts, in turns.
    pub duration: i64,
    /// Whether it never wears off.
    pub permanent: bool,
    /// Whether it only lands on a blow that drew blood.
    pub on_damage: bool,
    /// Its chance, out of a hundred.
    pub chance: i32,
}

/// One technique a style offers.
///
/// Reference: `ma_technique` (`martialarts.h:162-247`): the same `ma_requirements` a buff carries, plus what the move does - how likely it is to be chosen, whether it is defensive, and whether it
/// only fires on a critical. **What the move does to a body is the half not ported**: this is the requirement, which is what decides whether the move is offered at all.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Technique {
    /// Its id.
    pub id: String,
    /// What it is called.
    pub name: String,
    /// How often the move is chosen among those that qualify.
    pub weighting: i32,
    /// Whether it is a counter rather than an attack.
    pub defensive: bool,
    /// Whether it only fires on a critical hit.
    pub crit_tec: bool,
    /// A placeholder the reference never offers (`ma_technique::dummy`).
    pub dummy: bool,
    /// Whether it is offered in a critical moment **as well as** an ordinary one.
    pub crit_ok: bool,
    /// Whether it only works during a reach attack.
    pub reach_tec: bool,
    /// Whether it works during a reach attack as well as an ordinary one.
    pub reach_ok: bool,
    /// Whether it answers a dodge.
    pub dodge_counter: bool,
    /// Whether it answers a block.
    pub block_counter: bool,
    /// Whether it needs a loaded weapon.
    pub needs_ammo: bool,
    /// Whether it takes the target's weapon away.
    pub disarms: bool,
    /// Whether it takes the target's weapon for the striker.
    pub take_weapon: bool,
    /// How long it may knock the target down, or nothing.
    pub down_dur: i32,
    /// How long it may stun the target, or nothing.
    pub stun_dur: i32,
    /// The effects it may leave on the body it lands on.
    pub effects: Vec<MoveEffect>,
    /// What it may aim at: the reference's `attack_vectors` (`martialarts.h:70-97`).
    pub attack_vectors: Vec<AttackVector>,
    /// The requirement it states.
    pub requirement: BuffRequirement,
}

/// What a move may aim at.
///
/// Reference: `attack_vector` (`martialarts.h:70-97`): the limbs it may strike, whether it wants a weapon in hand, how much encumbrance a limb may carry, and **the count of each limb kind it
/// needs** (`limb_req`), which `choose_attack_vector` (`martialarts.cpp:1495-1515`) reads as a requirement rather than a preference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttackVector {
    /// Its id.
    pub id: String,
    /// Whether it is used with a weapon rather than a bare hand.
    pub weapon: bool,
    /// Whether no limb substitution happens.
    pub strict_limb_definition: bool,
    /// The absolute encumbrance a limb may carry.
    pub encumbrance_limit: i32,
    /// The percentage of a limb's hit points it needs.
    pub bp_hp_limit: i32,
    /// The limbs it may strike.
    pub limbs: Vec<String>,
    /// The limbs it needs, by kind and count.
    pub limb_req: Vec<VectorNeed>,
}

/// A move's need for a number of limbs of one kind.
///
/// Reference: `attack_vector::limb_req` (`martialarts.h:82`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorNeed {
    /// What kind of limb it needs.
    pub kind: String,
    /// How many of them it needs.
    pub count: i32,
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

/// Reads one of a move's attack vectors.
fn read_attack_vector(entry: &Value) -> Option<AttackVector> {
    let Value::Object(vector) = entry else {
        return None;
    };
    let limb_req = match vector.get("limb_req") {
        Some(Value::Array(needs)) => needs
            .iter()
            .filter_map(|need| {
                let Value::Object(need) = need else {
                    return None;
                };
                Some(VectorNeed {
                    kind: text(need, "type")?,
                    count: number(need, "count", 0),
                })
            })
            .collect(),
        _ => Vec::new(),
    };
    Some(AttackVector {
        id: text(vector, "id")?,
        weapon: flag(vector, "weapon"),
        strict_limb_definition: flag(vector, "strict_limb_definition"),
        encumbrance_limit: number(vector, "encumbrance_limit", 100),
        bp_hp_limit: number(vector, "bp_hp_limit", 10),
        limbs: words(vector, "limbs"),
        limb_req,
    })
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
#[allow(clippy::too_many_lines)]
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
    let mut techniques = Vec::new();
    if let Some(Value::Array(entries)) = object.get("techniques") {
        for entry in entries {
            let Value::Object(technique) = entry else {
                continue;
            };
            let Some(technique_id) = text(technique, "id") else {
                continue;
            };
            techniques.push(Technique {
                name: text(technique, "name").unwrap_or_default(),
                weighting: number(technique, "weighting", 0),
                defensive: flag(technique, "defensive"),
                crit_tec: flag(technique, "crit_tec"),
                dummy: flag(technique, "dummy"),
                crit_ok: flag(technique, "crit_ok"),
                reach_tec: flag(technique, "reach_tec"),
                reach_ok: flag(technique, "reach_ok"),
                dodge_counter: flag(technique, "dodge_counter"),
                block_counter: flag(technique, "block_counter"),
                needs_ammo: flag(technique, "needs_ammo"),
                disarms: flag(technique, "disarms"),
                take_weapon: flag(technique, "take_weapon"),
                down_dur: number(technique, "down_dur", 0),
                stun_dur: number(technique, "stun_dur", 0),
                attack_vectors: match technique.get("attack_vectors") {
                    Some(Value::Array(entries)) => entries
                        .iter()
                        .filter_map(read_attack_vector)
                        .collect(),
                    _ => Vec::new(),
                },
                effects: match technique.get("effects") {
                    Some(Value::Array(entries)) => entries
                        .iter()
                        .filter_map(|entry| {
                            let Value::Object(effect) = entry else {
                                return None;
                            };
                            Some(MoveEffect {
                                id: text(effect, "id")?,
                                duration: match effect.get("duration") {
                                    Some(Value::Int(turns)) => *turns,
                                    _ => return None,
                                },
                                permanent: flag(effect, "permanent"),
                                on_damage: flag(effect, "on_damage"),
                                chance: number(effect, "chance", 0),
                            })
                        })
                        .collect(),
                    _ => Vec::new(),
                },
                requirement: BuffRequirement {
                    id: technique_id.clone(),
                    unarmed_allowed: flag(technique, "unarmed_allowed"),
                    melee_allowed: flag(technique, "melee_allowed"),
                    strictly_unarmed: flag(technique, "strictly_unarmed"),
                    wall_adjacent: flag(technique, "wall_adjacent"),
                    weapon_categories: words(technique, "weapon_categories"),
                    min_skill: Vec::new(),
                },
                id: technique_id,
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
        techniques,
        id,
    })
}
