//! Traps: the smallest loader with every feature the bigger ones have.
//!
//! `trap::load` (`trap.cpp:45-100`) takes a mandatory id, name, one-character
//! symbol, colour, visibility, avoidance and difficulty, then optional flags,
//! radius, benign-ness and invisibility. It is the cheapest place to pin the
//! rule that a field read through `optional` starts at zero, and that a
//! mandatory field is inherited only when the object copies from something.

use cdda_core_types::colors::{NcColor, color_from_string, string_from_color};
use cdda_json::{Object, Value};
use std::collections::{BTreeMap, BTreeSet};

/// One resolved trap.
#[derive(Debug, Clone, PartialEq)]
pub struct Trap {
    /// Its id.
    pub id: String,
    /// The name a player sees, untranslated.
    pub name: String,
    /// The glyph.
    pub symbol: i32,
    /// The colour.
    pub color: NcColor,
    /// How easy the trap is to spot; smaller is easier.
    pub visibility: i32,
    /// How easy it is to avoid.
    pub avoidance: i32,
    /// How hard it is to disarm.
    pub difficulty: i32,
    /// How far the trap reaches.
    pub radius: i32,
    /// True for things that are not really traps, like funnels.
    pub benign: bool,
    /// True for traps that never show themselves.
    pub always_invisible: bool,
    /// Its flag set.
    pub flags: BTreeSet<String>,
    /// What the trap **does** when it is triggered, as the data names it - the reference's `action` (`trap.cpp:150-175`), which is `"eocs"` for a trap that runs programs
    /// and a named function such as `"bubble"` for one that does something of its own.
    pub action: Option<String>,
    /// The programs `action: "eocs"` runs, in order (`trap.cpp:168-175`).
    pub eocs: Vec<String>,
    /// What the trap says when the **player** steps on it (`trap.cpp:133`, `trap.h:258-266`).
    pub trigger_u: Option<String>,
    /// What the trap says when anybody else does, with `<npcname>` standing for their name.
    pub trigger_npc: Option<String>,
    /// The mapgen an `action: "map_regen"` trap rebuilds its tile with (`trap.cpp:154`, `map_regen`).
    pub map_regen: Option<String>,
    /// The spell an `action: "spell"` trap casts - the most common trap action in the data, thirty-four traps (`trap.h:165`, a `fake_spell`).
    pub spell: Option<TrapSpell>,
}

/// The spell a trap casts, and the level it casts it at.
///
/// Reference: `fake_spell` (`magic.h`, read by `spell_data::get_spell`, `magic.cpp:3571-3600`). The data writes `{"id": "summon_trapspider"}`, and a level when the trap
/// is meant to be stronger than the spell's first: `level` is a floor, `max_level` a ceiling, and the reference clamps one to the other before it sets the experience.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrapSpell {
    /// Which spell.
    pub id: String,
    /// The level the trap casts it at, zero when the data does not say.
    pub level: i32,
    /// The highest level it may reach, when the data names one.
    pub max_level: Option<i32>,
}

impl Trap {
    /// The name the reference prints for the trap's colour.
    #[must_use]
    pub fn color_name(&self) -> &'static str {
        string_from_color(self.color)
    }
}

/// Loads every trap in the given directories, in the reference's file order.
///
/// # Errors
///
/// Fails when a file cannot be read, a field is missing from a fresh object, or
/// a `copy-from` base is not defined.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<Vec<Trap>, LoadError> {
    let mut objects: BTreeMap<String, Object> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            collect(&value, &mut objects, &mut order);
        }
    }
    let mut out = Vec::new();
    for id in &order {
        let mut seen = Vec::new();
        out.push(resolve(id, &objects, &mut seen)?);
    }
    Ok(out)
}

/// Why a trap could not be loaded.
#[derive(Debug)]
pub struct LoadError {
    /// The problem.
    pub message: String,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LoadError {}

fn collect(value: &Value, out: &mut BTreeMap<String, Object>, order: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, out, order);
            }
        }
        Value::Object(object) => {
            if !matches!(object.get("type"), Some(Value::Str(text)) if text == "trap") {
                return;
            }
            let concrete = string(object, "id");
            let template = concrete.is_none() && object.get("abstract").is_some();
            let id = concrete.or_else(|| string(object, "abstract"));
            if let Some(id) = id {
                if !template && !out.contains_key(&id) {
                    order.push(id.clone());
                }
                out.insert(id, object.clone());
            }
        }
        _ => {}
    }
}

fn resolve(
    id: &str,
    objects: &BTreeMap<String, Object>,
    seen: &mut Vec<String>,
) -> Result<Trap, LoadError> {
    if seen.iter().any(|entry| entry == id) {
        return Err(LoadError {
            message: format!("cyclic copy-from through {id}: {seen:?}"),
        });
    }
    let Some(object) = objects.get(id) else {
        return Err(LoadError {
            message: format!("trap {id} is not defined"),
        });
    };
    seen.push(id.to_owned());
    let inherited = object.get("copy-from").is_some();
    let mut trap = match object.get("copy-from") {
        Some(Value::Str(base)) => {
            let mut base_trap = resolve(base, objects, seen)?;
            id.clone_into(&mut base_trap.id);
            base_trap
        }
        _ => Trap {
            id: id.to_owned(),
            name: String::new(),
            symbol: i32::from(b' '),
            color: color_from_string("white").unwrap_or_default(),
            visibility: 1,
            avoidance: 0,
            difficulty: 0,
            radius: 0,
            benign: false,
            always_invisible: false,
            flags: BTreeSet::new(),
            action: None,
            eocs: Vec::new(),
            trigger_u: None,
            trigger_npc: None,
            map_regen: None,
            spell: None,
        },
    };
    seen.pop();
    id.clone_into(&mut trap.id);
    apply(&mut trap, object, inherited)?;
    Ok(trap)
}

fn apply(trap: &mut Trap, object: &Object, inherited: bool) -> Result<(), LoadError> {
    let id = trap.id.clone();
    if let Some(name) = name_of(object) {
        trap.name = name;
    } else if !inherited {
        return Err(LoadError {
            message: format!("trap {id}: missing name"),
        });
    }
    // **What the trap does.** Reference: `trap::load` (`trap.cpp:150-175`) - `action` names the function, and `eocs` is only allowed beside `action: "eocs"`, which the
    // reference refuses to load without. The port keeps the refusal rather than reading the programs as if the action did not matter.
    // **The spell, and only beside its action.** Reference: `trap.cpp:160-167` throws for a `spell_data` without `action: "spell"`, so the port refuses the same pair, and
    // reads the two level members `fake_spell` clamps (`magic.h`, `magic.cpp:3571-3600`).
    if let Some(spell) = object.get("spell_data") {
        // Read from the **object**, not from the trap: the action is loaded a few lines below, and asking the trap here answers the default.
        let is_spell = matches!(object.get("action"), Some(Value::Str(name)) if name == "spell");
        if !is_spell {
            return Err(LoadError {
                message: format!("trap {id} has spell_data without action: spell"),
            });
        }
        let Value::Object(data) = spell else {
            return Err(LoadError {
                message: format!("trap {id} has a spell_data that is not an object"),
            });
        };
        let Some(Value::Str(spell_id)) = data.get("id") else {
            return Err(LoadError {
                message: format!("trap {id} names no spell id"),
            });
        };
        let whole = |member: &str| -> Result<Option<i32>, LoadError> {
            match data.get(member) {
                None => Ok(None),
                Some(Value::Int(number)) => {
                    i32::try_from(*number).map(Some).map_err(|_| LoadError {
                        message: format!("trap {id} has a {member} out of range"),
                    })
                }
                Some(_) => Err(LoadError {
                    message: format!("trap {id} has a {member} that is not a whole number"),
                }),
            }
        };
        trap.spell = Some(TrapSpell {
            id: spell_id.clone(),
            level: whole("level")?.unwrap_or(0),
            max_level: whole("max_level")?,
        });
    }
    // Reference: `optional( jo, was_loaded, "map_regen", map_regen, update_mapgen_none )` (`trap.cpp:154`) - one of the two actions the data names most often.
    if let Some(name) = string(object, "map_regen") {
        trap.map_regen = Some(name);
    }
    // Reference: `optional( jo, was_loaded, "trigger_message_u", trigger_message_u )` (`trap.cpp:133-134`) - both are optional, and a trap that has only one of them says
    // nothing at all, because `has_trigger_msg` requires the pair (`trap.h:258-260`).
    if let Some(text) = string(object, "trigger_message_u") {
        trap.trigger_u = Some(text);
    }
    if let Some(text) = string(object, "trigger_message_npc") {
        trap.trigger_npc = Some(text);
    }
    if let Some(Value::Str(action)) = object.get("action") {
        trap.action = Some(action.clone());
    }
    if let Some(Value::Array(programs)) = object.get("eocs") {
        if trap.action.as_deref() != Some("eocs") {
            return Err(LoadError {
                message: format!(r#"trap {id}: "eocs" without "action": "eocs""#),
            });
        }
        for program in programs {
            let Value::Str(name) = program else {
                continue;
            };
            trap.eocs.push(name.clone());
        }
    }
    if let Some(text) = string(object, "symbol") {
        let mut characters = text.chars();
        match (characters.next(), characters.next()) {
            (Some(character), None) => trap.symbol = character as i32,
            _ => {
                return Err(LoadError {
                    message: format!("trap {id}: symbol must be one character"),
                });
            }
        }
    } else if !inherited {
        return Err(LoadError {
            message: format!("trap {id}: missing symbol"),
        });
    }
    if let Some(text) = string(object, "color") {
        trap.color = color_from_string(&text).map_err(|error| LoadError {
            message: format!("trap {id}: color {text:?}: {error:?}"),
        })?;
    } else if !inherited {
        return Err(LoadError {
            message: format!("trap {id}: missing color"),
        });
    }
    for (member, slot) in [
        ("visibility", &mut trap.visibility),
        ("avoidance", &mut trap.avoidance),
        ("difficulty", &mut trap.difficulty),
    ] {
        match integer(object, member) {
            Some(value) => *slot = value,
            None if !inherited => {
                return Err(LoadError {
                    message: format!("trap {id}: missing {member}"),
                });
            }
            None => {}
        }
    }
    if let Some(radius) = integer(object, "trap_radius") {
        trap.radius = radius;
    } else if !inherited {
        trap.radius = 0;
    }
    if let Some(benign) = boolean(object, "benign") {
        trap.benign = benign;
    } else if !inherited {
        trap.benign = false;
    }
    if let Some(invisible) = boolean(object, "always_invisible") {
        trap.always_invisible = invisible;
    } else if !inherited {
        trap.always_invisible = false;
    }
    if object.get("flags").is_some() {
        trap.flags.clear();
        for flag in strings(object, "flags") {
            trap.flags.insert(flag);
        }
    }
    Ok(())
}

fn name_of(object: &Object) -> Option<String> {
    match object.get("name") {
        Some(Value::Str(text)) => Some(text.clone()),
        Some(Value::Object(inner)) => string(inner, "str"),
        _ => None,
    }
}

fn string(object: &Object, member: &str) -> Option<String> {
    match object.get(member) {
        Some(Value::Str(text)) => Some(text.clone()),
        _ => None,
    }
}

fn integer(object: &Object, member: &str) -> Option<i32> {
    match object.get(member) {
        Some(Value::Int(value)) => i32::try_from(*value).ok(),
        _ => None,
    }
}

fn boolean(object: &Object, member: &str) -> Option<bool> {
    match object.get(member) {
        Some(Value::Bool(value)) => Some(*value),
        _ => None,
    }
}

fn strings(object: &Object, member: &str) -> Vec<String> {
    match object.get(member) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}
