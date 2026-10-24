//! Constructions: what can be built, out of what, and how long it takes.
//!
//! `construction::load` (`construction.cpp:2180-2260`) reads a construction's group, category, the tile it applies to (`pre_terrain`), the tile it leaves
//! (`post_terrain`), its time, the skills it wants and the requirement group it draws its components from. The parts that are C function pointers in the reference -
//! `pre_special`, `post_special` and the rest - are named by string in the dump and not modelled here yet; the ledger says so.
//!
//! Time is the one field that needs explaining. The reference stores it in **moves**, and reads it from JSON with `to_moves<int>( read_from_json_string<time_duration> )`
//! (`construction.cpp:2216-2219`), which accepts either a number or a string like `"1 h 30 m"`. One turn is one second and one second is a hundred moves, so `"1 h"` is
//! 360,000 moves.

use cdda_json::{Object, Value};
use std::collections::BTreeMap;

/// One construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Construction {
    /// Its id, which is also what the requirement group is named after.
    pub id: String,
    /// The group its menu entry belongs to.
    pub group: String,
    /// The category it is listed under, `OTHER` when the data does not say.
    pub category: String,
    /// How long it takes, in moves.
    pub time: i64,
    /// The tiles it may be built on; empty means any.
    ///
    /// **Sorted and deduplicated**, because the reference holds it in a `std::set<std::string>` (`src/construction.h:51`): a dump of the reference's own list comes back in
    /// byte order, so `t_concrete_wall_P` sorts before `t_concrete_wall_b`.
    pub pre_terrain: Vec<String>,
    /// The tile it leaves behind.
    pub post_terrain: String,
    /// The skills it requires, and at what level.
    pub required_skills: BTreeMap<String, i32>,
    /// The requirement group its components, tools and qualities come from.
    pub requirements: String,
    /// What it consumes, as **groups of alternatives**: one item of one of each group satisfies it, in the quantity given. The third member is the reference's
    /// `"LIST"` marker: true when the name is another requirement group rather than an item.
    pub components: Vec<Vec<(String, i32, bool)>>,
    /// What it needs but does not consume, in the same shape. A negative count is a charge count rather than a number of items.
    pub tools: Vec<Vec<(String, i32, bool)>>,
    /// The tool qualities it needs, as `(quality, level, count)` in groups of alternatives.
    pub qualities: Vec<Vec<(String, i32, i32)>>,
    /// The other requirement groups it merges in, with their multiples (`using`).
    pub using: Vec<(String, i32)>,
    /// The note shown beside its requirements.
    pub pre_note: String,
    /// Whether its requirements are generated during finalisation, which is true only of starting a vehicle.
    pub vehicle_start: bool,
}

/// What a construction takes, with the groups it names in `using` merged in.
///
/// Reference: `finalize_construction` (`src/construction.cpp:2533-2537`), which folds every `using` group into the construction's own with
/// `requirement_data::operator+` and `operator*`: the groups are **appended**, each scaled by its multiple. A construction whose `using` names a group that is not defined
/// keeps only its own needs, which is what the reference does with a missing requirement.
#[must_use]
pub fn needs_of(
    construction: &Construction,
    groups: &std::collections::BTreeMap<String, crate::requirements::RequirementData>,
    subtypes: &std::collections::BTreeMap<String, Vec<String>>,
) -> crate::requirements::RequirementData {
    let mut needs = crate::requirements::RequirementData {
        components: construction.components.clone(),
        tools: construction.tools.clone(),
        qualities: construction.qualities.clone(),
    };
    for (id, multiple) in &construction.using {
        if let Some(group) = groups.get(id) {
            needs = needs.merged(&group.scaled(*multiple));
        }
    }
    // **The nested groups are expanded last**, because a `using` group may itself name another group (`inline_requirements`, `src/requirements.cpp:647-720`).
    needs.expand_nested(groups, subtypes)
}

/// Loads every construction in the given directories, in the reference's file order.
///
/// # Errors
///
/// Fails when a file cannot be read or a mandatory field is not a shape the reference accepts.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<Vec<Construction>, LoadError> {
    let mut out = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            collect(&value, &mut out, &mut seen, &path)?;
        }
    }
    Ok(out)
}

/// Why constructions could not be loaded.
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

fn collect(
    value: &Value,
    out: &mut Vec<Construction>,
    seen: &mut Vec<String>,
    path: &std::path::Path,
) -> Result<(), LoadError> {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, out, seen, path)?;
            }
        }
        Value::Object(object) => {
            if !matches!(object.get("type"), Some(Value::Str(text)) if text == "construction") {
                return Ok(());
            }
            // **An abstract is not a construction.** The reference keeps the two in different registries, and only concrete ids appear in the game.
            let Some(id) = string(object, "id") else {
                return Ok(());
            };
            if seen.iter().any(|entry| entry == &id) {
                return Err(LoadError {
                    message: format!("construction {id} is defined twice: {}", path.display()),
                });
            }
            seen.push(id.clone());
            out.push(read(object, &id));
        }
        _ => {}
    }
    Ok(())
}

fn read(object: &Object, id: &str) -> Construction {
    Construction {
        id: id.to_owned(),
        group: string(object, "group").unwrap_or_else(|| id.to_owned()),
        category: string(object, "category").unwrap_or_else(|| "OTHER".to_owned()),
        time: object.get("time").map_or(0, duration_in_moves),
        pre_terrain: {
            let mut tiles: Vec<String> = match object.get("pre_terrain") {
                Some(Value::Str(single)) => vec![single.clone()],
                Some(Value::Array(found)) => found
                    .iter()
                    .filter_map(|entry| match entry {
                        Value::Str(name) => Some(name.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            tiles.sort_unstable();
            tiles.dedup();
            tiles
        },
        post_terrain: string(object, "post_terrain").unwrap_or_default(),
        // **Two shapes for the same thing.** The reference reads `required_skills` as a list of pairs and, when that is absent, falls back to the legacy `skill` and
        // `difficulty` members with fabrication as the default skill (`construction.cpp:2203-2211`), so a construction with neither still wants fabrication at zero.
        required_skills: if object.get("required_skills").is_some() {
            pairs(object.get("required_skills"))
        } else {
            let mut skills = BTreeMap::new();
            let skill = string(object, "skill").unwrap_or_else(|| "fabrication".to_owned());
            let difficulty = match object.get("difficulty") {
                Some(Value::Int(level)) => i32::try_from(*level).unwrap_or(0),
                _ => 0,
            };
            skills.insert(skill, difficulty);
            skills
        },
        requirements: string(object, "requirements").unwrap_or_else(|| {
            // The reference names every construction's own requirement group after its id (`construction.cpp:2221`).
            format!("inline_construction_{id}")
        }),
        using: match object.get("using") {
            // **A bare string is one group with one multiple** (`src/construction.cpp:2226-2229`).
            Some(Value::Str(single)) => vec![(single.clone(), 1)],
            Some(Value::Array(entries)) => entries
                .iter()
                .filter_map(|entry| match entry {
                    Value::Array(pair) => Some((
                        pair.first().and_then(|name| match name {
                            Value::Str(name) => Some(name.clone()),
                            _ => None,
                        })?,
                        pair.get(1)
                            .and_then(|level| match level {
                                Value::Int(level) => i32::try_from(*level).ok(),
                                _ => None,
                            })
                            .unwrap_or(1),
                    )),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        },
        components: crate::requirements::pair_groups(object.get("components")),
        tools: crate::requirements::pair_groups(object.get("tools")),
        qualities: crate::requirements::quality_groups(object.get("qualities")),
        pre_note: string(object, "pre_note").unwrap_or_default(),
        vehicle_start: matches!(object.get("vehicle_start"), Some(Value::Bool(true))),
    }
}

/// A list of `[ id, level ]` pairs, which the reference reads into a map.
fn pairs(value: Option<&Value>) -> BTreeMap<String, i32> {
    let mut out = BTreeMap::new();
    if let Some(Value::Array(entries)) = value {
        for entry in entries {
            let Value::Array(pair) = entry else {
                continue;
            };
            let Some(Value::Str(id)) = pair.first() else {
                continue;
            };
            let level = match pair.get(1) {
                Some(Value::Int(level)) => i32::try_from(*level).unwrap_or(0),
                _ => 0,
            };
            out.insert(id.clone(), level);
        }
    }
    out
}

fn string(object: &Object, key: &str) -> Option<String> {
    match object.get(key) {
        Some(Value::Str(text)) => Some(text.clone()),
        _ => None,
    }
}

/// Reads a duration into moves, the way `read_from_json_string<time_duration>` then `to_moves<int>` does.
///
/// A bare number is a number of **turns** - the reference's own convention for a duration written as an integer - and a string is a sequence of `<number> <unit>` pairs
/// whose units are `t` (turns), `s`, `m`, `h`, `d` for days and `w` for weeks. One turn is one second and one second is a hundred moves.
#[must_use]
pub fn duration_in_moves(value: &Value) -> i64 {
    match value {
        // A bare number is already in moves in the constructions the reference reads: `construction.cpp` only converts when the member is a **string**.
        Value::Int(moves) => *moves,
        Value::Str(text) => parse_duration(text).saturating_mul(100),
        _ => 0,
    }
}

/// The number of **turns** in a duration string such as `"1 h 30 m"`.
#[must_use]
pub fn parse_duration(text: &str) -> i64 {
    let mut total: i64 = 0;
    let mut number: Option<i64> = None;
    for word in text.split_whitespace() {
        if let Ok(parsed) = word.parse::<i64>() {
            number = Some(parsed);
            continue;
        }
        let (count, unit) = match word.chars().next() {
            Some(first) if first.is_ascii_digit() => {
                let split = word.find(|character: char| !character.is_ascii_digit()).unwrap_or(word.len());
                let head = word.get(..split).unwrap_or_default();
                let unit = word.get(split..).unwrap_or_default();
                (number.take().or_else(|| head.parse().ok()), unit)
            }
            _ => (number.take(), word),
        };
        let unit_turns = match unit {
            // A turn is a second, so both units count one.
            "t" | "turn" | "turns" | "s" | "sec" | "secs" | "second" | "seconds" => 1,
            "m" | "min" | "mins" | "minute" | "minutes" => 60,
            "h" | "hr" | "hrs" | "hour" | "hours" => 3_600,
            "d" | "day" | "days" => 86_400,
            "w" | "week" | "weeks" => 604_800,
            _ => 0,
        };
        total = total.saturating_add(count.unwrap_or(0).saturating_mul(unit_turns));
    }
    total
}
