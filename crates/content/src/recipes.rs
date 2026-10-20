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

/// One crafting recipe, as far as this loader reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    /// The recipe's id, which is not the same as the item it makes.
    pub id: String,
    /// The item the recipe produces.
    pub result: String,
    /// Whether the reference has blacklisted it, which removes it from play without deleting it.
    pub blacklisted: bool,
    /// What the recipe needs, as groups of alternatives.
    ///
    /// The nesting is the meaning: each inner list is a set of items *any one of which*
    /// satisfies that requirement, and the outer list must all be satisfied. Flattening either
    /// level changes the recipe - the difference between "a rag or a leather patch" and "a rag
    /// and a leather patch" is one vector deep.
    pub components: Vec<Vec<Component>>,
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
            components: components_of(object)?,
        });
    }
    Some((recipes, count))
}
