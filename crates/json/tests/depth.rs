//! Nesting depth: the reference refuses the 65th nested value.

use cdda_json::{ParseError, Value, parse};

/// `[` repeated `depth` times, an optional innermost value, then the closers.
fn nested_arrays(depth: usize, inner: &[u8]) -> Vec<u8> {
    let mut input = vec![b'['; depth];
    input.extend_from_slice(inner);
    input.extend(std::iter::repeat_n(b']', depth));
    input
}

/// `{"a":` repeated `depth` times, an innermost value, then the closers.
fn nested_objects(depth: usize, inner: &[u8]) -> Vec<u8> {
    let mut input = Vec::new();
    for _ in 0..depth {
        input.extend_from_slice(b"{\"a\":");
    }
    input.extend_from_slice(inner);
    input.extend(std::iter::repeat_n(b'}', depth));
    input
}

#[test]
fn nesting_up_to_64_values_is_allowed() {
    // `FLATBUFFERS_MAX_PARSING_DEPTH` is 64 and the guard wraps the value
    // parser, so the 64th nested *value* is the deepest accepted one
    // (src/third-party/flatbuffers/idl.h:41-42, idl_parser.cpp:3015-3016).
    // An empty container at the bottom does not count as another value, because
    // the delimiters loop breaks before parsing one.
    assert!(matches!(
        parse(&nested_arrays(64, b"")),
        Ok(Value::Array(_))
    ));
    assert!(matches!(
        parse(&nested_objects(63, b"{}")),
        Ok(Value::Object(_))
    ));
    assert!(matches!(
        parse(&nested_objects(63, b"1")),
        Ok(Value::Object(_))
    ));
}

#[test]
fn the_65th_nested_value_is_rejected() {
    // The counter is incremented before it is checked, so the message names 65
    // (idl_parser.cpp:158-161, idl_parser.cpp:169). Positions verified against
    // the reference build.
    let error: Option<ParseError> = parse(&nested_arrays(64, b"0")).err();
    assert_eq!(
        error.map(|e| (e.message, e.line, e.column)),
        Some(("maximum parsing depth 65 reached".to_string(), 1, 65))
    );
    let error: Option<ParseError> = parse(&nested_objects(64, b"1")).err();
    assert_eq!(
        error.map(|e| (e.message, e.line, e.column)),
        Some(("maximum parsing depth 65 reached".to_string(), 1, 321))
    );
}
