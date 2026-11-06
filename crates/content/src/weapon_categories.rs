//! Which proficiencies a kind of weapon has, and therefore which ones make it cheaper to swing.
//!
//! Reference: `weapon_category` (`weapon_category.h`) and the two rules that read it - `wielded_weapon_categories` (`melee.cpp:570-578`), which answers the item's own category set,
//! and `get_total_melee_stamina_cost` (`melee.cpp:980-992`), which walks each category's `category_proficiencies()`.
//!
//! **The pairing is the data's, not a naming convention**: `weapon_categories.json` states which proficiencies belong to a category, and an item states which categories it belongs to.
//! `prof_short_swords_master` happens to look like a short sword's proficiency, but a port that guessed from the name would get the common case right and the corpus wrong.

use cdda_json::Value;

/// One kind of weapon, and the proficiencies that make it cheaper to swing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponCategory {
    /// Its id, as an item's `weapon_category` names it (`SHORT_SWORDS`).
    pub id: String,
    /// The proficiencies a swinger of this kind may have, in the data's own order.
    pub proficiencies: Vec<String>,
}

/// Reads every weapon category in a data file.
#[must_use]
pub fn from_data(value: &Value) -> Vec<WeaponCategory> {
    let single = [value.clone()];
    let entries: &[Value] = match value {
        Value::Array(entries) => entries,
        Value::Object(_) => &single,
        _ => &[],
    };
    let mut out = Vec::new();
    for entry in entries {
        let Value::Object(object) = entry else {
            continue;
        };
        // A data file holds many object types side by side, so the entry's own type is the filter.
        if !matches!(object.get("type"), Some(Value::Str(kind)) if kind == "weapon_category") {
            continue;
        }
        let Some(Value::Str(id)) = object.get("id") else {
            continue;
        };
        let proficiencies = match object.get("proficiencies") {
            Some(Value::Array(list)) => list
                .iter()
                .filter_map(|prof| match prof {
                    Value::Str(prof) => Some(prof.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        out.push(WeaponCategory {
            id: id.clone(),
            proficiencies,
        });
    }
    out
}
