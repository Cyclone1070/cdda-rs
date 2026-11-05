//! What a round does when it lands.
//!
//! Reference: `islot_ammo::damage`, `count` and `shot_spread` (`src/itype.h:1090-1101`). **This is where a firearm's blow lives**: every gun in the guns dump states a `damage` of zero because a
//! gun's damage is its cartridge's, and `item::gun_damage( true )` adds exactly this (`item.cpp:11173-11180`).
//!
//! The reference also carries `shot_damage`, which is the **per-pellet** figure and is patched only for shot (`item_factory.cpp:527-532`) - a rifle round reports zero there. The dump writes both so
//! the difference is on the page; the port reads `damage`, which is the round's whole blow.

use cdda_json::{Object, Value};

/// One round's own numbers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AmmoType {
    /// The item's id.
    pub id: String,
    /// The round's whole blow, before the gun's barrel scales it: `ammo->damage`.
    pub damage: f64,
    /// The blow by kind, which is how the data states it: a cartridge is bullet, a rock is bash.
    pub damage_bash: i32,
    /// Cut damage.
    pub damage_cut: i32,
    /// Stab damage, which an arrow states.
    pub damage_stab: i32,
    /// Bullet damage, which every cartridge states.
    pub damage_bullet: i32,
    /// How many pellets one round puts in the air: **twenty for birdshot, one for a rifle round**.
    pub count: i32,
    /// The angle between those pellets, in arcminutes.
    pub shot_spread: i32,
    /// The chance in a hundred that a fired round can be picked up again.
    pub recovery_chance: i32,
    /// The reference's per-pellet figure, kept because the dump writes it and a reader will ask what it is.
    pub shot_damage: f64,
    /// How hard the round kicks: `ammo->recoil`, which `item::gun_recoil` adds to the gun's own.
    pub recoil: i32,
    /// The **ammunition type** the round belongs to - `islot_ammo::type` (`itype.h:1063`), one a round. A gun names a type (`"shot"`); the rounds that fit it are keyed by their own ids
    /// (`shot_bird`, `shot_00`), so the two namespaces must not be confused.
    pub ammo_type: String,
    /// How loud the round is: `islot_ammo::loudness` (`itype.h:1111`), which the factory fills from the round's damage when the data leaves it unstated.
    pub loudness: i32,
}

/// Reads one round from a dump row.
///
/// # Errors
///
/// When a member that is always written is missing or the wrong shape.
pub fn read_one(id: &str, object: &Object) -> Result<AmmoType, String> {
    Ok(AmmoType {
        id: id.to_owned(),
        damage: float(object, "damage")?,
        damage_bash: number(object, "damage_bash")?,
        damage_cut: number(object, "damage_cut")?,
        damage_stab: number(object, "damage_stab")?,
        damage_bullet: number(object, "damage_bullet")?,
        count: number(object, "count")?,
        shot_spread: number(object, "shot_spread")?,
        recovery_chance: number(object, "recovery_chance")?,
        shot_damage: float(object, "shot_damage")?,
        recoil: number(object, "recoil")?,
        loudness: number(object, "loudness")?,
        ammo_type: match object.get("ammo_types") {
            Some(Value::Str(kind)) => kind.clone(),
            _ => String::new(),
        },
    })
}

/// Reads a whole dump: `{"harness":"ammo","version":1,"ammo":[...]}`.
///
/// # Errors
///
/// When the dump is not the shape this loader speaks, or a row is malformed.
pub fn load_dump(value: &Value) -> Result<Vec<AmmoType>, String> {
    let Value::Object(root) = value else {
        return Err("the ammo dump is not an object".to_owned());
    };
    let Some(Value::Array(rows)) = root.get("ammo") else {
        return Err("the ammo dump has no `ammo` array".to_owned());
    };
    let mut rounds = Vec::with_capacity(rows.len());
    for row in rows {
        let Value::Object(object) = row else {
            continue;
        };
        let Some(Value::Str(id)) = object.get("id") else {
            continue;
        };
        rounds.push(read_one(id, object)?);
    }
    Ok(rounds)
}

/// An integer member, which every row states.
///
/// # Errors
///
/// When the member is missing or not an integer.
fn number(object: &Object, key: &str) -> Result<i32, String> {
    match object.get(key) {
        Some(Value::Int(value)) => i32::try_from(*value).map_err(|_| format!("`{key}` is too large")),
        Some(Value::Float(value)) => {
            #[allow(clippy::cast_possible_truncation)]
            let value = *value as i32;
            Ok(value)
        }
        Some(_) => Err(format!("`{key}` is not a number")),
        None => Err(format!("`{key}` is missing")),
    }
}

/// A decimal member, which the dump writes for the two sums.
///
/// # Errors
///
/// When the member is missing or not a number.
fn float(object: &Object, key: &str) -> Result<f64, String> {
    match object.get(key) {
        Some(Value::Float(value)) => Ok(*value),
        Some(Value::Int(value)) => {
            #[allow(clippy::cast_precision_loss)]
            let value = *value as f64;
            Ok(value)
        }
        Some(_) => Err(format!("`{key}` is not a number")),
        None => Err(format!("`{key}` is missing")),
    }
}
