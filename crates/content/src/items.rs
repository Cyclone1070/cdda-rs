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
    /// Weight in grams.
    pub weight: i64,
    /// What the item weighs when its caller asks for the **integral** weight, in grams.
    ///
    /// Reference: `itype::integral_weight` (`src/itype.h:1327`), read by `item::weight( integral: true )` (`src/item.cpp:7256-7261`). The loader copies `weight` into it
    /// when the data does not say (`src/item_factory.cpp:398-400`) - so the two are equal for almost every type - and the data may say **zero** on purpose, which three
    /// gunmods do because a gunmod's weight does not count toward the gun's integral weight.
    pub integral_weight: i64,
    /// Volume in millilitres.
    pub volume: i64,
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
    /// Nutrition, in the reference's own units, for a comestible; zero for anything else.
    pub nutr: i32,
    /// How much thirst it quenches.
    pub quench: i32,
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
    /// `comestible_type` for food and drink, which separates drugs from food.
    pub comestible_type: String,
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

impl ItemType {
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
        !self.comestible_type.is_empty()
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

    /// Whether the item is counted in charges rather than as objects
    /// (`itype.h:1602-1604`): it stacks, or it is ammunition, or it is a
    /// non-solid comestible.
    #[must_use]
    pub fn counts_by_charge(&self) -> bool {
        self.stackable
            || self.subtypes.contains("AMMO")
            || (!self.comestible_type.is_empty() && self.phase_is_liquid())
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
            item.derive_gun(&known_qualities);
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
        // Unset until the data or the fallback says otherwise.
        integral_weight: -1,
        volume: 0,
        integral_volume: -1,
        price: 0,
        category: String::new(),
        materials: BTreeMap::new(),
        flags: BTreeSet::new(),
        is_gun: false,
        gun_skill: None,
        is_gunmod: false,
        explode_in_fire: false,
        nutr: 0,
        quench: 0,
        charges_to_use: 0,
        ammo_effects: BTreeSet::new(),
        cookoff: false,
        special_cookoff: false,
        qualities: BTreeMap::new(),
        subtypes: BTreeSet::new(),
        melee: BTreeMap::new(),
        comestible_type: String::new(),
        stackable: false,
        phase: "SOLID".to_owned(),
        primary_material: String::new(),
        armor_parts: Vec::new(),
        armor_entries: Vec::new(),
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
    if let Some(category) = string(object, "category") {
        item.category = category;
    }
    apply_slots(item, object);
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
    // **The legacy spelling of a single subtype.** Older items say `"sub": "fire"` where newer ones say `"subtypes": [ "fire" ]`, and the reference reads both into the
    // same slot (`itype::sub`), which is what `item_controller->subtype_replacement` walks when it expands a requirement (`src/requirements.cpp:740-744`). A lit torch is
    // written that way, so without this a requirement that names `fire` never learns about it.
    if let Some(Value::Str(sub)) = object.get("sub") {
        item.subtypes.insert(sub.clone());
    }
    apply_armor_slots(item, object);
    apply_flat_slots(item, object);
    apply_melee_slots(item, object);
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
fn apply_flat_slots(item: &mut ItemType, object: &Object) {
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
    // Ammunition states its effects as `effects` (`item_factory.cpp:2990`), not `ammo_effects`,
    // which is only the name the reference gives the field once read.
    let is_ammo_now = strings(object, "subtypes")
        .iter()
        .any(|kind| kind == "AMMO");
    if is_ammo_now && object.get("effects").is_some() {
        item.ammo_effects.clear();
        for effect in strings(object, "effects") {
            item.ammo_effects.insert(effect);
        }
    }
    if let Some(kind) = string(object, "comestible_type") {
        item.comestible_type = kind;
    }
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
