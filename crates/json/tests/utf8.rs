//! UTF-8: strings are byte sequences, and the reference insists they are valid
//! UTF-8.

use cdda_json::{Value, parse};

/// The message of the parse error, if the input was rejected.
fn message(input: &[u8]) -> Option<String> {
    parse(input).err().map(|e| e.message)
}

#[test]
fn invalid_utf8_in_a_string_is_rejected() {
    // `ValidateUTF8` runs over the decoded string whenever it contained an
    // escape or a byte outside printable ASCII
    // (src/third-party/flatbuffers/idl_parser.cpp:474-477, util.h:521-561):
    // lone high bytes, truncated sequences, overlong encodings and encoded
    // surrogates are all refused.
    for input in [
        &b"[\"\xff\"]"[..],
        &b"[\"\\xff\"]"[..],
        &b"[\"\xc3\"]"[..],
        &b"[\"\xc0\xaf\"]"[..],
        &b"[\"\xed\xa0\x80\"]"[..],
        &b"{\"\xff\":1}"[..],
    ] {
        assert_eq!(
            message(input).as_deref(),
            Some("illegal UTF-8 sequence"),
            "input {input:?}"
        );
    }
}

#[test]
fn valid_utf8_in_a_string_is_accepted() {
    assert_eq!(
        parse(b"[\"\xc3\xa9\"]"),
        Ok(Value::Array(vec![Value::Str("é".to_string())]))
    );
    assert_eq!(
        parse(b"[\"\\xc3\\xa9\"]"),
        Ok(Value::Array(vec![Value::Str("é".to_string())]))
    );
    // DEL is outside printable ASCII but is valid UTF-8, so it survives.
    assert_eq!(
        parse(b"[\"\x7f\"]"),
        Ok(Value::Array(vec![Value::Str("\u{7f}".to_string())]))
    );
}
