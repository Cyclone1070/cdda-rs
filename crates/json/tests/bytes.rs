//! Bytes: the byte order mark, NUL, and non-ASCII input.

use cdda_json::{Value, parse};

#[test]
fn a_byte_order_mark_is_allowed_only_at_the_very_start() {
    // `SkipByteOrderMark` runs once, before the first token, and insists on all
    // three bytes (src/third-party/flatbuffers/idl_parser.cpp:319-329).
    assert_eq!(
        parse(b"\xef\xbb\xbf{}"),
        Ok(Value::Object(cdda_json::Object::new()))
    );
    assert_eq!(
        parse(b"\xef{}").err().map(|e| e.message),
        Some("invalid utf-8 byte order mark".to_string())
    );
    // Anywhere else the bytes are ordinary (illegal) input.
    assert_eq!(
        parse(b" \xef\xbb\xbf{}").err().map(|e| e.message),
        Some("illegal character: code: -17".to_string())
    );
}

#[test]
fn a_nul_byte_ends_the_input() {
    // The parser is handed a NUL-terminated buffer and stops at the first NUL
    // (idl_parser.cpp:344-347), so an embedded NUL silently truncates the file.
    assert_eq!(parse(b"{\"a\":1}\x00{\"b\":2}"), parse(b"{\"a\":1}"));
    assert_eq!(
        parse(b"{\"a\":1,\x00\"b\":2}").err().map(|e| e.message),
        Some("expecting: string constant instead got: end of file".to_string())
    );
}
