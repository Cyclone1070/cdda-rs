//! The comestible slot: what eating a thing does, as the data states it.
//!
//! Reference: `islot_comestible` (`src/itype.h:147-248`) and `islot_comestible::deserialize` (`src/item_factory.cpp:3405-3441`). The slot is **not** a container: its
//! members are ordinary scalars, so a child that states `calories` keeps whatever `quench`, `spoils_in` and `fun` it inherited — `Item_factory::load_slot`
//! (`src/item_factory.cpp:2859-2868`) reuses the parent's slot object and then reads into it with `was_loaded`, which is `optional`'s keep-if-absent rule applied
//! member by member. That is the opposite of a stated `flags` array, which clears first.
//!
//! Two numbers do not survive the trip as written:
//!
//! * `calories` is **multiplied by 1,000** during finalisation (`src/item_factory.cpp:460`), so `"calories": 26` is stored as 26,000 and means 26 kcal.
//! * `nutr` is *derived* — `get_default_nutr()` is `default_nutrition.kcal() / kcal_per_nutr` with `kcal_per_nutr = base_metabolic_rate / ( 12 * 24 )`, which is
//!   2500/288 ≈ 8.68 kcal per unit of nutrition (`src/itype.h:231-239`). The port computes it the same way rather than reading it, and the gate compares the result.

use cdda_json::{Object, Value};
use std::collections::BTreeMap;

/// What the reference keeps about a comestible.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Comestible {
    /// Its subtype: `FOOD`, `DRINK`, `MED`, and the rest. `mandatory`.
    pub comestible_type: String,
    /// The tool needed to consume it - a lighter for a cigarette. **`"null"`** when none, which is how the reference spells its null item id; the port keeps that
    /// spelling rather than an empty string so a recording compares equal without a special case per field.
    pub tool: String,
    /// How many charges it has by default.
    pub charges: i32,
    /// How many uses fit in its volume, which the item level copies for its own `stack_size`.
    pub stack_size: i32,
    /// How much thirst it quenches, which may be **negative**.
    pub quench: i32,
    /// How much morale eating it gives, which may be **negative**.
    pub fun: i32,
    /// How much of a stimulant it is.
    pub stim: i32,
    /// How much it shifts sleepiness.
    pub sleepiness_mod: i32,
    /// How healthy it is, which may be **negative**.
    pub healthy: i32,
    /// The odds of catching a parasite from it, in the reference's own units.
    pub parasites: i32,
    /// How long until it rots, in turns; zero means it never does.
    pub spoils_in: i64,
    /// The penalty to fun for eating the same thing twice in two days. Defaults to **-1**.
    pub monotony_penalty: i32,
    /// Nutrition in **kilocalories**, as the data writes it.
    ///
    /// The reference multiplies this by 1,000 during finalisation and keeps the thousandths (`src/item_factory.cpp:460`); the port keeps the kilocalories so that
    /// `proportional` and `relative` adjust the same number the data states, and multiplies in [`Comestible::calories`] instead. Multiplying during the load would
    /// multiply a child's inherited value a second time - which is exactly what the first version of this loader did, and the gate named all 54 types it broke.
    pub kcal: i64,
    /// The item this cooks like, for recipe matching. Empty when it is itself.
    pub cooks_like: String,
    /// What smoking it produces. Empty when it cannot be smoked.
    pub smoking_result: String,
    /// The addiction potential an addiction without a weight takes. Defaults to 0.
    pub addiction_potential: i32,
    /// The pet food categories it feeds. Sorted, as the reference's `std::set` is.
    pub petfood: Vec<String>,
    /// Vitamins and how many units of each, after every mass was converted to units.
    pub vitamins: BTreeMap<String, i32>,
    /// Addictions and the potential of each. Sorted, as the reference's `std::map` is.
    pub addictions: BTreeMap<String, i32>,
    /// Diseases it may carry and the odds of each.
    pub contamination: BTreeMap<String, f32>,
    /// The temperature it freezes at, in degrees Celsius.
    pub freezing_point: f32,
    /// The material it is mostly made of, which **replaces** its material list. `"null"` when it has none.
    pub primary_material: String,
    /// What crawls out of it when it rots away.
    pub rot_spawn: RotSpawn,
}

/// What an item spawns when it rots: a monster or a monster group, how often, and how many.
///
/// Reference: `rot_spawn_data` (`src/itype.h:135-143`), loader `item.cpp:16200-16206`. Two things about it are worth stating:
///
/// * `monster` and `group` are **mutually exclusive**; the game draws one or the other (`src/map.cpp:8337-8338`).
/// * a prototype that **states** `rot_spawn` gets every member read with `was_loaded = false`, so the unstated ones take their **defaults** rather than the
///   parent's values. That is why the egg abstract, which states only `"chance": 0`, carries no monster at all and its children each restate theirs.
#[derive(Debug, Clone, PartialEq)]
pub struct RotSpawn {
    /// The monster that spawns. `"mon_null"` when it spawns a group instead.
    pub monster: String,
    /// The monster group that spawns. `"GROUP_NULL"` when it spawns a single monster.
    pub group: String,
    /// The chance out of 100 that anything spawns at all.
    pub chance: f32,
    /// How many, as an inclusive range.
    pub amount: (i32, i32),
}

impl Default for RotSpawn {
    fn default() -> Self {
        Self {
            monster: "mon_null".to_owned(),
            group: "GROUP_NULL".to_owned(),
            chance: 0.0,
            amount: (1, 1),
        }
    }
}

impl RotSpawn {
    /// Reads a stated `rot_spawn`, from the **defaults** rather than from the inherited value.
    ///
    /// # Errors
    ///
    /// If `chance` is not a number or `amount` is not a two-number array.
    pub fn read(object: &Object) -> Result<Self, super::mapdata::LoadError> {
        let mut value = Self::default();
        value.monster = super::items::text_or(object, "monster", &value.monster);
        value.group = super::items::text_or(object, "group", &value.group);
        if let Some(number) = object.get("chance") {
            value.chance = super::items::as_f32("rot_spawn.chance", number)?;
        }
        if let Some(number) = object.get("amount") {
            let Value::Array(pair) = number else {
                return Err(super::mapdata::LoadError {
                    message: "rot_spawn.amount is a two-number array".to_owned(),
                });
            };
            let [low, high] = pair.as_slice() else {
                return Err(super::mapdata::LoadError {
                    message: "rot_spawn.amount is a two-number array".to_owned(),
                });
            };
            value.amount = (
                super::items::as_i32("rot_spawn.amount", low)?,
                super::items::as_i32("rot_spawn.amount", high)?,
            );
        }
        Ok(value)
    }
}

/// How many thousandths of a kilocalorie one unit of nutrition is worth: `base_metabolic_rate / ( 12 * 24 )` (`src/itype.h:132`, `:231`).
///
/// The reference writes it as `2500.0f / 288`, a **float** division, and divides in that type — so the port divides in `f32` too. Doing it in `f64` changes the
/// truncation for a handful of values, which the gate over 1,686 types would catch.
#[must_use]
pub fn kcal_per_nutr() -> f32 {
    2500.0 / (12.0 * 24.0)
}

impl Comestible {
    /// The stored nutrition, in thousandths of a kilocalorie, which is what the reference's slot holds after finalisation.
    #[must_use]
    pub fn calories(&self) -> i64 {
        self.kcal * 1000
    }

    /// Its nutrition in the reference's own units: `get_default_nutr()` (`src/itype.h:237-239`).
    ///
    /// `nutrients::kcal()` is the stored thousandths divided by 1,000 — an **integer** division, so 26,900 becomes 26 — and that integer is then divided by
    /// `kcal_per_nutr` in `float` and truncated to `int` by the return type.
    // The reference divides an `int` by a `float` and returns an `int`, so both casts are the arithmetic, not a shortcut.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    #[must_use]
    pub fn nutr(&self) -> i32 {
        let kcal = i32::try_from(self.kcal).unwrap_or(i32::MAX);
        (kcal as f32 / kcal_per_nutr()) as i32
    }
}

/// Reads the slot out of an item object, keeping what an inherited slot already had.
///
/// `inherited` is the slot the prototype's `copy-from` parent supplied, or `None`. A member that is absent leaves the inherited value alone — that is
/// `load_slot`'s `was_loaded` path, and it is why this takes the parent's value rather than starting empty.
///
/// # Errors
///
/// If a vitamin is not registered, or a vitamin amount written as a mass does not parse.
pub fn read(
    object: &Object,
    inherited: Option<&Comestible>,
    vitamins: &BTreeMap<String, super::vitamins::Vitamin>,
) -> Result<Option<Comestible>, super::mapdata::LoadError> {
    let is_comestible = super::items::string(object, "comestible_type").is_some()
        || super::items::strings(object, "subtypes")
            .iter()
            .any(|kind| kind == "COMESTIBLE")
        || inherited.is_some();
    if !is_comestible {
        return Ok(None);
    }
    let mut value = inherited.cloned().unwrap_or_else(|| Comestible {
        tool: "null".to_owned(),
        primary_material: "null".to_owned(),
        // `-1` means "not stated", which finalisation turns into 2 or 0 (`src/item_factory.cpp:495-497`). It has to stay distinct from a stated 0.
        monotony_penalty: -1,
        ..Comestible::default()
    });
    if let Some(kind) = super::items::string(object, "comestible_type") {
        value.comestible_type = kind;
    }
    if let Some(tool) = super::items::string(object, "tool") {
        value.tool = tool;
    }
    value.charges = adjusted(object, "charges", value.charges);
    value.stack_size = adjusted(object, "stack_size", value.stack_size);
    value.quench = adjusted(object, "quench", value.quench);
    value.fun = adjusted(object, "fun", value.fun);
    value.stim = adjusted(object, "stim", value.stim);
    value.sleepiness_mod = adjusted(object, "sleepiness_mod", value.sleepiness_mod);
    value.healthy = adjusted(object, "healthy", value.healthy);
    value.parasites = adjusted(object, "parasites", value.parasites);
    value.monotony_penalty = adjusted(object, "monotony_penalty", value.monotony_penalty);
    value.addiction_potential = adjusted(object, "addiction_potential", value.addiction_potential);
    value.kcal = adjusted_i64(object, "calories", value.kcal);
    if let Some(spoils) = spoils_in(object) {
        value.spoils_in = spoils;
    }
    if let Some(cooks_like) = super::items::string(object, "cooks_like") {
        value.cooks_like = cooks_like;
    }
    if let Some(result) = super::items::string(object, "smoking_result") {
        value.smoking_result = result;
    }
    if object.get("petfood").is_some() {
        value.petfood = super::items::strings(object, "petfood");
    }
    // **`addiction_type` falls back to the item's own `addiction_potential`**, and that member is read two lines earlier in the reference
    // (`src/item_factory.cpp:3432-3435`), so an addiction written without a weight takes a value this object may have inherited.
    if object.get("addiction_type").is_some() {
        value.addictions = read_addictions(object, value.addiction_potential);
    }
    // Vitamins are a **list of pairs**, not a map, and their amounts may be masses; `nutrients::finalize_vitamins` converts each mass on its own vitamin's scale.
    //
    // An object that states none keeps what it inherited, and a comestible that ends up with **no** vitamins and a non-negative healthy rating gets them derived from
    // its materials instead (`src/item_factory.cpp:762-776`) - which the caller does, because it is the only one holding the material registry.
    if object.get("vitamins").is_some() {
        value.vitamins = super::vitamins::read_list(object, vitamins)?;
    } else {
        // **`extend.vitamins` adds to the inherited map and `delete.vitamins` takes ids away** - and they are only reached when this object does not state
        // `vitamins` itself, because a stated container member clears first (`src/generic_factory.h:1168-1192`). `protein_shake` is the one prototype that uses
        // it: it inherits its parent's calcium and extends with `[["vitC", "20 mg"], ["fruit_allergen", 1]]`, and 20 mg of `vitC` is **21 units** on that
        // vitamin's 938 μg scale.
        if let Some(extend) = super::items::sub_object(object, "extend")
            && extend.get("vitamins").is_some()
        {
            for (id, units) in super::vitamins::read_list(extend, vitamins)? {
                value.vitamins.insert(id, units);
            }
        }
        if let Some(delete) = super::items::sub_object(object, "delete")
            && let Some(Value::Array(entries)) = delete.get("vitamins")
        {
            for entry in entries {
                if let Value::Str(id) = entry {
                    value.vitamins.remove(id);
                }
            }
        }
    }
    value.rot_spawn = match super::items::sub_object(object, "rot_spawn") {
        Some(inner) => RotSpawn::read(inner)?,
        None => inherited
            .map(|slot| slot.rot_spawn.clone())
            .unwrap_or_default(),
    };

    if object.get("contamination").is_some() {
        value.contamination = read_contamination(object);
    }
    if let Some(point) = object.get("freezing_point").and_then(super::items::numeric) {
        #[allow(clippy::cast_possible_truncation)]
        {
            value.freezing_point = point as f32;
        }
    }
    if let Some(material) = super::items::string(object, "primary_material") {
        value.primary_material = material;
    }
    Ok(Some(value))
}

/// A numeric member as the reference's reader leaves it: a stated value, else the inherited one, then `relative` adds or `proportional` scales it.
///
/// Reference: `generic_typed_reader::operator()` (`src/generic_factory.h:1251-1259`) tries `read_normal`, then `handle_proportional`, then `do_relative` — and
/// **returns after the first that applies**, so a member with both blocks gets only the proportional. `proportional` multiplies in `double` and the compound
/// assignment truncates toward zero (`src/generic_factory.h:806-820`); `relative` adds and the reader's own bound check runs after the add.
fn adjusted(object: &Object, member: &str, current: i32) -> i32 {
    i32::try_from(adjusted_i64(object, member, i64::from(current))).unwrap_or(current)
}

// The reference reads an `int` (or `int64_t`) through `double`, and `member *= scalar` truncates; both casts are that arithmetic.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn adjusted_i64(object: &Object, member: &str, current: i64) -> i64 {
    let stated = match object.get(member) {
        Some(Value::Int(value)) => Some(*value),
        Some(Value::Float(value)) => Some(value.trunc() as i64),
        _ => None,
    };
    let base = stated.unwrap_or(current);
    if let Some(scalar) = block_value(object, "proportional", member) {
        // `member *= scalar` on an integer member truncates toward zero, which is why a 0.5 scale takes 32 to 16 rather than 16.0.
        return (base as f64 * scalar).trunc() as i64;
    }
    match block_value(object, "relative", member) {
        Some(adder) => base.saturating_add(adder.trunc() as i64),
        None => base,
    }
}

/// `addiction_type`, in both forms the data uses: a list of ids, or a list of one-key objects naming an id and its weight.
///
/// Reference: `weighted_string_id_reader<addiction_id, int>` (`src/generic_factory.h:1430-1462`) with `default_addict_potential` as its default weight: an entry
/// written as a bare string takes the item's potential, an entry written as `{"alcohol": 5}` takes **5**. Only 3 comestibles use the object form, and 113 use the
/// bare string, so the fallback is the common path and the object form is the rare one.
#[allow(clippy::cast_possible_truncation)]
fn read_addictions(object: &Object, potential: i32) -> BTreeMap<String, i32> {
    let mut out = BTreeMap::new();
    // **A bare string is the common form** - 113 comestibles write `"addiction_type": "alcohol"` rather than a one-element array - and it means the item's own
    // `addiction_potential`.
    if let Some(Value::Str(id)) = object.get("addiction_type") {
        out.insert(id.clone(), potential);
        return out;
    }
    let Some(Value::Array(entries)) = object.get("addiction_type") else {
        return out;
    };
    for entry in entries {
        match entry {
            Value::Str(id) => {
                out.insert(id.clone(), potential);
            }
            // The object form names its members: `{"addiction": "alcohol", "potential": 5}`. The reference's weighted reader is shape-blind - the first string
            // member is the id and the first number is the weight (`src/generic_factory.h:1437-1453`) - so a bare one-key object works too, and the port scans the
            // same way rather than insisting on the names.
            Value::Object(pair) => {
                let mut id: Option<String> = None;
                let mut weight = potential;
                for (key, value) in pair.members() {
                    match value {
                        Value::Str(name) => id = Some(name.clone()),
                        Value::Int(weight_value) => {
                            weight = i32::try_from(*weight_value).unwrap_or(potential);
                        }
                        Value::Float(weight_value) => weight = *weight_value as i32,
                        _ => id = Some(key.clone()),
                    }
                }
                if let Some(id) = id {
                    out.insert(id, weight);
                }
            }
            _ => {}
        }
    }
    out
}

/// `contamination`: a list of `{"disease": ..., "probability": ...}` objects in every place the content uses it.
///
/// Reference: `weighted_string_id_reader<diseasetype_id, float>` with a default weight of **1.0** (`src/item_factory.cpp:3427-3428`), which the port keeps for an
/// entry that names a disease without a probability.
fn read_contamination(object: &Object) -> BTreeMap<String, f32> {
    let mut out = BTreeMap::new();
    let Some(Value::Array(entries)) = object.get("contamination") else {
        return out;
    };
    for entry in entries {
        let Value::Object(pair) = entry else {
            continue;
        };
        let mut disease = None;
        let mut probability = 1.0_f64;
        for (key, value) in pair.members() {
            match key.as_str() {
                "disease" | "id" => {
                    if let Value::Str(id) = value {
                        disease = Some(id.clone());
                    }
                }
                "probability" | "chance" => {
                    if let Some(chance) = super::items::numeric(value) {
                        probability = chance;
                    }
                }
                _ => {}
            }
        }
        if let Some(disease) = disease {
            #[allow(clippy::cast_possible_truncation)]
            out.insert(disease, probability as f32);
        }
    }
    out
}

#[allow(clippy::cast_precision_loss)]
fn block_value(object: &Object, block: &str, member: &str) -> Option<f64> {
    let inner = super::items::sub_object(object, block)?;
    match inner.get(member) {
        Some(Value::Int(value)) => Some(*value as f64),
        Some(Value::Float(value)) => Some(*value),
        _ => None,
    }
}

/// `spoils_in`, in turns.
///
/// The content writes it as a duration **string** in all 716 places that state it (`"4 hours"`, `"1 day"`), which the reference reads through its time reader;
/// a whole number of turns is accepted too, because the reader takes an integer as turns (`src/units_reader.h`).
fn spoils_in(object: &Object) -> Option<i64> {
    object.get("spoils_in").map(|value| match value {
        Value::Int(turns) => *turns,
        Value::Str(text) => crate::constructions::parse_duration(text),
        _ => 0,
    })
}

/// Runs the two per-prototype rules that `finalize_pre` applies once the whole `copy-from` chain is loaded.
///
/// Reference: `item_factory.cpp:762-776`, which runs per **prototype** during finalisation. Two consequences the recording settled:
///
/// * the derivation sees the **raw** inherited slot, not a parent's derived one, so each prototype derives its own numbers;
/// * it runs after `primary_material` has replaced the material list (`:463-467`), which is why `mead` - materials `["alcohol", "honey"]`, primary material
///   `alcohol` - has no honey vitamins at all.
pub(crate) fn finalise(
    item: &mut super::items::ItemType,
    registries: &super::items::Registries<'_>,
) {
    // **A boring food gets boring.** `monotony_penalty` starts at `-1` and becomes **0 when any of the item's materials is `junk`**, else 2
    // (`src/item_factory.cpp:495-497`) — the one member whose default is computed from the material list rather than from a constant.
    if let Some(slot) = item.comestible.as_mut()
        && slot.monotony_penalty == -1
    {
        slot.monotony_penalty = i32::from(!item.materials.contains_key("junk")) * 2;
    }
    if let Some(slot) = item.comestible.as_mut()
        && slot.vitamins.is_empty()
        && slot.healthy >= 0
    {
        slot.vitamins = super::vitamins::derived_from_materials(
            &item.materials,
            slot.healthy,
            registries.vitamins,
            registries.materials,
        );
    }
}
