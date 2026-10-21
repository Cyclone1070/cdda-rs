//! Objects: the reference's member order, and duplicate keys.

use cdda_json::{Value, parse};

/// The keys of a parsed object, in stored order.
fn keys(parsed: &Result<Value, cdda_json::ParseError>) -> Option<Vec<&str>> {
    if let Ok(Value::Object(object)) = parsed {
        Some(object.members().iter().map(|(k, _)| k.as_str()).collect())
    } else {
        None
    }
}

#[test]
fn members_are_stored_in_the_references_key_order() {
    // flexbuffers sorts every map's keys with `strcmp` when the map is closed
    // (src/third-party/flatbuffers/flexbuffers.h:1127-1151), so document order
    // is *not* preserved; ASCII order is.
    assert_eq!(
        keys(&parse(b"{\"b\":1,\"a\":2,\"B\":3,\"_\":4,\"0\":5}")),
        Some(vec!["0", "B", "_", "a", "b"])
    );
}

#[test]
fn duplicate_keys_are_a_hard_error() {
    // `idl_parser.cpp:3033-3034` turns the builder's duplicate-key flag into an
    // error, for any map, however deeply nested.
    for input in [
        &b"{\"a\":1,\"a\":2}"[..],
        &b"{\"o\":{\"a\":1,\"a\":2}}"[..],
        &b"[{\"a\":1,\"a\":2}]"[..],
    ] {
        let error = parse(input).err();
        assert_eq!(
            error.map(|e| (e.message, e.at_eof)),
            Some(("FlexBuffers map has duplicate keys".to_string(), true)),
            "input {input:?}"
        );
    }
}
