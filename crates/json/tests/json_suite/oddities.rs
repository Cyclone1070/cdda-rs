//! Inputs the reference accepts that a strict JSON reader would not, and vice
//! versa.

use cdda_json::{ParseError, Value, parse};

/// The message of the parse error, if the input was rejected.
fn message(input: &[u8]) -> Option<String> {
    parse(input).err().map(|e| e.message)
}

/// The member `key` of a parsed object, if the input was an object with it.
fn member<'a>(parsed: &'a Result<Value, ParseError>, key: &str) -> Option<&'a Value> {
    if let Ok(Value::Object(object)) = parsed {
        object.get(key)
    } else {
        None
    }
}

#[test]
fn words_that_are_not_json_literals_are_rejected() {
    // Only `true`, `false` and `null` are recognised, case-sensitively, and a
    // bare identifier is never a value (`idl_parser.cpp:3061-3072`). A strict
    // reader agrees here: `nan` and `inf` are not numbers.
    for word in ["nan", "NaN", "inf", "Infinity", "True", "NULL", "TRUE"] {
        assert_eq!(
            message(word.as_bytes()).as_deref(),
            Some(format!("cannot parse value starting with: {word}").as_str()),
            "word {word}"
        );
    }
}

#[test]
fn a_signed_word_is_a_string_constant() {
    // A `+`/`-` followed by an identifier start is lexed as a *string constant*
    // (`idl_parser.cpp:505-514`), so these are strings, not numbers, and not
    // errors.
    assert_eq!(
        parse(b"[-inf,+nan,+PI]"),
        Ok(Value::Array(vec![
            Value::Str("-inf".to_string()),
            Value::Str("+nan".to_string()),
            Value::Str("+PI".to_string()),
        ]))
    );
    // Single-quoted strings, keys included, are accepted as well
    // (`idl_parser.cpp:365-366`).
    let parsed = parse(b"{'a':'b'}");
    assert_eq!(
        member(&parsed, "a"),
        Some(&Value::Str("b".to_string())),
        "single-quoted key and value"
    );
}
