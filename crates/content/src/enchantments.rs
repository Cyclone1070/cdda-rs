//! The enchantments that data files never name.
//!
//! Reference: `enchantment::load_inline_enchantment` (`src/magic_enchantment.cpp:267-285`) and its four call sites - a mutation (`mutation_data.cpp:440-445`),
//! a bionic (`bionics.cpp:379-384`), an effect (`effect.cpp:1609-1614`) and a vehicle part (`veh_type.cpp:306-311`).
//!
//! A slot named `enchantments` holds a list; each element is either a **string**, which names an enchantment somebody else defined, or an **object**, which the
//! reference registers on the spot under a name it invents:
//!
//! ```text
//! INLINE_ENCH_<owner id>_<the element's index>
//! ```
//!
//! Two details decide whether the names come out right, and both are the sort of thing a count cannot catch:
//!
//! - the index counts **every** element, so an object after a string is `_2`, not `_1`;
//! - the counter belongs to one owner, and a new owner starts again at zero.

use cdda_json::Value;
use std::collections::BTreeSet;

/// The JSON types that can own inline enchantments, with the member that names the owner.
///
/// The reference spells the owner differently at each call site, and one of them is a trap: a mutation and an effect use their **id**, and a vehicle part uses
/// its id, but a bionic uses its **name** - the words a player reads, so its enchantments are called `INLINE_ENCH_Alarm System_0` and not
/// `INLINE_ENCH_bio_alarm_0` (`bionics.cpp:381`). A count cannot tell those apart; the registry's digest can, and did.
const OWNERS: [(&str, &str); 4] = [
    ("mutation", "id"),
    ("bionic", "name"),
    ("effect_type", "id"),
    ("vehicle_part", "id"),
];

/// Every inline enchantment id under one parsed document, sorted.
///
/// Sorted rather than in load order because this answers "which names exist"; the order the reference registers them in is a property of the walk, which the
/// registry does not record.
#[must_use]
pub fn inline_ids(value: &Value) -> Vec<String> {
    let mut out = BTreeSet::new();
    collect(value, &mut out);
    out.into_iter().collect()
}

/// The words a JSON member holds, whichever of the reference's spellings it uses: a plain string, or the object form `{"str": "..."}` that a translatable one
/// takes. The reference asks for a `std::string` at every call site and gets exactly this.
fn text_of(value: &Value) -> Option<&str> {
    match value {
        Value::Str(text) => Some(text),
        Value::Object(object) => match object.get("str")? {
            Value::Str(text) => Some(text),
            _ => None,
        },
        _ => None,
    }
}

fn collect(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, out);
            }
        }
        Value::Object(object) => {
            if let Some(Value::Str(kind)) = object.get("type")
                && let Some((_, owner_member)) = OWNERS.iter().find(|(owns, _)| owns == kind)
                && let Some(Some(owner)) = object.get(owner_member).map(text_of)
                && let Some(Value::Array(slots)) = object.get("enchantments")
            {
                for (index, slot) in slots.iter().enumerate() {
                    if matches!(slot, Value::Object(_)) {
                        out.insert(format!("INLINE_ENCH_{owner}_{index}"));
                    }
                }
            }
            for (_, member) in object.members() {
                collect(member, out);
            }
        }
        _ => {}
    }
}
