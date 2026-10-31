//! Reading the data's snippet categories.
//!
//! Reference: `/snippets/types` (`snippet.cpp`): a snippet object names a `category` and gives `text`, which may be one string or a list of entries that
//! are either a string or an object with `id`, `text` and `weight`. A line with no id is named after its category and its position, because
//! `same_snippet` remembers a line by id and the data does not always write one.
//!
//! The store is the sim's (`cdda_sim::snippets`); this module only reads.

use cdda_json::Object;

/// One line of a category, as the data writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// The line's id, written or made from the category and its position.
    pub id: String,
    /// What the line says.
    pub text: String,
    /// How likely it is against its neighbours; the data's default is 100.
    pub weight: i32,
}

/// Every snippet category in one JSON value, in load order.
///
/// A file may hold one snippet object or a list of them, and a category may be written more than once - the reference appends, so the port does too.
#[must_use]
pub fn categories(value: &cdda_json::Value) -> Vec<(String, Vec<Line>)> {
    let single = [value.clone()];
    let entries: &[cdda_json::Value] = match value {
        cdda_json::Value::Array(items) => items,
        cdda_json::Value::Object(_) => &single,
        _ => &[],
    };
    let mut out: Vec<(String, Vec<Line>)> = Vec::new();
    for entry in entries {
        let cdda_json::Value::Object(object) = entry else {
            continue;
        };
        let Some(cdda_json::Value::Str(category)) = object.get("category") else {
            continue;
        };
        let lines = read_lines(category, object);
        if lines.is_empty() {
            continue;
        }
        match out.iter_mut().find(|(known, _)| known == category) {
            Some((_, existing)) => existing.extend(lines),
            None => out.push((category.clone(), lines)),
        }
    }
    out
}

/// The lines of one category, however the data wrote them.
fn read_lines(category: &str, object: &Object) -> Vec<Line> {
    let mut lines = Vec::new();
    match object.get("text") {
        Some(cdda_json::Value::Str(text)) => lines.push(Line {
            id: format!("{category}_{}", lines.len()),
            text: text.clone(),
            weight: 100,
        }),
        Some(cdda_json::Value::Array(entries)) => {
            for entry in entries {
                match entry {
                    cdda_json::Value::Str(text) => lines.push(Line {
                        id: format!("{category}_{}", lines.len()),
                        text: text.clone(),
                        weight: 100,
                    }),
                    cdda_json::Value::Object(one) => {
                        let Some(cdda_json::Value::Str(text)) = one.get("text") else {
                            continue;
                        };
                        let id = match one.get("id") {
                            Some(cdda_json::Value::Str(id)) => id.clone(),
                            _ => format!("{category}_{}", lines.len()),
                        };
                        let weight = match one.get("weight") {
                            Some(cdda_json::Value::Int(weight)) => i32::try_from(*weight).unwrap_or(100),
                            _ => 100,
                        };
                        lines.push(Line {
                            id,
                            text: text.clone(),
                            weight,
                        });
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    lines
}
