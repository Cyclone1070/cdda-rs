//! Numbers: the reference's unusual acceptances.

use cdda_json::{Value, parse};

#[test]
fn out_of_range_integers_are_accepted_and_become_zero() {
    // The reference converts integer tokens with `StringToInt`, i.e. `strtoll`
    // base 10 with the `errno` check, which yields 0 on overflow
    // (src/third-party/flatbuffers/util.h:303-327, util.h:407-410); a
    // hexadecimal integer token is not decimal either, so it is 0 as well
    // (verified against the reference build).
    assert_eq!(
        parse(b"[9223372036854775807,9223372036854775808,12345678901234567890123,-9223372036854775809,0x10,-0]"),
        Ok(Value::Array(vec![
            Value::Int(i64::MAX),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
        ]))
    );
}

#[test]
fn unusual_but_accepted_number_forms() {
    // Leading `+` and leading zeros are accepted, `.5` and `1.` are floats, and
    // a hexadecimal float keeps its C99 meaning because floats go through
    // `strtod` (idl_parser.cpp:3054-3059). Overflowing floats become infinity
    // and underflowing ones zero, with no error (verified against the reference
    // build).
    assert_eq!(
        parse(b"[+1,01,.5,1.,0x1p3,1e400,1e-400]"),
        Ok(Value::Array(vec![
            Value::Int(1),
            Value::Int(1),
            Value::Float(0.5),
            Value::Float(1.0),
            Value::Float(8.0),
            Value::Float(f64::INFINITY),
            Value::Float(0.0),
        ]))
    );
}
