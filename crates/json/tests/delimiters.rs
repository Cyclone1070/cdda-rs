//! Separators, columns and the exact wording of the errors around them.
//!
//! Every expectation here was verified against the reference parser (see
//! `crates/json/tools/reference_probe.cpp`), positions included.

/// The rejection as the reference prints it, position prefix included.
///
/// The prefix is `line:column:` while the cursor is inside the input and `EOF:`
/// once it has run out (`idl_parser.cpp:126-145`), so this is the form an error
/// message can be compared in.
fn rejection(input: &[u8]) -> Option<String> {
    cdda_json::parse(input).err().map(|error| error.text())
}

#[test]
fn trailing_and_missing_commas_are_rejected() {
    // `strict_json` keeps the empty-element exception only before the first
    // element, so a trailing comma runs into a value parse
    // (src/third-party/flatbuffers/idl_parser.cpp:1191-1192, :1382-1394).
    assert_eq!(
        rejection(b"[1,]"),
        Some("EOF: error: cannot parse value starting with: ]".to_string())
    );
    assert_eq!(
        rejection(b"{\"a\":1,}"),
        Some("EOF: error: expecting: string constant instead got: }".to_string())
    );
    assert_eq!(
        rejection(b"[1 2]"),
        Some("1:4: error: expecting: , instead got: integer constant".to_string())
    );
    assert_eq!(
        rejection(b"{\"a\":1 \"b\":2}"),
        Some("1:10: error: expecting: , instead got: string constant".to_string())
    );
}

#[test]
fn keys_must_be_strings_and_have_a_colon() {
    // `ParseTableDelimiters` expects a string-constant key under `strict_json`
    // and then a colon (`idl_parser.cpp:1202-1212`).
    assert_eq!(
        rejection(b"{a:1}"),
        Some("1:2: error: expecting: string constant instead got: a".to_string())
    );
    assert_eq!(
        rejection(b"{\"a\" 1}"),
        Some("1:6: error: expecting: : instead got: integer constant".to_string())
    );
    assert_eq!(
        rejection(b"[,]"),
        Some("1:2: error: cannot parse value starting with: ,".to_string())
    );
}
