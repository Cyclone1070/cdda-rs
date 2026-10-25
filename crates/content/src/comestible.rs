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

/// What the reference keeps about a comestible.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
#[must_use]
pub fn read(object: &Object, inherited: Option<&Comestible>) -> Option<Comestible> {
    let is_comestible = super::items::string(object, "comestible_type").is_some()
        || super::items::strings(object, "subtypes").iter().any(|kind| kind == "COMESTIBLE")
        || inherited.is_some();
    if !is_comestible {
        return None;
    }
    let mut value = inherited.cloned().unwrap_or_else(|| Comestible {
        tool: "null".to_owned(),
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
    Some(value)
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
