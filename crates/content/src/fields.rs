//! Field prototypes: what each kind of field looks like at each intensity.
//!
//! Reference: `field_type` (`src/field_type.h`), reached through `--dump-fields`. A field's
//! number of levels is also its maximum intensity, which is what the runtime clamps to - so this
//! loader is what makes "intensity 3" mean something.

use cdda_core_types::rng::Rolls;
use cdda_json::{Object, Value};

/// One intensity of a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLevel {
    /// The displayed name.
    pub name: String,
    /// The ASCII glyph drawn for this intensity.
    pub sym: u32,
    /// The colour id drawn for this intensity.
    pub color: String,
    /// Whether standing in it is hazardous.
    pub dangerous: bool,
    /// Whether light passes through this intensity.
    pub transparent: bool,
    /// Extra movement cost to enter.
    pub move_cost: i32,
    /// The upgrade chance and period for this intensity, if it can grow on its own.
    pub upgrade_chance: i32,
    /// How often the upgrade chance is rolled, in turns.
    pub upgrade_duration: i64,
    /// The chance in a hundred that this intensity spawns a monster group.
    pub spawn_chance: i32,
    /// How many spawns to attempt.
    pub spawn_count: i32,
    /// How far from the field they appear.
    pub spawn_radius: i32,
    /// The monster group to draw from, empty when it spawns nothing.
    pub spawn_group: String,
}

/// One kind of field, with its levels in order (index 0 is intensity 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldType {
    /// The field's id, e.g. `fd_smoke`.
    pub id: String,
    /// Which field draws when several share a tile; higher wins.
    pub priority: i32,
    /// How much faster the field ages away under water, in turns.
    pub underwater_age_speedup: i64,
    /// How much faster it ages away outdoors, in turns.
    pub outdoor_age_speedup: i64,
    /// The chance in a hundred that this gas spreads to a neighbour each turn.
    pub percent_spread: i32,
    /// The intensities, weakest first.
    pub levels: Vec<FieldLevel>,
}

impl FieldType {
    /// The highest intensity this field can reach.
    #[must_use]
    pub fn max_intensity(&self) -> u8 {
        u8::try_from(self.levels.len()).unwrap_or(u8::MAX)
    }
}

fn int_of(object: &Object, key: &str) -> Option<i64> {
    match object.get(key)? {
        Value::Int(value) => Some(*value),
        _ => None,
    }
}

fn bool_of(object: &Object, key: &str) -> Option<bool> {
    match object.get(key)? {
        Value::Bool(value) => Some(*value),
        _ => None,
    }
}

fn str_of(object: &Object, key: &str) -> Option<String> {
    match object.get(key)? {
        Value::Str(value) => Some(value.clone()),
        _ => None,
    }
}

/// Reads the field dump.
///
/// # Errors
///
/// Returns the reason the dump could not be read: a shape it does not understand, a field with no
/// levels, or a level missing a piece the runtime needs.
pub fn from_dump(value: &Value) -> Result<Vec<FieldType>, String> {
    let Value::Object(root) = value else {
        return Err("the dump's root is not an object".to_string());
    };
    let Some(Value::Array(entries)) = root.get("prototypes") else {
        return Err("the dump has no prototypes array".to_string());
    };
    let mut fields = Vec::with_capacity(entries.len());
    for entry in entries {
        let Value::Object(object) = entry else {
            return Err("a field is not an object".to_string());
        };
        let Some(id) = str_of(object, "id") else {
            return Err("a field has no id".to_string());
        };
        let priority = int_of(object, "priority").unwrap_or(0);
        let underwater_age_speedup = int_of(object, "underwater_age_speedup").unwrap_or(0);
        let outdoor_age_speedup = int_of(object, "outdoor_age_speedup").unwrap_or(0);
        let percent_spread = int_of(object, "percent_spread").unwrap_or(0);
        let Some(Value::Array(levels)) = object.get("levels") else {
            return Err(format!("{id} has no levels"));
        };
        if levels.is_empty() {
            return Err(format!("{id} has no levels at all"));
        }
        let mut parsed = Vec::with_capacity(levels.len());
        for level in levels {
            let Value::Object(level) = level else {
                return Err(format!("a level of {id} is not an object"));
            };
            let sym = level.get("sym").and_then(|v| match v {
                Value::Int(code) => u32::try_from(*code).ok(),
                _ => None,
            });
            let Some(sym) = sym else {
                return Err(format!("a level of {id} has no glyph"));
            };
            parsed.push(FieldLevel {
                name: str_of(level, "name").unwrap_or_default(),
                sym,
                color: str_of(level, "color").unwrap_or_default(),
                dangerous: bool_of(level, "dangerous").unwrap_or(false),
                transparent: bool_of(level, "transparent").unwrap_or(true),
                move_cost: i32::try_from(int_of(level, "move_cost").unwrap_or(0)).unwrap_or(0),
                upgrade_chance: i32::try_from(int_of(level, "upgrade_chance").unwrap_or(0))
                    .unwrap_or(0),
                upgrade_duration: int_of(level, "upgrade_duration").unwrap_or(0),
                spawn_chance: i32::try_from(int_of(level, "spawn_chance").unwrap_or(0))
                    .unwrap_or(0),
                spawn_count: i32::try_from(int_of(level, "spawn_count").unwrap_or(0)).unwrap_or(0),
                spawn_radius: i32::try_from(int_of(level, "spawn_radius").unwrap_or(0))
                    .unwrap_or(0),
                spawn_group: str_of(level, "spawn_group").unwrap_or_default(),
            });
        }
        fields.push(FieldType {
            id,
            priority: i32::try_from(priority).unwrap_or(0),
            underwater_age_speedup,
            outdoor_age_speedup,
            percent_spread: i32::try_from(percent_spread).unwrap_or(0),
            levels: parsed,
        });
    }
    Ok(fields)
}

/// Finds a field by id.
#[must_use]
pub fn find<'a>(fields: &'a [FieldType], id: &str) -> Option<&'a FieldType> {
    fields.iter().find(|field| field.id == id)
}

/// The field ids the runtime uses, built from the dump.
///
/// An id is the field's position in the dump plus one, so zero stays free for the null field that
/// `add_field` refuses. The numbering is the port's own: the reference keys fields by string, and
/// a number is what lets a tile store one in a byte.
#[derive(Debug, Clone, Default)]
pub struct FieldRegistry {
    ids: std::collections::HashMap<String, u32>,
}

impl FieldRegistry {
    /// Builds the registry from a loaded field list.
    #[must_use]
    pub fn new(fields: &[FieldType]) -> Self {
        let mut ids = std::collections::HashMap::with_capacity(fields.len());
        for (index, field) in fields.iter().enumerate() {
            let Ok(id) = u32::try_from(index + 1) else {
                continue;
            };
            ids.insert(field.id.clone(), id);
        }
        Self { ids }
    }

    /// The id for a field name, if the reference defines it.
    #[must_use]
    pub fn id_of(&self, name: &str) -> Option<cdda_core_types::field::FieldId> {
        self.ids
            .get(name)
            .copied()
            .map(cdda_core_types::field::FieldId)
    }
}

/// What a monster-spawning field produces when its turn comes up.
///
/// Reference: `field_processor_monster_spawn` (`map_field.cpp:762-786`). The level's count and chance are tested **before any
/// dice are spent**, so a level with either at zero is free; then **one** `one_in(chance)` decides whether anything spawns at
/// all, and only then is the group asked, repeatedly, until the count is used up.
///
/// Returns the monsters' ids. The caller places them at random passable squares within the level's radius, which is the part of
/// the reference that needs a map rather than a table.
///
/// **Declared gap:** the reference asks `GetResultFromGroup` for a **batch** and passes its count in by reference, so a pack
/// entry can consume several points of the count and spawn several monsters at once. Packs are not ported, so a batch here is
/// always one monster.
pub fn spawn_from_field<'a>(
    spawn_chance: i32,
    spawn_count: i32,
    spawn_group: &str,
    groups: &'a [crate::monster_groups::MonsterGroup],
    rolls: &mut Rolls,
) -> Vec<&'a str> {
    let mut produced = Vec::new();
    if spawn_count <= 0 || spawn_chance <= 0 || spawn_group.is_empty() {
        return produced;
    }
    if !rolls.one_in(spawn_chance) {
        return produced;
    }
    let mut remaining = spawn_count;
    while remaining > 0 {
        match crate::monster_groups::pick_monster(spawn_group, groups, rolls) {
            Some(monster) => produced.push(monster),
            None => break,
        }
        remaining -= 1;
    }
    produced
}
