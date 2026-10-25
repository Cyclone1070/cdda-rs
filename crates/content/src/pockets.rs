//! What a thing can hold, read from its `pocket_data`.
//!
//! Reference: `pocket_data` (`src/item_pocket.h:470-495`) as loaded by `item_pocket` (`src/item_pocket.cpp:150-200`), and the seven questions
//! `item_pocket` asks of it: the capacity, the weight it takes, whether a liquid goes in, whether a gas goes in, and the three compatibility
//! limits — a longest side, a shortest side and a smallest volume.
//!
//! **The one rule a copier would get wrong** is the longest side. The data usually states none, and then the reference fills it in at load time
//! with `default_length_from_volume( volume_capacity ) * sqrt( 2 )` — the diagonal of a cube of the pocket's capacity, because a long thin thing
//! can lie along it. So a 500 ml pocket takes a 113 mm item, and a port that left the limit at zero would take anything.

use cdda_json::{Object, Value};

use crate::items::default_length_from_volume;

/// The capacity a pocket has when the data states none: `max_volume_for_container` (`src/item_pocket.h:470`), 200,000,000 ml.
pub const MAX_VOLUME_FOR_CONTAINER: i64 = 200_000_000;

/// The weight a pocket takes when the data states none: `max_weight_for_container` (`src/item_pocket.h:471`), 2,000,000 kg in grams.
pub const MAX_WEIGHT_FOR_CONTAINER: i64 = 2_000_000_000;

/// One pocket of a thing, as the rules read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PocketSpec {
    /// What it is for, as the reference names it: `CONTAINER`, `MAGAZINE`, `MAGAZINE_WELL`, `MOD`, and so on.
    pub kind: String,
    /// Whether a liquid may go in (`pocket_data::watertight`, `src/item_pocket.h:512`).
    pub watertight: bool,
    /// Whether a gas may go in (`pocket_data::airtight`, `src/item_pocket.h:514`).
    pub airtight: bool,
    /// Whether the pocket keeps its own shape (`pocket_data::rigid`, `src/item_pocket.h:516`).
    pub rigid: bool,
    /// How much fits, in millilitres.
    pub max_volume_ml: i64,
    /// How heavy a thing may be, in grams.
    pub max_weight_grams: i64,
    /// The longest side that fits, in millimetres.
    pub max_length_mm: i64,
    /// The shortest side that fits, in millimetres; zero when the pocket does not care.
    pub min_length_mm: i64,
    /// The smallest volume that fits, in millilitres; zero when the pocket does not care.
    pub min_volume_ml: i64,
}

/// Reads the pockets a thing states.
///
/// An object with no `pocket_data` has none — the reference gives a thing pockets only when its data asks for them.
#[must_use]
pub fn read_pockets(object: &Object) -> Vec<PocketSpec> {
    match object.get("pocket_data") {
        Some(Value::Array(entries)) => specs_of(entries),
        _ => Vec::new(),
    }
}

/// Reads an array of pocket objects: a thing's own `pocket_data`, or a gunmod's `pocket_mods`, which the reference loads through the same reader
/// (`src/item_factory.cpp:3357`).
#[must_use]
pub fn specs_of(entries: &[Value]) -> Vec<PocketSpec> {
    entries.iter().filter_map(read_pocket).collect()
}

/// Reads one pocket, with the reference's defaults.
///
/// **A pocket with an `ammo_restriction` skips every limit.** `pocket_data::load` puts the capacity, the weight, the three compatibility limits and their defaults
/// inside `if( ammo_restriction.empty() )` (`src/item_pocket.cpp:163-177`), so an ammunition pocket keeps the struct's own initialisers: a capacity of
/// `max_volume_for_container` — 200,000,000 ml, which no item is ever too big for — and a longest side of **zero**, which the rules never ask because the check is
/// `it.length() > data->max_item_length` only for pockets that state one... and `0_mm` would refuse everything, so the restriction branch is a different kind of pocket
/// altogether. Watertight, airtight and rigid are read **outside** that branch, so they still apply.
fn read_pocket(entry: &Value) -> Option<PocketSpec> {
    let Value::Object(entry) = entry else {
        return None;
    };
    let restricted = entry.get("ammo_restriction").is_some();
    let (max_volume_ml, max_weight_grams, max_length_mm, min_length_mm, min_volume_ml) = if restricted
    {
        (MAX_VOLUME_FOR_CONTAINER, MAX_WEIGHT_FOR_CONTAINER, 0, 0, 0)
    } else {
        let max_volume_ml =
            volume(entry, "max_contains_volume").unwrap_or(MAX_VOLUME_FOR_CONTAINER);
        (
            max_volume_ml,
            weight(entry, "max_contains_weight").unwrap_or(MAX_WEIGHT_FOR_CONTAINER),
            // Reference: `item_pocket.cpp:174-176`. The diagonal of a cube of the capacity, because the longest thing that fits in a box is its diagonal.
            length(entry, "max_item_length")
                .unwrap_or_else(|| diagonal_mm(default_length_from_volume(max_volume_ml))),
            length(entry, "min_item_length").unwrap_or(0),
            volume(entry, "min_item_volume").unwrap_or(0),
        )
    };
    Some(PocketSpec {
        kind: text(entry, "pocket_type").unwrap_or_else(|| "CONTAINER".to_owned()),
        watertight: flag(entry, "watertight"),
        airtight: flag(entry, "airtight"),
        rigid: flag(entry, "rigid"),
        max_volume_ml,
        max_weight_grams,
        max_length_mm,
        min_length_mm,
        min_volume_ml,
    })
}

/// The diagonal of a square of the given side, rounded down: `default_length_from_volume( v ) * sqrt( 2 )` (`src/item_pocket.cpp:176`).
///
/// The reference multiplies a millimetre length by `M_SQRT2` as a double and truncates into a whole millimetre, so the product is computed in the same order here.
fn diagonal_mm(side_mm: i64) -> i64 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let product = (side_mm as f64 * std::f64::consts::SQRT_2) as i64;
    product
}

fn text(object: &Object, member: &str) -> Option<String> {
    match object.get(member) {
        Some(Value::Str(value)) => Some(value.clone()),
        _ => None,
    }
}

fn flag(object: &Object, member: &str) -> bool {
    matches!(object.get(member), Some(Value::Bool(true)))
}

fn volume(object: &Object, member: &str) -> Option<i64> {
    match object.get(member) {
        Some(Value::Str(value)) => cdda_core_types::units::read_volume(value).ok(),
        Some(Value::Int(value)) => Some(*value),
        _ => None,
    }
}

fn weight(object: &Object, member: &str) -> Option<i64> {
    match object.get(member) {
        Some(Value::Str(value)) => cdda_core_types::units::read_mass(value).ok().map(|mg| mg / 1_000),
        Some(Value::Int(value)) => Some(*value),
        _ => None,
    }
}

fn length(object: &Object, member: &str) -> Option<i64> {
    match object.get(member) {
        Some(Value::Str(value)) => cdda_core_types::units::read_length(value).ok(),
        Some(Value::Int(value)) => Some(*value),
        _ => None,
    }
}
