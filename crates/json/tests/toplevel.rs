//! What a whole document is: exactly one value, and nothing after it is looked
//! at.

use cdda_json::{Value, parse};

#[test]
fn empty_input_is_an_error() {
    // `StartParseFile` refuses a file whose first token is EOF, so whitespace
    // and comments alone are empty too
    // (src/third-party/flatbuffers/idl_parser.cpp:3113-3123).
    for input in [&b""[..], &b"   \n\t "[..], &b"// only a comment\n"[..]] {
        let error = parse(input).err();
        assert_eq!(
            error.map(|e| (e.message, e.at_eof)),
            Some(("input file is empty".to_string(), true)),
            "input {input:?}"
        );
    }
}

#[test]
fn only_the_first_value_is_read() {
    // `ParseFlexBuffer` parses one value and never asks whether the input ended
    // (`idl_parser.cpp:3077-3086`), so trailing text is silently ignored. A
    // strict JSON reader rejects all four of these.
    assert_eq!(parse(b"{} {}"), parse(b"{}"));
    assert_eq!(parse(b"[]]"), Ok(Value::Array(Vec::new())));
    assert_eq!(parse(b"1 2"), Ok(Value::Int(1)));
    assert_eq!(parse(b"{\"a\":1}this is not json"), parse(b"{\"a\":1}"));
}
