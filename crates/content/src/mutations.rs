//! Mutations: what the data says each one needs, and what it replaces.
//!
//! Reference: `mutation_branch` (`mutation.h:172-340`). The reference keeps the four sets on the branch itself; the port keeps them in one table keyed by id, so a
//! creature holds **ids** and every question about a mutation is answered from the data.

use std::collections::BTreeMap;

use cdda_json::{Object, Value};

/// One stat an enchantment moves, and by how much.
///
/// Reference: `enchantment::values_add` / `values_multiply` (`magic_enchantment.h:244-247`): a map from `enchant_vals::mod` to a number, kept as two lists because a
/// port does not need the enum - the data names the stat with a string.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnchantmentValue {
    /// The stat, by the data's own name: `STRENGTH`, `SPEED`, `HUNGER` and the rest of `enchant_vals::mod`.
    pub value: String,
    /// What it adds.
    pub add: f64,
    /// What it multiplies by, as a fraction: `0.1` is ten percent more.
    pub multiply: f64,
}

/// One mutation's place in the tree.
///
/// The four flags are the reference's own (`mutation.h:180-200`): a mutation can be purifiable, valid, a starting trait and a threshold **independently**, and the data
/// uses every combination - a threshold is `valid: false`, and most traits are not starting ones.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mutation {
    /// How many character-creation points it is worth.
    pub points: i32,
    /// How loud it is on the `@` screen; zero is invisible.
    pub visibility: i32,
    /// Whether a purifier can take it away.
    pub purifiable: bool,
    /// Whether the data marks it a valid mutation; **thresholds are not**, and they are named by other mutations' `threshreq`.
    pub valid: bool,
    /// Whether a character can start with it, which is what makes its removal a matter of chance rather than certainty.
    pub startingtrait: bool,
    /// Whether the mutation is a category threshold, which cannot be gained on points alone.
    pub threshold: bool,
    /// The thresholds that must all be held before this one can be - a **different** set from `prereqs`, where one is enough.
    pub threshreq: Vec<String>,
    /// How many units of the category's mutagen it consumes to gain: `vitamin_cost`, **100** when the data says nothing (`mutation.h:218`).
    pub vitamin_cost: i32,
    /// The ids of the enchantments it carries, for the record.
    pub enchantments: Vec<String>,
    /// The stats those enchantments move, followed to the values themselves.
    pub enchant_values: Vec<EnchantmentValue>,
    /// One of these must be held, if the list is not empty.
    pub prereqs: Vec<String>,
    /// One of these must be held as well, if the list is not empty.
    pub prereqs2: Vec<String>,
    /// Holding any of these means the mutation cannot be gained.
    pub cancels: Vec<String>,
    /// The categories it belongs to.
    pub categories: Vec<String>,
    /// Whether the mutation starts switched on (`starts_active`), which is what the game shows at character creation.
    pub starts_active: bool,
    /// What switching it on costs, in whichever of the four currencies its flags name (`cost`).
    pub cost: i32,
    /// How many turns it stays on before it can be switched again (`cooldown`, in turns).
    pub cooldown_turns: i32,
    /// Whether the cost is paid in hunger.
    pub hunger: bool,
    /// Whether the cost is paid in thirst.
    pub thirst: bool,
    /// Whether the cost is paid in sleepiness.
    pub sleepiness: bool,
    /// Whether the cost is paid in mana.
    pub mana: bool,
    /// The programs that run when it is switched on (`activated_eocs`).
    pub activated_eocs: Vec<String>,
    /// The programs that run when it is switched off (`deactivated_eocs`).
    pub deactivated_eocs: Vec<String>,
}

/// Every mutation the data defines.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mutations {
    by_id: BTreeMap<String, Mutation>,
}

impl Mutations {
    /// A table built from entries, for a gate that needs one mutation and not the corpus.
    #[must_use]
    pub fn from_entries(entries: impl IntoIterator<Item = (String, Mutation)>) -> Self {
        Self {
            by_id: entries.into_iter().collect(),
        }
    }

    /// What the data says about one mutation.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Mutation> {
        self.by_id.get(id)
    }

    /// Every mutation id the table holds, in the data's order.
    pub fn ids(&self) -> impl Iterator<Item = &String> {
        self.by_id.keys()
    }

    /// How many mutations the table holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// Reads the mutation dump.
///
/// Returns nothing when the dump is malformed, so a broken corpus file fails the load rather than half-loading.
#[must_use]
pub fn from_dump(root: &Value) -> Option<(Mutations, usize)> {
    let Value::Object(root) = root else {
        return None;
    };
    let Some(Value::Array(entries)) = root.get("mutations") else {
        return None;
    };
    let mut by_id = BTreeMap::new();
    for entry in entries {
        let Value::Object(entry) = entry else {
            return None;
        };
        let Some(Value::Str(id)) = entry.get("id") else {
            return None;
        };
        by_id.insert(id.clone(), mutation_of(entry));
    }
    let read = by_id.len();
    Some((Mutations { by_id }, read))
}

fn mutation_of(entry: &Object) -> Mutation {
    Mutation {
        points: whole(entry, "points"),
        visibility: whole(entry, "visibility"),
        purifiable: matches!(entry.get("purifiable"), Some(Value::Bool(true))),
        valid: matches!(entry.get("valid"), Some(Value::Bool(true))),
        startingtrait: matches!(entry.get("startingtrait"), Some(Value::Bool(true))),
        threshold: matches!(entry.get("threshold"), Some(Value::Bool(true))),
        threshreq: strings(entry, "threshreq"),
        vitamin_cost: int_of(entry, "vitamin_cost").unwrap_or(100),
        enchantments: strings(entry, "enchantments"),
        enchant_values: values_of(entry),
        prereqs: strings(entry, "prereqs"),
        prereqs2: strings(entry, "prereqs2"),
        cancels: strings(entry, "cancels"),
        categories: strings(entry, "category"),
        starts_active: matches!(entry.get("starts_active"), Some(Value::Bool(true))),
        cost: whole(entry, "cost"),
        cooldown_turns: whole(entry, "cooldown_turns"),
        hunger: matches!(entry.get("hunger"), Some(Value::Bool(true))),
        thirst: matches!(entry.get("thirst"), Some(Value::Bool(true))),
        sleepiness: matches!(entry.get("sleepiness"), Some(Value::Bool(true))),
        mana: matches!(entry.get("mana"), Some(Value::Bool(true))),
        activated_eocs: strings(entry, "activated_eocs"),
        deactivated_eocs: strings(entry, "deactivated_eocs"),
    }
}

fn whole(entry: &Object, key: &str) -> i32 {
    match entry.get(key) {
        Some(Value::Int(number)) => i32::try_from(*number).unwrap_or(0),
        _ => 0,
    }
}

fn strings(entry: &Object, key: &str) -> Vec<String> {
    match entry.get(key) {
        Some(Value::Array(list)) => list
            .iter()
            .filter_map(|one| match one {
                Value::Str(id) => Some(id.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// One mutation category's numbers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Category {
    /// The mutagen vitamin this category consumes - `vitamin` in `mutation_category.json`, which every category states.
    pub vitamin: String,
    /// The percent chance, per attempt, that a starting trait of this category is removed.
    pub base_removal_chance: i32,
    /// The category strength a threshold needs.
    pub threshold_min: i32,
}

/// Every mutation category the data defines, by id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Categories {
    by_id: BTreeMap<String, Category>,
}

impl Categories {
    /// What the data says about one category.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Category> {
        self.by_id.get(id)
    }

    /// How many categories the table holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// Reads the mutation-category dump.
///
/// Returns nothing when the dump is malformed, so a broken corpus file fails the load rather than half-loading.
#[must_use]
pub fn categories_from_dump(root: &Value) -> Option<(Categories, usize)> {
    let Value::Object(root) = root else {
        return None;
    };
    let Some(Value::Array(entries)) = root.get("categories") else {
        return None;
    };
    let mut by_id = BTreeMap::new();
    for entry in entries {
        let Value::Object(entry) = entry else {
            return None;
        };
        let Some(Value::Str(id)) = entry.get("id") else {
            return None;
        };
        by_id.insert(
            id.clone(),
            Category {
                vitamin: string_of(entry, "vitamin"),
                base_removal_chance: whole(entry, "base_removal_chance"),
                threshold_min: whole(entry, "threshold_min"),
            },
        );
    }
    let read = by_id.len();
    Some((Categories { by_id }, read))
}

fn values_of(entry: &Object) -> Vec<EnchantmentValue> {
    match entry.get("enchant_values") {
        Some(Value::Array(list)) => list
            .iter()
            .filter_map(|one| {
                let Value::Object(one) = one else {
                    return None;
                };
                let Value::Str(value) = one.get("value")? else {
                    return None;
                };
                Some(EnchantmentValue {
                    value: value.clone(),
                    add: number(one.get("add")),
                    multiply: number(one.get("multiply")),
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn number(member: Option<&Value>) -> f64 {
    match member {
        Some(Value::Float(value)) => *value,
        Some(Value::Int(value)) => {
            #[allow(clippy::cast_precision_loss)]
            let value = *value as f64;
            value
        }
        _ => 0.0,
    }
}

/// One string out of the dump, empty when it is not there.
fn string_of(entry: &Object, name: &str) -> String {
    match entry.get(name) {
        Some(Value::Str(value)) => value.clone(),
        _ => String::new(),
    }
}

/// One number out of the dump, or nothing when it is not there.
fn int_of(entry: &Object, name: &str) -> Option<i32> {
    match entry.get(name) {
        Some(Value::Int(value)) => i32::try_from(*value).ok(),
        _ => None,
    }
}
