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
#[derive(Debug, Clone, PartialEq)]
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
    /// What it does to the character while it lasts: the per-intensity table behind `base_mods` and `scaling_mods`.
    pub mods: ModTable,
}

impl EffectType {
    /// What the effect adds to one of its modifiers at a given intensity: `get_mod_value` (`src/effect.cpp:563-582`).
    ///
    /// A missing entry is zero, which is how an effect that says nothing about, say, pain has no effect on it. Otherwise the answer is `base` plus `scaling` for every
    /// intensity above the first — so a modifier with a scaling and no base grows from nothing, and one with a base and no scaling is flat.
    #[must_use]
    pub fn mod_value(&self, argument: &str, action: ModAction, level: u8, intensity: i32) -> f64 {
        let Some(cells) = self.mods.get(argument) else {
            return 0.0;
        };
        let Some(values) = cells.get(&(action, level)) else {
            return 0.0;
        };
        values.base + values.scaling * f64::from(intensity - 1)
    }

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

/// One of the eight things a modifier can set: `mod_action` (`src/effect.h:70-79`), in the enum's own order, because the loader keys the table by the number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ModAction {
    /// How much of something to do.
    Amount,
    /// The bottom of a chance.
    ChanceBot,
    /// The top of a chance.
    ChanceTop,
    /// The top of a range.
    Max,
    /// The top of a range, as a value.
    MaxVal,
    /// The bottom of a range.
    Min,
    /// The bottom of a range, as a value.
    MinVal,
    /// How often it happens.
    Tick,
}

/// The two numbers a modifier cell holds: `mod_type` (`src/effect.h:60-65`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ModValues {
    /// The value at intensity one.
    pub base: f64,
    /// How much each further intensity adds.
    pub scaling: f64,
}

/// A modifier table: effect name, then (action, reduction level).
pub type ModTable = BTreeMap<String, BTreeMap<(ModAction, u8), ModValues>>;

/// Every modifier the loader knows, by the effect it applies to: `effect_type::load_mod_data` (`src/effect.cpp:333-560`).
///
/// The reference writes this table as thirty-one `extract_effect` calls, each a JSON member name and the action it sets. It is transcribed here in the same order so a
/// reader can diff the two side by side.
const MOD_KEYS: [(&str, &[(&str, ModAction)]); 31] = [
    ("STR", &[("str_mod", ModAction::Min)]),
    ("DEX", &[("dex_mod", ModAction::Min)]),
    ("PER", &[("per_mod", ModAction::Min)]),
    ("INT", &[("int_mod", ModAction::Min)]),
    ("SPEED", &[("speed_mod", ModAction::Min)]),
    ("PAIN", &[("pain_amount", ModAction::Amount), ("pain_min", ModAction::Min), ("pain_max", ModAction::Max), ("pain_max_val", ModAction::MaxVal), ("pain_chance", ModAction::ChanceTop), ("pain_chance_bot", ModAction::ChanceBot), ("pain_tick", ModAction::Tick)]),
    ("HURT", &[("hurt_amount", ModAction::Amount), ("hurt_min", ModAction::Min), ("hurt_max", ModAction::Max), ("hurt_chance", ModAction::ChanceTop), ("hurt_chance_bot", ModAction::ChanceBot), ("hurt_tick", ModAction::Tick)]),
    ("SLEEP", &[("sleep_amount", ModAction::Amount), ("sleep_min", ModAction::Min), ("sleep_max", ModAction::Max), ("sleep_chance", ModAction::ChanceTop), ("sleep_chance_bot", ModAction::ChanceBot), ("sleep_tick", ModAction::Tick)]),
    ("PKILL", &[("pkill_amount", ModAction::Amount), ("pkill_min", ModAction::Min), ("pkill_max", ModAction::Max), ("pkill_max_val", ModAction::MaxVal), ("pkill_chance", ModAction::ChanceTop), ("pkill_chance_bot", ModAction::ChanceBot), ("pkill_tick", ModAction::Tick)]),
    ("STIM", &[("stim_amount", ModAction::Amount), ("stim_min", ModAction::Min), ("stim_max", ModAction::Max), ("stim_min_val", ModAction::MinVal), ("stim_max_val", ModAction::MaxVal), ("stim_chance", ModAction::ChanceTop), ("stim_chance_bot", ModAction::ChanceBot), ("stim_tick", ModAction::Tick)]),
    ("HEALTH", &[("health_amount", ModAction::Amount), ("health_min", ModAction::Min), ("health_max", ModAction::Max), ("health_min_val", ModAction::MinVal), ("health_max_val", ModAction::MaxVal), ("health_chance", ModAction::ChanceTop), ("health_chance_bot", ModAction::ChanceBot), ("health_tick", ModAction::Tick)]),
    ("H_MOD", &[("h_mod_amount", ModAction::Amount), ("h_mod_min", ModAction::Min), ("h_mod_max", ModAction::Max), ("h_mod_min_val", ModAction::MinVal), ("h_mod_max_val", ModAction::MaxVal), ("h_mod_chance", ModAction::ChanceTop), ("h_mod_chance_bot", ModAction::ChanceBot), ("h_mod_tick", ModAction::Tick)]),
    ("RAD", &[("rad_amount", ModAction::Amount), ("rad_min", ModAction::Min), ("rad_max", ModAction::Max), ("rad_max_val", ModAction::MaxVal), ("rad_chance", ModAction::ChanceTop), ("rad_chance_bot", ModAction::ChanceBot), ("rad_tick", ModAction::Tick)]),
    ("HUNGER", &[("hunger_amount", ModAction::Amount), ("hunger_min", ModAction::Min), ("hunger_max", ModAction::Max), ("hunger_min_val", ModAction::MinVal), ("hunger_max_val", ModAction::MaxVal), ("hunger_chance", ModAction::ChanceTop), ("hunger_chance_bot", ModAction::ChanceBot), ("hunger_tick", ModAction::Tick)]),
    ("THIRST", &[("thirst_amount", ModAction::Amount), ("thirst_min", ModAction::Min), ("thirst_max", ModAction::Max), ("thirst_min_val", ModAction::MinVal), ("thirst_max_val", ModAction::MaxVal), ("thirst_chance", ModAction::ChanceTop), ("thirst_chance_bot", ModAction::ChanceBot), ("thirst_tick", ModAction::Tick)]),
    ("PERSPIRATION", &[("perspiration_amount", ModAction::Amount), ("perspiration_min", ModAction::Min), ("perspiration_max", ModAction::Max), ("perspiration_min_val", ModAction::MinVal), ("perspiration_max_val", ModAction::MaxVal), ("perspiration_chance", ModAction::ChanceTop), ("perspiration_chance_bot", ModAction::ChanceBot), ("perspiration_tick", ModAction::Tick)]),
    ("SLEEPINESS", &[("sleepiness_amount", ModAction::Amount), ("sleepiness_min", ModAction::Min), ("sleepiness_max", ModAction::Max), ("sleepiness_min_val", ModAction::MinVal), ("sleepiness_max_val", ModAction::MaxVal), ("sleepiness_chance", ModAction::ChanceTop), ("sleepiness_chance_bot", ModAction::ChanceBot), ("sleepiness_tick", ModAction::Tick)]),
    ("STAMINA", &[("stamina_amount", ModAction::Amount), ("stamina_min", ModAction::Min), ("stamina_max", ModAction::Max), ("stamina_max_val", ModAction::MaxVal), ("stamina_chance", ModAction::ChanceTop), ("stamina_chance_bot", ModAction::ChanceBot), ("stamina_tick", ModAction::Tick)]),
    ("BLOOD_PRESSURE", &[("blood_pressure_amount", ModAction::Amount), ("blood_pressure_min", ModAction::Min), ("blood_pressure_max", ModAction::Max), ("blood_pressure_max_val", ModAction::MaxVal), ("blood_pressure_min_val", ModAction::MinVal), ("blood_pressure_chance", ModAction::ChanceTop), ("blood_pressure_chance_bot", ModAction::ChanceBot), ("blood_pressure_tick", ModAction::Tick)]),
    ("HEART_RATE", &[("heart_rate_amount", ModAction::Amount), ("heart_rate_min", ModAction::Min), ("heart_rate_max", ModAction::Max), ("heart_rate_max_val", ModAction::MaxVal), ("heart_rate_min_val", ModAction::MinVal), ("heart_rate_chance", ModAction::ChanceTop), ("heart_rate_chance_bot", ModAction::ChanceBot), ("heart_rate_tick", ModAction::Tick)]),
    ("RESPIRATORY_RATE", &[("respiratory_rate_amount", ModAction::Amount), ("respiratory_rate_min", ModAction::Min), ("respiratory_rate_max", ModAction::Max), ("respiratory_rate_max_val", ModAction::MaxVal), ("respiratory_rate_min_val", ModAction::MinVal), ("respiratory_rate_chance", ModAction::ChanceTop), ("respiratory_rate_chance_bot", ModAction::ChanceBot), ("respiratory_rate_tick", ModAction::Tick)]),
    ("COUGH", &[("cough_chance", ModAction::ChanceTop), ("cough_chance_bot", ModAction::ChanceBot), ("cough_tick", ModAction::Tick)]),
    ("VOMIT", &[("vomit_chance", ModAction::ChanceTop), ("vomit_chance_bot", ModAction::ChanceBot), ("vomit_tick", ModAction::Tick)]),
    ("HEAL_RATE", &[("healing_rate", ModAction::Amount)]),
    ("HEAL_HEAD", &[("healing_head", ModAction::Amount)]),
    ("HEAL_TORSO", &[("healing_torso", ModAction::Amount)]),
    ("DODGE", &[("dodge_mod", ModAction::Min)]),
    ("HIT", &[("hit_mod", ModAction::Min)]),
    ("BASH", &[("bash_mod", ModAction::Min)]),
    ("CUT", &[("cut_mod", ModAction::Min)]),
    ("SIZE", &[("size_mod", ModAction::Min)]),
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

/// Reads one effect type from a single object, the way the reference's `load_effect_type` does.
///
/// The harness synthesises effect types at run time to exercise rules that no shipped type covers, and a recording of one is only reproducible if the port can read one
/// the same way — so this is public for the same reason `serde` is: a fixture is data.
#[must_use]
pub fn read_one(id: &str, object: &Object) -> EffectType {
    read(id, object)
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
        mods: mods(object),
    }
}

/// Reads the modifier table: `effect_type::load_mod_data` and `extract_effect` (`src/effect.cpp:287-560`).
///
/// The two members are read **by index**, and the index is the reduction level: `pain_min: [ 1, 0 ]` is one at the base level and zero when the effect is reduced. A
/// member that only `scaling_mods` states leaves the base level at zero, and the two are merged into one cell per (action, level) pair.
fn mods(object: &Object) -> ModTable {
    let mut out: ModTable = BTreeMap::new();
    for (member, scaling) in [("base_mods", false), ("scaling_mods", true)] {
        let Some(Value::Object(group)) = object.get(member) else {
            continue;
        };
        for (name, keys) in &MOD_KEYS {
            for (json_key, action) in *keys {
                let Some(Value::Array(values)) = group.get(json_key) else {
                    continue;
                };
                for (level, value) in values.iter().enumerate() {
                    // The data writes whole numbers without a decimal point, and `get_float` reads both.
                    let number = match value {
                        Value::Float(number) => *number,
                        Value::Int(number) => {
                            #[allow(clippy::cast_precision_loss)]
                            let number = *number as f64;
                            number
                        }
                        _ => continue,
                    };
                    let Ok(level) = u8::try_from(level) else {
                        continue;
                    };
                    let cell = out
                        .entry((*name).to_owned())
                        .or_default()
                        .entry((*action, level))
                        .or_default();
                    if scaling {
                        cell.scaling = number;
                    } else {
                        cell.base = number;
                    }
                }
            }
        }
    }
    out.retain(|_, cells| !cells.is_empty());
    out
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
