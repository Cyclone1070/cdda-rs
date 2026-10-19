//! The writer, mirroring `JsonOut` (src/json.h:735-1000, src/json.cpp:2226-2440).

use cdda_json::{Value, Writer, parse};

/// A small document with one of everything.
fn sample() -> Value {
    let Ok(value) =
        parse(br#"{"b":[2,3.5,true,null,"x\ny"],"a":{"nested":-1},"s":"q\"\\\u00e9","z":1e-7}"#)
    else {
        return Value::Null;
    };
    value
}

#[test]
fn compact_output_has_no_whitespace_and_the_references_number_style() {
    // Compact is `pretty_print = false`: `,` and `:` with no padding, floats
    // printed with `std::fixed` and `showpoint` at the default precision of 6
    // (json.cpp:2226-2237, json.cpp:2256-2272, json.cpp:2275-2284).
    let mut writer = Writer::new(false);
    writer.start_object(false);
    writer.member("a");
    writer.write_int(1);
    writer.member("b");
    writer.start_array(false);
    writer.write_int(2);
    writer.write_float(3.5);
    writer.write_bool(true);
    writer.write_null();
    writer.write_str("x\ny");
    writer.end_array();
    writer.end_object();
    assert_eq!(
        writer.into_string(),
        "{\"a\":1,\"b\":[2,3.500000,true,null,\"x\\ny\"]}"
    );
}

#[test]
fn a_parsed_document_writes_back_in_stored_key_order() {
    // Objects keep the reference's key order, so output order is a property of
    // the value, not of the iteration order at the call site.
    let mut writer = Writer::new(false);
    writer.write_value(&sample());
    assert_eq!(
        writer.into_string(),
        "{\"a\":{\"nested\":-1},\"b\":[2,3.500000,true,null,\"x\\ny\"],\"s\":\"q\\\"\\\\\u{e9}\",\"z\":0.000000}"
    );
}

#[test]
fn pretty_output_wraps_only_the_outermost_levels() {
    // Expected bytes produced by the reference's own `JsonOut` (see
    // tools/reference_probe.cpp, writer mode). With `wrap = false` the
    // separator after a member breaks the line only while the indentation is
    // less than two levels deep, and pads with a space below that
    // (src/json.cpp:2256-2314).
    let mut writer = Writer::new(true);
    writer.write_value(&sample());
    assert_eq!(
        writer.into_string(),
        "{\n  \"a\": { \"nested\": -1 },\n  \"b\": [ 2, 3.500000, true, null, \"x\\ny\" ],\n  \"s\": \"q\\\"\\\\\u{e9}\",\n  \"z\": 0.000000\n}"
    );
}

#[test]
fn escapes_and_numbers_follow_the_references_stream() {
    // Expected bytes produced by the reference's own `JsonOut` through
    // tools/reference_probe.cpp's writer mode. Control bytes other than the
    // five with short escapes become `\u00XY` with an upper-case low nibble,
    // `/` and DEL are not escaped, and floats never use exponent notation
    // (src/json.cpp:2363-2404, src/json.cpp:2226-2237).
    let mut writer = Writer::new(false);
    writer.start_array(false);
    writer.write_str("\u{1}\u{b}\u{1b}/");
    writer.write_str("a\u{7f}b");
    writer.write_float(f64::INFINITY);
    writer.write_float(f64::NEG_INFINITY);
    writer.write_float(0.1);
    writer.end_array();
    assert_eq!(
        writer.into_string(),
        "[\"\\u0001\\u000B\\u001B/\",\"a\u{7f}b\",inf,-inf,0.100000]"
    );

    // The two spellings C++ uses that a Rust `Display` would not produce.
    let mut writer = Writer::new(false);
    writer.write_float(f64::NAN);
    assert_eq!(writer.into_string(), "nan");
    let mut writer = Writer::new(false);
    writer.write_float(1e300);
    assert_eq!(
        writer.into_string(),
        "1000000000000000052504760255204420248704468581108159154915854115511802457988908195786371375080447864043704443832883878176942523235360430575644792184786706982848387200926575803737830233794788090059368953234970799945081119038967640880074652742780142494579258788820056842838115669472196386865459400540160.000000"
    );
}

#[test]
fn a_written_document_does_not_always_read_back_the_same() {
    // Two findings, both confirmed against the reference's own `JsonOut`
    // (json.cpp:2226-2237): floats are printed with six decimals, so values
    // with more precision are rounded on the way out, and the non-finite
    // spellings the stream produces are `inf`/`nan`, which the reader rejects.
    let value = match parse(b"[0.123456789]") {
        Ok(value) => value,
        Err(error) => return assert_eq!(error.text(), "the sample must parse"),
    };
    let mut writer = Writer::new(false);
    writer.write_value(&value);
    assert_eq!(writer.as_str(), "[0.123457]");
    assert_eq!(
        parse(b"[0.123457]"),
        Ok(Value::Array(vec![Value::Float(0.123_457)]))
    );

    let mut writer = Writer::new(false);
    writer.write_value(&Value::Array(vec![Value::Float(f64::INFINITY)]));
    assert_eq!(writer.as_str(), "[inf]");
    assert_eq!(
        parse(b"[inf]").err().map(|e| e.message),
        Some("cannot parse value starting with: inf".to_string())
    );
}
