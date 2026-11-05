//! Which parts a body has, and that every one of them is a part the reference knows.
//!
//! Sources: `corpus/anatomy-parts.json` is the game's own `anatomy.json` (the type the reference loads), and
//! `corpus/anatomy.json` is `--dump-anatomy`'s list of every body part the reference built. A gate on either one
//! alone would be weaker: the first says which names appear, the second says which names exist, and only together
//! do they say an anatomy lists parts that are real.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::anatomies::Anatomy;
use cdda_content::body_parts::BodyPartType;
use cdda_json::{Value, parse};
use std::collections::BTreeSet;
use std::path::Path;

fn read(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus").join(name);
    let text = std::fs::read(path).unwrap_or_else(|_| panic!("{name} exists"));
    parse(&text).unwrap_or_else(|_| panic!("{name} is valid JSON"))
}

fn anatomies() -> Vec<Anatomy> {
    let value = read("anatomy-parts.json");
    let Value::Array(prototypes) = value else {
        panic!("the file is an array");
    };
    cdda_content::anatomies::load_objects(&prototypes).expect("the anatomies load")
}

fn parts() -> Vec<BodyPartType> {
    let Value::Object(root) = read("anatomy.json") else {
        panic!("the dump's root is an object");
    };
    let Some(Value::Array(parts)) = root.get("parts") else {
        panic!("the dump has parts");
    };
    cdda_content::body_parts::load_objects(parts).expect("the parts load")
}

#[test]
fn a_person_has_two_of_each_limb_and_one_head() {
    let all = anatomies();
    assert!(all.len() >= 2, "the data holds {} anatomies", all.len());
    let human = all
        .iter()
        .find(|anatomy| anatomy.id == "human_anatomy")
        .expect("a person's anatomy");
    assert_eq!(human.parts.len(), 12, "twelve parts: {}", human.parts.join(", "));
    for wanted in ["head", "torso", "arm_l", "arm_r", "leg_l", "leg_r", "eyes", "mouth"] {
        assert!(
            human.parts.iter().any(|part| part == wanted),
            "a person has a {wanted}"
        );
    }
    let default = all
        .iter()
        .find(|anatomy| anatomy.id == "default_anatomy")
        .expect("the fallback anatomy");
    assert_eq!(default.parts, vec!["torso".to_owned(), "head".to_owned()]);
}

#[test]
fn every_part_an_anatomy_lists_is_a_part_the_reference_built() {
    let known: BTreeSet<String> = parts().into_iter().map(|part| part.id).collect();
    let mut checked = 0;
    for anatomy in anatomies() {
        for part in &anatomy.parts {
            checked += 1;
            assert!(
                known.contains(part),
                "anatomy {} lists {part}, which the reference never built",
                anatomy.id
            );
        }
    }
    assert!(checked >= 14, "only {checked} memberships were checked");
}
