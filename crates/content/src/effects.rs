//! Effect types: the 453 prototypes that every "you are now X" in the game names.
//!
//! Reference: `effect_type` (`src/effect.h:104-260`) and `load_effect_type` (`src/effect.cpp:1510-1620`). This family is unlike every other loader in the crate: it is
//! **not** a `generic_factory`, so there is no inheritance to resolve and no `copy-from` to merge — each object is read flat, with a default for every member it does
//! not state. That is also why the prototype digest could not see it: the digest walks the `generic_factory` registry, and effects are a plain `std::map`
//! (`src/effect.cpp:58`), so the harness now appends them by hand.
//!
//! The members here are the ones a **rule** reads: the intensity and duration ceiling, the decay settings that decide when an effect fades, the lists that make one
//! effect resist, remove or block another, and the rating that colours the message. The text — names, descriptions, memorial logs — is content the port will need for
//! the interface and for nothing else, and is not loaded yet.

use cdda_json::{Object, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// What a message about this effect looks like: `game_message_type` (`src/enums.h:361-377`).
///
/// The numbers matter: the recording compares them, and they are the enum's own order, not an alphabetisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rating {
    /// Something good happened.
    Good,
    /// Something bad happened.
    Bad,
    /// Both at once.
    Mixed,
    /// A warning about danger.
    Warning,
    /// Pure information.
    Info,
    /// Nothing much, which is the default.
    Neutral,
    /// Only shown in debug mode.
    Debug,
    /// A custom headshot.
    Headshot,
    /// A custom critical.
    Critical,
    /// A custom grazing hit.
    Grazing,
}

impl Rating {
    /// The enum's numeric value, which is what the recording holds.
    #[must_use]
    pub const fn value(self) -> i32 {
        match self {
            Self::Good => 0,
            Self::Bad => 1,
            Self::Mixed => 2,
            Self::Warning => 3,
            Self::Info => 4,
            Self::Neutral => 5,
            Self::Debug => 6,
            Self::Headshot => 7,
            Self::Critical => 8,
            Self::Grazing => 9,
        }
    }

    /// Reads the data's spelling: `process_rating` (`src/effect.cpp:701-715`). Anything else reads as no rating at all, which the loader takes as neutral.
    #[must_use]
    pub fn read(text: &str) -> Option<Self> {
        match text {
            "good" => Some(Self::Good),
            "bad" => Some(Self::Bad),
            "mixed" => Some(Self::Mixed),
            "neutral" => Some(Self::Neutral),
            _ => None,
        }
    }
}

/// One effect type, as far as the rules that read it are concerned.
// Eight flags in one prototype is what the reference has; splitting them into a bitfield struct would make the loader harder to read against the source, not easier.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectType {
    /// Its id.
    pub id: String,
    /// The highest intensity it can reach: the data may state it, and **one** is what it gets when it says nothing (`src/effect.cpp:1582`).
    pub max_intensity: i32,
    /// The longest it can last, in turns: 365 days unless the data says otherwise.
    pub max_duration: i64,
    /// How much of its duration each added intensity adds, as a percentage of the original, and a hundred when the data says nothing.
    pub dur_add_perc: i32,
    /// How much duration one added intensity adds outright, in turns.
    pub int_add_val: i32,
    /// How many intensity steps decay away at a time; **minus one** when the data says nothing, which is what keeps most effects from decaying at all.
    pub int_decay_step: i32,
    /// How often, in turns, an intensity step decays.
    pub int_decay_tick: i32,
    /// The duration that separates one intensity from the next.
    pub int_dur_factor: i64,
    /// Whether the effect is removed when its intensity decays to nothing.
    pub int_decay_remove: bool,
    /// Whether it only ever lands on the body parts that have hit points.
    pub main_parts_only: bool,
    /// Whether the effect is shown in the description.
    pub show_in_info: bool,
    /// Whether the intensity is shown next to the name, which it **is** unless the data says otherwise.
    pub show_intensity: bool,
    /// Whether its pain scales with the size of the body part.
    pub pain_sizing: bool,
    /// Whether its damage scales with the size of the body part.
    pub hurt_sizing: bool,
    /// Whether it makes the character cough.
    pub harmful_cough: bool,
    /// Whether it impairs movement, which a monster's move check reads.
    ///
    /// **Not a data member at all**: the loader looks the id up in a hardcoded set (`src/effect.cpp:1497-1509`), so the ten effects that trap or hold a creature are
    /// the only ones that can ever answer true.
    pub impairs_movement: bool,
    /// Whether a painkiller addict takes less of it.
    pub pkill_addict_reduces: bool,
    /// The ratings of its apply messages, in the order the data states them.
    pub apply_ratings: Vec<Rating>,
    /// Its flags.
    pub flags: BTreeSet<String>,
    /// The traits that resist it.
    pub resist_traits: Vec<String>,
    /// The effects that resist it.
    pub resist_effects: Vec<String>,
    /// The effects it removes when applied.
    pub removes_effects: Vec<String>,
    /// The effects it blocks.
    pub blocks_effects: Vec<String>,
}

impl EffectType {
    /// The rating of the message an intensity shows: `get_rating` (`src/effect.cpp:610-614`).
    ///
    /// The intensity is clamped into the message list, so an effect with a single message shows that message's rating at every intensity — and an effect with several
    /// shows the second one at an intensity of one.
    #[must_use]
    pub fn rating(&self, intensity: usize) -> Rating {
        if self.apply_ratings.is_empty() {
            return Rating::Neutral;
        }
        let last = self.apply_ratings.len() - 1;
        self.apply_ratings
            .get(intensity.min(last))
            .copied()
            .unwrap_or(Rating::Neutral)
    }
}

/// The effects a monster cannot move under: `hardcoded_movement_impairing` (`src/effect.cpp:1497-1509`).
///
/// This is the one member of an effect type that the data cannot set — the set is in the source, and an effect in it impairs movement whatever its JSON says.
pub const MOVEMENT_IMPAIRING: [&str; 10] = [
    "beartrap",
    "crushed",
    "downed",
    "grabbed",
    "heavysnare",
    "in_pit",
    "lightsnare",
    "tied",
    "webbed",
    "worked_on",
];

/// The default `max_duration`: 365 days (`src/effect.h:196`), in turns.
pub const DEFAULT_MAX_DURATION: i64 = 365 * 24 * 60 * 60;

/// Reads every effect type under `dirs`.
///
/// # Errors
///
/// If a file cannot be read or parsed.
pub fn load_paths(dirs: &[&Path]) -> Result<BTreeMap<String, EffectType>, crate::mapdata::LoadError> {
    let mut out: BTreeMap<String, EffectType> = BTreeMap::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| crate::mapdata::LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            let Value::Array(entries) = &*value else {
                continue;
            };
            for entry in entries {
                let Value::Object(object) = entry else {
                    continue;
                };
                if !matches!(object.get("type"), Some(Value::Str(kind)) if kind == "effect_type") {
                    continue;
                }
                let Some(id) = crate::items::string(object, "id") else {
                    continue;
                };
                out.insert(id.clone(), read(&id, object));
            }
        }
    }
    Ok(out)
}

/// Reads one effect type from its object.
fn read(id: &str, object: &Object) -> EffectType {
    EffectType {
        id: id.to_owned(),
        max_intensity: number(object, "max_intensity").unwrap_or(1),
        max_duration: duration(object, "max_duration").unwrap_or(DEFAULT_MAX_DURATION),
        dur_add_perc: number(object, "dur_add_perc").unwrap_or(100),
        int_add_val: number(object, "int_add_val").unwrap_or(0),
        int_decay_step: number(object, "int_decay_step").unwrap_or(-1),
        int_decay_tick: number(object, "int_decay_tick").unwrap_or(0),
        int_dur_factor: duration(object, "int_dur_factor").unwrap_or(0),
        int_decay_remove: flag(object, "int_decay_remove"),
        main_parts_only: flag(object, "main_parts_only"),
        show_in_info: flag(object, "show_in_info"),
        show_intensity: !matches!(object.get("show_intensity"), Some(Value::Bool(false))),
        pain_sizing: flag(object, "pain_sizing"),
        hurt_sizing: flag(object, "hurt_sizing"),
        harmful_cough: flag(object, "harmful_cough"),
        // Derived from the id, never read from the data.
        impairs_movement: MOVEMENT_IMPAIRING.contains(&id),
        pkill_addict_reduces: flag(object, "pkill_addict_reduces"),
        apply_ratings: ratings(object),
        flags: crate::items::strings(object, "flags").into_iter().collect(),
        resist_traits: crate::items::strings(object, "resist_traits"),
        resist_effects: crate::items::strings(object, "resist_effects"),
        removes_effects: crate::items::strings(object, "removes_effects"),
        blocks_effects: crate::items::strings(object, "blocks_effects"),
    }
}

/// The ratings of the effect's apply messages: `load_apply_msgs` (`src/effect.cpp:746-763`).
///
/// The data writes either a single message with an optional `"rating"` beside it, or an array of `[message, rating]` pairs. The rating is the **second** member of each
/// pair, and a pair without one is neutral. Only the ratings are kept: the text is interface, and nothing decides anything from it.
fn ratings(object: &Object) -> Vec<Rating> {
    if let Some(Value::Array(messages)) = object.get("apply_message") {
        return messages
            .iter()
            .map(|message| match message {
                Value::Array(pair) => pair
                    .get(1)
                    .and_then(|rating| match rating {
                        Value::Str(text) => Rating::read(text),
                        _ => None,
                    })
                    .unwrap_or(Rating::Neutral),
                _ => Rating::Neutral,
            })
            .collect();
    }
    // A single message, or none at all: either way there is exactly one entry, and its rating is the member beside it.
    let rating = if let Some(Value::Str(text)) = object.get("rating") {
        Rating::read(text).unwrap_or(Rating::Neutral)
    } else {
        Rating::Neutral
    };
    vec![rating]
}

/// A member that may be a whole number or a duration string, in turns.
fn duration(object: &Object, key: &str) -> Option<i64> {
    match object.get(key) {
        Some(Value::Int(turns)) => Some(*turns),
        Some(Value::Str(text)) => Some(crate::constructions::parse_duration(text)),
        _ => None,
    }
}

/// A whole-number member.
fn number(object: &Object, key: &str) -> Option<i32> {
    match object.get(key) {
        Some(Value::Int(value)) => Some(i32::try_from(*value).unwrap_or(0)),
        _ => None,
    }
}

/// A boolean member, false when absent.
fn flag(object: &Object, key: &str) -> bool {
    matches!(object.get(key), Some(Value::Bool(true)))
}
