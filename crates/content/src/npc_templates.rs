//! NPC templates: the people the data writes by hand, who a mapgen or a mission places.
//!
//! Reference: `npc_template` (`npc.h`), loaded by `npc_template::load` and copied onto a person by `npc::load_npc_template` (`npc.cpp:520-568`). A template names the **class** that
//! shapes the person (`npc_class`), the attitude and mission they start with, their first chat topic, their faction, and - for the ones the story needs - a fixed name.
//!
//! The port loads what its rules can act on: the class, the name pieces, the gender, the attitude and mission **as the enumerators' own names**, the chat topic, the faction and
//! the mission the person offers. The template's `age`, `height`, `personality` and starting stats belong to the character-creation rules the port has not ported, and are left
//! unread rather than guessed.

use std::collections::BTreeMap;

use cdda_json::Value;

use crate::monsters::LoadError;

/// One NPC template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcTemplate {
    /// Its id, as the data writes it.
    pub id: String,
    /// The class that shapes the person, empty when the data names none.
    pub class: String,
    /// The name the data fixes, empty for a person whose name is rolled.
    pub name_unique: String,
    /// The suffix the player sees after the name ("Bone Seer"), empty when the data gives none.
    pub name_suffix: String,
    /// `"male"`, `"female"`, or empty for a rolled gender.
    pub gender: String,
    /// The attitude they start with, as the `npc_attitude` enumerator's own name (`npc.h:103-124`).
    pub attitude: String,
    /// The mission they start on, as the `npc_mission` enumerator's own name (`npc.h:169-183`).
    pub mission: String,
    /// Their first chat topic.
    pub chat: String,
    /// Their faction, which the port has no factions for but keeps for the mission rules that will need one.
    pub faction: String,
    /// The mission this person offers, empty when they offer none.
    pub mission_offered: String,
}

/// Loads every NPC template under `dirs`.
///
/// # Errors
///
/// Reports a file that cannot be read or a template without an id.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<Vec<NpcTemplate>, LoadError> {
    let mut by_id: BTreeMap<String, NpcTemplate> = BTreeMap::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            let Value::Array(objects) = value.as_ref() else {
                continue;
            };
            for object in objects {
                let Some(template) = read_template(object) else {
                    continue;
                };
                by_id.insert(template.id.clone(), template);
            }
        }
    }
    Ok(by_id.into_values().collect())
}

fn read_template(object: &Value) -> Option<NpcTemplate> {
    let Value::Object(fields) = object else {
        return None;
    };
    match fields.get("type") {
        Some(Value::Str(kind)) if kind == "npc" => {}
        _ => return None,
    }
    let Some(Value::Str(id)) = fields.get("id") else {
        return None;
    };
    let text = |member: &str| match fields.get(member) {
        Some(Value::Str(found)) => found.clone(),
        _ => String::new(),
    };
    let number = |member: &str| match fields.get(member) {
        Some(Value::Int(found)) => i32::try_from(*found).unwrap_or(0),
        _ => 0,
    };
    Some(NpcTemplate {
        id: id.clone(),
        class: text("class"),
        name_unique: text("name_unique"),
        name_suffix: text("name_suffix"),
        gender: text("gender"),
        attitude: attitude_named(number("attitude")),
        mission: mission_named(number("mission")),
        chat: text("chat"),
        faction: text("faction"),
        mission_offered: text("mission_offered"),
    })
}

/// The attitude an enumerator number names, as the reference spells it.
///
/// Reference: `enum npc_attitude : int` (`npc.h:103-124`). The `LEGACY_*` slots are **real enumerators with no behaviour**, kept because the numbers after them are fixed by the
/// data; the port names them `"legacy_N"` rather than pretending they mean something.
#[must_use]
pub fn attitude_named(number: i32) -> String {
    match number {
        1 => "talk",
        2 => "legacy_1",
        3 => "follow",
        4 => "legacy_2",
        5 => "lead",
        6 => "wait",
        7 => "legacy_6",
        8 => "mug",
        9 => "wait_for_leave",
        10 => "kill",
        11 => "flee",
        12 => "legacy_3",
        13 => "heal",
        14 => "legacy_4",
        15 => "legacy_5",
        16 => "activity",
        17 => "flee_temp",
        18 => "recover_goods",
        // Zero and anything the data invents are the reference's `NPCATT_NULL`, the attitude that means "no particular way".
        _ => "null",
    }
    .to_owned()
}

/// The mission an enumerator number names.
///
/// Reference: `enum npc_mission : int` (`npc.h:169-183`).
#[must_use]
pub fn mission_named(number: i32) -> String {
    match number {
        1 => "legacy_1",
        2 => "shelter",
        3 => "shopkeep",
        4 => "legacy_2",
        5 => "legacy_3",
        6 => "guard_ally",
        7 => "guard",
        8 => "guard_patrol",
        9 => "activity",
        10 => "travelling",
        // Zero and anything the data invents are `NPC_MISSION_NULL`.
        _ => "null",
    }
    .to_owned()
}
