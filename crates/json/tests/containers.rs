//! Empty containers.

use cdda_json::{Object, Value, parse};

#[test]
fn empty_object_parses_to_an_empty_object() {
    assert_eq!(parse(b"{}"), Ok(Value::Object(Object::new())));
}

#[test]
fn empty_array_parses_to_an_empty_array() {
    assert_eq!(parse(b"[]"), Ok(Value::Array(Vec::new())));
}
