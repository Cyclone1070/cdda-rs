//! Malformed strings and escapes, and the message each one gets.

use cdda_json::parse;

/// The message of the parse error, if the input was rejected.
fn message(input: &[u8]) -> Option<String> {
    parse(input).err().map(|e| e.message)
}

#[test]
fn malformed_escapes_are_rejected_with_the_references_words() {
    // Each message and the check that produces it:
    // unknown escape -> idl_parser.cpp:456; short \x and \u -> :307-312;
    // surrogates -> :429-453, :459-472.
    assert_eq!(
        message(b"[\"\\q\"]").as_deref(),
        Some("unknown escape code in string constant")
    );
    assert_eq!(
        message(b"[\"\\x1\"]").as_deref(),
        Some("escape code must be followed by 2 hex digits")
    );
    assert_eq!(
        message(b"[\"\\u12\"]").as_deref(),
        Some("escape code must be followed by 4 hex digits")
    );
    assert_eq!(
        message(b"[\"\\uD800\"]").as_deref(),
        Some("illegal Unicode sequence (unpaired high surrogate)")
    );
    assert_eq!(
        message(b"[\"\\uDC00\"]").as_deref(),
        Some("illegal Unicode sequence (unpaired low surrogate)")
    );
    assert_eq!(
        message(b"[\"\\uD800\\uD800\"]").as_deref(),
        Some("illegal Unicode sequence (multiple high surrogates)")
    );
    assert_eq!(
        message(b"[\"\\uD800\\n\"]").as_deref(),
        Some("illegal Unicode sequence (unpaired high surrogate)")
    );
}

#[test]
fn unterminated_and_control_byte_strings_are_rejected() {
    // The closing quote is found by scanning for it; the terminating NUL and
    // any other control byte are refused first (idl_parser.cpp:369-371).
    assert_eq!(
        message(b"{\"a").as_deref(),
        Some("illegal character in string constant")
    );
    assert_eq!(
        message(b"[\"a\nb\"]").as_deref(),
        Some("illegal character in string constant")
    );
    assert_eq!(
        message(b"[\"a\tb\"]").as_deref(),
        Some("illegal character in string constant")
    );
}
