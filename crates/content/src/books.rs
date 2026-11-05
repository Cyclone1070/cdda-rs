//! The book slot: what a book teaches, and what it costs in time and intelligence.
//!
//! Reference: `islot_book` (`src/itype.h:569-620`) and the item factory that fills it. The slot is a plain set of scalars on the item's type, which is why this loader reads a **record** rather than
//! walking the corpus: the port's item loader already resolves `copy-from` for everything else, and a second walk that resolved it again would be a second source of truth about what a book is.
//!
//! **The one number that does not survive as written is `time`**, which the data writes as a duration string (`"35 m"`) and the reference stores as turns.

use cdda_json::{Object, Value};

/// What a book teaches.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Book {
    /// The skill it raises, or `"null"` for none - the reference's `skill_id::NULL_ID()`.
    pub skill: String,
    /// The martial art it teaches, or `"null"`.
    pub martial_art: String,
    /// The level it can take a reader to (`islot_book::level`).
    pub level: i32,
    /// The level a reader needs to understand it (`islot_book::req`).
    pub req: i32,
    /// How fun reading it is, which may be **negative** (`islot_book::fun`).
    pub fun: i32,
    /// The intelligence it asks for (`islot_book::intel`).
    pub intel: i32,
    /// How long one skill point takes, in **turns** (`islot_book::time`).
    pub time_turns: i64,
    /// How many chapters a fun book has, after which it is less fun (`islot_book::chapters`).
    pub chapters: i32,
    /// Whether chapters are counted per item instance rather than per type (`islot_book::generic`).
    pub generic: bool,
    /// The proficiencies it teaches, in the data's order.
    pub proficiencies: Vec<(String, String)>,
}

impl Book {
    /// Reads the slot from one item record.
    ///
    /// **Every field keeps the reference's default when the record leaves it out**, which is what `optional` does for a slot object: a child that states only `time` inherits the rest.
    #[must_use]
    pub fn read(object: &Object) -> Self {
        let text = |key: &str| match object.get(key) {
            Some(Value::Str(value)) => value.clone(),
            _ => String::new(),
        };
        let number = |key: &str| match object.get(key) {
            Some(Value::Int(value)) => i32::try_from(*value).unwrap_or(0),
            _ => 0,
        };
        // The data names the same fields two ways across this version: `read_skill` in the records, `skill` in the slot.
        let skill = match text("read_skill") {
            empty if empty.is_empty() => text("skill"),
            named => named,
        };
        let mut proficiencies = Vec::new();
        if let Some(Value::Array(list)) = object.get("proficiencies") {
            for entry in list {
                if let Value::Object(entry) = entry {
                    let name = match entry.get("proficiency") {
                        Some(Value::Str(value)) => value.clone(),
                        _ => String::new(),
                    };
                    let bonus = match entry.get("bonus") {
                        Some(Value::Str(value)) => value.clone(),
                        _ => String::new(),
                    };
                    if !name.is_empty() {
                        proficiencies.push((name, bonus));
                    }
                }
            }
        }
        Self {
            skill,
            martial_art: text("martial_art"),
            level: number("max_level"),
            req: number("required_level"),
            fun: number("read_fun"),
            intel: number("intelligence"),
            time_turns: read_duration_turns(object.get("time")),
            chapters: number("chapters"),
            generic: matches!(object.get("generic"), Some(Value::Bool(true))),
            proficiencies,
        }
    }

    /// Whether the slot is worth keeping for this record: a book with a time and a skill is a book the reading rules can use.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        self.time_turns > 0 && !self.skill.is_empty()
    }
}

/// A duration as the data writes it (`"35 m"`, `"2 h"`, `"30 s"`), in turns.
///
/// Reference: `time_duration::from_string` in the JSON reader, with a turn being one second. **A duration with no unit is turns**, which is the reference's own default.
#[must_use]
pub fn read_duration_turns(value: Option<&Value>) -> i64 {
    let Some(Value::Str(text)) = value else {
        return 0;
    };
    let mut parts = text.split_whitespace();
    let Some(amount) = parts.next().and_then(|number| number.parse::<i64>().ok()) else {
        return 0;
    };
    let unit = parts.next().unwrap_or("turns");
    match unit {
        "s" | "second" | "seconds" => amount,
        "m" | "minute" | "minutes" => amount * 60,
        "h" | "hour" | "hours" => amount * 3600,
        "d" | "day" | "days" => amount * 86_400,
        _ => amount,
    }
}
