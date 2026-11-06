//! Recipes and the flags that decide whether they can be learned.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

/// A recipe the reference refuses to teach is marked, and the loader keeps the mark.
///
/// Reference: `recipe::never_learn` (`recipe.h:173`), read by `learn_recipe` (`character_crafting.cpp:32-40`) which **returns without teaching** when it is set. The count is the gate's non-vacuity:
/// the reference's own dump marks **1,114 of its 6,291 recipes**, and a loader that dropped the field would leave every one of them learnable.
#[test]
fn the_never_learn_flag_survives_the_dump() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/recipes.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let value = cdda_json::parse(text.as_bytes()).expect("the recipe dump parses");
    let (recipes, count) = cdda_content::recipes::from_dump(&value).expect("the recipe dump loads");
    assert_eq!(recipes.len(), count, "the dump agrees with its own count");

    let never = recipes.iter().filter(|recipe| recipe.never_learn).count();
    assert_eq!(never, 1114, "**1,114 recipes can never be learned** - the reference's own number");
    assert!(
        recipes.iter().any(|recipe| !recipe.never_learn),
        "and most can be, so the flag is not simply true everywhere"
    );
}

/// A practice recipe carries the range its difficulty is clamped into, and most recipes carry none.
///
/// Reference: `recipe::practice_data` (`recipe.h:72-84`) - `min_difficulty`, `max_difficulty` and `skill_limit` - which `get_difficulty` and `get_skill_cap` read (`recipe.cpp:61-77`). The reference's own
/// dump marks **124 of 6,291** recipes as practice recipes, so the count is the gate's non-vacuity: a loader that invented a block for every recipe, or dropped it for all of them, fails.
#[test]
fn practice_recipes_carry_their_range() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/recipes.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let value = cdda_json::parse(text.as_bytes()).expect("the recipe dump parses");
    let (recipes, _) = cdda_content::recipes::from_dump(&value).expect("the recipe dump loads");

    let practice: Vec<_> = recipes.iter().filter(|recipe| recipe.practice.is_some()).collect();
    assert_eq!(practice.len(), 124, "**exactly 124 practice recipes**, the reference's own number");
    let first = practice.first().and_then(|recipe| recipe.practice).expect("one of them is first");
    assert!(first.min_difficulty <= first.max_difficulty, "the range is a range: {first:?}");
    assert!(
        practice.iter().any(|recipe| recipe.practice.is_some_and(|block| block.skill_limit < 20)),
        "and at least one of them states a real skill limit rather than the maximum"
    );
    assert!(
        recipes.iter().any(|recipe| recipe.practice.is_none()),
        "while an ordinary recipe has no block at all"
    );
}

/// A recipe answers its own difficulty and teaching cap, with practice recipes following the crafter.
///
/// Reference: `recipe::get_difficulty` / `recipe::get_skill_cap` (`recipe.cpp:61-77`). The gate reads **real recipes** from the reference's dump: a practice recipe's difficulty is the crafter's level
/// inside its range and its cap is `skill_limit - 1`, while an ordinary recipe answers its stated difficulty and caps at `difficulty * 1.25` truncated.
#[test]
fn a_recipe_answers_its_own_difficulty_and_cap() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/recipes.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let value = cdda_json::parse(text.as_bytes()).expect("the recipe dump parses");
    let (recipes, _) = cdda_content::recipes::from_dump(&value).expect("the recipe dump loads");

    let practice = recipes
        .iter()
        .find(|recipe| recipe.practice.is_some())
        .expect("a practice recipe exists");
    let block = practice.practice.expect("just found");
    assert_eq!(practice.difficulty_for(0), block.min_difficulty, "**below the range, the minimum**");
    assert_eq!(practice.difficulty_for(99), block.max_difficulty, "and above it, the maximum");
    assert_eq!(practice.teaching_cap(), block.skill_limit - 1, "the cap is the limit minus one");

    let ordinary = recipes
        .iter()
        .find(|recipe| recipe.practice.is_none() && recipe.difficulty > 0)
        .expect("an ordinary recipe exists");
    assert_eq!(ordinary.difficulty_for(0), ordinary.difficulty, "an ordinary recipe ignores the crafter");
    assert_eq!(
        ordinary.teaching_cap(),
        (f64::from(ordinary.difficulty) * 1.25) as i32,
        "and caps at its difficulty times one and a quarter, truncated"
    );
}

/// Every monster in the reference's dump is classified by the two ladders, and the counts show which rung does the work.
///
/// Reference: `mtype::bloodType` / `mtype::gibType` (`mtype.cpp:431-470`). The dump carries **1,154 monster prototypes** with their flags, materials and species, so the ladders can be
/// walked over all of them rather than over a fixture: the counts are the gate, and they say how much of the corpus each rung decides.
#[test]
fn the_monster_corpus_walks_both_ladders() {
    let monsters = crate::corpus::monsters_loaded();
    assert_eq!(monsters.len(), 1154, "**every prototype**, not a sample");

    let mut acid = 0;
    let mut bile = 0;
    let mut arthropod = 0;
    let mut veggy = 0;
    let mut insect = 0;
    let mut blood = 0;
    let mut falls_through = 0;
    let mut gibbed = 0;
    for monster in &monsters {
        let traits = cdda_content::blood::BloodTraits {
            acid: monster.flags.contains("ACID_BLOOD"),
            bile: monster.flags.contains("BILE_BLOOD"),
            arthropod: monster.flags.contains("ARTHROPOD_BLOOD"),
            warm: monster.flags.contains("WARM"),
        };
        let materials: Vec<&str> = monster.materials.keys().map(String::as_str).collect();
        match cdda_content::blood::blood_field(&traits, &materials) {
            Some("fd_acid") => acid += 1,
            Some("fd_bile") => bile += 1,
            Some("fd_blood_invertebrate") => arthropod += 1,
            Some("fd_blood_veggy") => veggy += 1,
            Some("fd_blood_insect") => insect += 1,
            Some("fd_blood") => blood += 1,
            _ => falls_through += 1,
        }
        if cdda_content::blood::gib_field(&materials, monster.species.contains("MOLLUSK")).is_some() {
            gibbed += 1;
        }
    }

    assert_eq!(acid, 20, "twenty acid-blooded prototypes");
    assert_eq!(bile, 8, "eight bile-blooded");
    assert_eq!(arthropod, 24, "twenty-four with an arthropod's blood");
    assert!(veggy > 0 && insect > 0, "and plants and insects are both represented: {veggy} and {insect}");
    assert!(blood > 500, "while the ordinary warm fleshy thing is the common case: {blood}");
    assert!(
        falls_through > 0,
        "**and some fall through to a bleed type the port does not load yet**: {falls_through}"
    );
    assert!(gibbed > 1000, "most of the corpus gibs into something: {gibbed}");
}

/// A monster with no blood of its own falls through to its species', and a rate of zero stops it dead.
///
/// Reference: `mtype::get_bleed_type` (`mtype.cpp:412-422`), the fall-through at the end of `bloodType`. The dump carries **1,154 prototypes with their `bleed_rate` and 34 species with
/// their `bleeds` field**, so the rule can be checked against the reference's own numbers rather than a fixture: the counts are the gate.
#[test]
fn a_monster_falls_through_to_its_species_blood() {
    // The **dump**, not the game's own files: it is the shape these loaders speak, and it carries the species table the reference's generator keeps beside the prototypes.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let value = cdda_json::parse(text.as_bytes()).expect("the monster dump parses");
    let cdda_json::Value::Object(root) = &value else {
        return;
    };
    let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
        return;
    };
    let monsters = cdda_content::monsters::load_objects(prototypes).expect("the monster dump loads");
    let species: std::collections::BTreeMap<String, cdda_content::monsters::SpeciesType> =
        cdda_content::monsters::species_from_dump(&value)
            .into_iter()
            .map(|entry| (entry.id.clone(), entry))
            .collect();
    assert_eq!(species.len(), 31, "**every species the monsters name** is in the dump");

    let mut falls_through = 0;
    let mut answered_by_species = 0;
    let mut rate_zero = 0;
    for monster in &monsters {
        let traits = cdda_content::blood::BloodTraits {
            acid: monster.flags.contains("ACID_BLOOD"),
            bile: monster.flags.contains("BILE_BLOOD"),
            arthropod: monster.flags.contains("ARTHROPOD_BLOOD"),
            warm: monster.flags.contains("WARM"),
        };
        let materials: Vec<&str> = monster.materials.keys().map(String::as_str).collect();
        if cdda_content::blood::blood_field(&traits, &materials).is_some() {
            continue;
        }
        falls_through += 1;
        if monster.bleed_rate <= 0 {
            rate_zero += 1;
        }
        let names: Vec<&str> = monster.species.iter().map(String::as_str).collect();
        let found = cdda_content::blood::bleed_field(monster.bleed_rate, &names, |kind| {
            species.get(kind).map(|entry| entry.bleeds.as_str())
        });
        if found.is_some() {
            answered_by_species += 1;
        }
    }

    assert!(falls_through > 0, "some prototypes have no blood of their own: {falls_through}");
    assert!(
        answered_by_species > 0,
        "**and some of those are answered by their species**, which is what the fall-through is for: {answered_by_species}"
    );
    assert!(rate_zero > 0, "while a bleed rate of zero stops the fall-through: {rate_zero}");
}

/// The monster dump carries each type's own death sentence and what it leaves, and most types state neither.
///
/// Reference: `monster_death_effect` (`mtype.h:274-285`). **321 of the 1,154 types state a sentence** and **199 say they leave nothing at all**, so a loader that dropped either field would
/// give every monster the same death - which is the sort of flattening a corpus count catches and a fixture does not.
#[test]
fn the_monster_dump_carries_its_death_data() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let value = cdda_json::parse(text.as_bytes()).expect("the monster dump parses");
    let cdda_json::Value::Object(root) = &value else {
        return;
    };
    let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
        return;
    };
    let monsters = cdda_content::monsters::load_objects(prototypes).expect("the monster dump loads");

    let with_message = monsters.iter().filter(|monster| !monster.death_message.is_empty()).count();
    assert_eq!(with_message, 321, "**321 types state their own death sentence**");
    assert!(
        monsters.iter().any(|monster| monster.death_message.contains("%s")),
        "and at least one of them has a placeholder for the name"
    );
    let nothing = monsters.iter().filter(|monster| monster.corpse_type == "none").count();
    assert_eq!(nothing, 199, "while 199 say they leave nothing at all");
    assert!(
        monsters.iter().any(|monster| monster.corpse_type == "normal"),
        "and the ordinary type still leaves a corpse"
    );
}

/// The dump says which types name their own broken item and which name a death-drop group - and the answers are lopsided.
///
/// Reference: `mtype::broken_itype` (`mtype.h:367`) and `mtype::death_drops` (`mtype.h:404`). **Not one of the 1,154 types sets `broken_itype`**, so the reference's own string rule
/// (`mon_manhack` -> `broken_manhack`) is the whole rule for this corpus, and **339 name a death-drop group** while the rest leave their loot to an inline group the dump cannot name.
#[test]
fn the_monster_dump_carries_its_broken_item_and_drops() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let value = cdda_json::parse(text.as_bytes()).expect("the monster dump parses");
    let cdda_json::Value::Object(root) = &value else {
        return;
    };
    let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
        return;
    };
    let monsters = cdda_content::monsters::load_objects(prototypes).expect("the monster dump loads");

    assert_eq!(
        monsters.iter().filter(|monster| !monster.broken_itype.is_empty()).count(),
        0,
        "**no type names its own broken item**, which is what makes the string rule the whole rule"
    );
    let droppers = monsters.iter().filter(|monster| !monster.death_drops.is_empty()).count();
    assert_eq!(droppers, 339, "339 types name the group their loot comes from");
    assert!(
        monsters.iter().any(|monster| monster.death_drops == "default_zombie_death_drops"),
        "and the ordinary zombie is one of them"
    );
    assert!(
        monsters.iter().all(|monster| monster.death_drops.is_empty()
            || monster.death_drops.chars().all(|letter| letter.is_ascii_graphic() || letter == '_')),
        "with every id a name the data wrote, not a number the loader generated"
    );
}

/// Three types mend in the dark, and the dump says which.
///
/// Reference: `mtype::regenerates_in_dark` (`mtype.h:541`) with `monster.cpp:3487-3494`. **Three of 1,154** is the sort of count that has to be pinned: a loader that read the flag as a
/// number, or a dump that dropped it, would give every monster the same answer and every count would still look plausible.
#[test]
fn three_monster_types_mend_in_the_dark() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let value = cdda_json::parse(text.as_bytes()).expect("the monster dump parses");
    let cdda_json::Value::Object(root) = &value else {
        return;
    };
    let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
        return;
    };
    let monsters = cdda_content::monsters::load_objects(prototypes).expect("the monster dump loads");
    let dark: Vec<&str> = monsters
        .iter()
        .filter(|monster| monster.regenerates_in_dark)
        .map(|monster| monster.id.as_str())
        .collect();
    assert_eq!(dark.len(), 3, "**three types mend by the dark**: {dark:?}");
    // The loader pushes the null monster after the data's own, so the count is the corpus plus that one.
    assert_eq!(monsters.len(), 1155, "out of the whole corpus, plus the null monster the reference keeps at zero");
}
