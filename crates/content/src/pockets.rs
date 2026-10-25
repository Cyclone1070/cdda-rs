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
///
/// The flags are kept as flags because they are what the reference keeps (`pocket_data`, `src/item_pocket.h:470-495`) and each one is read by a different rule;
/// folding them into a state machine would invent a structure the data does not have.
#[allow(clippy::struct_excessive_bools)]
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
    /// The one biggest item that fits, in millilitres; `None` when the pocket does not care (`pocket_data::max_item_volume`, `src/item_pocket.h:477`).
    ///
    /// The reference asks it as `charges_per_volume_recursive` against the **per-charge** volume, and skips the question for liquids, gases and frozen liquids
    /// (`src/item_pocket.cpp:1411-1418`).
    pub max_item_volume_ml: Option<i64>,
    /// Whether it holds exactly one item, like a holster (`src/item_pocket.h:488`).
    pub holster: bool,
    /// Whether its contents are ablative armour and never count against it (`src/item_pocket.h:490`).
    pub ablative: bool,
    /// Whether it may be sealed, which is what a pocket of a sealed thing reports (`src/item_pocket.h:130-136`).
    pub sealed: bool,
    /// The item ids it accepts, in the data's order; empty when it accepts any (`pocket_data::item_id_restriction`).
    pub items: Vec<String>,
    /// The item flags it needs, in the data's order; empty when it needs none (`pocket_data::flag_restrictions`).
    pub flags: Vec<String>,
    /// The materials it wants its contents made of, in the data's order; empty when it does not care (`pocket_data::material_restriction`).
    pub materials: Vec<String>,
    /// The ammunition types it takes, from the keys of `ammo_restriction`; empty when it takes anything (`pocket_data::ammo_restriction`).
    ///
    /// A pocket that states one takes **only** ammunition, and only of those types - and the reference returns success right there, skipping the length, the volume
    /// and the weight entirely (`src/item_pocket.cpp:1375-1395`).
    pub ammo: Vec<String>,
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
    let ablative = flag(entry, "ablative");
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
        max_item_volume_ml: volume(entry, "max_item_volume"),
        // The reference flags an ablative pocket as a holster as well, so that it only ever holds one thing (`src/item_pocket.cpp:196-199`).
        holster: flag(entry, "holster") || ablative,
        ablative,
        sealed: entry.get("sealed_data").is_some(),
        items: id_list(entry, "item_restriction"),
        flags: id_list(entry, "flag_restriction"),
        materials: id_list(entry, "material_restriction"),
        ammo: ammo_types_of(entry),
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

/// A list of ids, **in the order the data writes them**.
///
/// The reference keeps these in a `cata::flat_set`, which is an append-if-absent vector rather than a sorted one, so its iteration order is the data's order and not
/// an alphabetical one: a wallet's flags come out `CREDIT_CARD_SHAPED` and then `BANK_NOTE_SHAPED`, which is how the wallet's JSON writes them.
pub(crate) fn id_list(object: &Object, member: &str) -> Vec<String> {
    match object.get(member) {
        Some(Value::Array(entries)) => entries
            .iter()
            .filter_map(|entry| match entry {
                Value::Str(id) => Some(id.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The ammunition types an `ammo_restriction` names, which are its keys.
fn ammo_types_of(object: &Object) -> Vec<String> {
    match object.get("ammo_restriction") {
        Some(Value::Object(restriction)) => restriction
            .members()
            .iter()
            .map(|(ammo, _)| ammo.clone())
            .collect(),
        _ => Vec::new(),
    }
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
