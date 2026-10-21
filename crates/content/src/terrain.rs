//! Terrain prototypes: what the ground is made of, and what burns.
//!
//! Reference: `ter_t` and its flags (`src/mapdata.h:634-637`). Flammability is not a stored field:
//! the reference derives it from three flags, and so does the port, because a stored copy could
//! disagree with the flags it came from.
//!
//! Gated on `--dump-terrain`: **1,769** terrains, of which **643** are flammable.

use cdda_json::Value;

/// One piece of terrain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terrain {
    /// Its id, e.g. `t_door_c`.
    pub id: String,
    /// How much movement it costs to enter; zero means impassable.
    pub move_cost: i32,
    /// Its flags, as the data writes them.
    pub flags: Vec<String>,
}

impl Terrain {
    /// Whether fire can consume this terrain.
    ///
    /// Reference: `mapdata_common_t::is_flammable` (`src/mapdata.h:634-637`), which is any of
    /// `FLAMMABLE`, `FLAMMABLE_ASH` or `FLAMMABLE_HARD`.
    #[must_use]
    pub fn is_flammable(&self) -> bool {
        self.flags.iter().any(|flag| {
            matches!(
                flag.as_str(),
                "FLAMMABLE" | "FLAMMABLE_ASH" | "FLAMMABLE_HARD"
            )
        })
    }

    /// Whether the terrain blocks movement entirely.
    #[must_use]
    pub fn is_impassable(&self) -> bool {
        self.move_cost == 0
    }
}

/// Reads the terrain dump.
///
/// # Errors
///
/// Returns the reason the dump could not be read: a shape it does not understand, or a terrain with
/// no id.
pub fn from_dump(value: &Value) -> Result<Vec<Terrain>, String> {
    from_dump_by_kind(value, "terrain")
}

/// Reads only the furniture entries of the terrain dump.
///
/// # Errors
///
/// The same reasons as [`from_dump`].
pub fn furniture_from_dump(value: &Value) -> Result<Vec<Terrain>, String> {
    from_dump_by_kind(value, "furniture")
}

/// Reads the entries of one kind from the terrain dump.
///
/// The dump holds both terrains and furniture in one list, each tagged with a `kind`; the reference
/// keeps them in two registries, so they are split here rather than being counted together.
///
/// # Errors
///
/// Returns the reason the dump could not be read.
pub fn from_dump_by_kind(value: &Value, kind: &str) -> Result<Vec<Terrain>, String> {
    let Value::Object(root) = value else {
        return Err("the dump's root is not an object".to_string());
    };
    let Some(Value::Array(entries)) = root.get("prototypes") else {
        return Err("the dump has no prototypes array".to_string());
    };
    let mut terrains = Vec::with_capacity(entries.len());
    for entry in entries {
        let Value::Object(object) = entry else {
            return Err("a terrain is not an object".to_string());
        };
        if matches!(object.get("kind"), Some(Value::Str(entry_kind)) if entry_kind != kind) {
            continue;
        }

        let Some(Value::Str(id)) = object.get("id") else {
            return Err("a terrain has no id".to_string());
        };
        let move_cost = match object.get("move_cost") {
            Some(Value::Int(cost)) => i32::try_from(*cost).unwrap_or(0),
            _ => 0,
        };
        let mut flags = Vec::new();
        if let Some(Value::Array(list)) = object.get("flags") {
            for flag in list {
                let Value::Str(text) = flag else {
                    return Err(format!("a flag of {id} is not a string"));
                };
                flags.push(text.clone());
            }
        }
        terrains.push(Terrain {
            id: id.clone(),
            move_cost,
            flags,
        });
    }
    Ok(terrains)
}

/// One tile's terrain, as the map keeps it.
#[must_use]
pub fn find<'a>(terrains: &'a [Terrain], id: &str) -> Option<&'a Terrain> {
    terrains.iter().find(|terrain| terrain.id == id)
}
