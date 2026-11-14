//! Vehicle parts and vehicle prototypes.
//!
//! Reference: `vehicles::parts::get_all` and `vehicles::get_all_prototypes`
//! (`src/veh_type.h:52,45`), reached through `--dump-vehicles`. Ids only for now: a part also
//! carries its durability, its requirements and what it can be installed on, and those come next.

use cdda_json::{Object, Value};

/// Every vehicle part and vehicle prototype the reference defines.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Vehicles {
    /// The parts, sorted by id.
    pub parts: Vec<PartSpec>,
    /// The prototypes, sorted by id.
    pub prototypes: Vec<Prototype>,
}

/// A vehicle a prototype builds: its name and the parts it is made of, each in its own square.
///
/// Reference: `vehicle_prototype` (`src/veh_type.h:487-511`) - `id`, `name`, and `parts`, a list of `part_def` placements. A square may hold several parts: a frame carries
/// a seat, a seatbelt and controls, which is what a vehicle is rather than a violation of one-part-per-square.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Prototype {
    /// Its id.
    pub id: String,
    /// The name a player sees, untranslated.
    pub name: String,
    /// The parts it is built from, in the data's order.
    pub parts: Vec<Placement>,
}

/// One part a prototype puts in one square.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Placement {
    /// The part's id, which must be a part the store holds.
    pub part: String,
    /// Its offset from the vehicle's origin, in squares.
    pub x: i32,
    /// Its offset from the vehicle's origin, in squares.
    pub y: i32,
    /// The cosmetic variant the data names, or empty.
    pub variant: String,
    /// How much ammunition it starts with, when the data says.
    pub with_ammo: i32,
}

/// What one vehicle part **is**.
///
/// Reference: `vpart_info` (`src/veh_type.h:353-400`) as `--dump-vehicles` reports it: the item the part is made of (`base_item`), the slot it occupies (`location`), the
/// damage it takes before it breaks (`durability`), the damage it deals in a collision (`dmg_mod`), the string flags it carries and its flat damage reduction. The
/// invented turret parts carry the gun they mount as their `item`, which is the only trace in the store of where they came from.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PartSpec {
    /// Its id.
    pub id: String,
    /// The name a player sees, untranslated.
    pub name: String,
    /// How much damage it takes before it breaks.
    pub durability: i32,
    /// What percentage of its own damage it deals in a collision (`dmg_mod`, an integer percentage).
    pub damage_modifier: i32,
    /// What this part makes, in watts, or draws when negative - the reference's `vpart_info::power` (`src/veh_type.h`), which is a `units::power` and only means
    /// something for an engine or an alternator.
    pub power: i32,
    /// What this wheel resists rolling with, and `None` for a part that is not a wheel.
    ///
    /// Reference: `vpart_info::wheel_info::rolling_resistance` (`src/veh_type.h:142`), a float that defaults to 1 - a bicycle wheel is 0.45. Only a part with
    /// wheel info has one, which is why this is `Option` and not a defaulted number: a frame `0.0` would look like a frictionless wheel.
    pub rolling_resistance: Option<f32>,
    /// The item it is made of.
    pub item: String,
    /// The slot of the vehicle tile it occupies.
    pub location: String,
    /// How much cargo it holds, in millilitres.
    pub size: i64,
    /// Its string flags.
    pub flags: std::collections::BTreeSet<String>,
    /// Flat damage it shaves off each kind, by damage type id.
    pub damage_reduction: std::collections::BTreeMap<String, f32>,
    /// The fuel a part that burns fuel needs, as the dump writes it - `null` for everything that does not burn fuel. This is an **engine's** fuel, not a tank's contents.
    pub fuel_type: String,
    /// Whether the part can hold fuel or ammunition at all - the reference's `is_fuel_store`, which asks the base item whether it is a watertight container or a magazine.
    pub fuel_store: bool,
    /// Whether the part is a battery, which `fuel_left` sums through its own branch.
    pub battery: bool,
    /// What its cargo weighs, as a percentage of the items' own weight - `vpart_info::cargo_weight_modifier` (`veh_type.h:452`), 100 when the data says nothing, which
    /// every part in the current data does. `calc_mass_center` scales a part's contents by it (`vehicle.cpp:8504-8506`), so a part that is 50% "weightless" cargo halves what
    /// it adds.
    pub cargo_weight_modifier: i64,
    /// The fuels an engine can burn, **in the order it tries them** - the reference's `engine_info->fuel_opts` (`veh_type.h:132`), which `finalize` fills from
    /// `fuel_type` when the JSON names none (`veh_type.cpp:650-652`). Empty for everything that is not an engine.
    pub fuel_opts: Vec<String>,
}

/// The flags that take a gun out of the running for a turret part.
///
/// Reference: `mountable_gun_filter` (`src/veh_type.cpp:473-491`). A bionic weapon is part of a person, `NO_TURRET` says so outright, something that cannot be
/// unwielded cannot be mounted, a pseudo item is not a real thing, a reload-and-shoot weapon has no magazine to feed a turret, and a strength-drawn bow needs
/// its archer.
const NOT_MOUNTABLE: [&str; 6] = [
    "BIONIC_WEAPON",
    "NO_TURRET",
    "NO_UNWIELD",
    "PSEUDO",
    "RELOAD_AND_SHOOT",
    "STR_DRAW",
];

/// The turret parts the reference invents for the guns that could be mounted.
///
/// Reference: `vehicles::parts::finalize` (`src/veh_type.cpp:565-640`). This is why a port that only reads the `vehicle_part` entries has 462 parts where the
/// reference has 701: the missing 239 are never written down, they are made from the guns.
#[must_use]
pub fn turret_parts(items: &[crate::items::ItemType]) -> Vec<String> {
    let mut parts: Vec<String> = items
        .iter()
        .filter(|item| item.is_gun && !item.is_gunmod)
        .filter(|item| !NOT_MOUNTABLE.iter().any(|flag| item.flags.contains(*flag)))
        .map(|item| format!("turret_{}", item.id))
        .collect();
    parts.sort();
    parts
}


/// Reads one prototype object from the dump.
fn prototype_of(entry: &Value) -> Result<Prototype, String> {
    let Value::Object(object) = entry else {
        return Err("a prototype is not an object".to_owned());
    };
    let text = |key: &str| match object.get(key) {
        Some(Value::Str(value)) => Ok(value.clone()),
        _ => Err(format!("a prototype has no {key}")),
    };
    let mut parts = Vec::new();
    match object.get("parts") {
        Some(Value::Array(items)) => {
            for item in items {
                let Value::Object(placement) = item else {
                    return Err("a placement is not an object".to_owned());
                };
                let field = |key: &str| match placement.get(key) {
                    Some(Value::Str(value)) => Ok(value.clone()),
                    _ => Err(format!("a placement has no {key}")),
                };
                let number = |key: &str| match placement.get(key) {
                    Some(Value::Int(value)) => Ok(*value),
                    _ => Err(format!("a placement's {key} is not a number")),
                };
                parts.push(Placement {
                    part: field("part")?,
                    x: i32::try_from(number("x")?).unwrap_or(0),
                    y: i32::try_from(number("y")?).unwrap_or(0),
                    variant: field("variant")?,
                    with_ammo: i32::try_from(number("with_ammo")?).unwrap_or(0),
                });
            }
        }
        _ => return Err("a prototype has no parts".to_owned()),
    }
    Ok(Prototype {
        id: text("id")?,
        name: text("name")?,
        parts,
    })
}

/// Reads one part object from the dump.
#[allow(clippy::cast_precision_loss)]
fn part_of(entry: &Value) -> Result<PartSpec, String> {
    let Value::Object(object) = entry else {
        return Err("a part is not an object".to_owned());
    };
    let text = |key: &str| match object.get(key) {
        Some(Value::Str(value)) => Ok(value.clone()),
        _ => Err(format!("a part has no {key}")),
    };
    let number = |key: &str| match object.get(key) {
        Some(Value::Int(value)) => Ok(*value),
        _ => Err(format!("a part's {key} is not a number")),
    };
    let rolling_resistance = match object.get("rolling_resistance") {
        Some(Value::Float(value)) => {
            #[allow(clippy::cast_possible_truncation)]
            Some(*value as f32)
        }
        Some(Value::Int(value)) => {
            #[allow(clippy::cast_precision_loss)]
            Some(*value as f32)
        }
        Some(Value::Null) | None => None,
        _ => return Err("a part's rolling_resistance is not a number".to_owned()),
    };
    let mut fuel_opts = Vec::new();
    match object.get("fuel_opts") {
        Some(Value::Array(items)) => {
            for item in items {
                match item {
                    Value::Str(fuel) => fuel_opts.push(fuel.clone()),
                    _ => return Err("a part's fuel option is not a string".to_owned()),
                }
            }
        }
        _ => return Err("a part has no fuel_opts".to_owned()),
    }
    let mut flags = std::collections::BTreeSet::new();
    match object.get("flags") {
        Some(Value::Array(items)) => {
            for item in items {
                match item {
                    Value::Str(flag) => {
                        flags.insert(flag.clone());
                    }
                    _ => return Err("a part flag is not a string".to_owned()),
                }
            }
        }
        _ => return Err("a part has no flags".to_owned()),
    }
    let mut damage_reduction = std::collections::BTreeMap::new();
    match object.get("damage_reduction") {
        Some(Value::Object(kinds)) => {
            for (kind, amount) in kinds.members() {
                let amount = match amount {
                    Value::Float(value) => *value,
                    Value::Int(value) => *value as f64,
                    _ => return Err("a part's damage reduction is not a number".to_owned()),
                };
                #[allow(clippy::cast_possible_truncation)]
                damage_reduction.insert(kind.clone(), amount as f32);
            }
        }
        _ => return Err("a part has no damage_reduction".to_owned()),
    }
    Ok(PartSpec {
        id: text("id")?,
        name: text("name")?,
        durability: i32::try_from(number("durability")?).unwrap_or(0),
        damage_modifier: i32::try_from(number("damage_modifier")?).unwrap_or(100),
        rolling_resistance,
        power: i32::try_from(number("power")?).unwrap_or(0),
        item: text("item")?,
        location: text("location")?,
        size: number("size")?,
        flags,
        damage_reduction,
        cargo_weight_modifier: number("cargo_weight_modifier")?,
        fuel_type: text("fuel_type")?,
        fuel_store: number("fuel_store")? != 0,
        battery: number("battery")? != 0,
        fuel_opts,
    })
}

fn count_of(object: &Object, key: &str, listed: usize) -> Result<(), String> {
    let Some(Value::Int(count)) = object.get(key) else {
        return Err(format!("the dump has no {key}"));
    };
    if usize::try_from(*count).ok() != Some(listed) {
        return Err(format!("{key} disagrees with the ids listed"));
    }
    Ok(())
}

/// Reads the vehicle dump.
///
/// The counts are checked against the arrays, as with the starting kit: a dump that contradicts
/// itself is an error, not something to paper over.
///
/// # Errors
///
/// Returns the reason the dump could not be read, or that its counts disagree with its contents.
pub fn from_dump(value: &Value) -> Result<Vehicles, String> {
    let Value::Object(root) = value else {
        return Err("the dump's root is not an object".to_string());
    };
    // The dump's parts are objects since version 2: an id list could not answer a question about a vehicle.
    let parts = match root.get("parts") {
        Some(Value::Array(entries)) => entries.iter().map(part_of).collect::<Result<Vec<_>, String>>()?,
        _ => return Err("the dump has no parts array".to_string()),
    };
    let prototypes = match root.get("prototypes") {
        Some(Value::Array(entries)) => entries
            .iter()
            .map(prototype_of)
            .collect::<Result<Vec<_>, String>>()?,
        _ => return Err("the dump has no prototypes array".to_string()),
    };
    count_of(root, "part_count", parts.len())?;
    count_of(root, "prototype_count", prototypes.len())?;
    Ok(Vehicles { parts, prototypes })
}
