//! Spells: what the data says a spell is, before anything can cast one.
//!
//! Reference: `class spell_type` (`src/magic.h:191-380`) and its loader (`magic.cpp:1310-1520`). Almost every number
//! a spell carries is a `dbl_or_var` - either a constant or an arithmetic/dice expression - and only the constants
//! are read here. A spell whose number is an expression keeps `None`, because reading a dice expression as its mean
//! would be wrong in a way no amount of testing the loader could show.

use crate::materials::LoadError;
use cdda_json::{Object, Value};

/// One number that grows with the spell's level: where it starts, how much a level adds, and where it stops.
///
/// Reference: the `min_*` / `*_increment` / `max_*` triples in `spell_type`. `None` means the data wrote an
/// expression rather than a number.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Scaling {
    /// The value at level zero.
    pub start: Option<f64>,
    /// What one level adds.
    pub increment: Option<f64>,
    /// The value the scaling stops at.
    pub end: Option<f64>,
}

impl Scaling {
    /// What the number is at a level, when all three parts are plain numbers.
    ///
    /// Reference: `dbl_or_var::evaluate` in `spell::get_damage` and friends - the value climbs by the increment with
    /// the level and stops at the maximum. An expression anywhere in the triple answers `None` rather than a guess.
    #[must_use]
    pub fn at(&self, level: i32) -> Option<f64> {
        let (Some(start), Some(increment), Some(end)) = (self.start, self.increment, self.end) else {
            return None;
        };
        #[allow(clippy::cast_precision_loss)]
        let climbed = start + increment * f64::from(level);
        // The reference clamps towards the maximum when the increment is positive and towards it when negative;
        // `min`/`max` on the pair gets both without a branch.
        Some(if increment >= 0.0 {
            climbed.min(end)
        } else {
            climbed.max(end)
        })
    }
}

/// A spell as the data describes it.
#[derive(Debug, Clone, PartialEq)]
pub struct Spell {
    /// Its id.
    pub id: String,
    /// The name a player sees.
    pub name: String,
    /// Which effect it runs. This string is what the port dispatches on, the way the reference looks it up in
    /// `spell_effect`'s function table.
    pub effect_name: String,
    /// The effect's parameter, whose meaning depends on the effect.
    pub effect_str: String,
    /// The damage type it deals, or empty.
    pub damage_type: String,
    /// The area shape: `line`, `blast`, `cone` and so on.
    pub shape: String,
    /// How it is paid for: `MANA`, `BIONIC`, `HP`, `STAMINA` and so on.
    pub energy_source: String,
    /// Damage, as it grows with level.
    pub damage: Scaling,
    /// Range, as it grows with level.
    pub range: Scaling,
    /// Area of effect radius, as it grows with level.
    pub area: Scaling,
    /// Energy, as it grows with level: the base, the increment and the final value.
    pub energy: Scaling,
    /// Casting time in moves, as it grows with level.
    pub casting_time: Scaling,
    /// The difficulty of casting it.
    pub difficulty: Option<f64>,
    /// The highest level it can reach.
    pub max_level: Option<f64>,
}

/// Reads one spell from a dump entry.
///
/// # Errors
///
/// When the entry has no `id`, which is the only field without which nothing else can be looked up.
pub fn read_spell(object: &Object) -> Result<Spell, LoadError> {
    let Some(Value::Str(id)) = object.get("id") else {
        return Err(LoadError {
            message: "a spell with no id".to_owned(),
        });
    };
    let scaling = |start: &str, increment: &str, end: &str| Scaling {
        start: number(object, start),
        increment: number(object, increment),
        end: number(object, end),
    };
    Ok(Spell {
        id: id.clone(),
        name: text(object, "name"),
        effect_name: text(object, "effect_name"),
        effect_str: text(object, "effect_str"),
        damage_type: text(object, "damage_type"),
        shape: text(object, "shape"),
        energy_source: text(object, "energy_source"),
        damage: scaling("min_damage", "damage_increment", "max_damage"),
        range: scaling("min_range", "range_increment", "max_range"),
        area: scaling("min_aoe", "aoe_increment", "max_aoe"),
        energy: scaling("base_energy_cost", "energy_increment", "final_energy_cost"),
        casting_time: scaling(
            "base_casting_time",
            "casting_time_increment",
            "final_casting_time",
        ),
        difficulty: number(object, "difficulty"),
        max_level: number(object, "max_level"),
    })
}

/// Reads every spell in a dump.
///
/// # Errors
///
/// When an entry cannot be read.
pub fn load_dump(value: &Value) -> Result<Vec<Spell>, LoadError> {
    let Value::Object(root) = value else {
        return Err(LoadError {
            message: "a spell dump is an object".to_owned(),
        });
    };
    let Some(Value::Array(spells)) = root.get("spells") else {
        return Err(LoadError {
            message: "a spell dump has a spells array".to_owned(),
        });
    };
    let mut read = Vec::with_capacity(spells.len());
    for spell in spells {
        let Value::Object(object) = spell else {
            continue;
        };
        read.push(read_spell(object)?);
    }
    Ok(read)
}

/// A string field, or empty when it is missing.
fn text(object: &Object, key: &str) -> String {
    match object.get(key) {
        Some(Value::Str(value)) => value.clone(),
        _ => String::new(),
    }
}

/// A number field, or [`None`] when the data wrote an expression or nothing.
fn number(object: &Object, key: &str) -> Option<f64> {
    match object.get(key) {
        #[allow(clippy::cast_precision_loss)]
        Some(Value::Int(value)) => Some(*value as f64),
        Some(Value::Float(value)) => Some(*value),
        _ => None,
    }
}
