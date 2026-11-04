//! What a gun is worth before anyone aims it.
//!
//! Reference: `itype::gun` (`itype.h`), the `gun_type` a prototype carries when it can be fired, and the three members this port needs first: `dispersion`, `skill_used` and `range`.
//! The table is read from the oracle's own dump (`--dump-guns`, 406 types) rather than from the game's JSON, because the dump is what the reference resolved after inheritance, mods
//! and overrides - which is the only figure the port can be equal to.

use cdda_json::{Object, Value};

/// One gun's own numbers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GunType {
    /// The item's id.
    pub id: String,
    /// The weapon's own dispersion, before gunmods, ammo or the shooter: `gun_type::dispersion`.
    pub dispersion: i32,
    /// The skill it is fired with: `gun_type::skill_used`, such as `rifle` or `archery`.
    pub skill: String,
    /// Its range in squares: `gun_type::range`.
    pub range: i32,
    /// Its recoil: `gun_type::recoil`.
    pub recoil: i32,
    /// The ammo it takes, or the empty string for a launcher that states none.
    pub ammo: String,
}


/// Reads one gun from a dump row: `--dump-guns`.
///
/// # Errors
///
/// When a member that is always written is missing or the wrong shape.
pub fn read_one(id: &str, object: &Object) -> Result<GunType, String> {
    let mut gun = GunType {
        id: id.to_owned(),
        ..GunType::default()
    };
    gun.dispersion = number(object, "dispersion", gun.dispersion)?;
    gun.range = number(object, "range", gun.range)?;
    gun.recoil = number(object, "recoil", gun.recoil)?;
    if let Some(Value::Str(skill)) = object.get("skill") {
        gun.skill.clone_from(skill);
    }
    if let Some(Value::Str(ammo)) = object.get("ammo") {
        gun.ammo.clone_from(ammo);
    }
    Ok(gun)
}

/// Reads a whole dump: `{"harness":"guns","version":1,"guns":[...]}`.
///
/// # Errors
///
/// When the dump is not the shape this loader speaks.
pub fn load_dump(value: &Value) -> Result<Vec<GunType>, String> {
    let Value::Object(root) = value else {
        return Err("the guns dump is not an object".to_owned());
    };
    let Some(Value::Array(rows)) = root.get("guns") else {
        return Err("the guns dump has no `guns` array".to_owned());
    };
    let mut guns = Vec::with_capacity(rows.len());
    for row in rows {
        let Value::Object(object) = row else {
            continue;
        };
        let Some(Value::Str(id)) = object.get("id") else {
            continue;
        };
        guns.push(read_one(id, object)?);
    }
    Ok(guns)
}

/// An integer member, or the value it started with when the member says nothing.
///
/// # Errors
///
/// When the member exists and is not a number.
fn number(object: &Object, key: &str, fallback: i32) -> Result<i32, String> {
    match object.get(key) {
        Some(Value::Int(value)) => {
            i32::try_from(*value).map_err(|_| format!("`{key}` is too large"))
        }
        Some(Value::Float(value)) => {
            #[allow(clippy::cast_possible_truncation)]
            let value = *value as i32;
            Ok(value)
        }
        Some(_) => Err(format!("`{key}` is not a number")),
        None => Ok(fallback),
    }
}
