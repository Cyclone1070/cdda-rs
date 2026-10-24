//! Requirement groups: what a construction or a recipe takes, as groups of alternatives.
//!
//! `requirement_data` (`src/requirements.h:200-230`) holds three lists of groups — components, tools and qualities — and two operators that the reference uses to combine
//! them: `operator+` **appends** one data's groups to another's, and `operator*` scales every count by a multiple with a floor of `-1` (`src/requirements.cpp:332-400`).
//! A construction merges the groups it names in `using` into its own with exactly that pair of operators (`src/construction.cpp:2533-2537`).
//!
//! Counts have two meanings and both matter: a **component**'s count is how many are consumed, and a **tool**'s negative count is a number of charges rather than a number
//! of items — `claw_bar: -1` means "a claw bar, in any state" while `angle_grinder: 200` means "two hundred charges of grinding".

use cdda_json::{Object, Value};
use std::collections::BTreeMap;

/// One alternative of a component or tool group: `(what, how many, whether it is a nested group)`.
pub type Alternative = (String, i32, bool);

/// One alternative of a quality group: `(quality, level, count)`.
pub type QualityAlternative = (String, i32, i32);

/// A list of groups, each group a list of alternatives.
pub type Groups = Vec<Vec<Alternative>>;

/// A list of quality groups.
pub type QualityGroups = Vec<Vec<QualityAlternative>>;

/// Which of a requirement data's lists a helper should walk.
type Getter = fn(&RequirementData) -> &Groups;

/// One requirement group's contents: what it consumes, what it needs and what qualities it wants.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequirementData {
    /// Components, as groups of alternatives `(item, count, is_a_requirement_group)`.
    pub components: Groups,
    /// Tools, as groups of alternatives `(item, count, is_a_requirement_group)`; a negative count is a charge count.
    pub tools: Groups,
    /// Qualities, as groups of alternatives `(quality, level, count)`.
    pub qualities: QualityGroups,
}

impl RequirementData {
    /// This data with every count scaled by `scalar`, with the reference's floor of `-1`.
    ///
    /// Reference: `requirement_data::operator*` (`src/requirements.cpp:332-347`). Quality counts are **not** scaled, which is the reference's own choice and not an
    /// oversight to fix here.
    #[must_use]
    pub fn scaled(&self, scalar: i32) -> Self {
        let scale = |count: i32| count.saturating_mul(scalar).max(-1);
        Self {
            components: self
                .components
                .iter()
                .map(|group| {
                    group
                        .iter()
                        .map(|(item, count, nested)| (item.clone(), scale(*count), *nested))
                        .collect()
                })
                .collect(),
            tools: self
                .tools
                .iter()
                .map(|group| {
                    group
                        .iter()
                        .map(|(item, count, nested)| (item.clone(), scale(*count), *nested))
                        .collect()
                })
                .collect(),
            qualities: self.qualities.clone(),
        }
    }

    /// This data with another's groups **appended**, which is the reference's merge.
    ///
    /// Reference: `requirement_data::operator+` (`src/requirements.cpp:375-400`). Components and tools are simply appended: two groups of the same component stay two
    /// groups and both must be satisfied. **Qualities are not**: a new group that an old one already satisfies replaces it, and one that satisfies an old group is dropped
    /// entirely (`a_satisfies_b`, `src/requirements.cpp:349-370`), which is why a construction that asks for `PRY 3` twice asks for it once.
    #[must_use]
    pub fn merged(&self, other: &Self) -> Self {
        let mut qualities = self.qualities.clone();
        for incoming in &other.qualities {
            let mut add = true;
            for existing in &mut qualities {
                if satisfies(incoming, existing) {
                    add = false;
                    existing.clone_from(incoming);
                    break;
                }
                if satisfies(existing, incoming) {
                    add = false;
                    break;
                }
            }
            if add {
                qualities.push(incoming.clone());
            }
        }
        Self {
            components: self
                .components
                .iter()
                .chain(other.components.iter())
                .cloned()
                .collect(),
            tools: self.tools.iter().chain(other.tools.iter()).cloned().collect(),
            qualities,
        }
    }

    /// This data with every alternative that names **another requirement group** replaced by that group's own alternatives.
    ///
    /// Reference: `inline_requirements` (`src/requirements.cpp:647-720`), which the reference runs over every loaded group in `requirement_data::finalize`
    /// (`src/requirements.cpp:722-748`). Three rules, all of them the reference's:
    ///
    /// 1. a name in the **tools** list expands against the named group's **tools**, and a name in the components list against its components;
    /// 2. expansion is **recursive**, so a group may name another group;
    /// 3. two alternatives of the same item in one group keep the **smaller count**, because the group is an "or" and the cheaper is the one that can be satisfied.
    #[must_use]
    pub fn expand_nested(
        &self,
        groups: &BTreeMap<String, Self>,
        subtypes: &BTreeMap<String, Vec<String>>,
    ) -> Self {
        Self {
            // **Subtypes are a tool's rule and not a component's.** The reference's `finalize` walks `tools` and leaves `components` alone
            // (`src/requirements.cpp:732-747`), which is why a construction that consumes a `forge` consumes a forge and not every variant of one.
            components: expand_list(&self.components, groups, subtypes, false, |data| {
                &data.components
            }),
            tools: expand_list(&self.tools, groups, subtypes, true, |data| &data.tools),
            qualities: self.qualities.clone(),
        }
    }
}

/// Every requirement group the data defines, by id.
///
/// # Errors
///
/// Fails when a file cannot be read or a group is defined twice.
pub fn load_paths(
    dirs: &[&std::path::Path],
) -> Result<BTreeMap<String, RequirementData>, LoadError> {
    let mut out: BTreeMap<String, RequirementData> = BTreeMap::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            collect(&value, &mut out, &path)?;
        }
    }
    Ok(out)
}

/// Why requirement groups could not be loaded.
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
    out: &mut BTreeMap<String, RequirementData>,
    path: &std::path::Path,
) -> Result<(), LoadError> {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, out, path)?;
            }
        }
        Value::Object(object) => {
            if !matches!(object.get("type"), Some(Value::Str(text)) if text == "requirement") {
                return Ok(());
            }
            let Some(Value::Str(id)) = object.get("id") else {
                return Ok(());
            };
            let data = RequirementData {
                components: pair_groups(object.get("components")),
                tools: pair_groups(object.get("tools")),
                qualities: quality_groups(object.get("qualities")),
            };
            if out.insert(id.clone(), data).is_some() {
                return Err(LoadError {
                    message: format!("requirement {id} is defined twice: {}", path.display()),
                });
            }
        }
        _ => {}
    }
    Ok(())
}

/// Reads groups of `[ id, count ]` alternatives, keeping the reference's order.
#[must_use]
pub fn pair_groups(value: Option<&Value>) -> Groups {
    let Some(Value::Array(groups)) = value else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for group in groups {
        let mut alternatives = Vec::new();
        match group {
            Value::Array(entries) => {
                for entry in entries {
                    if let Some(alternative) = pair(entry) {
                        alternatives.push(alternative);
                    }
                }
            }
            single => {
                if let Some(alternative) = pair(single) {
                    alternatives.push(alternative);
                }
            }
        }
        out.push(alternatives);
    }
    out
}

fn pair(entry: &Value) -> Option<Alternative> {
    match entry {
        // **A bare string is the construction format** (`tool_comp::load`, `src/requirements.cpp:245-248`): `[ "paint_brush" ]` means one such tool, held and not
        // spent, which the reference writes as a count of `-1`.
        Value::Str(name) => Some((name.clone(), -1, false)),
        Value::Array(pair) => {
            let Some(Value::Str(id)) = pair.first() else {
                return None;
            };
            let count = match pair.get(1) {
                Some(Value::Int(count)) => i32::try_from(*count).ok()?,
                _ => 1,
            };
            // The reference marks a nested requirement group with a third member, `"LIST"` (`src/requirements.h`, `item_comp::load`).
            let nested = matches!(pair.get(2), Some(Value::Str(marker)) if marker == "LIST");
            Some((id.clone(), count, nested))
        }
        Value::Object(object) => {
            let id = string(object, "id")?;
            let count = match object.get("count") {
                Some(Value::Int(count)) => i32::try_from(*count).ok()?,
                _ => 1,
            };
            Some((id, count, false))
        }
        _ => None,
    }
}

/// Reads groups of quality alternatives as `(quality, level, count)`.
#[must_use]
pub fn quality_groups(value: Option<&Value>) -> QualityGroups {
    let Some(Value::Array(groups)) = value else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for group in groups {
        let mut alternatives = Vec::new();
        let entries: Vec<&Value> = match group {
            Value::Array(entries) => entries.iter().collect(),
            single => vec![single],
        };
        for entry in entries {
            let Value::Object(object) = entry else {
                continue;
            };
            let Some(id) = string(object, "id") else {
                continue;
            };
            let level = match object.get("level") {
                Some(Value::Int(level)) => i32::try_from(*level).unwrap_or(1),
                _ => 1,
            };
            let count = match object.get("count") {
                Some(Value::Int(count)) => i32::try_from(*count).unwrap_or(1),
                _ => 1,
            };
            alternatives.push((id, level, count));
        }
        out.push(alternatives);
    }
    out
}

/// Expands one list of groups against a requirement registry, the way `inline_requirements` does.
fn expand_list(
    list: &[Vec<Alternative>],
    groups: &BTreeMap<String, RequirementData>,
    subtypes: &BTreeMap<String, Vec<String>>,
    expand_subtypes: bool,
    getter: Getter,
) -> Vec<Vec<(String, i32, bool)>> {
    list.iter()
        .map(|group| {
            let mut expanded: Vec<(String, i32, bool)> = Vec::new();
            for (name, count, nested) in group {
                expand_one(
                    name,
                    *count,
                    *nested,
                    groups,
                    subtypes,
                    expand_subtypes,
                    getter,
                    &mut expanded,
                    &mut Vec::new(),
                );
            }
            expanded
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn expand_one(
    name: &str,
    count: i32,
    nested: bool,
    groups: &BTreeMap<String, RequirementData>,
    subtypes: &BTreeMap<String, Vec<String>>,
    expand_subtypes: bool,
    getter: Getter,
    out: &mut Vec<Alternative>,
    seen: &mut Vec<String>,
) {
    // **Only a name flagged as a requirement is a group.** `[ "surface_heat", 1, "LIST" ]` names a group; a bare `[ "autoclave", 1 ]` names an item that happens to
    // share its name with one, and the reference keeps the item.
    if nested
        && let Some(group) = groups.get(name)
    {
        if seen.iter().any(|entry| entry == name) {
            add_alternative(out, name, count);
            return;
        }
        seen.push(name.to_owned());
        // **The group is scaled by the outer count first** (`req * comp.count`), with the same floor of `-1` that `operator*` uses — so a tool written `-1` stays `-1`
        // however many times it is asked for. And **only the group's first alternative-group is inlined**: `to_inline.front()` (`src/requirements.cpp:701-708`).
        let multiplied = group.scaled(count);
        if let Some(alternatives) = getter(&multiplied).first() {
            for (inner, inner_count, inner_nested) in alternatives {
                expand_one(
                    inner,
                    *inner_count,
                    *inner_nested,
                    groups,
                    subtypes,
                    expand_subtypes,
                    getter,
                    out,
                    seen,
                );
            }
        }
        seen.pop();
        return;
    }
    // **A tool becomes itself and its subtypes**, which the reference substitutes in `finalize` (`src/requirements.cpp:732-747`): a fire may be a fire or a lit torch, and
    // both alternatives stay in the group.
    match subtypes.get(name) {
        Some(replacements) if expand_subtypes && !replacements.is_empty() => {
            add_alternative(out, name, count);
            for replacement in replacements {
                add_alternative(out, replacement, count);
            }
        }
        _ => add_alternative(out, name, count),
    }
}

/// Adds one alternative, keeping the smaller count when the same item is already there.
fn add_alternative(out: &mut Vec<Alternative>, name: &str, count: i32) {
    if let Some(existing) = out.iter_mut().find(|(item, _, _)| item == name) {
        existing.1 = existing.1.min(count);
        return;
    }
    out.push((name.to_owned(), count, false));
}

/// Whether `a` covers `b`: same quality, at least the level, the same count.
///
/// Reference: `a_satisfies_b` (`src/requirements.cpp:349-353`).
fn satisfies(a: &[(String, i32, i32)], b: &[(String, i32, i32)]) -> bool {
    b.iter().all(|wanted| {
        a.iter()
            .any(|held| held.0 == wanted.0 && held.1 >= wanted.1 && held.2 == wanted.2)
    })
}

fn string(object: &Object, key: &str) -> Option<String> {
    match object.get(key) {
        Some(Value::Str(text)) => Some(text.clone()),
        _ => None,
    }
}
