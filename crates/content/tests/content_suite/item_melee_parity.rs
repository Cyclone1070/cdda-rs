//! The loader's melee numbers and the reference's own dump agree, for every item type.
//!
//! The registry gate (`sim_suite::melee_rules`) proves the registry **carries** what it is given; this one proves the **loader** gives it the right thing. The two are different
//! mistakes: round 1096 found the loader reading an object `to_hit` as the -2 default while every registry test passed, because the registry's fixture was built from the dump.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_json::{Value, parse};
use std::collections::BTreeMap;

/// What the reference says about one item type.
struct Dumped {
    to_hit: i64,
    bash: f64,
    cut: f64,
    stab: f64,
}

fn dumped() -> BTreeMap<String, Dumped> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/thrown-damage.json");
    let bytes = std::fs::read(path).expect("the item dump exists");
    let root = parse(&bytes).expect("the dump is valid JSON");
    let Value::Object(root) = root else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump has no types");
    };
    let number = |value: Option<&Value>| -> f64 {
        match value {
            Some(Value::Float(value)) => *value,
            Some(Value::Int(value)) => {
                #[allow(clippy::cast_precision_loss)]
                {
                    *value as f64
                }
            }
            _ => 0.0,
        }
    };
    types
        .iter()
        .filter_map(|one| {
            let Value::Object(fields) = one else {
                return None;
            };
            let Some(Value::Str(id)) = fields.get("id") else {
                return None;
            };
            #[allow(clippy::cast_possible_truncation)]
            let to_hit = number(fields.get("to_hit")) as i64;
            Some((
                id.clone(),
                Dumped {
                    to_hit,
                    bash: number(fields.get("melee_bash")),
                    cut: number(fields.get("melee_cut")),
                    stab: number(fields.get("melee_stab")),
                },
            ))
        })
        .collect()
}

/// Every item the game ships is loaded with the melee numbers the reference gives it.
#[test]
fn every_items_melee_numbers_are_the_references() {
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded = cdda_content::items::load_paths(&paths).expect("the game's items load");
    let by_id: BTreeMap<&str, &cdda_content::items::ItemType> =
        loaded.iter().map(|item| (item.id.as_str(), item)).collect();
    let expected = dumped();
    assert_eq!(expected.len(), 9_912, "every type in the dump");

    let mut wrong: Vec<String> = Vec::new();
    let mut compared = 0_usize;
    for (id, want) in &expected {
        let Some(item) = by_id.get(id.as_str()) else {
            wrong.push(format!("{id}: not loaded"));
            continue;
        };
        compared += 1;
        let bash = item.melee.get("bash").copied().unwrap_or(0.0);
        let cut = item.melee.get("cut").copied().unwrap_or(0.0);
        let stab = item.melee.get("stab").copied().unwrap_or(0.0);
        if i64::from(item.to_hit) != want.to_hit
            || (bash - want.bash).abs() > f64::EPSILON
            || (cut - want.cut).abs() > f64::EPSILON
            || (stab - want.stab).abs() > f64::EPSILON
        {
            wrong.push(format!(
                "{id}: to_hit {} vs {}, melee ({bash}, {cut}, {stab}) vs ({}, {}, {})",
                item.to_hit, want.to_hit, want.bash, want.cut, want.stab
            ));
        }
    }
    let first_few: Vec<&String> = wrong.iter().take(5).collect();
    assert!(
        wrong.is_empty(),
        "{} of {compared} wrong: {first_few:?}",
        wrong.len()
    );
    assert!(compared > 9_000, "and most of them were compared: {compared}");
}

/// A weapon's category names the proficiencies that make it cheaper to swing, and the data pairs them up.
///
/// Reference: `wielded_weapon_categories` (`melee.cpp:570-578`) reads the wielded item's `weapon_category`, and `get_total_melee_stamina_cost` (`melee.cpp:980-992`) then walks each
/// category's `category_proficiencies()` asking for a `melee_attack`/`stamina` bonus. **The pairing is the data's**: `weapon_categories.json` states which proficiencies belong to a
/// category, and the item states which categories it belongs to, so a port that guessed the link from names would get the common case right and the corpus wrong.
#[test]
fn a_weapon_category_names_its_proficiencies() {
    use std::collections::BTreeSet;

    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data/json");
    let file = data.join("weapon_categories.json");
    let Ok(text) = std::fs::read_to_string(&file) else {
        return;
    };
    let value = cdda_json::parse(text.as_bytes()).expect("the weapon categories parse");
    let categories = cdda_content::weapon_categories::from_data(&value);
    assert_eq!(categories.len(), 25, "**twenty-five weapon categories** in the data");

    let swords = categories.iter().find(|category| category.id == "SHORT_SWORDS").expect("short swords are a category");
    assert_eq!(
        swords.proficiencies,
        vec![
            "prof_short_swords_familiar".to_owned(),
            "prof_short_swords_pro".to_owned(),
            "prof_short_swords_master".to_owned()
        ],
        "and the data names the three proficiencies in its own order"
    );
    let named: BTreeSet<&str> = categories.iter().flat_map(|category| category.proficiencies.iter().map(String::as_str)).collect();
    assert!(named.len() > 70, "the categories name {} proficiencies between them", named.len());
    assert!(
        categories.iter().all(|category| !category.proficiencies.is_empty()),
        "and not one of them is empty"
    );
}

/// A weapon states which kind of weapon it is, and the data states it as a list.
///
/// Reference: `itype::weapon_category` (`itype.h`), read by `wielded_weapon_categories` (`melee.cpp:570-578`). **The data writes it two ways** - a bare string for most items and an array
/// where a weapon belongs to more than one kind - so a loader that read only strings would silently drop every grouped weapon. The gate counts both forms over the whole corpus.
#[test]
fn a_weapon_states_its_category() {
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded = cdda_content::items::load_paths(&paths).expect("the game's items load");

    let categorised: Vec<&cdda_content::items::ItemType> = loaded
        .iter()
        .filter(|item| !item.weapon_category.is_empty())
        .collect();
    assert!(
        categorised.len() > 500,
        "**hundreds of types name their weapon category**: {}",
        categorised.len()
    );
    assert!(
        categorised.iter().any(|item| item.weapon_category.len() > 1),
        "and at least one belongs to more than one kind, which is the array form"
    );
    assert!(
        categorised
            .iter()
            .all(|item| item.weapon_category.iter().all(|category| !category.is_empty())),
        "with every name a real one"
    );
}

/// A gun or a bow states the strength it wants, and a real bow wants more than nothing.
///
/// Reference: `item::get_min_str` (`item.cpp:15673-15694`), whose `type->min_str` comes from the data's `min_strength` (`item_factory.cpp:4089`). **A bow is the interesting case**: it is
/// exactly the kind of gun whose requirement the archery proficiencies reduce, so a loader that dropped the field would leave the reduction with nothing to reduce.
#[test]
fn a_weapon_states_the_strength_it_wants() {
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded = cdda_content::items::load_paths(&paths).expect("the game's items load");

    let demanding: Vec<&cdda_content::items::ItemType> = loaded
        .iter()
        .filter(|item| item.min_strength > 0)
        .collect();
    // **Twenty, and that small number is the finding**: the field only appears where an item states it, and the reference's own default is zero for everything else.
    assert_eq!(demanding.len(), 20, "twenty types say how strong a hand they need");
    assert!(
        demanding.iter().any(|item| item.gun_skill.as_deref() == Some("archery")),
        "and at least one of them is a bow, which is the one the archery proficiencies reduce"
    );
    // The largest is **90**, which is a mounted weapon rather than a typo: the field is the data's own number and the port passes it through unchanged.
    assert_eq!(
        demanding.iter().map(|item| item.min_strength).max(),
        Some(90),
        "and the largest requirement in the corpus is ninety"
    );
}

/// Armour states how much of the vitals it covers, separately from how much of the part it covers.
///
/// Reference: `item_factory.cpp:3163` - `optional( jo, false, "cover_vitals", cover_vitals, 0 )`, whose default is **zero and not the part's coverage**. That is the number
/// `Creature::get_crit_factor` (`creature.cpp:890-901`) subtracts, so a loader that defaulted it to the part's coverage would make every jacket stop every critical.
#[test]
fn armour_states_its_vitals_coverage() {
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded = cdda_content::items::load_paths(&paths).expect("the game's items load");

    let vest = loaded
        .iter()
        .find(|item| item.id == "ballistic_vest_esapi")
        .expect("the data has a ballistic vest");
    let torso = vest
        .armor_parts
        .iter()
        .find(|part| part.covers.iter().any(|part| part == "torso"))
        .expect("the vest covers the torso");
    assert_eq!(torso.cover_vitals, 90, "**the vest's torso entry covers ninety percent of the vitals**");

    let covering: Vec<&cdda_content::items::ItemType> = loaded
        .iter()
        .filter(|item| item.armor_parts.iter().any(|part| part.cover_vitals > 0))
        .collect();
    assert!(
        covering.len() > 50,
        "dozens of types state a vitals figure: {}",
        covering.len()
    );
    // **Most things state nothing**, and the default is zero: a plain shirt is not vitals armour.
    let shirt = loaded.iter().find(|item| item.id == "longshirt").expect("the data has a long shirt");
    assert!(
        shirt.armor_parts.iter().all(|part| part.cover_vitals == 0),
        "**and a long shirt covers none of the vitals**"
    );
}

/// A garment that states no melee or ranged coverage is met by the coverage it does state.
///
/// Reference: `optional( jo, false, "cover_melee", cover_melee, coverage )` and the same for `cover_ranged` (`item_factory.cpp:3159-3160`). **The fallback is the entry's own coverage and
/// not zero** - a loader that defaulted them to nothing would make every ordinary coat stop no blow at all, which is the class of error the vitals field already had to be fixed for.
#[test]
fn a_garment_states_its_melee_coverage_or_its_plain_one() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [root.join("core"), root.join("json")];
    let dirs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let items = cdda_content::items::load_paths(&dirs).expect("the game's items load");

    let mut stated = 0;
    let mut fallen_back = 0;
    for item in &items {
        for part in &item.armor_parts {
            // **The fallback is the entry's own `coverage`**, whatever it is - a garment that covers a sub-part and none of the whole torso states zero and keeps zero.
            if part.cover_melee == part.coverage && part.cover_ranged == part.coverage {
                fallen_back += 1;
            } else {
                stated += 1;
            }
        }
    }
    assert!(fallen_back > 500, "most garments state nothing extra: {fallen_back}");
    assert!(
        stated > 0,
        "**and some state a different figure, which is why the twins exist at all**"
    );
    assert_eq!(
        fallen_back + stated,
        items.iter().map(|item| item.armor_parts.len()).sum::<usize>(),
        "every armour entry was counted"
    );
}
