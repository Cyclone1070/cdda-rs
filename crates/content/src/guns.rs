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
    /// The weapon's own blow before its ammo: `item::gun_damage()`'s total, which is **zero for almost every firearm** because a gun's damage is its cartridge's.
    pub damage: f64,
    /// The same blow by kind, which is how the data states it: a bow's is stab, a launcher's is bash.
    pub damage_bash: i32,
    /// Cut damage, which a blade launcher states.
    pub damage_cut: i32,
    /// Stab damage, which every bow states.
    pub damage_stab: i32,
    /// Bullet damage, which the few guns that carry their own state.
    pub damage_bullet: i32,
    /// How many rounds one shot spends: `item::ammo_required()` (`item.cpp:11535-11550`), which is the type's `ammo_to_fire` before gunmods.
    pub ammo_to_fire: i32,
    /// How steady the weapon is in the hands: `gun_type::handling`, in the data's own whole units, which `item::gun_recoil` divides by ten.
    pub handling: i32,
    /// The sights' own error: `gun_type::sight_dispersion`, which `item::sight_dispersion` hands to `Character::effective_dispersion` and the shooter's eyes make worse.
    pub sight_dispersion: i32,
    /// How loud the weapon itself is: `gun_type::loudness`, which `item::gun_noise` adds to the round's own.
    pub loudness: i32,
    /// How many shots one trigger pull makes: the `qty` of the gun's `DEFAULT` firing mode (`islot_gun::modes`, `itype.h:826`; `item::burst_size`, `item.cpp:11640-11650`). **One for a
    /// single-shot weapon**, and the data goes as high as fifty.
    pub burst: i32,
    /// How well made the weapon is: `gun_type::durability`, which `Character::handle_gun_damage` reads as `one_in( ( 2 << durability ) * 3 )` for a mechanical malfunction
    /// (`ranged.cpp:790`). **The data's range is narrow and the chance moves by factors of two across it**, which is why it is a column rather than a constant.
    pub durability: i32,
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
    gun.damage_bash = number(object, "damage_bash", gun.damage_bash)?;
    gun.damage_cut = number(object, "damage_cut", gun.damage_cut)?;
    gun.damage_stab = number(object, "damage_stab", gun.damage_stab)?;
    gun.damage_bullet = number(object, "damage_bullet", gun.damage_bullet)?;
    gun.ammo_to_fire = number(object, "ammo_to_fire", gun.ammo_to_fire)?;
    gun.handling = number(object, "handling", gun.handling)?;
    gun.sight_dispersion = number(object, "sight_dispersion", gun.sight_dispersion)?;
    gun.loudness = number(object, "loudness", gun.loudness)?;
    gun.burst = number(object, "burst", gun.burst)?;
    gun.durability = number(object, "durability", gun.durability)?;
    if let Some(Value::Float(damage)) = object.get("damage") {
        gun.damage = *damage;
    } else if let Some(Value::Int(damage)) = object.get("damage") {
        #[allow(clippy::cast_precision_loss)]
        let damage = *damage as f64;
        gun.damage = damage;
    }
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

/// **How far a gun carries** - the reference's `item::gun_range( bool with_ammo )` (`item.cpp:11282-11300`).
///
/// The arithmetic is: the gun's own range, plus every gunmod's, all multiplied by the gunmods' range multipliers; then, when the gun has ammunition, the round's own range is **added**
/// and its multiplier multiplied in; the result is clamped to `[0, cap]`.
///
/// **This is where a firearm's range lives in this baseline.** The guns' own `range` is zero for almost every firearm - an M249 states none - and the cartridge states it: 36 for a
/// .223, 14 for a 9mm, 18 for 00 shot, 13 for a .22 long rifle. A port that read only the gun would believe every firearm had a range of nothing.
#[must_use]
pub fn gun_range(base: i32, mods: &[RangeBonus], ammo: Option<RangeBonus>, cap: i32) -> i32 {
    let mut total = base;
    let mut multiplier = 1.0_f32;
    for each in mods {
        total += each.range;
        multiplier *= each.multiplier;
    }
    if let Some(round) = ammo {
        total += round.range;
        multiplier *= round.multiplier;
    }
    let scaled = total.saturating_add(range_share(total, multiplier));
    scaled.clamp(0, cap)
}

/// The floating-point share of the multiplier, kept as its own step so the integer cast is one place with one reason.
fn range_share(total: i32, multiplier: f32) -> i32 {
    // The reference multiplies in `float` and truncates; a range that overflows an `i32` cannot be reached by any data this port carries, and the clamp above bounds the answer.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let share = (total as f32 * multiplier) as i32 - total;
    share
}

/// **A range bonus**: a gunmod's, or a round's own - `gunmod::range` and `gunmod::range_multiplier`, `ammo::range` and `ammo::range_multiplier`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RangeBonus {
    /// How many squares it adds.
    pub range: i32,
    /// What it multiplies the whole by.
    pub multiplier: f32,
}
