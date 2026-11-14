//! Crafting recipes, read from the reference's own dump.
//!
//! Reference: `recipe_dictionary` (`src/recipe_dictionary.cpp`), reached through
//! `--dump-recipes`. The loader is deliberately narrow at first: ids and results only, because
//! the first thing worth proving is that the port finds the same recipes and that each makes
//! the item the reference says it makes. Components, tools and requirements follow once that
//! gate is green.

use cdda_json::{Object, Value};

/// One alternative within a requirement group: an item and how many of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    /// The item that satisfies this alternative.
    pub item: String,
    /// How many are needed.
    pub count: i32,
}

/// One required tool, or one alternative for a tool slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    /// The item used as a tool.
    pub item: String,
    /// How many charges the recipe spends, or `-1` for a tool that must be held but is not
    /// consumed - the reference's own encoding, not a count of minus one. Reading it as a
    /// number would let a recipe spend negative charges or none at all.
    pub count: i32,
}

/// One required quality, such as sewing at level two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quality {
    /// The quality's id.
    pub quality: String,
    /// The minimum level that satisfies it.
    pub level: i32,
    /// How many items with that quality are needed.
    pub count: i32,
}

/// A practice recipe's difficulty range and skill limit (`recipe.h:72-84`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Practice {
    /// The difficulty is never below this.
    pub min_difficulty: i32,
    /// Nor above this.
    pub max_difficulty: i32,
    /// The crafter's primary skill cannot pass this through this recipe.
    pub skill_limit: i32,
}

/// One crafting recipe, as far as this loader reads it.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::derive_partial_eq_without_eq)]
pub struct Recipe {
    /// The recipe's id, which is not the same as the item it makes.
    pub id: String,
    /// The item the recipe produces.
    pub result: String,
    /// Whether the reference has blacklisted it, which removes it from play without deleting it.
    pub blacklisted: bool,
    /// The practice block, when this recipe is a **practice** recipe (`recipe::practice_data`, `recipe.h:72-84`).
    ///
    /// A practice recipe trains a skill instead of making a thing, and its difficulty is not stated: it follows the crafter's level between `min_difficulty` and `max_difficulty`, and it stops teaching
    /// at `skill_limit`. **124 of the 6,291 recipes are practice recipes**, so the port needs the block to answer `get_difficulty` without inventing numbers.
    pub practice: Option<Practice>,
    /// Whether the recipe **can never be learned**, which `learn_recipe` refuses (`character_crafting.cpp:32-40`).
    ///
    /// The data uses it for practice recipes and the companions alike - **1,114 of the 6,291 recipes in the reference's own dump** carry it, so a loader that dropped the field would leave more than a
    /// thousand recipes learnable that the reference refuses to teach.
    pub never_learn: bool,
    /// What the recipe needs, as groups of alternatives.
    ///
    /// The nesting is the meaning: each inner list is a set of items *any one of which*
    /// satisfies that requirement, and the outer list must all be satisfied. Flattening either
    /// level changes the recipe - the difference between "a rag or a leather patch" and "a rag
    /// and a leather patch" is one vector deep.
    pub components: Vec<Vec<Component>>,
    /// Tools the recipe needs, as groups of alternatives.
    pub tools: Vec<Vec<Tool>>,
    /// Qualities the recipe needs, as groups of alternatives.
    pub qualities: Vec<Vec<Quality>>,
    /// The skill it trains, which is also the skill that gates it.
    pub skill: String,
    /// How skilled the crafter must be, on the reference's own scale.
    pub difficulty: i32,
    /// How much a batch saves, as the reference's `batch_rscale`: zero for a recipe that gains nothing from batching.
    ///
    /// Reference: `batch_time_factors` (`recipe.cpp:242-246`) - the first number over a hundred, so `[25, 8]` is a quarter off at the scale's far end.
    pub batch_rscale: f64,
    /// How many items the batch saving reaches its 99.5% limit at, the reference's `batch_rsize`.
    pub batch_rsize: i32,
    /// The other things a craft leaves behind, as `(item, count per batch)`.
    ///
    /// Reference: `recipe::create_byproducts` (`recipe.cpp:872-900`) through `create_byproducts_legacy`, which multiplies the count by the batch. **The grouped
    /// form** (`byproducts` naming an item group) is not read here, because picking from a group needs the item-group store at craft time; the loader keeps only the
    /// pair form and a recipe whose byproducts are a group keeps none.
    pub byproducts: Vec<(String, i32)>,
    /// The proficiencies the craft teaches and uses.
    ///
    /// Reference: `recipe_proficiency` (`recipe.h:50-61`): `id`, `time_multiplier` (how much faster the craft is with it) and `learning_time_mult` (how long it takes
    /// to learn **by this craft**, defaulting to one). `craft_proficiency_gain` (`crafting.cpp:1036-1090`) divides by the first and multiplies by the second.
    pub proficiencies: Vec<CraftProficiency>,
    /// The container the result is made **in**, when the recipe names one.
    ///
    /// Reference: `contained` and `container` (`recipe.cpp:236-239`): `assign( jo, "contained", ... )`, `assign( jo, "container", ... )`, and `contained |=` the
    /// container being non-empty. `create_result` then returns **one item** - the container with the result inside it (`recipe.cpp:768-771`) - which is why a canning
    /// recipe's product is the can rather than what went into it.
    pub container: Option<String>,
    /// Byproducts that are an **inline item group**: each entry is an item and the two ends of the count to draw between.
    ///
    /// Reference: `byproduct_group` (`recipe.cpp:380-386`), loaded through `item_group::load_item_group` and turned into items by `create_byproducts`
    /// (`recipe.cpp:872-900`). The data writes it out in the recipe itself - `{"item": "arrow_wood_heavy", "count": [0, 4]}` - so the count is a **range** and the
    /// rule draws from it once per batch. Entries that name a group rather than an item are refused by the loader rather than guessed at.
    pub byproduct_group: Vec<(String, i32, i32)>,
    /// How many of the result one batch makes, all told.
    ///
    /// Reference: `recipe::create_results` (`recipe.cpp:790-825`) makes `result_mult` results per batch item, and `create_result` (`recipe.cpp:754`) gives each of
    /// them `charges ?: the item's own count` - so the number one batch makes is the **product** of the two, and that product is what the reference's own dump
    /// reports as `made_per_batch`. The port keeps the product rather than the factors because the factors are members the reference does not expose and the
    /// product is what every caller wants; a recipe that states neither makes one.
    pub made_per_batch: i32,
    /// How long one batch takes, in moves.
    ///
    /// Reference: `recipe::time`, read with `to_moves<int>( read_from_json_string<time_duration> )` - so `"20 m"` is
    /// twelve hundred seconds and a hundred and twenty thousand moves. A recipe that states no time is zero, which the
    /// reference would treat as instant, and no recipe in the data does.
    pub time_moves: i64,
}

fn member<'a>(object: &'a Object, key: &str) -> Option<&'a Value> {
    object.get(key)
}

fn string_of(object: &Object, key: &str) -> Option<String> {
    match member(object, key)? {
        Value::Str(text) => Some(text.clone()),
        _ => None,
    }
}

fn components_of(object: &Object) -> Option<Vec<Vec<Component>>> {
    let Value::Array(groups) = member(object, "components")? else {
        return None;
    };
    let mut out = Vec::with_capacity(groups.len());
    for group in groups {
        let Value::Array(alternatives) = group else {
            return None;
        };
        let mut inner = Vec::with_capacity(alternatives.len());
        for alternative in alternatives {
            let Value::Object(component) = alternative else {
                return None;
            };
            let Value::Int(count) = member(component, "count")? else {
                return None;
            };
            inner.push(Component {
                item: string_of(component, "item")?,
                count: i32::try_from(*count).ok()?,
            });
        }
        out.push(inner);
    }
    Some(out)
}

fn tools_of(object: &Object) -> Option<Vec<Vec<Tool>>> {
    let Value::Array(groups) = member(object, "tools")? else {
        return None;
    };
    let mut out = Vec::with_capacity(groups.len());
    for group in groups {
        let Value::Array(alternatives) = group else {
            return None;
        };
        let mut inner = Vec::with_capacity(alternatives.len());
        for alternative in alternatives {
            let Value::Object(tool) = alternative else {
                return None;
            };
            let Value::Int(count) = member(tool, "count")? else {
                return None;
            };
            inner.push(Tool {
                item: string_of(tool, "item")?,
                count: i32::try_from(*count).ok()?,
            });
        }
        out.push(inner);
    }
    Some(out)
}

/// The proficiencies a recipe uses and teaches, as the data states them.
///
/// Reference: `recipe_proficiency` (`recipe.h:50-61`): `time_multiplier` is how much faster the craft goes with it, and `learning_time_multiplier` how long learning
/// it **by this craft** takes. Both default to one, which is the reference's own default.
fn taught_by(object: &Object) -> Vec<CraftProficiency> {
    let entries = match member(object, "proficiencies") {
        Some(Value::Array(entries)) => entries.as_slice(),
        _ => &[],
    };
    let mut found = Vec::with_capacity(entries.len());
    for entry in entries {
        let Value::Object(entry) = entry else {
            continue;
        };
        let Some(Value::Str(id)) = member(entry, "proficiency") else {
            continue;
        };
        found.push(CraftProficiency {
            id: id.clone(),
            time_multiplier: number_of(entry, "time_multiplier").unwrap_or(1.0),
            learning_time_mult: number_of(entry, "learning_time_multiplier").unwrap_or(1.0),
        });
    }
    found
}

/// One proficiency a craft teaches.
#[derive(Debug, Clone, PartialEq)]
pub struct CraftProficiency {
    /// Which proficiency, as the data names it - `prof_tailor`.
    pub id: String,
    /// How much faster the craft goes for someone who knows it.
    pub time_multiplier: f64,
    /// How long learning it **by this craft** takes, as a multiple of the proficiency's own time.
    pub learning_time_mult: f64,
}

/// A number written as a number, when it is one.
fn number_of(object: &Object, key: &str) -> Option<f64> {
    match member(object, key) {
        Some(Value::Float(value)) => Some(*value),
        Some(Value::Int(value)) => {
            #[allow(clippy::cast_precision_loss)]
            let value = *value as f64;
            Some(value)
        }
        _ => None,
    }
}

/// An inline `byproduct_group`, as `(item, low, high)` ranges.
///
/// Reference: `byproduct_group` (`recipe.cpp:380-386`) is an item group written in the recipe itself, whose counts may be one number or two ends to draw between.
/// Entries that name another group rather than an item are skipped: following a group inside a group is the group model's job, not the recipe's.
fn groups_of(object: &Object) -> Vec<(String, i32, i32)> {
    let Some(Value::Array(entries)) = member(object, "byproduct_group") else {
        return Vec::new();
    };
    let mut drawn = Vec::with_capacity(entries.len());
    for entry in entries {
        let Value::Object(entry) = entry else {
            continue;
        };
        let Some(Value::Str(item)) = member(entry, "item") else {
            continue;
        };
        let ends = match member(entry, "count") {
            Some(Value::Int(count)) => {
                let count = i32::try_from(*count).unwrap_or(0);
                (count, count)
            }
            Some(Value::Array(pair)) => match (pair.first(), pair.get(1)) {
                (Some(Value::Int(low)), Some(Value::Int(high))) => (
                    i32::try_from(*low).unwrap_or(0),
                    i32::try_from(*high).unwrap_or(0),
                ),
                _ => continue,
            },
            _ => continue,
        };
        drawn.push((item.clone(), ends.0.min(ends.1), ends.0.max(ends.1)));
    }
    drawn
}

fn qualities_of(object: &Object) -> Option<Vec<Vec<Quality>>> {
    let Value::Array(groups) = member(object, "qualities")? else {
        return None;
    };
    let mut out = Vec::with_capacity(groups.len());
    for group in groups {
        let Value::Array(alternatives) = group else {
            return None;
        };
        let mut inner = Vec::with_capacity(alternatives.len());
        for alternative in alternatives {
            let Value::Object(quality) = alternative else {
                return None;
            };
            let (Value::Str(quality_id), Value::Int(level), Value::Int(count)) = (
                member(quality, "quality")?,
                member(quality, "level")?,
                member(quality, "count")?,
            ) else {
                return None;
            };
            inner.push(Quality {
                quality: quality_id.clone(),
                level: i32::try_from(*level).ok()?,
                count: i32::try_from(*count).ok()?,
            });
        }
        out.push(inner);
    }
    Some(out)
}

/// The recipe's `batch_time_factors`, as `(scale, size)`.
///
/// Reference: `[percent, size]` (`recipe.cpp:242-246`) - the first is divided by a hundred and the second is the count that earns it. A recipe without the member
/// gains nothing from batching, which is `(0.0, 0)`.
fn batch_of(object: &cdda_json::Object) -> (f64, i32) {
    let Some(Value::Array(factors)) = member(object, "batch_time_factors") else {
        return (0.0, 0);
    };
    let scale = match factors.first() {
        Some(Value::Int(percent)) => f64::from(i32::try_from(*percent).unwrap_or(0)) / 100.0,
        _ => 0.0,
    };
    let size = match factors.get(1) {
        Some(Value::Int(count)) => i32::try_from(*count).unwrap_or(0),
        _ => 0,
    };
    (scale, size)
}

/// Reads the recipe dump.
///
/// Returns the recipes in the dump's order and the count the reference reported. The two are
/// compared by the caller: a dump that disagrees with its own count is a broken dump, and the
/// loader should not paper over it.
#[must_use]
pub fn from_dump(value: &Value) -> Option<(Vec<Recipe>, usize)> {
    let Value::Object(root) = value else {
        return None;
    };
    let Value::Array(entries) = member(root, "recipes")? else {
        return None;
    };
    let count = match member(root, "count")? {
        Value::Int(number) => usize::try_from(*number).ok()?,
        _ => return None,
    };
    let mut recipes = Vec::with_capacity(entries.len());
    for entry in entries {
        let Value::Object(object) = entry else {
            return None;
        };
        recipes.push(Recipe {
            id: string_of(object, "id")?,
            result: string_of(object, "result")?,
            blacklisted: matches!(member(object, "blacklisted"), Some(Value::Bool(true))),
            never_learn: matches!(member(object, "never_learn"), Some(Value::Bool(true))),
            practice: practice_of(object),
            components: components_of(object)?,
            tools: tools_of(object)?,
            qualities: qualities_of(object)?,
            skill: string_of(object, "skill").unwrap_or_default(),
            difficulty: match member(object, "difficulty") {
                Some(Value::Int(value)) => i32::try_from(*value).unwrap_or(0),
                _ => 0,
            },
            // From the oracle's dump when it is a dump (`time_moves`, already in moves), and from the game's own JSON
            // otherwise, where it is a duration string.
            time_moves: match member(object, "time_moves") {
                Some(Value::Int(moves)) => *moves,
                _ => member(object, "time").map_or(0, crate::constructions::duration_in_moves),
            },
            byproducts: match member(object, "byproducts") {
                Some(Value::Array(list)) => list
                    .iter()
                    .filter_map(|entry| match entry {
                        Value::Array(pair) => match (pair.first(), pair.get(1)) {
                            (Some(Value::Str(item)), Some(Value::Int(count))) => {
                                Some((item.clone(), i32::try_from(*count).unwrap_or(0)))
                            }
                            _ => None,
                        },
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            },
            // The dump answers the question the members cannot: **which item the rule produces**, which is the result for an ordinary recipe and the container for a
            // `contained` one (`create_result`, `recipe.cpp:768-771`). The game's own JSON names the container directly, so both shapes are read here.
            proficiencies: taught_by(object),
            container: {
                let dump = match member(object, "made_item") {
                    Some(Value::Str(id))
                        if object.get("result") != Some(&Value::Str(id.clone())) =>
                    {
                        Some(id.clone())
                    }
                    _ => None,
                };
                dump.or_else(|| {
                    let named = match member(object, "container") {
                        Some(Value::Str(id)) if !id.is_empty() => Some(id.clone()),
                        _ => None,
                    };
                    let contained = matches!(member(object, "contained"), Some(Value::Bool(true)));
                    if contained { named } else { None }
                })
            },
            byproduct_group: groups_of(object),
            // The dump states the product; the game's own JSON states the factors separately, and the product is what the rule uses.
            made_per_batch: {
                let stated = member(object, "made_per_batch");
                if let Some(Value::Int(made)) = stated {
                    i32::try_from(*made).unwrap_or(1).max(1)
                } else {
                    let mut product: i32 = 1;
                    for key in ["result_mult", "charges"] {
                        if let Some(Value::Int(count)) = member(object, key) {
                            product =
                                product.saturating_mul(i32::try_from(*count).unwrap_or(1).max(1));
                        }
                    }
                    product.max(1)
                }
            },
            batch_rscale: batch_of(object).0,
            batch_rsize: batch_of(object).1,
        });
    }
    Some((recipes, count))
}

impl Recipe {
    /// Whether what is on hand satisfies this recipe's components for a given batch.
    ///
    /// Reference: `requirement_data::has_comps` (`src/requirements.cpp:895-940`), which is the part of
    /// `can_make_with_inventory` about items. Each group is a set of **alternatives**: one of them being
    /// present in sufficient number satisfies the group, and every group must be satisfied. Quantities
    /// scale with the batch, and an item counted in charges is asked for in charges by the caller.
    ///
    /// Not modelled: the reference's check that one pile cannot satisfy two overlapping groups
    /// (`check_enough_materials`), which needs the inventory modelled rather than asked.
    #[must_use]
    pub fn components_available(&self, batch: i32, available: impl Fn(&str) -> i32) -> bool {
        let batch = batch.max(1);
        self.components.iter().all(|alternatives| {
            alternatives.iter().any(|component| {
                available(&component.item) >= component.count.saturating_mul(batch)
            })
        })
    }
}

/// What a recipe's tools ask of the inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolNeed {
    /// It must be present but is not spent, in this many copies.
    Held(i32),
    /// It must be present and this many charges are spent per batch.
    Charges(i32),
}

impl Recipe {
    /// What each tool group wants, with the reference's encoding resolved.
    ///
    /// Reference: `tool_comp::by_charges` and `tool_comp::has` (`src/requirements.cpp:952-990`). A count
    /// of `-1` is not "minus one charge": it means the tool is needed and not consumed, which is why the
    /// encoding is resolved here rather than left for a caller to read as a number.
    #[must_use]
    pub fn tool_needs(&self) -> Vec<Vec<(String, ToolNeed)>> {
        self.tools
            .iter()
            .map(|alternatives| {
                alternatives
                    .iter()
                    .map(|tool| {
                        // A negative count and a zero both mean "held, not spent": the reference
                        // uses -1 for a tool that is only needed, and a zero would spend nothing.
                        let need = match tool.count {
                            count if count <= 0 => ToolNeed::Held(1),
                            charges => ToolNeed::Charges(charges),
                        };
                        (tool.item.clone(), need)
                    })
                    .collect()
            })
            .collect()
    }

    /// Whether the tools are on hand for a given batch.
    ///
    /// One alternative of each group satisfies it. A held tool needs that many copies; a tool that spends
    /// charges needs `count * batch` of them.
    #[must_use]
    pub fn tools_available(
        &self,
        batch: i32,
        held_count: impl Fn(&str) -> i32,
        charges_of: impl Fn(&str) -> i32,
    ) -> bool {
        let batch = batch.max(1);
        self.tool_needs().iter().all(|alternatives| {
            alternatives.iter().any(|(item, need)| match need {
                ToolNeed::Held(count) => held_count(item) >= *count,
                ToolNeed::Charges(count) => charges_of(item) >= count.saturating_mul(batch),
            })
        })
    }

    /// Whether the qualities are on hand for a given batch.
    ///
    /// Reference: `quality_requirement::has` (`src/requirements.cpp:932-940`), which asks the inventory
    /// for `has_quality( type, level, count )`: an item whose quality is **at least** the required level
    /// counts, and there must be that many items. Qualities are not spent, so the batch does not change
    /// the requirement.
    #[must_use]
    pub fn qualities_available(&self, batch: i32, items_with: impl Fn(&str, i32) -> i32) -> bool {
        let _ = batch;
        self.qualities.iter().all(|alternatives| {
            alternatives
                .iter()
                .any(|quality| items_with(&quality.quality, quality.level) >= quality.count)
        })
    }

    /// Whether a crafter at this skill level can make the recipe at all.
    ///
    /// Reference: `Character::has_recipe_requirements` and the difficulty check in
    /// `Character::can_craft` (`src/crafting.cpp`): the crafter's level in the recipe's skill must reach
    /// the recipe's difficulty. Knowing the recipe is a separate question, so it is a separate argument -
    /// the reference keeps the two apart and a character can be skilled enough for a recipe it has never
    /// learned.
    #[must_use]
    pub fn skilled_enough(&self, knows: bool, level: i32) -> bool {
        knows && level >= self.difficulty
    }

    /// Whether everything the recipe needs is on hand: components, tools and qualities.
    #[must_use]
    pub fn can_make(
        &self,
        batch: i32,
        available: impl Fn(&str) -> i32,
        held_count: impl Fn(&str) -> i32,
        charges_of: impl Fn(&str) -> i32,
        items_with: impl Fn(&str, i32) -> i32,
    ) -> bool {
        self.components_available(batch, available)
            && self.tools_available(batch, held_count, charges_of)
            && self.qualities_available(batch, items_with)
    }
}

/// The practice block, when the dump carries one.
fn practice_of(object: &Object) -> Option<Practice> {
    let Value::Object(block) = member(object, "practice")? else {
        return None;
    };
    let number = |key: &str| match member(block, key) {
        Some(Value::Int(value)) => i32::try_from(*value).ok(),
        _ => None,
    };
    Some(Practice {
        min_difficulty: number("min_difficulty")?,
        max_difficulty: number("max_difficulty")?,
        skill_limit: number("skill_limit")?,
    })
}

/// A practice recipe's difficulty, which follows the crafter inside the range the data gives it.
///
/// Reference: `recipe::get_difficulty` (`recipe.cpp:61-68`): a practice recipe answers `clamp( skill level, min_difficulty, max_difficulty )` and every other recipe answers its own `difficulty`.
/// **The clamp is what keeps a practice recipe teaching**: it is never too easy and never out of reach, because inside the range the difficulty **is** the crafter's level.
#[must_use]
pub fn practice_difficulty(skill_level: i32, min_difficulty: i32, max_difficulty: i32) -> i32 {
    skill_level.clamp(min_difficulty, max_difficulty)
}

/// The level at which a recipe stops teaching.
///
/// Reference: `recipe::get_skill_cap` (`recipe.cpp:70-77`): a practice recipe stops at `skill_limit - 1`; an ordinary one at `difficulty * 1.25`, **truncated** - so a difficulty of five caps at six.
#[must_use]
pub fn skill_cap(practice_skill_limit: Option<i32>, difficulty: i32) -> i32 {
    if let Some(limit) = practice_skill_limit {
        return limit - 1;
    }
    #[allow(clippy::cast_possible_truncation)]
    let cap = (f64::from(difficulty) * 1.25) as i32;
    cap
}

impl Recipe {
    /// How hard this recipe is for a crafter of this level - the reference's `get_difficulty`.
    #[must_use]
    pub fn difficulty_for(&self, skill_level: i32) -> i32 {
        self.practice.map_or(self.difficulty, |practice| {
            practice_difficulty(
                skill_level,
                practice.min_difficulty,
                practice.max_difficulty,
            )
        })
    }

    /// The level at which this recipe stops teaching - the reference's `get_skill_cap`.
    #[must_use]
    pub fn teaching_cap(&self) -> i32 {
        skill_cap(
            self.practice.map(|practice| practice.skill_limit),
            self.difficulty,
        )
    }
}
