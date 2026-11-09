//! A `math` given where a **value** is expected, not an effect.
//!
//! Reference: `get_dbl_or_var` / `read_var_value` accept the `{ "math": [...] }` form wherever a number is taken (`npctalk.cpp`), and the data uses it for a
//! `switch`'s value: `EOC_YUGG_DEATH` switches on `{ "math": [ "rand(4)" ] }`, which is a **list of statements** rather than a single string. The port read the
//! string form and refused the list, so the program could not be read at all.

#![allow(clippy::panic, clippy::expect_used)]

#[test]
fn a_math_given_as_a_list_of_statements_is_a_value() {
    let value = cdda_json::parse(br#"{"math": ["rand(4)"]}"#).expect("JSON");
    assert!(
        cdda_content::dialogue::read_expression(&value).is_some(),
        "**a one-statement `math` is a value**, which is how the data switches on a random number"
    );
    let sum = cdda_json::parse(br#"{"math": ["2 + 3"]}"#).expect("JSON");
    assert!(
        cdda_content::dialogue::read_expression(&sum).is_some(),
        "and so is any expression the language reads"
    );
    let empty = cdda_json::parse(br#"{"math": []}"#).expect("JSON");
    assert!(
        cdda_content::dialogue::read_expression(&empty).is_none(),
        "an empty list is not a value, and is still refused"
    );
}
