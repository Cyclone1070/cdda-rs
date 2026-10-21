//! Scalar values: the keywords and the number kinds.

use cdda_json::{Value, parse};

#[test]
fn keywords_are_recognised_inside_an_array() {
    assert_eq!(
        parse(b"[true,false,null]"),
        Ok(Value::Array(vec![
            Value::Bool(true),
            Value::Bool(false),
            Value::Null
        ]))
    );
}

#[test]
fn integers_and_floats_keep_their_kind() {
    // `1` is an integer token, while `1.0` and `1e1` are float tokens: the
    // reference branches on the token kind, not on the value
    // (src/third-party/flatbuffers/idl_parser.cpp:3050-3060).
    assert_eq!(
        parse(b"[1,1.0,1e1,1.5,-2]"),
        Ok(Value::Array(vec![
            Value::Int(1),
            Value::Float(1.0),
            Value::Float(10.0),
            Value::Float(1.5),
            Value::Int(-2),
        ]))
    );
}
