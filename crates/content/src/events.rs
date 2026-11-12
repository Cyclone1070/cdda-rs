//! The **event vocabulary**: what each event type's arguments mean.
//!
//! Reference: `event_type` (`event.h:24-137`) and the `event_spec` table (`:173-990`). A `trigger_event` entry names a type and gives its arguments positionally
//! (`npctalk.cpp:7894-7911`), and a position means whatever that type's field list says at that index - so the table is data the port must have, not a convention it can assume.
//!
//! The table is dumped from the reference by `--dump-events` and lives in `corpus/events.json`, because `cata::event::get_fields` returns an unordered map and the order is
//! what an argument list fills. Nothing here is transcribed by hand: a name the dump does not carry is a name this port does not know.

use std::collections::BTreeMap;

/// Every event type's argument names, in the order its `args` list fills them.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct EventTable {
    fields: BTreeMap<String, Vec<String>>,
}

impl EventTable {
    /// Reads a `--dump-events` dump, or nothing when it is not that dump.
    #[must_use]
    pub fn read(document: &cdda_json::Value) -> Option<Self> {
        let cdda_json::Value::Object(root) = document else {
            return None;
        };
        if let Some(cdda_json::Value::Str(harness)) = root.get("harness")
            && harness != "events"
        {
            return None;
        }
        let cdda_json::Value::Array(events) = root.get("events")? else {
            return None;
        };
        let mut fields = BTreeMap::new();
        for event in events {
            let cdda_json::Value::Object(entry) = event else {
                return None;
            };
            let Some(cdda_json::Value::Str(kind)) = entry.get("type") else {
                return None;
            };
            let cdda_json::Value::Array(declared) = entry.get("fields")? else {
                return None;
            };
            let mut names = Vec::new();
            for field in declared {
                let cdda_json::Value::Object(field) = field else {
                    return None;
                };
                let Some(cdda_json::Value::Str(name)) = field.get("name") else {
                    return None;
                };
                names.push(name.clone());
            }
            fields.insert(kind.clone(), names);
        }
        Some(Self { fields })
    }

    /// What each position of a `trigger_event` argument list means, or nothing when the type is unknown.
    #[must_use]
    pub fn fields_of(&self, kind: &str) -> Option<&[String]> {
        // **Unknown is unknown**: a type the dump does not carry gets no names, rather than an empty list that reads like "this type takes no arguments".
        self.fields.get(kind).map(Vec::as_slice)
    }

    /// How many event types the table holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Whether the table is empty, which only an unread dump is.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}
