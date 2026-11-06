//! What each proficiency costs to learn, and what it needs first.
//!
//! Reference: `proficiency` (`proficiency.h:60-115`): `time_to_learn` (`proficiency.h:84`) is the number `practice` compares the practised time against, `can_learn`
//! says whether the data allows it to be learned at all, and `required_proficiencies` is the prerequisite set `has_prereqs` checks before it can be practised.
//!
//! The dump is the reference's own answer - `proficiency::get_all()` with those three fields - because the port loads data through dumps and never through the
//! reference's source.

use cdda_json::Value;
use std::collections::BTreeMap;

/// One proficiency, as the data defines it.
#[derive(Debug, Clone, PartialEq)]
pub struct Proficiency {
    /// Which proficiency.
    pub id: String,
    /// How long it takes to learn, in seconds.
    pub seconds: i64,
    /// Whether the data allows it to be learned by practising.
    pub can_learn: bool,
    /// The proficiencies that must be known first.
    pub required: Vec<String>,
    /// What it is good for, by the part of the game it helps (`proficiency::_bonuses`, `proficiency.h:87`).
    ///
    /// Each entry is a **category** - `melee_attack`, `athlete`, `archery` in this corpus - and the bonuses inside it. `bonus_for` takes the **first** entry of the type it is asked for,
    /// so the order is the data's and is kept.
    pub bonuses: Vec<(String, Vec<(String, f32)>)>,
}

impl Proficiency {
    /// The first bonus of this type in this category, which is what `proficiency::bonus_for` answers (`proficiency.cpp:224-238`).
    #[must_use]
    pub fn bonus_for(&self, category: &str, kind: &str) -> Option<f32> {
        self.bonuses
            .iter()
            .filter(|(name, _)| name == category)
            .flat_map(|(_, values)| values.iter())
            .find(|(bonus_kind, _)| bonus_kind == kind)
            .map(|(_, value)| *value)
    }
}

/// Every proficiency the data defines, in the reference's order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Proficiencies {
    /// The proficiencies by id.
    pub by_id: BTreeMap<String, Proficiency>,
}

impl Proficiencies {
    /// What one proficiency costs to learn, when the data defines it.
    #[must_use]
    pub fn seconds_for(&self, id: &str) -> Option<i64> {
        self.by_id.get(id).map(|prof| prof.seconds)
    }

    /// Whether a creature could learn this one, given what it already knows.
    ///
    /// Reference: `proficiency_set::has_prereqs` (`proficiency.cpp`) and `proficiency::can_learn`: a proficiency the data marks unlearnable is never learned, and one
    /// whose prerequisites are unknown waits for them.
    #[must_use]
    pub fn learnable_by(&self, id: &str, knows: &dyn Fn(&str) -> bool) -> bool {
        match self.by_id.get(id) {
            Some(prof) => prof.can_learn && prof.required.iter().all(|need| knows(need)),
            None => false,
        }
    }
}

/// Reads the proficiency dump.
///
/// Returns nothing when the dump is malformed, so a broken corpus file fails the load rather than half-loading.
#[must_use]
pub fn from_dump(root: &Value) -> Option<(Proficiencies, usize)> {
    let Value::Object(root) = root else {
        return None;
    };
    let Some(Value::Array(entries)) = root.get("proficiencies") else {
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
        let Some(Value::Int(seconds)) = entry.get("seconds") else {
            return None;
        };
        let can_learn = matches!(entry.get("can_learn"), Some(Value::Bool(true)));
        let required = match entry.get("required") {
            Some(Value::Array(needs)) => needs
                .iter()
                .filter_map(|need| match need {
                    Value::Str(id) => Some(id.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        // A version-one dump carries no bonuses at all, which is a proficiency that is good for nothing rather than a broken entry: the port reads the field when it is there
        // and answers an empty list when it is not.
        let bonuses = match entry.get("bonuses") {
            Some(Value::Array(categories)) => categories
                .iter()
                .filter_map(|category| {
                    let Value::Object(category) = category else {
                        return None;
                    };
                    let Some(Value::Str(name)) = category.get("category") else {
                        return None;
                    };
                    let Some(Value::Array(values)) = category.get("values") else {
                        return None;
                    };
                    let values: Vec<(String, f32)> = values
                        .iter()
                        .filter_map(|bonus| {
                            let Value::Object(bonus) = bonus else {
                                return None;
                            };
                            let Some(Value::Str(kind)) = bonus.get("type") else {
                                return None;
                            };
                            #[allow(clippy::cast_possible_truncation)]
                            let value = match bonus.get("value") {
                                Some(Value::Float(value)) => *value as f32,
                                #[allow(clippy::cast_precision_loss)]
                                Some(Value::Int(value)) => *value as f32,
                                _ => return None,
                            };
                            Some((kind.clone(), value))
                        })
                        .collect();
                    Some((name.clone(), values))
                })
                .collect(),
            _ => Vec::new(),
        };
        by_id.insert(
            id.clone(),
            Proficiency {
                id: id.clone(),
                seconds: *seconds,
                can_learn,
                required,
                bonuses,
            },
        );
    }
    let count = by_id.len();
    Some((Proficiencies { by_id }, count))
}
