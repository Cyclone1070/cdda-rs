//! Item types, the largest loader in the content set.
//!
//! Only the fields a first slice needs are ported here — identity, name and
//! plural, glyph, colour, weight, volume, price, materials, category and flags —
//! but the rules they follow are the ones every remaining field will follow:
//!
//! * `id` may be a string or an array; an array makes one prototype per entry.
//! * a name is either text or a `{ "str": ..., "str_pl": ... }` object, and a
//!   liquid always reports its singular (`itype.cpp:107-115`).
//! * weight, volume and price accept a whole number in the base unit or a
//!   quantity string, which is integer terms with units and no decimals.
//! * `material` is a weighted list: names, or `[ name, weight ]` pairs.

use cdda_core_types::colors::{NcColor, color_from_string, string_from_color};
use cdda_core_types::units::{read_length, read_mass, read_money, read_volume};

use crate::materials::Material;
use cdda_json::{Object, Value};
use std::collections::{BTreeMap, BTreeSet};

/// One resolved item type, as far as this slice reads it.
#[derive(Debug, Clone, PartialEq)]
// This mirrors the reference's `itype`, which is a flat bag of about forty booleans. Grouping ours
// would read better and drift from the thing it is transcribed from, so the shape stays and the lint
// is answered here once instead of at every use.
#[allow(clippy::struct_excessive_bools)]
pub struct ItemType {
    /// Its id.
    pub id: String,
    /// The singular name a player sees, untranslated.
    pub name: String,
    /// The plural name, which for a liquid is the singular.
    pub name_plural: String,
    /// The glyph, as a code point.
    pub symbol: u32,
    /// The colour.
    pub color: NcColor,
    /// Weight in **milligrams**, which is the reference's `units::mass` base (`src/units.h`: one milligram is one unit). The data writes `"40 g"`, which is 40,000
    /// here — reading it as grams would make every item a thousand times too heavy, which the food-summary gate caught the moment it compared a bite's weight.
    pub weight: i64,
    /// What the item weighs when its caller asks for the **integral** weight, in milligrams (see `weight`).
    ///
    /// Reference: `itype::integral_weight` (`src/itype.h:1327`), read by `item::weight( integral: true )` (`src/item.cpp:7256-7261`). The loader copies `weight` into it
    /// when the data does not say (`src/item_factory.cpp:398-400`) - so the two are equal for almost every type - and the data may say **zero** on purpose, which three
    /// gunmods do because a gunmod's weight does not count toward the gun's integral weight.
    pub integral_weight: i64,
    /// Volume in millilitres.
    pub volume: i64,
    /// The ammunition types it is ammunition for, when it is ammunition at all (`itype::ammo->type`, loaded from `"ammo_type"`).
    pub ammo_types: Vec<String>,
    /// The pockets it holds things in, as the data states them (`src/item_pocket.cpp:150-200`).
    pub pockets: Vec<crate::pockets::PocketSpec>,
    /// The pockets it **adds** to whatever it is installed on: `pocket_mods`, which only a gunmod states (`src/item_factory.cpp:3357`).
    pub added_pockets: Vec<crate::pockets::PocketSpec>,
    /// The mods a gun is made with, in the reference's order: `built_in_mods` first, then `default_mods` (`src/item.cpp:383-391`).
    pub gun_mods: Vec<String>,
    /// The longest side the data states, in millimetres; [`UNSET_LENGTH`] when it states none.
    ///
    /// Reference: `itype::longest_side`, loaded from `"longest_side"` (`src/item_factory.cpp:4076`) and **filled in at finalisation** from the effective volume when
    /// unset (`src/item_factory.cpp:812-817`). The field is kept as loaded here, and [`ItemType::longest_side`] does the filling, because the two are different facts: a
    /// type that states 750 mm for a 235 ml rod is telling you something the cube root would get wrong.
    pub longest_side: i64,
    /// How many of it make a whole stack, which is what a charge count is divided by when the volume of a pile is worked out.
    pub stack_size: i32,
    /// The default number of charges the type is created with, which is `"count"` in the data (`src/item_factory.cpp:2988`) and is the fallback for a stack size.
    pub default_charges: i32,
    /// What the item takes up when its caller asks for the **integral** volume, in millilitres.
    ///
    /// Reference: `itype::integral_volume`, read by `item::volume( integral: true )` (`src/item.cpp:7540-7544`). Loaded with the same rule as the integral weight: the
    /// ordinary volume is copied over only when the field is **still unset** (`src/item_factory.cpp:394-396`), so a deliberate zero stays zero - 97 types say zero.
    pub integral_volume: i64,
    /// Price in cents.
    pub price: i64,
    /// Category id, empty when the item states none.
    pub category: String,
    /// Materials and their portions.
    pub materials: BTreeMap<String, i32>,
    /// The faults this type can acquire, with their weights, after `fault_group` expansion has run.
    ///
    /// Reference: `itype::faults` (`src/itype.h`), a weighted list filled by the `faults` array in `item_factory.cpp:4168-4183` and completed by the group
    /// expansion in `item_factory.cpp:919-936`. Empty for anything that cannot fail.
    pub faults: Vec<(String, i32)>,
    /// The `fault_group` entries this type pointed at, waiting for finalisation to expand them.
    pub fault_groups: Vec<(i32, i32, f64, String)>,
    /// What using it does, by **action name**, as far as the actor data is ported (`src/iuse.h:279-300`). A map, not a list, because the reference keeps
    /// `use_methods` in a `std::map` and three behaviours follow from that alone: sorted order, collapse on a repeated name, and the `repair_item` rename.
    pub use_actions: BTreeMap<String, crate::use_actions::UseAction>,
    /// The per-action charge scale (`itype::ammo_scale`, `src/itype.h:1410`): absent means one, and a **zero is a real zero**, not "unset".
    pub ammo_scale: BTreeMap<String, i32>,
    /// The type's own countdown, in seconds: an item of a type that has one is **active** and counting down from the moment it becomes that type
    /// (`itype::countdown_interval`, read at `src/item_factory.cpp:4195`, used at `src/item.cpp:682-686`).
    pub countdown_interval_seconds: i64,
    /// Qualities the item has **only while charged** (`"charged_qualities": [ [ "OVEN", 2 ] ]`), which lend actions without a charge scale.
    pub charged_qualities: BTreeMap<String, i32>,
    /// Its flag set.
    pub flags: BTreeSet<String>,
    /// Whether it has gun data, which is what makes it a candidate for an invented turret part (`veh_type.cpp:484`).
    pub is_gun: bool,
    /// Whether it is a gun **mod**: those never get a turret part, whatever else is true of them.
    pub is_gunmod: bool,
    /// The skill a gun is fired with, as its own data writes it (`"gun": { "skill": "rifle" }`).
    pub gun_skill: Option<String>,
    /// Whether it blows up when a fire reaches it.
    pub explode_in_fire: bool,
    /// The comestible slot, when the type has one (`src/itype.h:147-248`). `None` is not food at all, which is a different thing from food with no nutrition.
    pub comestible: Option<crate::comestible::Comestible>,
    /// How many charges one use spends: one for food, the tool's own rate for tools.
    pub charges_to_use: i32,
    /// The ammo effects it declares, which decide whether its ammunition cooks off.
    pub ammo_effects: BTreeSet<String>,
    /// Whether its ammo cooks off in a fire.
    pub cookoff: bool,
    /// Whether its ammo cooks off in a special way.
    pub special_cookoff: bool,
    /// What it is worth as a tool, by quality id. A **negative** level means worse than bare hands, which the reference uses for things like a saw's butchering.
    pub qualities: BTreeMap<String, i32>,
    /// The subtypes it declares, or the legacy type it was written as, carried
    /// so the category can be derived the way `calc_category` does.
    pub subtypes: BTreeSet<String>,
    /// Damage types and their amounts, from `melee_damage`.
    pub melee: BTreeMap<String, f64>,
    /// `stackable` as stated, which decides charge counting.
    pub stackable: bool,
    /// `phase`, which separates liquids from solids.
    pub phase: String,
    /// The material a comestible is mostly made of, which replaces its list.
    pub primary_material: String,
    /// The per-part armour entries: each material with the thickness it is laid
    /// on with, which body parts it covers, and which entry it belongs to.
    pub armor_parts: Vec<ArmorPart>,
    /// The sub-locations each armour entry covers, which weight its thickness
    /// against other entries on the same body part.
    pub armor_entries: Vec<BTreeSet<String>>,
}

/// Reads the measures a type carries: weight, integral weight, volume, longest side, integral volume and price.
///
/// Reference: `item_factory.cpp:390-420` for the two integral fallbacks and `units_reader` for the units. A bare number is in the unit's own base - grams for weight,
/// millilitres for volume, cents for price - and the two integral values fall back to the ordinary ones **only when still unset**, because `"integral_weight": "0 g"` is a
/// value and not an absence.
/// Reads the `faults` array: direct faults now, group references kept for finalisation.
///
/// Reference: `item_factory.cpp:4168-4183`. A direct entry states a `fault` and a `weight` (100 when unstated, and the reader takes a **fraction**, so the weight is
/// cast to a whole number); a group entry states a `fault_group` with `weight_override`, `weight_add` and `weight_mult`, and is **not** expanded here — the reference
/// defers that to finalisation (`item_factory.cpp:919-936`), which is why a directly-stated fault and a group-expanded one can collide in a way that depends on that
/// order.
fn read_faults(item: &mut ItemType, object: &Object) {
    let Some(Value::Array(entries)) = object.get("faults") else {
        return;
    };
    for entry in entries {
        let Value::Object(entry) = entry else {
            continue;
        };
        if let Some(Value::Str(fault)) = entry.get("fault") {
            // **A weight is an integer in the reference's weighted list**, and the reader takes a float (`get_float`) so a fractional weight truncates here rather
            // than being carried around as a real.
            let weight = match entry.get("weight") {
                #[allow(clippy::cast_possible_truncation)]
                Some(Value::Float(weight)) => *weight as i32,
                Some(Value::Int(weight)) => i32::try_from(*weight).unwrap_or(100),
                _ => 100,
            };
            item.faults.push((fault.clone(), weight));
        } else if let Some(Value::Str(group)) = entry.get("fault_group") {
            let override_weight = match entry.get("weight_override") {
                Some(Value::Int(weight)) => i32::try_from(*weight).unwrap_or(-1),
                _ => -1,
            };
            let add = match entry.get("weight_add") {
                Some(Value::Int(weight)) => i32::try_from(*weight).unwrap_or(0),
                _ => 0,
            };
            let multiply = match entry.get("weight_mult") {
                Some(Value::Float(weight)) => *weight,
                Some(Value::Int(weight)) => f64::from(i32::try_from(*weight).unwrap_or(1)),
                _ => 1.0,
            };
            item.fault_groups
                .push((override_weight, add, multiply, group.clone()));
        }
    }
}

/// Expands every `fault_group` an item type pointed at, in the order the reference does.
///
/// Reference: `item_factory.cpp:919-936`, which runs once per type at the end of loading. A group with `weight_override == -1` **replaces** the weight of every fault
/// in it with `( weight + weight_add ) * weight_mult`, truncating the product to a whole number; any other override **adds** each fault at that one weight. A replace
/// keeps the fault where it already stood in the list; an add appends.
pub fn expand_fault_groups(item: &mut ItemType, groups: &[crate::faults::FaultGroup]) {
    for (override_weight, add, multiply, group_id) in item.fault_groups.clone() {
        let Some(group) = groups.iter().find(|group| group.id == group_id) else {
            continue;
        };
        for (fault, weight) in &group.entries {
            if override_weight == -1 {
                #[allow(clippy::cast_possible_truncation)]
                let made = ((f64::from(*weight) + f64::from(add)) * multiply) as i32;
                if let Some(slot) = item.faults.iter_mut().find(|(id, _)| id == fault) {
                    slot.1 = made;
                } else {
                    item.faults.push((fault.clone(), made));
                }
            } else if !item.faults.iter().any(|(id, _)| id == fault) {
                item.faults.push((fault.clone(), override_weight));
            }
        }
    }
    item.fault_groups.clear();
}

fn read_measures(item: &mut ItemType, object: &Object) -> Result<(), LoadError> {
    // A bare number is in the unit's own base: grams for weight, millilitres for
    // volume, cents for price (`units_reader`).
    item.weight = adjusted(object, "weight", item.weight, read_mass, 1_000)?;
    // The integral weight falls back to the ordinary one, which is the loader's own rule (`src/item_factory.cpp:398-400`): the two agree unless the data says otherwise,
    // and `"integral_weight": "0 g"` is a value, not an absence.
    // **The fallback is the reference's own**, not a per-key default: the loader copies `weight` into `integral_weight` only when the value is still unset
    // (`-1`, `src/item_factory.cpp:398-400`). Defaulting the key to `weight` here would overwrite an inherited zero - which is exactly what a gunmod writes when its
    // weight should not count toward the gun's.
    item.integral_weight = adjusted(
        object,
        "integral_weight",
        item.integral_weight,
        read_mass,
        1_000,
    )?;
    if item.integral_weight < 0 {
        item.integral_weight = item.weight;
    }
    item.volume = adjusted(object, "volume", item.volume, read_volume, 1)?;
    item.longest_side = adjusted(
        object,
        "longest_side",
        item.longest_side,
        read_length,
        UNSET_LENGTH,
    )?;
    item.integral_volume = adjusted(
        object,
        "integral_volume",
        item.integral_volume,
        read_volume,
        1,
    )?;
    if item.integral_volume < 0 {
        item.integral_volume = item.volume;
    }
    item.price = adjusted(object, "price", item.price, read_money, 1)?;
    Ok(())
}

/// Adds the pockets a gun's installed mods contribute, which is what a gun with `pocket_mods` in its stock actually holds.
///
/// Reference: `item::update_modified_pockets` (`src/item.cpp:1362-1387`), run at construction. A **CONTAINER** pocket named `pocket_mods` is added to the gun's
/// own pocket list, and a `MAGAZINE` or `MAGAZINE_WELL` one replaces the gun's well; both are appended in the order the mods are installed, which is built-ins
/// first (`src/item.cpp:383-391`). Only the added CONTAINER pockets show up in what a constructed item reports it holds, which is why they are the ones that
/// change an item's behaviour here.
/// Reads a type's `use_action` member, and its `extend`/`delete` operators.
///
/// **A stated `use_action` replaces the inherited list; `extend` adds to it and `delete` takes names away.**
///
/// Reference: the container overload of `optional` (`src/generic_factory.h:1168-1192`) **clears** the container when the member is present, and only falls through
/// to the `extend`/`delete` objects when it is not. So a type that inherits five actions and states one of its own has **one**, not six — the opposite of a merge,
/// which is what an earlier version of this did and what the corpus caught on four types.
///
/// # Errors
///
/// If a ported actor's fields are malformed.
fn apply_use_actions(item: &mut ItemType, object: &Object) -> Result<(), LoadError> {
    if object.get("use_action").is_some() {
        let stated = crate::use_actions::read_use_actions(object)?;
        item.use_actions = stated.methods;
        item.ammo_scale = stated.ammo_scale;
        return Ok(());
    }
    if let Some(Value::Object(extend)) = object.get("extend") {
        let stated = crate::use_actions::read_use_actions(extend)?;
        for (name, action) in stated.methods {
            item.use_actions.entry(name).or_insert(action);
        }
        for (name, scale) in stated.ammo_scale {
            item.ammo_scale.entry(name).or_insert(scale);
        }
    }
    if let Some(Value::Object(delete)) = object.get("delete") {
        let removed = crate::use_actions::read_use_actions(delete)?;
        for name in removed.methods.keys() {
            item.use_actions.remove(name);
            item.ammo_scale.remove(name);
        }
    }
    Ok(())
}

/// Adds the actions a type's **qualities** and its **kind** lend it, which is why an item's action list is longer than its own JSON.
///
/// Reference: `Item_factory::finalize` (`src/item_factory.cpp:333-377`) and `finalize_pre` (`808-810`). Five rules, and every one uses `emplace_usage`, which adds
/// an action **only if it is already in the reference's function table** and never overwrites one that is there:
///
/// 1. a quality whose level is **at least** a usage's level lends that usage's actions, each at an `ammo_scale` of **zero** — the reference says so outright:
///    actions from a quality "do not consume ammo";
/// 2. the same for `charged_qualities`, **without** the scale, because the reference does not know what it would be;
/// 3. a type with a **mod slot** lends `GUNMOD_ATTACH` (a gunmod) or `TOOLMOD_ATTACH` (anything else), at zero — and a gunmod has a slot whether or not it says so
///    (`finalize_pre` creates one);
/// 4. a **gun** lends `detach_gunmods` and `modify_gunmods`, both at zero;
/// 5. anything that can **plug in** gets a `link_up` scale of zero, whether or not its actor states one (`808-810`): a cable moves power, not charges.
pub fn add_quality_usages(
    item: &mut ItemType,
    qualities: &BTreeMap<String, crate::qualities::Quality>,
) {
    fn lend(item: &mut ItemType, action: &str) {
        item.use_actions
            .entry(action.to_owned())
            .or_insert_with(|| crate::use_actions::UseAction::Other {
                kind: action.to_owned(),
            });
    }
    let owned: Vec<(String, i32)> = item
        .qualities
        .iter()
        .map(|(quality, level)| (quality.clone(), *level))
        .collect();
    for (quality, level) in owned {
        let Some(def) = qualities.get(&quality) else {
            continue;
        };
        for (needed, actions) in &def.usages {
            if level >= *needed {
                for action in actions {
                    lend(item, action);
                    item.ammo_scale.entry(action.clone()).or_insert(0);
                }
            }
        }
    }
    let charged: Vec<(String, i32)> = item
        .charged_qualities
        .iter()
        .map(|(quality, level)| (quality.clone(), *level))
        .collect();
    for (quality, level) in charged {
        let Some(def) = qualities.get(&quality) else {
            continue;
        };
        for (needed, actions) in &def.usages {
            if level >= *needed {
                for action in actions {
                    lend(item, action);
                }
            }
        }
    }
    if item.is_gunmod || item.subtypes.contains("TOOLMOD") {
        let action = if item.is_gunmod {
            "GUNMOD_ATTACH"
        } else {
            "TOOLMOD_ATTACH"
        };
        lend(item, action);
        item.ammo_scale.entry(action.to_owned()).or_insert(0);
    } else if item.is_gun {
        for action in ["detach_gunmods", "modify_gunmods"] {
            lend(item, action);
            item.ammo_scale.entry(action.to_owned()).or_insert(0);
        }
    }
    if item.use_actions.contains_key("link_up") {
        item.ammo_scale.entry("link_up".to_owned()).or_insert(0);
    }
}

/// Gives every gun its mods' pockets, and gives every mod the well it needs.
///
/// Reference: `Item_factory::finalize`'s pocket pass. A gun whose mods carry pockets gains them, and a mod that can hold an item gets a well for it, so the two
/// halves of an installation agree about where the item lives.
pub fn apply_mod_pockets(types: &mut [ItemType]) {
    let by_id: BTreeMap<String, (Vec<crate::pockets::PocketSpec>, Vec<crate::pockets::PocketSpec>)> =
        types
            .iter()
            .map(|item| (item.id.clone(), (item.added_pockets.clone(), item.pockets.clone())))
            .collect();
    for item in types.iter_mut() {
        if item.gun_mods.is_empty() {
            continue;
        }
        let mut added: Vec<crate::pockets::PocketSpec> = Vec::new();
        let mut well: Option<crate::pockets::PocketSpec> = None;
        for id in &item.gun_mods {
            let Some((pockets, _)) = by_id.get(id) else {
                continue;
            };
            for pocket in pockets {
                if pocket.kind == "CONTAINER" {
                    added.push(pocket.clone());
                } else if pocket.kind == "MAGAZINE" || pocket.kind == "MAGAZINE_WELL" {
                    well = Some(pocket.clone());
                }
            }
        }
        if !added.is_empty() || well.is_some() {
            if let Some(well) = well {
                item.pockets.retain(|pocket| pocket.kind != "MAGAZINE_WELL");
                item.pockets.push(well);
            }
            item.pockets.extend(added);
        }
    }
}

/// Reads `"ammo_type"`, which a type states as one caliber or as a list of them.
fn read_ammo_types(object: &Object) -> Vec<String> {
    match object.get("ammo_type") {
        Some(Value::Str(ammo)) => vec![ammo.clone()],
        Some(Value::Array(entries)) => entries
            .iter()
            .filter_map(|entry| match entry {
                Value::Str(ammo) => Some(ammo.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Reads `"count"`, the number of charges a freshly made item starts with (`src/item_factory.cpp:2988`), keeping the inherited value when the data is silent.
fn read_default_charges(object: &Object, inherited: i32) -> i32 {
    match object.get("count") {
        Some(Value::Int(count)) => i32::try_from(*count).unwrap_or(1).max(1),
        _ => inherited,
    }
}

/// What `longest_side` holds when the data states none: the reference's `-1_mm` sentinel (`src/item_factory.cpp:4076`).
pub const UNSET_LENGTH: i64 = -1;

/// The length the reference gives a thing of a given volume: the cube root, rounded to the centimetre, in millimetres.
///
/// Reference: `units::default_length_from_volume` (`src/units.h:740-746`): `round( cbrt( millilitres ) )` centimetres. Written as an integer search rather than a float
/// root, so the rounding is exact at a boundary and the rule holds for the large volumes a vehicle part has.
#[must_use]
pub fn default_length_from_volume(volume_ml: i64) -> i64 {
    if volume_ml <= 0 {
        return 0;
    }
    // Rounding the **root**, not the volume: the answer is the whole centimetre nearest `cbrt( ml )`, so a centimetre `cm` is the answer while the root has not
    // passed `cm + 0.5`. In integers that is `8 * ml >= ( 2cm - 1 )^3`, which is exact and needs no floating point.
    let mut cm: i64 = 0;
    loop {
        let next = (2 * cm + 1) * (2 * cm + 1) * (2 * cm + 1);
        if 8 * volume_ml < next {
            break;
        }
        cm += 1;
    }
    cm * 10
}

impl ItemType {
    /// Its longest side, in millimetres, as `item::length` reports it for an empty item.
    ///
    /// Reference: `item::length` (`src/item.cpp:7343-7398`) over `itype::longest_side`, which the factory fills in at finalisation from the **effective** volume - the
    /// volume of one charge for something counted by charges (`src/item_factory.cpp:812-817`). That is why a fifty-litre water item is not a 370 mm cube: its own volume is
    /// divided by the stack size first.
    ///
    /// A liquid has **no** length (`src/item.cpp:7345-7347`), and so does a soft thing with nothing in it; a gas is measured like anything else. Guns and corpses are not modelled: a gun's length
    /// depends on its mods, and a corpse's on the monster, and both are their own pieces of work.
    #[must_use]
    pub fn longest_side(&self, materials: &BTreeMap<String, Material>) -> i64 {
        // A **liquid** has no length; a gas is measured like anything else (`src/item.cpp:7345-7347` asks `made_of( phase_id::LIQUID )` and nothing about gas).
        if self.phase == "liquid" {
            return 0;
        }
        if self.is_soft(materials) {
            return 0;
        }
        if self.longest_side >= 0 {
            return self.longest_side;
        }
        let effective_volume = if self.counts_by_charge() {
            let stack = self.effective_stack_size();
            if stack > 0 { self.volume / stack } else { self.volume }
        } else {
            self.volume
        };
        default_length_from_volume(effective_volume)
    }

    /// Whether the thing is soft: `item::is_soft` (`src/item.cpp:15199-15211`).
    ///
    /// The flags win over the materials, and the materials decide the rest: an item is soft when **every** material it is made of is soft - which is vacuously
    /// true for an item made of nothing, and is what gives the debug items a length of zero.
    #[must_use]
    pub fn is_soft(&self, materials: &BTreeMap<String, Material>) -> bool {
        if self.flags.contains("SOFT") {
            return true;
        }
        if self.flags.contains("HARD") {
            return false;
        }
        // `std::all_of` over an **empty** map is true, so an item made of nothing at all counts as soft - which is how a debug item with no materials
        // gets a length of zero.
        self.materials
            .keys()
            .all(|id| materials.get(id).is_some_and(|material| material.soft))
    }

    /// Whether the item cannot be taken out of whatever holds it.
    ///
    /// Reference: `item::is_irremovable` (`src/item.cpp:10356-10359`), which is the `IRREMOVABLE`
    /// flag and nothing else. It matters when a container is destroyed: an irremovable item stays
    /// where it was, which is the rule the burning loop was missing.
    #[must_use]
    pub fn is_irremovable(&self) -> bool {
        self.flags.contains("IRREMOVABLE")
    }

    /// Derives the two cookoff fields from the ammo effects, the way `item_factory.cpp:512-522` does.
    ///
    /// Ammunition whose effects include incendiary or cook-off cooks off in a fire, unless it is made
    /// of hydrocarbons or oil, which the reference exempts. A special cook-off is its own effect.
    /// Whether this is a gun, and whether it is a gun mod, read off the slots the loader filled.
    ///
    /// A gun is a gun because its data says so: the dump states it outright and a data file states it as a subtype. `veh_type.cpp:484` asks both questions of
    /// every item in the game, so both paths have to end in the same answer.
    pub fn derive_gun(&mut self, known_qualities: &BTreeSet<String>) {
        self.is_gun = self.is_gun || self.subtypes.contains("GUN");
        self.is_gunmod = self.is_gunmod || self.subtypes.contains("GUNMOD");
        // **A gun is also a quality.** The reference gives every gun that is neither a gun mod nor a primitive ranged weapon the `GUN` quality at level one, and the
        // quality named after the skill it fires with, taking the larger of what the data said and one (`src/item_factory.cpp:845-852`). That is why `is_gun` and a
        // `GUN` quality are the same fact told twice, and why anything that counts qualities without this rule undercounts guns by hundreds.
        if self.is_gun && !self.is_gunmod && !self.flags.contains("PRIMITIVE_RANGED_WEAPON") {
            self.qualities.entry("GUN".to_owned()).and_modify(|level| *level = (*level).max(1)).or_insert(1);
            if let Some(skill) = &self.gun_skill {
                let quality = skill.to_uppercase();
                if known_qualities.contains(&quality) {
                    self.qualities
                        .entry(quality)
                        .and_modify(|level| *level = (*level).max(1))
                        .or_insert(1);
                }
            }
        }
    }

    /// Derives the two cookoff fields from the ammo effects, the way `item_factory.cpp:512-522` does.
    pub fn derive_cookoff(&mut self) {
        let oily =
            self.materials.contains_key("hydrocarbons") || self.materials.contains_key("oil");
        // Only ammunition cooks off: the reference derives these inside its ammo loader, so an item
        // with no ammunition data never gets the flags (`item_factory.cpp:512`).
        if oily || !self.subtypes.contains("AMMO") || self.ammo_effects.is_empty() {
            self.cookoff = false;
            self.special_cookoff = false;
            return;
        }
        self.cookoff =
            self.ammo_effects.contains("INCENDIARY") || self.ammo_effects.contains("COOKOFF");
        self.special_cookoff = self.ammo_effects.contains("SPECIAL_COOKOFF");
    }

    /// Whether it can be eaten or drunk at all.
    #[must_use]
    pub fn is_comestible(&self) -> bool {
        self.comestible.is_some()
    }

    /// Whether a fire makes it go off.
    ///
    /// Reference: `item::will_explode_in_fire` (`src/item.cpp:13314-13325`): the type's own
    /// `explode_in_fire`, or ammunition that cooks off. A fire that reaches one of these detonates
    /// it rather than burning it, which is why the burning loop asks first.
    #[must_use]
    pub fn will_explode_in_fire(&self) -> bool {
        self.explode_in_fire || self.cookoff || self.special_cookoff
    }
}

/// One material of one armour entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ArmorPart {
    /// The material's name.
    pub material: String,
    /// Its thickness in millimetres, at the single precision the reference
    /// carries it in.
    pub thickness: f32,
    /// The body parts the entry covers.
    pub covers: Vec<String>,
    /// The sub-locations it covers on each of them, as entry indices.
    pub entry: usize,
}

impl ItemType {
    /// Whether the item carries the comestible slot.
    fn uses_comestible(&self) -> bool {
        self.subtypes.contains("COMESTIBLE")
    }

    /// Adds the flags a material brings with it, even in trace amounts
    /// (`item_factory.cpp:3550-3591`).
    fn add_material_flags(&mut self) {
        for (material, flag) in MATERIAL_FLAGS {
            if self.materials.contains_key(*material) {
                self.flags.insert((*flag).to_owned());
            }
        }
    }

    /// Adds the flags the non-player side needs in order to act sensibly with
    /// the item (`item_factory.cpp:3619-3646`). The chain matters: a dangerous
    /// item makes the NPC throw it, a thrown item is an alternative attack, and
    /// anything dangerous, fake or built in is something traders refuse.
    fn add_implied_flags(&mut self) {
        let has = |flags: &BTreeSet<String>, flag: &str| flags.contains(flag);
        if has(&self.flags, "DANGEROUS") {
            self.flags.insert("NPC_THROW_NOW".to_owned());
        }
        if has(&self.flags, "BOMB") {
            self.flags.insert("NPC_ACTIVATE".to_owned());
        }
        if has(&self.flags, "NPC_THROW_NOW") {
            self.flags.insert("NPC_THROWN".to_owned());
        }
        if has(&self.flags, "NPC_ACTIVATE") || has(&self.flags, "NPC_THROWN") {
            self.flags.insert("NPC_ALT_ATTACK".to_owned());
        }
        if has(&self.flags, "DANGEROUS")
            || has(&self.flags, "PSEUDO")
            || has(&self.flags, "INTEGRATED")
        {
            self.flags.insert("TRADER_AVOID".to_owned());
        }
    }

    /// How many of it make a full stack, after the reference's finalisation.
    ///
    /// Reference: `item_factory.cpp:400-414`: for anything counted by charges the stack size is the comestible's or the ammunition's own, falling back to the item's
    /// default charge count — which is what makes a battery a stack of **100**. The reference also carries a "cap at 200" branch, but it is not what reaches the data: the
    /// corpus reports gunpowder at 4,540 and chain links at 425, so a port that capped would disagree with 34 types and with every length that divides by them.
    ///
    /// The loader keeps `stack_size` as the data states it; this fills in the rest.
    #[must_use]
    pub fn effective_stack_size(&self) -> i64 {
        if !self.counts_by_charge() {
            return i64::from(self.stack_size).max(1);
        }
        let stated = i64::from(self.stack_size);
        if stated > 0 {
            // The data's own stack size, however large: gunpowder states 4,540 and chain links 425, and the reference reports both unchanged.
            stated
        } else {
            // The fallback is `charges_default()` (`src/itype.cpp:139-149`), which is the **comestible's** `def_charges` - the slot's `charges`, not the item-level
            // `count` - so a bottle of bleach is a stack of two and its volume is halved. The food-summary gate found this on 219 drinks.
            let stated_charges = self
                .comestible
                .as_ref()
                .map_or(self.default_charges, |slot| slot.charges);
            i64::from(stated_charges).max(1)
        }
    }

    /// What a stack of `charges` of this type weighs, in **milligrams**: `item::weight` (`src/item.cpp:7230-7299`).
    ///
    /// Two rules, and the second is the one that surprises: a charge-counted item's weight is the type's multiplied by its charges, and an item flagged **`NO_DROP`**
    /// weighs **nothing at all** — "items that don't drop aren't really there", says the reference, and acetylene is the one comestible that says so. The food-summary
    /// gate found it: acetylene's data states 40 g and the reference's item weighs 0.
    #[must_use]
    pub fn item_weight(&self, charges: i32) -> i64 {
        if self.flags.contains("NO_DROP") {
            return 0;
        }
        self.weight.saturating_mul(i64::from(charges.max(1)))
    }

    /// Whether the item is counted in charges rather than as objects
    /// (`itype.h:1602-1604`): it stacks, or it is ammunition, or it is a
    /// non-solid comestible.
    #[must_use]
    pub fn counts_by_charge(&self) -> bool {
        self.stackable
            || self.subtypes.contains("AMMO")
            || (self.comestible.is_some() && self.phase_is_liquid())
    }

    /// The largest damage this type can take before it is destroyed, and **zero** for a type counted in charges.
    ///
    /// Reference: `itype::damage_max` (`src/itype.h:1572-1574`) is `count_by_charges() ? 0 : damage_max_`, and `damage_max_` is a constant 4,000
    /// (`src/itype.h:1549`) — not data, so every type in the game answers 4,000 or 0.
    #[must_use]
    pub fn damage_max(&self) -> i32 {
        if self.counts_by_charge() { 0 } else { 4_000 }
    }

    /// How many damage increments a type takes before it is destroyed, and **zero** when it does not degrade at all.
    ///
    /// Reference: the finalisation in `item_factory.cpp:4115-4126`. The data states a `degradation_multiplier`, but the loader **zeroes it** unless the type is a
    /// vehicle part and is not counted in charges (`if( count_by_charges() || category_force != item_category_veh_parts ) { degrade_mult = 0.0f; }`), and it reads the
    /// category **as stated or derived**, not as the player sees it. A multiplier small enough that `mult * 4000 <= 0.5` leaves the increments at zero; anything else
    /// divides 50 by it, rounded, and never below one.
    #[must_use]
    pub fn degrade_increments(&self, multiplier: f64) -> i32 {
        let multiplier = if self.counts_by_charge() || self.category != "veh_parts" {
            0.0
        } else {
            multiplier
        };
        if multiplier * f64::from(4_000) <= 0.5 {
            return 0;
        }
        // **A `NaN` multiplier leaves zero increments**, which the reference checks for by name (`std::isnan( adjusted_inc ) ? 0 : ...`): a data file can say
        // `"degradation_multiplier": .nan` and the reference treats it as "does not degrade" rather than as a crash.
        let adjusted = (50.0 / multiplier).max(1.0).round();
        // `as` on a float saturates and truncates; the reference's `std::round` already made it whole, and the `NaN` case is handled above it.
        #[allow(clippy::cast_possible_truncation)]
        let increments = adjusted as i32;
        if adjusted.is_nan() { 0 } else { increments }
    }

    /// How badly damaged a thing is, from nothing to destroyed.
    ///
    /// Reference: `itype::damage_level` (`src/itype.cpp:117-126`). Zero damage is level zero **whatever the type**; a charge-counted type answers 5 for any damage at
    /// all, because it has no partial damage to speak of; and everything else is `1 + 4 * damage / damage_max`, clamped to five. The multiplication comes **before** the
    /// division, which is what makes the four bands equal: at 1,000 of 4,000 the answer is 2, not 1.
    #[must_use]
    pub fn damage_level(&self, damage: i32) -> i32 {
        if damage == 0 {
            return 0;
        }
        if self.counts_by_charge() {
            return 5;
        }
        (1 + 4 * damage / self.damage_max()).clamp(0, 5)
    }

    /// The materials an armour derives from its per-part entries.
    ///
    /// Entries covering the same body part merge: a material both entries name
    /// keeps a thickness averaged by how much of the part each entry covers, and
    /// a material only the new entry names keeps its own thickness
    /// (`item_factory.cpp:1124-1150`). Each merged thickness then becomes a
    /// hundredth of a unit per body part (`item_factory.cpp:1452-1463`).
    fn armor_materials(&self, anatomy: &Anatomy) -> BTreeMap<String, i32> {
        /// One body part's worth of merged armour.
        struct Merged {
            part: String,
            sub_coverage: BTreeSet<String>,
            materials: Vec<(String, f32)>,
        }
        let mut merged: Vec<Merged> = Vec::new();
        for (index, sub_coverage) in self.armor_entries.iter().enumerate() {
            let materials: Vec<&ArmorPart> = self
                .armor_parts
                .iter()
                .filter(|part| part.entry == index)
                .collect();
            let Some(first) = materials.first() else {
                continue;
            };
            for body_part in &first.covers {
                // The weight of an entry is how much of the body part its
                // sub-locations cover; the weight it meets is the coverage the row
                // has accumulated from the entries before it. A row is created by
                // copying its first entry whole, so materials that entry names
                // twice stay apart - a helmet padded twice lists two shares.
                #[allow(clippy::cast_precision_loss)]
                let new_weight = anatomy.coverage(body_part, sub_coverage) as f32;
                let position = merged.iter().position(|entry| &entry.part == body_part);
                let Some(index) = position else {
                    merged.push(Merged {
                        part: body_part.clone(),
                        sub_coverage: sub_coverage.clone(),
                        materials: materials
                            .iter()
                            .map(|part| (part.material.clone(), part.thickness))
                            .collect(),
                    });
                    continue;
                };
                let Some(row) = merged.get_mut(index) else {
                    continue;
                };
                #[allow(clippy::cast_precision_loss)]
                let old_weight = anatomy.coverage(body_part, &row.sub_coverage) as f32;
                for part in &materials {
                    match row
                        .materials
                        .iter_mut()
                        .find(|(name, _)| name == &part.material)
                    {
                        Some((_, thickness)) => {
                            if new_weight + old_weight > 0.0 {
                                *thickness = (new_weight * part.thickness
                                    + old_weight * *thickness)
                                    / (old_weight + new_weight);
                            }
                        }
                        None => row.materials.push((part.material.clone(), part.thickness)),
                    }
                }
                row.sub_coverage.extend(sub_coverage.iter().cloned());
            }
        }
        // Each row adds its own hundredths to a whole-number total, so the
        // fraction is dropped once per row rather than once at the end: two
        // 1.525 mm layers make 304, not 305 (`item_factory.cpp:1452-1463`,
        // where the total is an `int` and the addition is a float).
        let mut totals: BTreeMap<String, i32> = BTreeMap::new();
        for entry in &merged {
            for (name, thickness) in &entry.materials {
                let running = totals.get(name).copied().unwrap_or(0);
                #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
                let updated = (running as f32 + thickness * 100.0) as i32;
                totals.insert(name.clone(), updated);
            }
        }
        totals
    }

    /// The name for more than one of the item, as the reference prints it.
    ///
    /// `nname` forces the singular for liquids (`itype.cpp:107-114`): you do not
    /// say "two puddings", you say "two pudding". Soups, ice cream and every
    /// drink in the content rely on this, so a port that prints the stated
    /// plural is wrong for all of them.
    #[must_use]
    pub fn plural_name(&self) -> &str {
        if self.phase_is_liquid() {
            &self.name
        } else {
            &self.name_plural
        }
    }

    /// Whether the item's phase is anything but solid.
    ///
    /// The phase is compared without regard to case: the content writes both
    /// `"liquid"` and `"solid"` in lower case, and the reference reads them as
    /// the same phases the rest of the code spells in capitals.
    fn phase_is_liquid(&self) -> bool {
        !self.phase.eq_ignore_ascii_case("SOLID")
    }

    /// Its phase, as the rules that ask `made_of( phase_id )` see it (`src/item.h:1100-1110`).
    ///
    /// Reference: `item::made_of( phase_id::LIQUID )` and its gas sibling, which read the prototype's `phase`. The reference's own default is `SOLID`, so anything
    /// that is not a liquid or a gas is a solid.
    #[must_use]
    pub fn phase(&self) -> &'static str {
        if self.phase.eq_ignore_ascii_case("LIQUID") {
            "liquid"
        } else if self.phase.eq_ignore_ascii_case("GAS") || self.phase.eq_ignore_ascii_case("PLASMA") {
            "gas"
        } else {
            "solid"
        }
    }

    /// The name the reference prints for the item's colour.
    #[must_use]
    pub fn color_name(&self) -> &'static str {
        string_from_color(self.color)
    }
}

/// Loads every item type in the given directories, in the reference's order.
///
/// # Errors
///
/// Fails when a file cannot be read, a required field is missing, or a quantity
/// string does not parse.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<Vec<ItemType>, LoadError> {
    let mut objects: BTreeMap<String, Object> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut categories: BTreeSet<String> = BTreeSet::new();
    // **The names a gun's skill may claim.** The reference gives a gun the quality named after its skill only when that name is a quality it knows
    // (`qual_gun_skill.is_valid()`, `src/item_factory.cpp:847-851`), so a launcher gets `GUN` and no `LAUNCHER`. Collecting the ids during the same walk costs nothing.
    let mut known_qualities: BTreeSet<String> = BTreeSet::new();
    // The quality registry, for the actions each quality lends. Loaded once, before the types are finalised.
    let qualities_usages = crate::qualities::load_paths(dirs).unwrap_or_default();
    // The vitamin registry, because a vitamin amount may be written as a **mass** and only the vitamin itself knows the scale
    // (`vitamin::units_from_mass`, `src/vitamin.cpp:136-144`).
    let vitamins = crate::vitamins::load_paths(dirs).map_err(|error| LoadError {
        message: error.message,
    })?;
    let material_registry: BTreeMap<String, crate::materials::Material> =
        crate::materials::load_paths(dirs)
            .map_err(|error| LoadError {
                message: format!("materials: {error}"),
            })?
            .into_iter()
            .map(|material| (material.id.clone(), material))
            .collect();
    let registries = Registries {
        vitamins: &vitamins,
        materials: &material_registry,
    };
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            collect(
                &value,
                &mut objects,
                &mut order,
                &mut categories,
                &mut known_qualities,
            );
        }
    }
    let densities = densities_of(dirs)?;
    let categories = categories_of(categories);
    let anatomy = anatomy_of(dirs)?;
    let mut out = Vec::new();
    for id in &order {
        let mut seen = Vec::new();
        for mut item in resolve(id, &objects, &mut seen, &densities, &registries)? {
            // **The derived vitamins run once per prototype, not once per object.** The reference copies the *raw* slot through `copy-from` and derives per
            // prototype (`src/item_factory.cpp:762-776`), so a child of a food derives from its own materials rather than inheriting the parent's numbers — which
            // is how `blood_mi-go` (material `mi-go_blood`, no vitamins) comes out all-zero under a `meat_mi-go` parent whose flesh carries iron.
            crate::comestible::finalise(&mut item, &registries);
            // The category is derived once, here, and only for a prototype: an
            // abstract that states none stays without one, so a child that
            // inherits from it derives from its own slots (`item_factory.cpp:385-388`).
            // A stated category that names nothing is as good as none: the
            // reference derives when the id is not registered.
            if !categories.contains(&item.category) {
                item.category = category_of(
                    &item.subtypes,
                    &item.melee,
                    item.comestible.as_ref().map_or("", |slot| slot.comestible_type.as_str()),
                );
            }
            // Things that stack by charge cannot be repaired, and the reference
            // adds the flag rather than asking the content to
            // (`item_factory.cpp:319-321`).
            if item.counts_by_charge() {
                item.flags.insert("NO_REPAIR".to_owned());
            }
            item.derive_gun(&known_qualities);
            // The actions a type's qualities and its kind lend are added **at finalisation**, after the last file is read and before anything asks the type what
            // it can do (`src/item_factory.cpp:333-377`).
            add_quality_usages(&mut item, &qualities_usages);

            // A comestible made mostly of one thing lists only that thing
            // (`item_factory.cpp:459-464`), and it happens before the allergen
            // flags are read off the materials.
            if item.uses_comestible() && !item.primary_material.is_empty() {
                item.materials.clear();
                item.materials.insert(item.primary_material.clone(), 1);
            }
            // Allergen and implied flags are read from the materials the object
            // states, and they are read *before* the armour entries fill the
            // material list in - so an armour that declares no `material` never
            // acquires an allergen flag (`item_factory.cpp:752-754` against
            // `item_factory.cpp:1452-1463`).
            item.add_material_flags();
            item.add_implied_flags();
            if item.materials.is_empty() && !item.armor_parts.is_empty() {
                item.materials = item.armor_materials(&anatomy);
            }
            // `LIGHT_7` and friends are read into the item's light emission and
            // then dropped from the flag set (`item_factory.cpp:434-456`).
            item.flags.retain(|flag| !flag.starts_with("LIGHT_"));
            // Whether the ammunition cooks off is derived last, from the effects and the
            // materials both, which is why it cannot be read off the object directly.
            item.derive_cookoff();
            out.push(item);
        }
    }
    apply_mod_pockets(&mut out);
    Ok(out)
}

/// Why an item type could not be loaded.
#[derive(Debug)]
pub struct LoadError {
    /// The problem.
    pub message: String,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LoadError {}

fn collect(
    value: &Value,
    out: &mut BTreeMap<String, Object>,
    order: &mut Vec<String>,
    categories: &mut BTreeSet<String>,
    known_qualities: &mut BTreeSet<String>,
) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, out, order, categories, known_qualities);
            }
        }
        Value::Object(object) => {
            if matches!(object.get("type"), Some(Value::Str(text)) if text == "tool_quality") {
                if let Some(id) = string(object, "id") {
                    known_qualities.insert(id);
                }
                return;
            }
            // The dispatcher registers categories as `ITEM_CATEGORY`
            // (`init.cpp:352`); a category the content registers is a valid
            // category for `calc_category` purposes.
            if matches!(object.get("type"), Some(Value::Str(text)) if text.eq_ignore_ascii_case("item_category"))
            {
                if let Some(id) = string(object, "id") {
                    categories.insert(id);
                }
                return;
            }
            if !matches!(object.get("type"), Some(Value::Str(text)) if text == "GENERIC" || text == "ITEM")
            {
                return;
            }
            let concrete = ids_of(object);
            let template = concrete.is_empty() && object.get("abstract").is_some();
            let keys: Vec<String> = if template {
                string(object, "abstract").into_iter().collect()
            } else {
                concrete
            };
            for key in keys {
                if !template && !out.contains_key(&key) {
                    order.push(key.clone());
                }
                out.insert(key, object.clone());
            }
        }
        _ => {}
    }
}

/// The ids of one object: a string, or an array of strings.
fn ids_of(object: &Object) -> Vec<String> {
    match object.get("id") {
        Some(Value::Str(text)) => vec![text.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Materials that make their item an allergen, or that only some eaters like.
///
/// One table in the reference covers both (`item_factory.cpp:3550-3591`): the
/// allergen half applies even to trace amounts, and the second half marks what
/// a carnivore, a bear or a hemovore is happy to eat.
const MATERIAL_FLAGS: &[(&str, &str)] = &[
    ("hflesh", "ALLERGEN_MEAT"),
    ("iflesh", "ALLERGEN_MEAT"),
    ("bread", "ALLERGEN_BREAD"),
    ("flesh", "ALLERGEN_MEAT"),
    ("cheese", "ALLERGEN_CHEESE"),
    ("blood", "ALLERGEN_MEAT"),
    ("hblood", "ALLERGEN_MEAT"),
    ("bone", "ALLERGEN_MEAT"),
    ("wheat", "ALLERGEN_WHEAT"),
    ("fruit", "ALLERGEN_FRUIT"),
    ("veggy", "ALLERGEN_VEGGY"),
    ("dried_vegetable", "ALLERGEN_DRIED_VEGETABLE"),
    ("bean", "ALLERGEN_VEGGY"),
    ("tomato", "ALLERGEN_VEGGY"),
    ("garlic", "ALLERGEN_VEGGY"),
    ("nut", "ALLERGEN_NUT"),
    ("mushroom", "ALLERGEN_VEGGY"),
    ("milk", "ALLERGEN_MILK"),
    ("egg", "ALLERGEN_EGG"),
    ("junk", "ALLERGEN_JUNK"),
    ("wool", "ALLERGEN_WOOL"),
    ("flesh", "CARNIVORE_OK"),
    ("hflesh", "CARNIVORE_OK"),
    ("iflesh", "CARNIVORE_OK"),
    ("blood", "CARNIVORE_OK"),
    ("hblood", "CARNIVORE_OK"),
    ("honey", "URSINE_HONEY"),
    ("blood", "HEMOVORE_FUN"),
    ("hblood", "HEMOVORE_FUN"),
];

/// The body parts and their sub-locations, which decide how much of a part an
/// armour entry covers and therefore how its materials merge.
#[derive(Debug, Default)]
struct Anatomy {
    /// Sub-location ids per body part.
    parts: BTreeMap<String, Vec<String>>,
    /// Each sub-location's weight and parent, and whether it hangs.
    sub_parts: BTreeMap<String, (i32, String, bool)>,
}

impl Anatomy {
    /// How much of a body part a set of sub-locations covers
    /// (`itype.cpp:323-352`): the sum of their weights, or all of it when the
    /// part has no sub-locations at all.
    fn coverage(&self, part: &str, sub_coverage: &BTreeSet<String>) -> i32 {
        if self.parts.get(part).is_none_or(Vec::is_empty) {
            return 100;
        }
        let mut primary = 0;
        let mut secondary = 0;
        for id in sub_coverage {
            let Some((weight, parent, hangs)) = self.sub_parts.get(id) else {
                continue;
            };
            if parent != part {
                continue;
            }
            if *hangs {
                secondary += weight;
            } else {
                primary += weight;
            }
        }
        primary.max(secondary)
    }
}

/// Reads the body-part registries.
fn anatomy_of(dirs: &[&std::path::Path]) -> Result<Anatomy, LoadError> {
    let mut anatomy = Anatomy::default();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            collect_anatomy(&value, &mut anatomy);
        }
    }
    Ok(anatomy)
}

fn collect_anatomy(value: &Value, anatomy: &mut Anatomy) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_anatomy(item, anatomy);
            }
        }
        Value::Object(object) => {
            let Some(Value::Str(kind)) = object.get("type") else {
                return;
            };
            let Some(id) = string(object, "id") else {
                return;
            };
            if kind.eq_ignore_ascii_case("body_part") {
                anatomy.parts.insert(id, strings(object, "sub_parts"));
            } else if kind.eq_ignore_ascii_case("sub_body_part") {
                let weight = integer(object, "max_coverage").unwrap_or(0);
                let parent = string(object, "parent").unwrap_or_default();
                let hangs = boolean(object, "secondary").unwrap_or(false);
                anatomy.sub_parts.insert(id, (weight, parent, hangs));
            }
        }
        _ => {}
    }
}

/// The categories the content registers, plus the ones the reference creates.
///
/// `calc_category` only derives a category when the current one is invalid
/// (`item_factory.cpp:385`), and an unregistered id is invalid. Items that name
/// a category the content never defines - "tool", "TOOL" - therefore land in a
/// derived one.
fn categories_of(mut categories: BTreeSet<String>) -> BTreeSet<String> {
    for built_in in [
        "guns",
        "magazines",
        "ammo",
        "tools",
        "clothing",
        "food",
        "books",
        "mods",
        "bionics",
        "weapons",
        "other",
        "drugs",
        "spare_parts",
        "container",
        "armor",
        "tool_armor",
    ] {
        categories.insert(built_in.to_owned());
    }
    categories
}

/// The density of every material, which the weight adjustment needs.
fn densities_of(dirs: &[&std::path::Path]) -> Result<BTreeMap<String, f64>, LoadError> {
    let materials = crate::materials::load_paths(dirs).map_err(|error| LoadError {
        message: format!("materials: {error}"),
    })?;
    Ok(materials
        .into_iter()
        .map(|material| (material.id, material.density))
        .collect())
}

fn resolve(
    id: &str,
    objects: &BTreeMap<String, Object>,
    seen: &mut Vec<String>,
    densities: &BTreeMap<String, f64>,
    registries: &Registries<'_>,
) -> Result<Vec<ItemType>, LoadError> {
    if seen.iter().any(|entry| entry == id) {
        return Err(LoadError {
            message: format!("cyclic copy-from through {id}: {seen:?}"),
        });
    }
    let Some(object) = objects.get(id) else {
        return Err(LoadError {
            message: format!("item {id} is not defined"),
        });
    };
    seen.push(id.to_owned());
    let inherited = object.get("copy-from").is_some();
    let mut base = match object.get("copy-from") {
        Some(Value::Str(from)) => resolve(from, objects, seen, densities, registries)?
            .into_iter()
            .next(),
        _ => None,
    };
    seen.pop();
    let mut item = base.take().unwrap_or_else(|| ItemType {
        id: id.to_owned(),
        name: String::new(),
        name_plural: String::new(),
        symbol: 0,
        color: color_from_string("white").unwrap_or_default(),
        weight: 0,
        stack_size: 0,
        default_charges: 1,
        // Unset until the data or the fallback says otherwise.
        integral_weight: -1,
        volume: 0,
        ammo_types: Vec::new(),
        faults: Vec::new(),
        fault_groups: Vec::new(),
        use_actions: BTreeMap::new(),
        ammo_scale: BTreeMap::new(),
        charged_qualities: BTreeMap::new(),
        countdown_interval_seconds: 0,
        pockets: Vec::new(),
        added_pockets: Vec::new(),
        gun_mods: Vec::new(),
        longest_side: UNSET_LENGTH,
        integral_volume: -1,
        price: 0,
        category: String::new(),
        materials: BTreeMap::new(),
        flags: BTreeSet::new(),
        is_gun: false,
        gun_skill: None,
        is_gunmod: false,
        explode_in_fire: false,
        comestible: None,
        charges_to_use: 0,
        ammo_effects: BTreeSet::new(),
        cookoff: false,
        special_cookoff: false,
        qualities: BTreeMap::new(),
        subtypes: BTreeSet::new(),
        melee: BTreeMap::new(),
        stackable: false,
        phase: "SOLID".to_owned(),
        primary_material: String::new(),
        armor_parts: Vec::new(),
        armor_entries: Vec::new(),
    });
    id.clone_into(&mut item.id);
    apply(&mut item, object, inherited, densities, registries)?;
    Ok(vec![item])
}

#[allow(clippy::too_many_lines)]
fn apply(
    item: &mut ItemType,
    object: &Object,
    inherited: bool,
    densities: &BTreeMap<String, f64>,
    registries: &Registries<'_>,
) -> Result<(), LoadError> {
    let id = item.id.clone();
    // The name is cleared before it is read (`item_factory.cpp:4126`), so an
    // item that copies from another and does not state its own name ends up
    // with no name at all. Two item types in the content do exactly that, and
    // the reference reports both with an empty name.
    let names = names_of(object);
    read_faults(item, object);
    apply_use_actions(item, object)?;
    // **The type's own countdown**, written in the game's duration grammar.
    if let Some(value) = object.get("countdown_interval") {
        item.countdown_interval_seconds = crate::use_actions::duration_of(value);
    }
    // **Qualities the item has only while charged**, written `[ [ "OVEN", 2 ] ]` like the others.
    if object.get("charged_qualities").is_some() {
        item.charged_qualities = crate::qualities::item_qualities_keyed(object, "charged_qualities");
    }
    item.name.clear();
    item.name_plural.clear();
    if let Some((singular, plural)) = names {
        item.name = singular;
        item.name_plural = plural;
    } else if !inherited {
        return Err(LoadError {
            message: format!("item {id}: missing name"),
        });
    }
    if let Some(text) = string(object, "symbol") {
        let mut characters = text.chars();
        match (characters.next(), characters.next()) {
            (Some(character), None) => item.symbol = character as u32,
            _ => {
                return Err(LoadError {
                    message: format!("item {id}: symbol must be one character"),
                });
            }
        }
    }
    if let Some(text) = string(object, "color") {
        item.color = color_from_string(&text).map_err(|error| LoadError {
            message: format!("item {id}: color {text:?}: {error:?}"),
        })?;
    }
    read_measures(item, object)?;
    // Only when the object states its own: a copy-from child that says nothing keeps the pockets it inherited, which is how a robe's pockets reach the
    // artifacts that copy from it.
    // Only when the object states its own: an arrow that copies from another keeps the caliber it inherited, which is the third time this pattern has come up
    // (pockets, a material's softness, and now a caliber).
    if object.get("ammo_type").is_some() {
        item.ammo_types = read_ammo_types(object);
    }
    if object.get("pocket_data").is_some() {
        item.pockets = crate::pockets::read_pockets(object);
    }
    if let Some(Value::Array(mods)) = object.get("pocket_mods") {
        item.added_pockets = crate::pockets::specs_of(mods);
    }
    // `built_in_mods` and `default_mods` are what a freshly constructed gun has installed, built-ins first (`src/item.cpp:383-391`).
    for member in ["built_in_mods", "default_mods"] {
        if let Some(Value::Array(mods)) = object.get(member) {
            for entry in mods {
                if let Value::Str(id) = entry {
                    item.gun_mods.push(id.clone());
                }
            }
        }
    }
    if let Some(category) = string(object, "category") {
        item.category = category;
    }
    apply_slots(item, object, registries)?;
    if let Some(entries) = object.get("material") {
        item.materials = materials_of(entries)?;
    }
    // `material` is a container, so the merge operators reach it: an object may
    // add a material to the list it inherited, or drop one.
    if let Some(extend) = sub_object(object, "extend")
        && let Some(entries) = extend.get("material")
    {
        for (material, portion) in materials_of(entries)? {
            item.materials.insert(material, portion);
        }
    }
    if let Some(delete) = sub_object(object, "delete")
        && let Some(Value::Array(entries)) = delete.get("material")
    {
        for entry in entries {
            if let Value::Str(name) = entry {
                item.materials.remove(name);
            }
        }
    }
    if let Some(replacements) = sub_object(object, "replace_materials") {
        replace_materials(item, replacements, densities);
    }
    flesh_for_human_flesh(&mut item.materials);
    // Every item keeps a volume: the reference raises anything at or below zero
    // to one millilitre once the object has finished loading
    // (`item_factory.cpp:426-432`).
    if item.volume <= 0 {
        item.volume = 1;
    }
    if object.get("flags").is_some() {
        item.flags.clear();
        for flag in strings(object, "flags") {
            item.flags.insert(flag);
        }
    }
    if let Some(extend) = sub_object(object, "extend") {
        for flag in strings(extend, "flags") {
            item.flags.insert(flag);
        }
    }
    if let Some(delete) = sub_object(object, "delete") {
        for flag in strings(delete, "flags") {
            item.flags.remove(&flag);
        }
    }
    // **A stated `primary_material` replaces the material list outright**: one material, portion 1 `(`src/item_factory.cpp:463-467`)`. This runs before the
    // derived vitamins below, which is why `mead` - materials `["alcohol", "honey"]`, primary material `alcohol` - has no honey vitamins at all: the honey was
    // dropped before anything could read it. Eleven comestibles are like that, and the recording found all eleven.
    if let Some(slot) = &item.comestible
        && !slot.primary_material.is_empty()
        && slot.primary_material != "null"
    {
        let primary = slot.primary_material.clone();
        item.materials.clear();
        item.materials.insert(primary, 1);
    }

    Ok(())
}

/// Reads everything that names a slot: the subtypes, the legacy type, the
/// armour entries, the comestible fields and the melee damage.
fn apply_slots(
    item: &mut ItemType,
    object: &Object,
    registries: &Registries<'_>,
) -> Result<(), LoadError> {
    // Slots are inherited: an object that copies and states no subtypes of its
    // own keeps the base's, and the category is derived from them.
    if let Some(Value::Array(subtypes)) = object.get("subtypes") {
        for subtype in subtypes {
            if let Value::Str(name) = subtype {
                item.subtypes.insert(name.clone());
            }
        }
    }
    if let Some(Value::Str(json_type)) = object.get("type")
        && json_type != "ITEM"
        && json_type != "GENERIC"
    {
        item.subtypes.insert(json_type.clone());
    }
    // **The legacy spelling of a single subtype.** Older items say `"sub": "fire"` where newer ones say `"subtypes": [ "fire" ]`, and the reference reads both into the
    // same slot (`itype::sub`), which is what `item_controller->subtype_replacement` walks when it expands a requirement (`src/requirements.cpp:740-744`). A lit torch is
    // written that way, so without this a requirement that names `fire` never learns about it.
    if let Some(Value::Str(sub)) = object.get("sub") {
        item.subtypes.insert(sub.clone());
    }
    apply_armor_slots(item, object);
    apply_flat_slots(item, object, registries)?;
    apply_melee_slots(item, object);
    // Kept as the data states it, **including zero**: zero is what tells the finalisation to use the default charge count, and clamping it here would hide that.
    if let Some(Value::Int(size)) = object.get("stack_size") {
        item.stack_size = i32::try_from(*size).unwrap_or(0).max(0);
    }
    item.default_charges = read_default_charges(object, item.default_charges);
    // **Inherited unless declared.** The reference copies the whole type and overrides only the members the child states, so an item that says nothing about its
    // qualities keeps its parent's - a great many tool variants do exactly that.
    if object.get("qualities").is_some() {
        item.qualities = crate::qualities::item_qualities(object);
    }
    // **Qualities are a container too**, so the merge operators reach them: a dutch oven adds an `OVEN` quality to the ones it inherited from a cooking pot, and a kettle
    // drops the `COOK` quality it does not deserve (`"delete": { "qualities": [ "COOK" ] }`).
    if let Some(extend) = sub_object(object, "extend")
        && extend.get("qualities").is_some()
    {
        for (quality, level) in crate::qualities::item_qualities(extend) {
            item.qualities.insert(quality, level);
        }
    }
    if let Some(delete) = sub_object(object, "delete")
        && let Some(Value::Array(entries)) = delete.get("qualities")
    {
        for entry in entries {
            if let Value::Str(name) = entry {
                item.qualities.remove(name);
            }
        }
    }
    // **Gun data is written flat**, not nested: `"skill": "pistol"` sits beside `"ammo"` and `"dispersion"` on the item itself, and the reference reads it into its gun
    // data whenever the object carries any (`src/item_factory.cpp`, `gun` loader). Only an object that looks like a gun is asked, so a tool that happens to mention a
    // skill is left alone. It is inherited like every other member.
    if let Some(Value::Str(skill)) = object.get("skill")
        && (item.subtypes.contains("GUN") || object.get("ammo").is_some())
    {
        item.gun_skill = Some(skill.clone());
    }
    Ok(())
}

/// Reads the `armor` slot: what covers what, and in what material.
fn apply_armor_slots(item: &mut ItemType, object: &Object) {
    if let Some(Value::Array(entries)) = object.get("armor") {
        item.armor_parts.clear();
        item.armor_entries.clear();
        for (index, entry) in entries.iter().enumerate() {
            let Value::Object(entry) = entry else {
                continue;
            };
            let covers = strings(entry, "covers");
            item.armor_entries
                .push(strings(entry, "specifically_covers").into_iter().collect());
            let Some(Value::Array(materials)) = entry.get("material") else {
                continue;
            };
            for material in materials {
                let Value::Object(material) = material else {
                    continue;
                };
                let Some(Value::Str(name)) = material.get("type") else {
                    continue;
                };
                #[allow(clippy::cast_possible_truncation)]
                let thickness = material.get("thickness").and_then(numeric).unwrap_or(0.0) as f32;
                item.armor_parts.push(ArmorPart {
                    material: name.clone(),
                    thickness,
                    covers: covers.clone(),
                    entry: index,
                });
            }
        }
    }
}

/// Reads the slots that are one value each: the material the thing is mostly made of, its phase, whether it stacks, whether it goes off in a fire, whether it
/// is a gun, and what it does when eaten.
/// The registries a slot may consult, which are not part of the item data itself.
pub(crate) struct Registries<'a> {
    /// Vitamin types, for a vitamin amount written as a mass.
    pub vitamins: &'a BTreeMap<String, crate::vitamins::Vitamin>,
    /// Materials, for the vitamins a comestible derives when it states none.
    pub materials: &'a BTreeMap<String, crate::materials::Material>,
}

fn apply_flat_slots(
    item: &mut ItemType,
    object: &Object,
    registries: &Registries<'_>,
) -> Result<(), LoadError> {
    if let Some(material) = string(object, "primary_material") {
        item.primary_material = material;
    }
    if let Some(phase) = string(object, "phase") {
        item.phase = phase;
    }
    if let Some(stackable) = boolean(object, "stackable") {
        item.stackable = stackable;
    }
    if let Some(explodes) = boolean(object, "explode_in_fire") {
        item.explode_in_fire = explodes;
    }
    if let Some(is_gun) = boolean(object, "is_gun") {
        item.is_gun = is_gun;
    }
    if let Some(is_gunmod) = boolean(object, "is_gunmod") {
        item.is_gunmod = is_gunmod;
    }
    // Ammunition states its effects as `effects` (`item_factory.cpp:2990`), not `ammo_effects`, which is only the name the reference gives the field once read.
    //
    // **The subtype is the inherited one**, not the object's own: a round that copies from another and says nothing about its type is still ammunition, and asking
    // the object alone skipped every such type. **The member is a container**, so a stated list is cleared first and `extend`/`delete` reach it — 231 rounds extend
    // their effects and 177 delete some, which the corpus caught as 238 of 736 types.
    if item.subtypes.contains("AMMO") {
        if object.get("effects").is_some() {
            item.ammo_effects.clear();
            for effect in strings(object, "effects") {
                item.ammo_effects.insert(effect);
            }
        }
        if let Some(extend) = sub_object(object, "extend") {
            for effect in strings(extend, "effects") {
                item.ammo_effects.insert(effect);
            }
        }
        if let Some(delete) = sub_object(object, "delete") {
            for effect in strings(delete, "effects") {
                item.ammo_effects.remove(&effect);
            }
        }
    }
    // The slot **merges** with what the parent supplied: `load_slot` reuses the inherited object and reads into it (`item_factory.cpp:2859-2868`), so a child that
    // states only `calories` keeps its parent's `quench` and `spoils_in`. The calories multiply lives in `comestible::read`, once per prototype.
    item.comestible = crate::comestible::read(object, item.comestible.as_ref(), registries.vitamins)
        .map_err(|error| LoadError {
            message: error.message,
        })?;

    Ok(())
}

/// Reads the `melee_damage` slot, with the `relative` and `proportional` forms the content uses to adjust it.
fn apply_melee_slots(item: &mut ItemType, object: &Object) {
    if let Some(melee) = sub_object(object, "melee_damage") {
        for (damage_type, amount) in melee.members() {
            if let Some(value) = numeric(amount) {
                item.melee.insert(damage_type.clone(), value);
            }
        }
    }
    if let Some(relative) =
        sub_object(object, "relative").and_then(|inner| sub_object(inner, "melee_damage"))
    {
        for (damage_type, amount) in relative.members() {
            if let Some(value) = numeric(amount) {
                *item.melee.entry(damage_type.clone()).or_insert(0.0) += value;
            }
        }
    }
    if let Some(proportional) =
        sub_object(object, "proportional").and_then(|inner| sub_object(inner, "melee_damage"))
    {
        for (damage_type, amount) in proportional.members() {
            if let Some(value) = numeric(amount)
                && let Some(current) = item.melee.get_mut(damage_type)
            {
                *current *= value;
            }
        }
    }
}

/// Swaps one material for another, and adjusts the weight by the density ratio
/// of the material's share (`item_factory.cpp:4001-4035`).
///
/// A steel sabre renamed to aluminium in `replace_materials` weighs less afterwards,
/// because the steel share of its weight is scaled by the ratio of the two
/// densities. Portions carry over, and the total used for the share is the one
/// before any replacement in the same object.
fn replace_materials(
    item: &mut ItemType,
    replacements: &Object,
    densities: &BTreeMap<String, f64>,
) {
    let total: i32 = item.materials.values().sum();
    for (old, new) in replacements.members() {
        let Value::Str(new_name) = new else {
            continue;
        };
        // The reference reads the old portion through `materials[ old ]`, which
        // inserts the material with a portion of zero when it is not there. That
        // zero survives: an item that swaps a material it never had ends up
        // listing it with no share (`item_factory.cpp:4007`)。
        let portion = *item.materials.entry(old.clone()).or_insert(0);
        if portion != 0
            && total != 0
            && let (Some(old_density), Some(new_density)) =
                (densities.get(old), densities.get(new_name))
            && *old_density != 0.0
        {
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
            let share = (item.weight as f64 * f64::from(portion) / f64::from(total)) as i64;
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
            let scaled = (share as f64 * (new_density / old_density)) as i64;
            item.weight += scaled - share;
        }
        if portion != 0 {
            let portion = item.materials.remove(old).unwrap_or(0);
            *item.materials.entry(new_name.clone()).or_insert(0) += portion;
        }
    }
}

/// Replaces the human-flesh material with plain flesh, keeping the portion
/// (`item_factory.cpp:3600-3617`). Only when flesh is not already there.
fn flesh_for_human_flesh(materials: &mut BTreeMap<String, i32>) {
    let Some(portion) = materials.remove("hflesh") else {
        return;
    };
    materials.entry("flesh".to_owned()).or_insert(portion);
}

/// A quantity that may be stated, adjusted, or scaled.
///
/// The order is the reader's, not `assign`'s: a plain member wins, then
/// `proportional`, then `relative` (`generic_factory.h:1236-1241`). That is why
/// the one item in the content that states both a proportional and a relative
/// weight gets the proportional one - a port that copied `assign.cpp:58-75`
/// would get it wrong by 500 grams.
fn adjusted(
    object: &Object,
    member: &str,
    current: i64,
    reader: fn(&str) -> Result<i64, cdda_core_types::units::UnitError>,
    int_scale: i64,
) -> Result<i64, LoadError> {
    if let Some(value) = quantity(object, member, reader, int_scale)? {
        return Ok(value);
    }
    if let Some(proportional) = sub_object(object, "proportional")
        && let Some(scalar) = number(proportional, member)
    {
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        return Ok((current as f64 * scalar) as i64);
    }
    if let Some(relative) = sub_object(object, "relative")
        && let Some(value) = quantity(relative, member, reader, int_scale)?
    {
        return Ok(current + value);
    }
    Ok(current)
}

/// The category the reference derives when the object states none.
///
/// Reference: `calc_category` (`item_factory.cpp:4827-4865`). The order matters:
/// the first slot that applies wins, and an item with no slot at all is a weapon
/// only if one of its damage types exceeds the melee threshold of five.
fn category_of(
    subtypes: &BTreeSet<String>,
    melee: &BTreeMap<String, f64>,
    comestible_type: &str,
) -> String {
    let has = |name: &str| subtypes.contains(name);
    let category = if has("GUN") && !has("GUNMOD") {
        "guns"
    } else if has("MAGAZINE") {
        "magazines"
    } else if has("AMMO") {
        "ammo"
    } else if has("TOOL") && !has("TOOLMOD") {
        "tools"
    } else if has("ARMOR") {
        "clothing"
    } else if has("COMESTIBLE") {
        // A medicinal comestible is a drug; everything else is food.
        if comestible_type == "MED" {
            "drugs"
        } else {
            "food"
        }
    } else if has("BOOK") {
        "books"
    } else if has("GUNMOD") || has("TOOLMOD") {
        "mods"
    } else if has("BIONIC") {
        "bionics"
    } else if melee.values().any(|amount| *amount > 5.0) {
        "weapons"
    } else {
        "other"
    };
    category.to_owned()
}

/// A number, from an integer or a float member.
pub(crate) fn numeric(value: &Value) -> Option<f64> {
    match value {
        Value::Int(number) =>
        {
            #[allow(clippy::cast_precision_loss)]
            Some(*number as f64)
        }
        Value::Float(number) => Some(*number),
        _ => None,
    }
}

/// A number, from an integer or a float member.
pub(crate) fn number(object: &Object, member: &str) -> Option<f64> {
    match object.get(member) {
        Some(Value::Int(value)) =>
        {
            #[allow(clippy::cast_precision_loss)]
            Some(*value as f64)
        }
        Some(Value::Float(value)) => Some(*value),
        _ => None,
    }
}

/// A name and its plural: text, or an object with `str` and `str_pl`.
fn names_of(object: &Object) -> Option<(String, String)> {
    match object.get("name") {
        Some(Value::Str(text)) => Some((text.clone(), plural_of(text))),
        Some(Value::Object(inner)) => {
            // `str_sp` is the spelling for a name whose plural is itself, as in
            // "boots (pair)"; `str` alone takes the default plural.
            let singular = string(inner, "str").or_else(|| string(inner, "str_sp"))?;
            let plural = string(inner, "str_pl")
                .or_else(|| string(inner, "str_sp"))
                .unwrap_or_else(|| plural_of(&singular));
            Some((singular, plural))
        }
        _ => None,
    }
}

/// The reference's default plural: append an `s`, with no English rules at all.
///
/// "search" becomes "searchs" and "seismograph" becomes "seismographs" in the
/// reference, so a port that knows about sibilants disagrees with it. Names that
/// need a real plural say so with `str_pl` or `str_sp`.
fn plural_of(singular: &str) -> String {
    format!("{singular}s")
}

fn quantity(
    object: &Object,
    member: &str,
    reader: fn(&str) -> Result<i64, cdda_core_types::units::UnitError>,
    int_scale: i64,
) -> Result<Option<i64>, LoadError> {
    match object.get(member) {
        Some(Value::Int(value)) => Ok(Some(value * int_scale)),
        Some(Value::Str(text)) => reader(text).map(Some).map_err(|error| LoadError {
            message: format!("{member} {text:?}: {error}"),
        }),
        _ => Ok(None),
    }
}

/// A weighted material list.
///
/// Three spellings all mean the same thing (`generic_factory.h:1416-1449`): a
/// name, a `[ name, weight ]` pair, or a one-or-two member object whose string
/// member is the name and whose number is the weight - the content writes
/// `{ "type": "wood", "portion": 19 }`. A name alone weighs one.
fn materials_of(entries: &Value) -> Result<BTreeMap<String, i32>, LoadError> {
    // A single material may be written as one name instead of a list.
    if let Value::Str(name) = entries {
        let mut out = BTreeMap::new();
        out.insert(name.clone(), 1);
        return Ok(out);
    }
    let Value::Array(items) = entries else {
        return Ok(BTreeMap::new());
    };
    let mut out = BTreeMap::new();
    for entry in items {
        match entry {
            Value::Str(name) => {
                out.insert(name.clone(), 1);
            }
            Value::Object(inner) => {
                let mut name = None;
                let mut weight = 1;
                for (_key, value) in inner.members() {
                    match value {
                        Value::Str(text) => name = Some(text.clone()),
                        Value::Int(number) => {
                            weight = i32::try_from(*number).unwrap_or(1);
                        }
                        Value::Float(number) => {
                            #[allow(clippy::cast_possible_truncation)]
                            {
                                weight = *number as i32;
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(name) = name {
                    out.insert(name, weight);
                }
            }
            Value::Array(pair) => {
                let Some(Value::Str(name)) = pair.first() else {
                    return Err(LoadError {
                        message: "material entry is not a name".to_owned(),
                    });
                };
                let weight = match pair.get(1) {
                    Some(Value::Int(value)) => i32::try_from(*value).unwrap_or(1),
                    _ => 1,
                };
                out.insert(name.clone(), weight);
            }
            _ => {}
        }
    }
    Ok(out)
}

pub(crate) fn string(object: &Object, member: &str) -> Option<String> {
    match object.get(member) {
        Some(Value::Str(text)) => Some(text.clone()),
        _ => None,
    }
}

pub(crate) fn integer(object: &Object, member: &str) -> Option<i32> {
    match object.get(member) {
        Some(Value::Int(value)) => i32::try_from(*value).ok(),
        _ => None,
    }
}

pub(crate) fn boolean(object: &Object, member: &str) -> Option<bool> {
    match object.get(member) {
        Some(Value::Bool(value)) => Some(*value),
        _ => None,
    }
}

pub(crate) fn text_or(object: &Object, member: &str, fallback: &str) -> String {
    match object.get(member) {
        Some(Value::Str(value)) => value.clone(),
        _ => fallback.to_owned(),
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub(crate) fn as_f32(member: &str, value: &Value) -> Result<f32, crate::mapdata::LoadError> {
    match value {
        Value::Int(number) => Ok(*number as f32),
        Value::Float(number) => Ok(*number as f32),
        other => Err(crate::mapdata::LoadError {
            message: format!("{member} is a number, not {other:?}"),
        }),
    }
}

#[allow(clippy::cast_possible_truncation)]
pub(crate) fn as_i32(member: &str, value: &Value) -> Result<i32, crate::mapdata::LoadError> {
    match value {
        Value::Int(number) => i32::try_from(*number).map_err(|_| crate::mapdata::LoadError {
            message: format!("{member} is out of range: {number}"),
        }),
        Value::Float(number) => Ok(*number as i32),
        other => Err(crate::mapdata::LoadError {
            message: format!("{member} is a number, not {other:?}"),
        }),
    }
}

pub(crate) fn sub_object<'a>(object: &'a Object, member: &str) -> Option<&'a Object> {
    match object.get(member) {
        Some(Value::Object(inner)) => Some(inner),
        _ => None,
    }
}

pub(crate) fn strings(object: &Object, member: &str) -> Vec<String> {
    match object.get(member) {
        // One value may be written instead of a list, which the reference's
        // flag reader accepts (`generic_factory.h:1099-1105`).
        Some(Value::Str(text)) => vec![text.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// What the reference says about eating each item, read from its own dump.
///
/// The nutr value is the reference's `get_default_nutr()` - kilocalories divided by its own
/// `kcal_per_nutr` - which is why it is read from the dump rather than recomputed here: the constant is
/// internal to the reference and reproducing it would be inventing a number.
#[must_use]
pub fn nutrition_from_dump(
    value: &cdda_json::Value,
) -> std::collections::BTreeMap<String, (i32, i32, i32)> {
    let mut out = std::collections::BTreeMap::new();
    let Value::Object(root) = value else {
        return out;
    };
    let Some(Value::Array(entries)) = root.get("prototypes") else {
        return out;
    };
    for entry in entries {
        let Value::Object(object) = entry else {
            continue;
        };
        let Some(Value::Str(id)) = object.get("id") else {
            continue;
        };
        let number = |key: &str| match object.get(key) {
            Some(Value::Int(value)) => i32::try_from(*value).unwrap_or(0),
            _ => 0,
        };
        out.insert(
            id.clone(),
            (number("nutr"), number("quench"), number("charges_to_use")),
        );
    }
    out
}
