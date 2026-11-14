//! What examining a square does, as far as the data goes.
//!
//! Reference: `map_data_common_t::examine_func` and the data-driven `examine_actor` beside it (`src/mapdata.h:500-505`). The JSON's `"examine_action"` fills **one of
//! two** things, and which one matters for a port:
//!
//! * a **bare name** — 73 of the 77 names in the content, 452 uses — becomes a hardcoded function (`iexamine_functions_from_string`, `src/iexamine.cpp:7741`). It
//!   carries no data at all, so the port keeps the name and nothing else;
//! * an **object** with a `"type"` — 4 names, 80 uses — becomes an actor that carries data, and the actor decides what the fields mean.
//!
//! The four object actors are `appliance_convert` (38), `cardreader` (21), `effect_on_condition` (20) and `mortar` (1). Their defaults are not decoration:
//! `consume_card`, `allow_hacking`, `despawn_monsters` and `query` are all **true** when unstated, and a card reader's `radius` is **3**.

use cdda_json::{Object, Value};
use std::collections::BTreeMap;

/// What a square's `examine_action` says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExamineAction {
    /// A hardcoded function, by name: `curtains`, `chainfence`, `water_source`, and the other 70.
    Named(String),
    /// Turns the square into an appliance: the furniture and terrain it becomes, and the item that represents it.
    ///
    /// Reference: `appliance_convert_examine_actor` (`src/iexamine_actors.h:22-37`, `load` at `src/iexamine_actors.cpp:56-61`). `item` is **mandatory**; the two
    /// positions are not, and a square that states neither becomes an appliance without changing what it looks like.
    ApplianceConvert {
        /// The furniture it becomes, or empty.
        furn_set: String,
        /// The terrain it becomes, or empty.
        ter_set: String,
        /// The item that stands for the appliance.
        item: String,
    },
    /// A card reader: what cards it takes, what it does to the map, and what it says.
    ///
    /// Reference: `cardreader_examine_actor` (`src/iexamine_actors.h:40-73`, `load` at `src/iexamine_actors.cpp:241-261`). **Either** a `mapgen_id`, which
    /// regenerates the whole overmap tile, **or** a `radius` (default **3**) with the terrain and furniture changes to make inside it — the two are exclusive in the
    /// loader, not merely in use.
    CardReader(Box<CardReader>),
    /// Runs a list of effect-on-conditions, by id.
    ///
    /// Reference: `eoc_examine_actor` (`src/iexamine_actors.h`). The ids are inline definitions as often as they are references, which is why the effect loader
    /// reads this slot by name.
    EffectOnCondition {
        /// The effect ids, in the data's order.
        ids: Vec<String>,
    },
    /// Fires a mortar, which needs the dialogue system's conditions and effects; **declared**, one use.
    Mortar,
}

impl ExamineAction {
    /// The name the reference knows this action by: the hardcoded function's name for a bare string, and the **actor's `type`** for an object.
    ///
    /// The distinction matters to a recording, which can only see what the reference holds: it has one name per square (`iexamine_actor::type` or a table entry), so
    /// a port that answered with its own Rust variant name would disagree on all 80 object tiles.
    #[must_use]
    pub fn kind(&self) -> &str {
        match self {
            Self::Named(name) => name,
            Self::ApplianceConvert { .. } => "appliance_convert",
            Self::CardReader(_) => "cardreader",
            Self::EffectOnCondition { .. } => "effect_on_condition",
            Self::Mortar => "mortar",
        }
    }
}

/// A card reader's data.
///
/// Four independent switches, because the reference has four: a data mirror with fewer fields would have to invent a grouping the reference does not have.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardReader {
    /// The item flags a card must carry (`"flags"`, mandatory).
    pub flags: Vec<String>,
    /// Whether the card is consumed when it works. Default **true**.
    pub consume_card: bool,
    /// Whether the reader can be hacked. Default **true**.
    pub allow_hacking: bool,
    /// Whether the monsters inside are removed. Default **true**.
    pub despawn_monsters: bool,
    /// A mapgen to regenerate the overmap tile with, or empty for the radius path.
    pub mapgen_id: String,
    /// How far the changes reach. Default **3**, and only read when there is no `mapgen_id`.
    pub radius: i32,
    /// What terrain becomes what, inside the radius.
    pub terrain_changes: BTreeMap<String, String>,
    /// What furniture becomes what, inside the radius.
    pub furniture_changes: BTreeMap<String, String>,
    /// Whether to ask before acting. Default **true**.
    pub query: bool,
    /// What to ask. Empty means the actor's own sentence.
    pub query_msg: String,
    /// What to say when the card works (mandatory).
    pub success_msg: String,
    /// What to say when the square is already in the target state (mandatory).
    pub redundant_msg: String,
}

/// Reads a square's `examine_action`.
///
/// # Errors
///
/// If a member is present with the wrong shape.
pub fn read(object: &Object) -> Result<Option<ExamineAction>, crate::mapdata::LoadError> {
    let Some(value) = object.get("examine_action") else {
        return Ok(None);
    };
    match value {
        // A bare name. **`"none"` would be a real action if anything stated it** — the reference's default function *is* `iexamine::none`, so a prototype that says
        // nothing and one that says `"none"` are indistinguishable in a dump. No prototype in this content writes it at all: all 1,208 `none` tiles in the recording
        // are defaults.
        Value::Str(name) => Ok(Some(ExamineAction::Named(name.clone()))),
        Value::Object(actor) => {
            let Some(Value::Str(kind)) = actor.get("type") else {
                return Ok(None);
            };
            match kind.as_str() {
                "appliance_convert" => Ok(Some(ExamineAction::ApplianceConvert {
                    furn_set: text(actor, "furn_set"),
                    ter_set: text(actor, "ter_set"),
                    item: text(actor, "item"),
                })),
                "cardreader" => Ok(Some(ExamineAction::CardReader(Box::new(read_cardreader(
                    actor,
                ))))),
                "effect_on_condition" => Ok(Some(ExamineAction::EffectOnCondition {
                    ids: effect_ids(actor),
                })),
                "mortar" => Ok(Some(ExamineAction::Mortar)),
                other => Ok(Some(ExamineAction::Named(other.to_owned()))),
            }
        }
        _ => Ok(None),
    }
}

fn read_cardreader(actor: &Object) -> CardReader {
    CardReader {
        flags: strings(actor, "flags"),
        consume_card: boolean(actor, "consume_card", true),
        allow_hacking: boolean(actor, "allow_hacking", true),
        despawn_monsters: boolean(actor, "despawn_monsters", true),
        mapgen_id: text(actor, "mapgen_id"),
        radius: match actor.get("radius") {
            Some(Value::Int(radius)) => i32::try_from(*radius).unwrap_or(3),
            _ => 3,
        },
        terrain_changes: id_map(actor, "terrain_changes"),
        furniture_changes: id_map(actor, "furn_changes"),
        query: boolean(actor, "query", true),
        query_msg: text(actor, "query_msg"),
        success_msg: text(actor, "success_msg"),
        redundant_msg: text(actor, "redundant_msg"),
    }
}

/// The effect ids a slot holds: each element is a name or an object with its own `id`.
fn effect_ids(actor: &Object) -> Vec<String> {
    let Some(Value::Array(entries)) = actor.get("effect_on_conditions") else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| match entry {
            Value::Str(id) => Some(id.clone()),
            Value::Object(effect) => match effect.get("id") {
                Some(Value::Str(id)) => Some(id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn text(object: &Object, key: &str) -> String {
    match object.get(key) {
        Some(Value::Str(text)) => text.clone(),
        _ => String::new(),
    }
}

fn strings(object: &Object, key: &str) -> Vec<String> {
    match object.get(key) {
        Some(Value::Array(entries)) => entries
            .iter()
            .filter_map(|entry| match entry {
                Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn boolean(object: &Object, key: &str, fallback: bool) -> bool {
    match object.get(key) {
        Some(Value::Bool(flag)) => *flag,
        _ => fallback,
    }
}

fn id_map(object: &Object, key: &str) -> BTreeMap<String, String> {
    let Some(Value::Object(entries)) = object.get(key) else {
        return BTreeMap::new();
    };
    entries
        .members()
        .iter()
        .filter_map(|(from, to)| match to {
            Value::Str(to) => Some((from.clone(), to.clone())),
            _ => None,
        })
        .collect()
}
