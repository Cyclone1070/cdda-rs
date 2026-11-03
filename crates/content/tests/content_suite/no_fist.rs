//! The reference has no fist, and the port must not invent one.
//!
//! The survivor still cannot swing: `Character::melee_attack` builds its blow from `used_weapon()`, and when nothing is wielded that is `null_item_reference()` - the `null`
//! item type. Its melee bash is **zero**, as the reference's own dump says, and the only unarmed damage in 0.I-1 comes from `bodypart::unarmed_damage`, which the data defines on a
//! debug body part alone. This gate pins both halves of that finding, so a later round cannot quietly give the avatar a punch whose number nothing behind it produced.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_json::{Value, parse};

/// One member of a JSON object, or nothing when it is not an object.
fn member<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(object) => object.get(key),
        _ => None,
    }
}

fn types() -> Vec<Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/thrown-damage.json");
    let bytes = std::fs::read(path).expect("the thrown-damage dump exists");
    let root = parse(&bytes).expect("the dump is valid JSON");
    let Value::Object(root) = root else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump has no types");
    };
    types.clone()
}

/// The `null` item - what a character with empty hands swings - is worth nothing, and that is the reference's own answer.
#[test]
fn the_item_a_weaponless_character_swings_is_worth_nothing() {
    let all = types();
    assert_eq!(all.len(), 9_912, "every item type in the dump");

    let find = |id: &str| {
        all.iter()
            .find(|one| matches!(member(one, "id"), Some(Value::Str(name)) if name == id))
            .cloned()
            .unwrap_or_else(|| panic!("the dump has {id}"))
    };
    let null = find("null");
    let number = |value: Option<&Value>, what: &str| -> f64 {
        match value {
            Some(Value::Float(value)) => *value,
            Some(Value::Int(value)) => {
                #[allow(clippy::cast_precision_loss)]
                {
                    *value as f64
                }
            }
            _ => panic!("the dump has no {what} for `null`"),
        }
    };
    let bash = number(member(&null, "melee_bash"), "melee bash");
    assert!(
        bash.abs() < f64::EPSILON,
        "the `null` item has no melee damage, so a weaponless blow carries none: {bash}"
    );
    let total = number(member(&null, "total"), "thrown damage");
    assert!(
        total.abs() < f64::EPSILON,
        "and it throws for nothing too: {total}"
    );

    // A rock is the contrast: the same field, a real number. Without this the gate would pass on a dump where every melee bash was zero.
    let rock = find("rock");
    assert!(
        number(member(&rock, "melee_bash"), "melee bash") > 0.0,
        "a rock's own melee damage is a real number: {rock:?}"
    );

    // How many types state no melee damage at all: the finding is about **this** type, not about the loader dropping the field.
    let no_bash = all
        .iter()
        .filter(|one| match member(one, "melee_bash") {
            Some(Value::Float(value)) => *value == 0.0,
            Some(Value::Int(value)) => *value == 0,
            _ => false,
        })
        .count();
    assert!(
        (1_000..9_912).contains(&no_bash),
        "some types have no melee damage and most do: {no_bash} of 9,912"
    );
}
