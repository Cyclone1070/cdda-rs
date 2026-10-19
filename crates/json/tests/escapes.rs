//! String escapes: decoded while lexing, exactly as the reference decodes them.

use cdda_json::{Value, parse};

/// The single string of a one-element array, or `None` if the shape differs.
fn element(parsed: &Result<Value, cdda_json::ParseError>) -> Option<Value> {
    if let Ok(Value::Array(items)) = parsed {
        items.first().cloned()
    } else {
        None
    }
}

#[test]
fn standard_escapes_are_decoded() {
    // `idl_parser.cpp:379-415`: n, t, r, b, f, quote, apostrophe, backslash,
    // slash.
    assert_eq!(
        parse(b"[\"a\\nb\\tc\\rd\\be\\ff\\\"g\\\\h\\/i\\'j\"]"),
        Ok(Value::Array(vec![Value::Str(
            "a\nb\tc\rd\u{8}e\u{c}f\"g\\h/i'j".to_string()
        )]))
    );
}

#[test]
fn non_json_escapes_are_decoded_too() {
    // `\xNN` and `\uNNNN` are not JSON escapes, but the reference accepts both
    // (idl_parser.cpp:416-455), including surrogate pairs.
    assert_eq!(
        element(&parse(b"[\"\\x41\"]")),
        Some(Value::Str("A".to_string()))
    );
    assert_eq!(
        element(&parse(b"[\"\\u00e9\"]")),
        Some(Value::Str("é".to_string()))
    );
    assert_eq!(
        element(&parse(b"[\"\\uD83D\\uDE00\"]")),
        Some(Value::Str("\u{1F600}".to_string()))
    );
    // Key prefixes agree up to the embedded NUL, so flexbuffers sees one key
    // twice; the reference compares keys as NUL-terminated C strings
    // (flexbuffers.h:1131-1149).
    assert_eq!(
        parse(b"{\"a\\u0000b\":1,\"a\\u0000c\":2}")
            .err()
            .map(|e| e.message),
        Some("FlexBuffers map has duplicate keys".to_string())
    );
    assert_eq!(
        element(&parse(b"[\"a\\u0000b\"]")),
        Some(Value::Str("a\u{0}b".to_string()))
    );
}
