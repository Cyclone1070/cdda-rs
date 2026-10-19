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
use cdda_core_types::units::{read_mass, read_money, read_volume};
use cdda_json::{Object, Value};
use std::collections::{BTreeMap, BTreeSet};

/// One resolved item type, as far as this slice reads it.
#[derive(Debug, Clone, PartialEq)]
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
    /// Weight in grams.
    pub weight: i64,
    /// Volume in millilitres.
    pub volume: i64,
    /// Price in cents.
    pub price: i64,
    /// Category id, empty when the item states none.
    pub category: String,
    /// Materials and their portions.
    pub materials: BTreeMap<String, i32>,
    /// Its flag set.
    pub flags: BTreeSet<String>,
    /// The subtypes it declares, or the legacy type it was written as, carried
    /// so the category can be derived the way `calc_category` does.
    pub subtypes: BTreeSet<String>,
    /// Damage types and their amounts, from `melee_damage`.
    pub melee: BTreeMap<String, f64>,
    /// `comestible_type` for food and drink, which separates drugs from food.
    pub comestible_type: String,
    /// `stackable` as stated, which decides charge counting.
    pub stackable: bool,
    /// `phase`, which separates liquids from solids.
    pub phase: String,
    /// The material a comestible is mostly made of, which replaces its list.
    pub primary_material: String,
    /// The per-part armour entries: each material with the thickness it is laid
    /// on with, and how many body parts that entry covers.
    pub armor_parts: Vec<ArmorPart>,
}

/// One material of one armour entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ArmorPart {
    /// The material's name.
    pub material: String,
    /// Its thickness in millimetres, at the single precision the reference
    /// carries it in.
    pub thickness: f32,
    /// How many body parts the entry covers, which is how often the portion is
    /// counted.
    pub covers: usize,
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

    /// Whether the item is counted in charges rather than as objects
    /// (`itype.h:1602-1604`): it stacks, or it is ammunition, or it is a
    /// non-solid comestible.
    #[must_use]
    pub fn counts_by_charge(&self) -> bool {
        self.stackable
            || self.subtypes.contains("AMMO")
            || (!self.comestible_type.is_empty() && self.phase_is_liquid())
    }

    /// Whether the item's phase is anything but solid.
    ///
    /// The phase is compared without regard to case: the content writes both
    /// `"liquid"` and `"solid"` in lower case, and the reference reads them as
    /// the same phases the rest of the code spells in capitals.
    fn phase_is_liquid(&self) -> bool {
        !self.phase.eq_ignore_ascii_case("SOLID")
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
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let bytes = std::fs::read(&path).map_err(|error| LoadError {
                message: format!("{}: {error}", path.display()),
            })?;
            let Ok(value) = cdda_json::parse(&bytes) else {
                continue;
            };
            collect(&value, &mut objects, &mut order, &mut categories);
        }
    }
    let densities = densities_of(dirs)?;
    let categories = categories_of(categories);
    let mut out = Vec::new();
    for id in &order {
        let mut seen = Vec::new();
        for mut item in resolve(id, &objects, &mut seen, &densities)? {
            // The category is derived once, here, and only for a prototype: an
            // abstract that states none stays without one, so a child that
            // inherits from it derives from its own slots (`item_factory.cpp:385-388`).
            // A stated category that names nothing is as good as none: the
            // reference derives when the id is not registered.
            if !categories.contains(&item.category) {
                item.category = category_of(&item.subtypes, &item.melee, &item.comestible_type);
            }
            // Things that stack by charge cannot be repaired, and the reference
            // adds the flag rather than asking the content to
            // (`item_factory.cpp:319-321`).
            if item.counts_by_charge() {
                item.flags.insert("NO_REPAIR".to_owned());
            }
            // A comestible made mostly of one thing lists only that thing
            // (`item_factory.cpp:459-464`), and it happens before the allergen
            // flags are read off the materials.
            if item.uses_comestible() && !item.primary_material.is_empty() {
                item.materials.clear();
                item.materials.insert(item.primary_material.clone(), 1);
            }
            // An armour that lists no materials of its own takes them from the
            // per-part entries, at a hundredth of a unit per millimetre of
            // thickness, counted once per covered body part
            // (`item_factory.cpp:1452-1463`).
            if item.materials.is_empty() && !item.armor_parts.is_empty() {
                // The reference adds these in single precision, and the rounding
                // is visible: a 1.05 mm layer over two body parts becomes 209
                // hundredths of a unit rather than 210.
                let mut totals: BTreeMap<String, f32> = BTreeMap::new();
                for part in &item.armor_parts {
                    let thickness = part.thickness;
                    #[allow(clippy::cast_precision_loss)]
                    let covers = part.covers as f32;
                    *totals.entry(part.material.clone()).or_insert(0.0) += thickness * covers;
                }
                for (material, total) in totals {
                    #[allow(clippy::cast_possible_truncation)]
                    item.materials.insert(material, (total * 100.0) as i32);
                }
            }
            item.add_material_flags();
            item.add_implied_flags();
            // `LIGHT_7` and friends are read into the item's light emission and
            // then dropped from the flag set (`item_factory.cpp:434-456`).
            item.flags.retain(|flag| !flag.starts_with("LIGHT_"));
            out.push(item);
        }
    }
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
) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, out, order, categories);
            }
        }
        Value::Object(object) => {
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
        Some(Value::Str(from)) => resolve(from, objects, seen, densities)?.into_iter().next(),
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
        volume: 0,
        price: 0,
        category: String::new(),
        materials: BTreeMap::new(),
        flags: BTreeSet::new(),
        subtypes: BTreeSet::new(),
        melee: BTreeMap::new(),
        comestible_type: String::new(),
        stackable: false,
        phase: "SOLID".to_owned(),
        primary_material: String::new(),
        armor_parts: Vec::new(),
    });
    id.clone_into(&mut item.id);
    apply(&mut item, object, inherited, densities)?;
    Ok(vec![item])
}

fn apply(
    item: &mut ItemType,
    object: &Object,
    inherited: bool,
    densities: &BTreeMap<String, f64>,
) -> Result<(), LoadError> {
    let id = item.id.clone();
    // The name is cleared before it is read (`item_factory.cpp:4126`), so an
    // item that copies from another and does not state its own name ends up
    // with no name at all. Two item types in the content do exactly that, and
    // the reference reports both with an empty name.
    let names = names_of(object);
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
    // A bare number is in the unit's own base: grams for weight, millilitres for
    // volume, cents for price (`units_reader`).
    item.weight = adjusted(object, "weight", item.weight, read_mass, 1_000)?;
    item.volume = adjusted(object, "volume", item.volume, read_volume, 1)?;
    item.price = adjusted(object, "price", item.price, read_money, 1)?;
    if let Some(category) = string(object, "category") {
        item.category = category;
    }
    apply_slots(item, object);
    if let Some(entries) = object.get("material") {
        item.materials = materials_of(entries)?;
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
    Ok(())
}

/// Reads everything that names a slot: the subtypes, the legacy type, the
/// armour entries, the comestible fields and the melee damage.
fn apply_slots(item: &mut ItemType, object: &Object) {
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
    if let Some(Value::Array(entries)) = object.get("armor") {
        item.armor_parts.clear();
        for entry in entries {
            let Value::Object(entry) = entry else {
                continue;
            };
            let covers = strings(entry, "covers").len();
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
                    covers,
                });
            }
        }
    }
    if let Some(material) = string(object, "primary_material") {
        item.primary_material = material;
    }
    if let Some(phase) = string(object, "phase") {
        item.phase = phase;
    }
    if let Some(stackable) = boolean(object, "stackable") {
        item.stackable = stackable;
    }
    if let Some(kind) = string(object, "comestible_type") {
        item.comestible_type = kind;
    }
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
        let portion = item.materials.get(old).copied().unwrap_or(0);
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
        if let Some(portion) = item.materials.remove(old) {
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
fn numeric(value: &Value) -> Option<f64> {
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
fn number(object: &Object, member: &str) -> Option<f64> {
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

fn string(object: &Object, member: &str) -> Option<String> {
    match object.get(member) {
        Some(Value::Str(text)) => Some(text.clone()),
        _ => None,
    }
}

fn boolean(object: &Object, member: &str) -> Option<bool> {
    match object.get(member) {
        Some(Value::Bool(value)) => Some(*value),
        _ => None,
    }
}

fn sub_object<'a>(object: &'a Object, member: &str) -> Option<&'a Object> {
    match object.get(member) {
        Some(Value::Object(inner)) => Some(inner),
        _ => None,
    }
}

fn strings(object: &Object, member: &str) -> Vec<String> {
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
