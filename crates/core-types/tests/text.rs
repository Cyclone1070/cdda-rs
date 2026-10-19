//! The reference's own expectations, ported one cycle at a time.
//!
//! Every case is copied from `references/Cataclysm-DDA/tests/string_formatter_test.cpp`
//! and cites the reference line it comes from. The reference helpers map onto
//! this file as follows: `throwing_string_format` (reference line 17) is
//! [`string_format`], which returns the reference's throw as `Err`, and
//! `test_for_expected` (reference line 42) is [`format`].

// Tests fail by panicking; the workspace denies `panic`/`expect_used` to keep
// library code total, not to change how a failing test reports itself.
#![allow(clippy::expect_used, clippy::panic)]
// The numeric literals are copied from the reference's table as written there,
// so that each row can be checked against its serial number at a glance.
#![allow(clippy::unreadable_literal)]
// `3.14159265` is the reference's own test value (serials 61 and 63).
#![allow(clippy::approx_constant)]

use cdda_core_types::text::{
    Arg, FormatError, FormatErrorKind, string_format, string_format_or_error,
};
use std::slice;

/// `throwing_string_format` (reference line 17), with a readable failure.
#[track_caller]
fn format(format_string: &str, args: &[Arg<'_>]) -> String {
    match string_format(format_string, args) {
        Ok(text) => text,
        Err(error) => panic!("{format_string:?} failed: {error}"),
    }
}

#[test]
fn literal_text_and_escaped_percent_are_copied_verbatim() {
    // `%%` is the only escape the parser knows (reference line 352).
    assert_eq!(format("", &[]), "");
    assert_eq!(format("no conversions here", &[]), "no conversions here");
    assert_eq!(format("%%", &[]), "%");
    assert_eq!(format("50%% off", &[]), "50% off");
    assert_eq!(format("%%%%", &[]), "%%");
    // The parser loops `while( const char c = consume_next_input() )`, so a null
    // character in the format ends the output there, as it does in the
    // reference (`src/string_formatter.h:347`, `src/string_formatter.cpp:8-11`).
    assert_eq!(format("a\0b", &[]), "a");
}

#[test]
fn integer_conversions_print_signed_decimals() {
    // Arguments beyond the last conversion are ignored (reference line 168).
    assert_eq!(
        format("", &[Arg::Str("whatever"), Arg::Int(5), Arg::Float(0.4)]),
        ""
    );
    // reference line 169
    assert_eq!(
        format(
            "%d %d %d %d %d",
            &[
                Arg::Int(1),
                Arg::Int(2),
                Arg::Int(3),
                Arg::Int(4),
                Arg::Int(5)
            ]
        ),
        "1 2 3 4 5"
    );
    // reference lines 270, 271, 275, 365-368
    assert_eq!(format("+%s+", &[Arg::Str("hello")]), "+hello+");
    assert_eq!(format("+%d+", &[Arg::Int(10)]), "+10+");
    assert_eq!(format("%d", &[Arg::Int(1024)]), "1024");
    assert_eq!(format("%d", &[Arg::Int(-1024)]), "-1024");
    assert_eq!(format("%i", &[Arg::Int(1024)]), "1024");
    assert_eq!(format("%i", &[Arg::Int(-1024)]), "-1024");
}

#[test]
fn sign_flags_and_field_width_apply_to_signed_decimals() {
    // (format, argument, expected) rows copied from the reference's table; the
    // comment gives the reference serial number of each row.
    let rows: &[(&str, i64, &str)] = &[
        ("% d", 42, " 42"),                       // serial 4
        ("% d", -42, "-42"),                      // serial 5
        ("% 5d", 42, "   42"),                    // serial 6
        ("% 5d", -42, "  -42"),                   // serial 7
        ("% 15d", 42, "             42"),         // serial 8
        ("% 15d", -42, "            -42"),        // serial 9
        ("%+d", 42, "+42"),                       // serial 10
        ("%+d", -42, "-42"),                      // serial 11
        ("%+5d", 42, "  +42"),                    // serial 12
        ("%+5d", -42, "  -42"),                   // serial 13
        ("%+15d", 42, "            +42"),         // serial 14
        ("%+15d", -42, "            -42"),        // serial 15
        ("% d", 1, " 1"),                         // serial 125
        ("%+ d", 1, "+1"),                        // serial 126
        ("%3d", 1234, "1234"),                    // serial 149
        ("%-1d", 2, "2"),                         // serial 152
        ("% d", 1024, " 1024"),                   // serial 212
        ("% d", -1024, "-1024"),                  // serial 213
        ("% i", 1024, " 1024"),                   // serial 214
        ("% i", -1024, "-1024"),                  // serial 215
        ("%1d", 1024, "1024"),                    // serial 249
        ("%1d", -1024, "-1024"),                  // serial 250
        ("%20d", 1024, "                1024"),   // serial 263
        ("%20d", -1024, "               -1024"),  // serial 264
        ("%-20d", 1024, "1024                "),  // serial 277
        ("%-20d", -1024, "-1024               "), // serial 278
    ];
    for &(format_string, value, expected) in rows {
        assert_eq!(
            format(format_string, &[Arg::Int(value)]),
            expected,
            "format {format_string:?} of {value}"
        );
    }
}

#[test]
fn zero_padding_and_precision_pad_the_digits_of_signed_decimals() {
    let rows: &[(&str, i64, &str)] = &[
        ("%10.5d", 4, "     00004"),                                // serial 3
        ("%0d", 42, "42"),                                          // serial 16
        ("%0d", -42, "-42"),                                        // serial 17
        ("%05d", 42, "00042"),                                      // serial 18
        ("%05d", -42, "-0042"),                                     // serial 19
        ("%015d", 42, "000000000000042"),                           // serial 20
        ("%015d", -42, "-00000000000042"),                          // serial 21
        ("%-d", 42, "42"),                                          // serial 22
        ("%-5d", 42, "42   "),                                      // serial 24
        ("%-5d", -42, "-42  "),                                     // serial 25
        ("%-15d", 42, "42             "),                           // serial 26
        ("%-15d", -42, "-42            "),                          // serial 27
        ("%-0d", 42, "42"),                                         // serial 28
        ("%-05d", 42, "42   "),                                     // serial 30
        ("%-05d", -42, "-42  "),                                    // serial 31
        ("%-015d", 42, "42             "),                          // serial 32
        ("%-015d", -42, "-42            "),                         // serial 33
        ("%0-d", 42, "42"),                                         // serial 34
        ("%0-5d", 42, "42   "),                                     // serial 36
        ("%0-5d", -42, "-42  "),                                    // serial 37
        ("%0-15d", 42, "42             "),                          // serial 38
        ("%0-15d", -42, "-42            "),                         // serial 39
        ("%+#25.22d", -1, "  -0000000000000000000001"),             // serial 113
        ("%.40d", 1, "0000000000000000000000000000000000000001"),   // serial 117
        ("% .40d", 1, " 0000000000000000000000000000000000000001"), // serial 119
        ("%.0d", 0, ""),                                            // serial 104
        ("%.20d", 1024, "00000000000000001024"),                    // serial 343
        ("%.20d", -1024, "-00000000000000001024"),                  // serial 344
        ("%20.5d", 1024, "               01024"),                   // serial 356
        ("%20.5d", -1024, "              -01024"),                  // serial 357
        ("%020.5d", 1024, "               01024"),                  // serial 369
        ("%020.5d", -1024, "              -01024"),                 // serial 370
        ("%20.0d", 1024, "                1024"),                   // serial 385
        ("%20.d", -1024, "               -1024"),                   // serial 386
        ("%20.d", 0, "                    "),                       // serial 387
    ];
    for &(format_string, value, expected) in rows {
        assert_eq!(
            format(format_string, &[Arg::Int(value)]),
            expected,
            "format {format_string:?} of {value}"
        );
    }
}

#[test]
fn unsigned_conversions_print_their_base_with_the_alternate_form() {
    let rows: &[(&str, i64, &str)] = &[
        ("%u", 4294966272, "4294966272"),                // serial 188
        ("%o", 511, "777"),                              // serial 189
        ("%o", 4294966785, "37777777001"),               // serial 190
        ("%x", 305441741, "1234abcd"),                   // serial 191
        ("%x", 3989525555, "edcb5433"),                  // serial 192
        ("%X", 305441741, "1234ABCD"),                   // serial 193
        ("%X", 3989525555, "EDCB5433"),                  // serial 194
        ("%o", 10, "12"),                                // serial 165
        ("%#o", 511, "0777"),                            // serial 239
        ("%#o", 4294966785, "037777777001"),             // serial 240
        ("%#x", 305441741, "0x1234abcd"),                // serial 241
        ("%#x", 3989525555, "0xedcb5433"),               // serial 242
        ("%#X", 305441741, "0X1234ABCD"),                // serial 243
        ("%#X", 3989525555, "0XEDCB5433"),               // serial 244
        ("%#o", 0, "0"),                                 // serial 245
        ("%#x", 0, "0"),                                 // serial 246
        ("%#X", 0, "0"),                                 // serial 247
        ("%#012x", 1, "0x0000000001"),                   // serial 129
        ("%#04.8x", 1, "0x00000001"),                    // serial 130
        ("%#-08.2x", 1, "0x01    "),                     // serial 131
        ("%#08o", 1, "00000001"),                        // serial 132
        ("%+o", 511, "777"),                             // serial 204
        ("%+o", 4294966785, "37777777001"),              // serial 205
        ("%+x", 305441741, "1234abcd"),                  // serial 206
        ("%+X", 3989525555, "EDCB5433"),                 // serial 209
        ("% o", 511, "777"),                             // serial 218
        ("% o", 4294966785, "37777777001"),              // serial 219
        ("% x", 305441741, "1234abcd"),                  // serial 220
        ("% X", 3989525555, "EDCB5433"),                 // serial 223
        ("%+ o", 511, "777"),                            // serial 232
        ("%+ x", 305441741, "1234abcd"),                 // serial 234
        ("%+ X", 3989525555, "EDCB5433"),                // serial 237
        ("%#.25o", -100, "0001777777777777777777634"),   // serial 106
        ("%#+24.20o", -100, " 01777777777777777777634"), // serial 107
        ("%#+21.18x", -100, " 0x00ffffffffffffff9c"),    // serial 105
        ("%#+18.21X", -100, "0X00000FFFFFFFFFFFFFF9C"),  // serial 108
        ("%#+20.24o", -100, "001777777777777777777634"), // serial 109
        ("%#-8.5o", 100, "00144   "),                    // serial 114
        ("%#-+ 08.5d", 100, "+00100  "),                 // serial 115
        ("%#-+ 08.5d", 100, "+00100  "),                 // serial 116
        ("%#20o", 511, "                0777"),          // serial 302
        ("%#20o", 4294966785, "        037777777001"),   // serial 303
        ("%#20x", 305441741, "          0x1234abcd"),    // serial 304
        ("%#20X", 3989525555, "          0XEDCB5433"),   // serial 307
        ("%#020o", 511, "00000000000000000777"),         // serial 308
        ("%#020x", 305441741, "0x00000000001234abcd"),   // serial 310
        ("%#020X", 3989525555, "0X0000000000EDCB5433"),  // serial 313
        ("%020u", 4294966272, "00000000004294966272"),   // serial 295
        ("%020o", 4294966785, "00000000037777777001"),   // serial 297
        ("%.20u", 4294966272, "00000000004294966272"),   // serial 348
        ("%20.5u", 4294966272, "          4294966272"),  // serial 361
        ("%20.10x", 3989525555, "          00edcb5433"), // serial 365
        ("%20.u", 0, "                    "),            // serial 393
        ("%20.0u", 0, "                    "),           // serial 394
        ("%20.x", 0, "                    "),            // serial 399
        ("%20.X", 0, "                    "),            // serial 402
    ];
    for &(format_string, value, expected) in rows {
        assert_eq!(
            format(format_string, &[Arg::Int(value)]),
            expected,
            "format {format_string:?} of {value}"
        );
    }
}

#[test]
fn character_conversions_print_exactly_one_character() {
    let rows: &[(&str, Arg<'_>, &str)] = &[
        ("%d", Arg::Char('5'), "53"),                       // reference line 172
        ("%c", Arg::Char('5'), "5"),                        // reference line 173
        ("%c", Arg::Int(53), "5"),                          // reference line 176
        ("%c", Arg::Char('a'), "a"),                        // serial 68
        ("%c", Arg::Int(32), " "),                          // serial 69
        ("%c", Arg::Int(36), "$"),                          // serial 70
        ("%c", Arg::Char('x'), "x"),                        // serial 195
        ("%+c", Arg::Char('x'), "x"),                       // serial 210
        ("% c", Arg::Char('x'), "x"),                       // serial 224
        ("%+ c", Arg::Char('x'), "x"),                      // serial 238
        ("%3c", Arg::Char('a'), "  a"),                     // serial 148
        ("%1c", Arg::Char('x'), "x"),                       // serial 261
        ("%20c", Arg::Char('x'), "                   x"),   // serial 275
        ("%-20c", Arg::Char('x'), "x                   "),  // serial 289
        ("%-020c", Arg::Char('x'), "x                   "), // serial 327
    ];
    for (format_string, argument, expected) in rows {
        assert_eq!(
            format(format_string, slice::from_ref(argument)),
            *expected,
            "format {format_string:?} of {argument:?}"
        );
    }
}

#[test]
fn string_conversions_apply_width_and_precision() {
    // Everything a non-string argument becomes when printed through `%s`
    // (`src/string_formatter.h:139-152`), and the reference's own rows.
    assert_eq!(format("%s", &[Arg::Char('5')]), "5"); // reference line 174
    assert_eq!(format("%s", &[Arg::Int(5)]), "5"); // reference line 179
    assert_eq!(format("%s", &[Arg::Str("55")]), "55"); // reference line 178
    assert_eq!(format("%s", &[Arg::Str("%%%%")]), "%%%%"); // serial 169
    assert_eq!(format("%s", &[Arg::Str("foo")]), "foo"); // reference line 181
    // Strings are cut, never expanded (serials 142, 381-384, 196-199).
    assert_eq!(format("%.1s", &[Arg::Str("foo")]), "f"); // serial 142
    assert_eq!(format("%.3s", &[Arg::Str("foobar")]), "foo"); // serial 2
    assert_eq!(format("%.1s", &[Arg::Str("ab")]), "a"); // reference line 198
    assert_eq!(format("%.3s", &[Arg::Str("ab")]), "ab"); // reference line 199
    assert_eq!(format("%.0s", &[Arg::Str("Hallo heimur")]), ""); // serial 381
    assert_eq!(format("%.s", &[Arg::Str("Hallo heimur")]), ""); // serial 383
    // Field width pads with spaces, and `-` pads on the right (serials 248-276).
    assert_eq!(format("%1s", &[Arg::Str("Hallo heimur")]), "Hallo heimur"); // serial 248
    assert_eq!(format("%20s", &[Arg::Str("Hallo")]), "               Hallo"); // serial 262
    assert_eq!(
        format("%-20s", &[Arg::Str("Hallo")]),
        "Hallo               "
    ); // serial 276
    assert_eq!(
        format("%0-20s", &[Arg::Str("Hallo")]),
        "Hallo               "
    ); // serial 314
    assert_eq!(format("%.20s", &[Arg::Str("Hallo heimur")]), "Hallo heimur"); // serial 342
    assert_eq!(
        format("%20.5s", &[Arg::Str("Hallo heimur")]),
        "               Hallo"
    ); // serial 355
    assert_eq!(
        format("%20.0s", &[Arg::Str("Hallo heimur")]),
        "                    "
    ); // serial 382
    assert_eq!(
        format("%20.s", &[Arg::Str("Hallo heimur")]),
        "                    "
    ); // serial 384
    // Flags other than `-` do nothing to a string (serials 197, 211, 225).
    assert_eq!(format("%+s", &[Arg::Str("Hallo heimur")]), "Hallo heimur"); // serial 197
    assert_eq!(format("% s", &[Arg::Str("Hallo heimur")]), "Hallo heimur"); // serial 211
    assert_eq!(format("%+ s", &[Arg::Str("Hallo heimur")]), "Hallo heimur"); // serial 225
}

#[test]
fn fixed_point_conversions_print_six_digits_unless_told_otherwise() {
    let rows: &[(&str, f64, &str)] = &[
        ("%.2f", 42.8952, "42.90"),           // serial 43
        ("%.2F", 42.8952, "42.90"),           // serial 44
        ("%.10f", 42.8952, "42.8952000000"),  // serial 45
        ("%1.2f", 42.8952, "42.90"),          // serial 46
        ("%6.2f", 42.8952, " 42.90"),         // serial 47
        ("%+6.2f", 42.8952, "+42.90"),        // serial 49
        ("%5.10f", 42.8952, "42.8952000000"), // serial 50
        ("%2.4f", 8.6, "8.6000"),             // serial 153
        ("%0f", 0.6, "0.600000"),             // serial 154
        ("%.0f", 0.6, "1"),                   // serial 155
        ("%010.2f", -3.5, "-000003.50"),      // checked against printf
        ("% 10.2f", 3.25, "      3.25"),      // checked against printf
        ("%.0f", 2.5, "2"),                   // printf rounds halves to even
        ("%f", 255.5, "255.500000"),
    ];
    for &(format_string, value, expected) in rows {
        assert_eq!(
            format(format_string, &[Arg::Float(value)]),
            expected,
            "format {format_string:?} of {value}"
        );
    }
    // A `float` argument to `%f` prints its double value, six digits by default
    // (`test_typed_printf<float>`, reference line 153).
    assert_eq!(
        format("%f", &[Arg::Float(f64::from(f32::MIN))]),
        "-340282346638528859811704183484516925440.000000"
    );
    // `%s` of a number is `std::to_string`, i.e. the same six digits
    // (`src/string_formatter.h:139-146`, reference line 168 passes 0.4).
    assert_eq!(format("", &[Arg::Float(0.4)]), "");
    assert_eq!(format("%s", &[Arg::Float(0.4)]), "0.400000");
}

#[test]
fn exponent_and_general_conversions_follow_printfs_rules() {
    // Expected strings checked against glibc's printf, since the reference's
    // table only covers `%g` (serial 161).
    let rows: &[(&str, f64, &str)] = &[
        ("%.4e", 1234.5678, "1.2346e+03"),
        ("%E", 0.000123456, "1.234560E-04"),
        ("%e", 0.0, "0.000000e+00"),
        ("%.0e", 123456789.0, "1e+08"),
        ("%2.4g", 8.6, "8.6"), // serial 161
        ("%.4g", 8.6, "8.6"),
        ("%g", 8.6, "8.6"),
        ("%.10g", 0.0001, "0.0001"),
        ("%g", 123456789.0, "1.23457e+08"),
        ("%.0g", 123456789.0, "1e+08"),
        ("%#.4g", 8.6, "8.600"),
        ("%.3g", 0.0001234, "0.000123"),
        ("%10.3g", 1234567.0, "  1.23e+06"),
        ("%.17g", 0.1, "0.10000000000000001"),
        ("%.2g", 9.999, "10"),
        ("%G", 123456789.0, "1.23457E+08"),
        ("%-12.4e", 1234.5678, "1.2346e+03  "),
        ("%+e", 1.5, "+1.500000e+00"),
    ];
    for &(format_string, value, expected) in rows {
        assert_eq!(
            format(format_string, &[Arg::Float(value)]),
            expected,
            "format {format_string:?} of {value}"
        );
    }
}

#[test]
fn hexadecimal_float_conversions_print_the_exact_bits() {
    // `%a` is in the reference's conversion list (src 305-314) but appears
    // nowhere in its table, so these rows were checked against glibc's printf.
    let rows: &[(&str, f64, &str)] = &[
        ("%.2a", 1.5, "0x1.80p+0"), // serial 28
        ("%a", 1.0, "0x1p+0"),      // serial 29
        ("%a", 0.0, "0x0p+0"),      // serial 30
        ("%A", 255.5, "0X1.FFP+7"), // serial 31
        ("%.0a", 1.0, "0x1p+0"),    // serial 32
        ("%.13a", 1.5, "0x1.8000000000000p+0"),
        ("%.15a", 1.5, "0x1.800000000000000p+0"),
        ("%a", 0.1, "0x1.999999999999ap-4"),
        ("%a", 1e-310, "0x1.2688b70e62bp-1030"),
        ("%a", -0.0, "-0x0p+0"),
        ("%#a", 1.0, "0x1.p+0"),
        ("%a", 2.0, "0x1p+1"),
        ("%a", f64::MIN_POSITIVE, "0x1p-1022"),
        ("%.2a", 0.1, "0x1.9ap-4"),
        ("%A", 0.1, "0X1.999999999999AP-4"),
        ("%+a", 1.0, "+0x1p+0"),
        ("%.2a", 255.5, "0x1.ffp+7"),
        ("%a", 1234567890123456789.0, "0x1.12210f47de981p+60"),
        ("%.2a", 0.0, "0x0.00p+0"),
        ("%#a", 0.0, "0x0.p+0"),
        ("%.0a", 0.0, "0x0p+0"),
        ("%.1a", 1.9375, "0x1.fp+0"),
        // An exact half is dropped, not rounded up, as glibc does it.
        ("%.1a", 1.96875, "0x1.fp+0"),
        ("%.1a", 1.99, "0x2.0p+0"),
        ("%.1a", 1.906, "0x1.ep+0"),
        ("%a", 0.5, "0x1p-1"),
    ];
    for &(format_string, value, expected) in rows {
        assert_eq!(
            format(format_string, &[Arg::Float(value)]),
            expected,
            "format {format_string:?} of {value:e}"
        );
    }
}

#[test]
fn pointer_conversion_prints_the_address_in_lowercase_hex() {
    assert_eq!(format("%p", &[Arg::Pointer(0)]), "0x0"); // checked against printf
    assert_eq!(format("%p", &[Arg::Pointer(0x1234)]), "0x1234");
    assert_eq!(
        format("%20p", &[Arg::Pointer(0x1234)]),
        "              0x1234"
    );
    assert_eq!(
        format("%-20p", &[Arg::Pointer(0x1234)]),
        "0x1234              "
    );
    // glibc zero-pads `%p` after the `0x`, though the standard calls it undefined.
    assert_eq!(
        format("%020p", &[Arg::Pointer(0x1234)]),
        "0x000000000000001234"
    );
}

#[test]
fn star_takes_the_width_and_precision_from_arguments() {
    let rows: &[(&str, &[Arg<'_>], &str)] = &[
        // The star consumes the next argument, before the one being formatted
        // (serials 208, 60, 143, 144, 61, 63, 211, 328-341, 403-415).
        ("%.*f", &[Arg::Int(2), Arg::Float(0.33333333)], "0.33"), // serial 208
        ("%*s", &[Arg::Int(4), Arg::Str("foo")], " foo"),         // serial 60
        ("%*sx\\n", &[Arg::Int(-3), Arg::Str("hi")], "hi x\\n"),  // serial 416
        (
            "%*.*f",
            &[Arg::Int(10), Arg::Int(2), Arg::Float(3.14159265)],
            "      3.14",
        ), // serial 61
        (
            "%-*.*f",
            &[Arg::Int(10), Arg::Int(2), Arg::Float(3.14159265)],
            "3.14      ",
        ), // serial 63
        ("%.*s", &[Arg::Int(1), Arg::Str("foo")], "f"),           // serial 143
        ("%*s", &[Arg::Int(-5), Arg::Str("foo")], "foo  "),       // serial 144
        (
            "%*s",
            &[Arg::Int(20), Arg::Str("Hallo")],
            "               Hallo",
        ), // serial 328
        (
            "%*d",
            &[Arg::Int(20), Arg::Int(1024)],
            "                1024",
        ), // serial 329
        (
            "%*d",
            &[Arg::Int(20), Arg::Int(-1024)],
            "               -1024",
        ), // serial 330
        (
            "%*c",
            &[Arg::Int(20), Arg::Char('x')],
            "                   x",
        ), // serial 341
        (
            "% -0+*.*s",
            &[Arg::Int(20), Arg::Int(5), Arg::Str("Hallo heimur")],
            "Hallo               ",
        ), // serial 403
        (
            "% -0+*.*d",
            &[Arg::Int(20), Arg::Int(5), Arg::Int(1024)],
            "+01024              ",
        ), // serial 404
        (
            "% -0+*.*d",
            &[Arg::Int(20), Arg::Int(5), Arg::Int(-1024)],
            "-01024              ",
        ), // serial 405
        (
            "% 0-+*.*i",
            &[Arg::Int(20), Arg::Int(5), Arg::Int(-1024)],
            "-01024              ",
        ), // serial 407
        (
            "%+ -0*.*o",
            &[Arg::Int(20), Arg::Int(5), Arg::Int(511)],
            "00777               ",
        ), // serial 410
        (
            "%+ -0*.*x",
            &[Arg::Int(20), Arg::Int(10), Arg::Int(3989525555)],
            "00edcb5433          ",
        ), // serial 413
        (
            "%-*.*f",
            &[Arg::Int(4), Arg::Int(7), Arg::Float(100.44)],
            "100.4400000",
        ), // reference line 157
    ];
    for (format_string, args, expected) in rows {
        assert_eq!(
            format(format_string, args),
            *expected,
            "format {format_string:?}"
        );
    }
}

#[test]
fn positional_conversions_name_their_argument() {
    // `%N$` picks the argument by number and does not advance the sequence
    // (serials 55, 56, and the `%6$-*5$.*4$f` pattern of reference line 165).
    assert_eq!(
        format("%1$s %2$s", &[Arg::Str("Hot"), Arg::Str("Pocket")]),
        "Hot Pocket" // serial 55
    );
    assert_eq!(
        format(
            "%1$.1f %2$s %3$ss",
            &[Arg::Float(12.0), Arg::Str("Hot"), Arg::Str("Pocket")]
        ),
        "12.0 Hot Pockets" // serial 56
    );
    // The width and precision come from named arguments too.
    assert_eq!(
        format(
            "%6$-*5$.*4$f",
            &[
                Arg::Str(""),
                Arg::Str(""),
                Arg::Str(""),
                Arg::Int(7),
                Arg::Int(4),
                Arg::Float(100.44)
            ]
        ),
        "100.4400000" // reference line 165, checked against printf: the
                      // width comes from `*5$` (the fourth argument) and the
                      // precision from `.*4$` (the fifth, which is 7)
    );
    // Without a `$` after a `*`, a digit is a plain width (`%2$s` style naming is
    // the only thing `$` is read for).
    assert_eq!(format("%2s", &[Arg::Str("x")]), " x");
    // A `%` followed by `0` is not a positional index (`read_argument_index`
    // only starts at '1').
    assert_eq!(format("%0d", &[Arg::Int(7)]), "7");
    // The lookahead that finds the `$` searches for a character outside
    // "012345678", a set the reference forgot to put '9' in
    // (`src/string_formatter.cpp:47`). An index whose second digit is '9' is
    // therefore not read as positional: `%19$s` is width 19 and then a `$`
    // conversion, which is an error — the same error the reference throws.
    assert_eq!(
        error_of("%19$s", &[Arg::Str("a")]).kind(),
        &FormatErrorKind::UnsupportedConversion('$')
    );
    // A positional index past the end is the usual missing-argument error.
    assert_eq!(
        error_of("%2$s", &[Arg::Str("a")]).to_string(),
        "Requested argument 1 but input has only 1 at: \"%2$s|\""
    );
}

/// `test_for_error` (reference line 52): the call must fail, and the failure is
/// returned rather than thrown.
#[track_caller]
fn error_of(format_string: &str, args: &[Arg<'_>]) -> FormatError {
    match string_format(format_string, args) {
        Ok(text) => panic!("{format_string:?} returned {text:?}, expected an error"),
        Err(error) => error,
    }
}

#[test]
fn malformed_formats_and_mismatched_arguments_are_errors() {
    // `string_formatter_errors` (reference lines 596-610).
    // `%6$-*5$.*4$f` needs six arguments and is given three, four and five.
    for args in [
        &[Arg::Int(1), Arg::Int(2), Arg::Int(3)][..],
        &[Arg::Int(1), Arg::Int(2), Arg::Int(3), Arg::Int(4)][..],
        &[
            Arg::Int(1),
            Arg::Int(2),
            Arg::Int(3),
            Arg::Int(4),
            Arg::Int(5),
        ][..],
    ] {
        assert!(string_format("%6$-*5$.*4$f", args).is_err());
    }
    // An invalid format specifier (reference line 603).
    let unsupported = error_of("%k", &[]);
    assert_eq!(
        unsupported.kind(),
        &FormatErrorKind::UnsupportedConversion('k')
    );
    assert_eq!(
        unsupported.to_string(),
        "Unsupported format conversion: k at: \"%k|\""
    );
    // A pointer cannot be printed as a string or as an integer (lines 605-606).
    assert!(string_format("%s", &[Arg::Pointer(0)]).is_err());
    assert!(string_format("%d", &[Arg::Pointer(0)]).is_err());
    // Nor can a string be printed as an integer (line 607).
    let mismatch = error_of("%d", &[Arg::Str("some string")]);
    assert_eq!(
        mismatch.to_string(),
        "Tried to convert argument of type string to signed long long int, \
         which is not possible at: \"%d|\""
    );
    // Too few arguments for the conversions given (line 609).
    let missing = error_of(
        "%d %d %d %d %d",
        &[Arg::Int(1), Arg::Int(2), Arg::Int(3), Arg::Int(4)],
    );
    assert_eq!(
        missing.to_string(),
        "Requested argument 4 but input has only 4 at: \"%d %d %d %d %d|\""
    );
    // `%f` of an integer is a *type* error in the reference, not a conversion:
    // `convert<double>` only accepts floating point (reference lines 95-101).
    assert!(string_format("%f", &[Arg::Int(7)]).is_err());
    // A `bool` argument is a compile error in the reference (lines 54-58 and
    // 161-164), which Rust cannot express, so it is a returned error here.
    for format_string in ["%d", "%s", "%c", "%f", "%p"] {
        assert!(
            string_format(format_string, &[Arg::Bool(true)]).is_err(),
            "{format_string} accepted a bool"
        );
    }
    // A `*` with a positional width needs its `$`
    // (`expected '$' after field precision`, reference line 80).
    let missing_dollar = error_of("%.*5f", &[Arg::Int(1), Arg::Float(1.0)]);
    assert_eq!(missing_dollar.kind(), &FormatErrorKind::ExpectedDollar);
    // A format that ends in a lone `%` reads the end of the string as a null
    // character and reports it as the unsupported conversion (reference lines
    // 8-11 and 321-322).
    let lone_percent = error_of("%", &[]);
    assert_eq!(
        lone_percent.kind(),
        &FormatErrorKind::UnsupportedConversion('\0')
    );
    assert_eq!(lone_percent.position(), 1);
}

#[test]
fn the_swallowing_entry_point_returns_the_error_text() {
    // `string_format` in the reference catches the exception and returns its
    // text as the result (reference lines 426-436, `handle_string_format_error`).
    assert_eq!(string_format_or_error("%d", &[Arg::Int(7)]), "7");
    assert_eq!(
        string_format_or_error("%d", &[Arg::Str("no")]),
        "Tried to convert argument of type string to signed long long int, \
         which is not possible at: \"%d|\""
    );
}

/// Every active `importet_test` row of the reference's table, transcribed
/// mechanically from `tests/string_formatter_test.cpp` — serial number, format,
/// arguments, expected text — so that none of the reference's checks is dropped.
static REFERENCE_TABLE: &[(u32, &str, &[Arg<'static>], &str)] = &[
    (1, "%.*f", &[Arg::Int(2), Arg::Float(0.33333333)], "0.33"),
    (2, "%.3s", &[Arg::Str("foobar")], "foo"),
    (3, "%10.5d", &[Arg::Int(4)], "     00004"),
    (416, "%*sx\\n", &[Arg::Int(-3), Arg::Str("hi")], "hi x\\n"),
    (4, "% d", &[Arg::Int(42)], " 42"),
    (5, "% d", &[Arg::Int(-42)], "-42"),
    (6, "% 5d", &[Arg::Int(42)], "   42"),
    (7, "% 5d", &[Arg::Int(-42)], "  -42"),
    (8, "% 15d", &[Arg::Int(42)], "             42"),
    (9, "% 15d", &[Arg::Int(-42)], "            -42"),
    (10, "%+d", &[Arg::Int(42)], "+42"),
    (11, "%+d", &[Arg::Int(-42)], "-42"),
    (12, "%+5d", &[Arg::Int(42)], "  +42"),
    (13, "%+5d", &[Arg::Int(-42)], "  -42"),
    (14, "%+15d", &[Arg::Int(42)], "            +42"),
    (15, "%+15d", &[Arg::Int(-42)], "            -42"),
    (16, "%0d", &[Arg::Int(42)], "42"),
    (17, "%0d", &[Arg::Int(-42)], "-42"),
    (18, "%05d", &[Arg::Int(42)], "00042"),
    (19, "%05d", &[Arg::Int(-42)], "-0042"),
    (20, "%015d", &[Arg::Int(42)], "000000000000042"),
    (21, "%015d", &[Arg::Int(-42)], "-00000000000042"),
    (22, "%-d", &[Arg::Int(42)], "42"),
    (23, "%-d", &[Arg::Int(-42)], "-42"),
    (24, "%-5d", &[Arg::Int(42)], "42   "),
    (25, "%-5d", &[Arg::Int(-42)], "-42  "),
    (26, "%-15d", &[Arg::Int(42)], "42             "),
    (27, "%-15d", &[Arg::Int(-42)], "-42            "),
    (28, "%-0d", &[Arg::Int(42)], "42"),
    (29, "%-0d", &[Arg::Int(-42)], "-42"),
    (30, "%-05d", &[Arg::Int(42)], "42   "),
    (31, "%-05d", &[Arg::Int(-42)], "-42  "),
    (32, "%-015d", &[Arg::Int(42)], "42             "),
    (33, "%-015d", &[Arg::Int(-42)], "-42            "),
    (34, "%0-d", &[Arg::Int(42)], "42"),
    (35, "%0-d", &[Arg::Int(-42)], "-42"),
    (36, "%0-5d", &[Arg::Int(42)], "42   "),
    (37, "%0-5d", &[Arg::Int(-42)], "-42  "),
    (38, "%0-15d", &[Arg::Int(42)], "42             "),
    (39, "%0-15d", &[Arg::Int(-42)], "-42            "),
    (43, "%.2f", &[Arg::Float(42.8952)], "42.90"),
    (44, "%.2F", &[Arg::Float(42.8952)], "42.90"),
    (45, "%.10f", &[Arg::Float(42.8952)], "42.8952000000"),
    (46, "%1.2f", &[Arg::Float(42.8952)], "42.90"),
    (47, "%6.2f", &[Arg::Float(42.8952)], " 42.90"),
    (49, "%+6.2f", &[Arg::Float(42.8952)], "+42.90"),
    (50, "%5.10f", &[Arg::Float(42.8952)], "42.8952000000"),
    (
        55,
        "%1$s %2$s",
        &[Arg::Str("Hot"), Arg::Str("Pocket")],
        "Hot Pocket",
    ),
    (
        56,
        "%1$.1f %2$s %3$ss",
        &[Arg::Float(12.0), Arg::Str("Hot"), Arg::Str("Pocket")],
        "12.0 Hot Pockets",
    ),
    (60, "%*s", &[Arg::Int(4), Arg::Str("foo")], " foo"),
    (
        61,
        "%*.*f",
        &[Arg::Int(10), Arg::Int(2), Arg::Float(3.14159265)],
        "      3.14",
    ),
    (
        63,
        "%-*.*f",
        &[Arg::Int(10), Arg::Int(2), Arg::Float(3.14159265)],
        "3.14      ",
    ),
    (66, "+%s+", &[Arg::Str("hello")], "+hello+"),
    (67, "+%d+", &[Arg::Int(10)], "+10+"),
    (68, "%c", &[Arg::Char('a')], "a"),
    (69, "%c", &[Arg::Int(32)], " "),
    (70, "%c", &[Arg::Int(36)], "$"),
    (71, "%d", &[Arg::Int(10)], "10"),
    (81, "%+8lld", &[Arg::Int(100)], "    +100"),
    (82, "%+.8lld", &[Arg::Int(100)], "+00000100"),
    (83, "%+10.8lld", &[Arg::Int(100)], " +00000100"),
    (85, "%-1.5lld", &[Arg::Int(-100)], "-00100"),
    (86, "%5lld", &[Arg::Int(100)], "  100"),
    (87, "%5lld", &[Arg::Int(-100)], " -100"),
    (88, "%-5lld", &[Arg::Int(100)], "100  "),
    (89, "%-5lld", &[Arg::Int(-100)], "-100 "),
    (90, "%-.5lld", &[Arg::Int(100)], "00100"),
    (91, "%-.5lld", &[Arg::Int(-100)], "-00100"),
    (92, "%-8.5lld", &[Arg::Int(100)], "00100   "),
    (93, "%-8.5lld", &[Arg::Int(-100)], "-00100  "),
    (94, "%05lld", &[Arg::Int(100)], "00100"),
    (95, "%05lld", &[Arg::Int(-100)], "-0100"),
    (96, "% lld", &[Arg::Int(100)], " 100"),
    (97, "% lld", &[Arg::Int(-100)], "-100"),
    (98, "% 5lld", &[Arg::Int(100)], "  100"),
    (99, "% 5lld", &[Arg::Int(-100)], " -100"),
    (100, "% .5lld", &[Arg::Int(100)], " 00100"),
    (101, "% .5lld", &[Arg::Int(-100)], "-00100"),
    (102, "% 8.5lld", &[Arg::Int(100)], "   00100"),
    (103, "% 8.5lld", &[Arg::Int(-100)], "  -00100"),
    (104, "%.0lld", &[Arg::Int(0)], ""),
    (
        105,
        "%#+21.18llx",
        &[Arg::Int(-100)],
        " 0x00ffffffffffffff9c",
    ),
    (
        106,
        "%#.25llo",
        &[Arg::Int(-100)],
        "0001777777777777777777634",
    ),
    (
        107,
        "%#+24.20llo",
        &[Arg::Int(-100)],
        " 01777777777777777777634",
    ),
    (
        108,
        "%#+18.21llX",
        &[Arg::Int(-100)],
        "0X00000FFFFFFFFFFFFFF9C",
    ),
    (
        109,
        "%#+20.24llo",
        &[Arg::Int(-100)],
        "001777777777777777777634",
    ),
    (
        113,
        "%+#25.22lld",
        &[Arg::Int(-1)],
        "  -0000000000000000000001",
    ),
    (114, "%#-8.5llo", &[Arg::Int(100)], "00144   "),
    (115, "%#-+ 08.5lld", &[Arg::Int(100)], "+00100  "),
    (116, "%#-+ 08.5lld", &[Arg::Int(100)], "+00100  "),
    (
        117,
        "%.40lld",
        &[Arg::Int(1)],
        "0000000000000000000000000000000000000001",
    ),
    (
        118,
        "% .40lld",
        &[Arg::Int(1)],
        " 0000000000000000000000000000000000000001",
    ),
    (
        119,
        "% .40d",
        &[Arg::Int(1)],
        " 0000000000000000000000000000000000000001",
    ),
    (125, "% d", &[Arg::Int(1)], " 1"),
    (126, "%+ d", &[Arg::Int(1)], "+1"),
    (129, "%#012x", &[Arg::Int(1)], "0x0000000001"),
    (130, "%#04.8x", &[Arg::Int(1)], "0x00000001"),
    (131, "%#-08.2x", &[Arg::Int(1)], "0x01    "),
    (132, "%#08o", &[Arg::Int(1)], "00000001"),
    (142, "%.1s", &[Arg::Str("foo")], "f"),
    (143, "%.*s", &[Arg::Int(1), Arg::Str("foo")], "f"),
    (144, "%*s", &[Arg::Int(-5), Arg::Str("foo")], "foo  "),
    (148, "%3c", &[Arg::Char('a')], "  a"),
    (149, "%3d", &[Arg::Int(1234)], "1234"),
    (152, "%-1d", &[Arg::Int(2)], "2"),
    (153, "%2.4f", &[Arg::Float(8.6)], "8.6000"),
    (154, "%0f", &[Arg::Float(0.6)], "0.600000"),
    (155, "%.0f", &[Arg::Float(0.6)], "1"),
    (161, "%2.4g", &[Arg::Float(8.6)], "8.6"),
    (162, "%-i", &[Arg::Int(-1)], "-1"),
    (163, "%-i", &[Arg::Int(1)], "1"),
    (164, "%+i", &[Arg::Int(1)], "+1"),
    (165, "%o", &[Arg::Int(10)], "12"),
    (169, "%s", &[Arg::Str("%%%%")], "%%%%"),
    (179, "%hhx", &[Arg::Char('a')], "61"),
    (182, "%s", &[Arg::Str("Hallo heimur")], "Hallo heimur"),
    (183, "%d", &[Arg::Int(1024)], "1024"),
    (184, "%d", &[Arg::Int(-1024)], "-1024"),
    (185, "%i", &[Arg::Int(1024)], "1024"),
    (186, "%i", &[Arg::Int(-1024)], "-1024"),
    (187, "%u", &[Arg::Int(1024)], "1024"),
    (188, "%u", &[Arg::Int(4294966272)], "4294966272"),
    (189, "%o", &[Arg::Int(511)], "777"),
    (190, "%o", &[Arg::Int(4294966785)], "37777777001"),
    (191, "%x", &[Arg::Int(305441741)], "1234abcd"),
    (192, "%x", &[Arg::Int(3989525555)], "edcb5433"),
    (193, "%X", &[Arg::Int(305441741)], "1234ABCD"),
    (194, "%X", &[Arg::Int(3989525555)], "EDCB5433"),
    (195, "%c", &[Arg::Char('x')], "x"),
    (197, "%+s", &[Arg::Str("Hallo heimur")], "Hallo heimur"),
    (198, "%+d", &[Arg::Int(1024)], "+1024"),
    (199, "%+d", &[Arg::Int(-1024)], "-1024"),
    (200, "%+i", &[Arg::Int(1024)], "+1024"),
    (201, "%+i", &[Arg::Int(-1024)], "-1024"),
    (204, "%+o", &[Arg::Int(511)], "777"),
    (205, "%+o", &[Arg::Int(4294966785)], "37777777001"),
    (206, "%+x", &[Arg::Int(305441741)], "1234abcd"),
    (207, "%+x", &[Arg::Int(3989525555)], "edcb5433"),
    (208, "%+X", &[Arg::Int(305441741)], "1234ABCD"),
    (209, "%+X", &[Arg::Int(3989525555)], "EDCB5433"),
    (210, "%+c", &[Arg::Char('x')], "x"),
    (211, "% s", &[Arg::Str("Hallo heimur")], "Hallo heimur"),
    (212, "% d", &[Arg::Int(1024)], " 1024"),
    (213, "% d", &[Arg::Int(-1024)], "-1024"),
    (214, "% i", &[Arg::Int(1024)], " 1024"),
    (215, "% i", &[Arg::Int(-1024)], "-1024"),
    (218, "% o", &[Arg::Int(511)], "777"),
    (219, "% o", &[Arg::Int(4294966785)], "37777777001"),
    (220, "% x", &[Arg::Int(305441741)], "1234abcd"),
    (221, "% x", &[Arg::Int(3989525555)], "edcb5433"),
    (222, "% X", &[Arg::Int(305441741)], "1234ABCD"),
    (223, "% X", &[Arg::Int(3989525555)], "EDCB5433"),
    (224, "% c", &[Arg::Char('x')], "x"),
    (225, "%+ s", &[Arg::Str("Hallo heimur")], "Hallo heimur"),
    (226, "%+ d", &[Arg::Int(1024)], "+1024"),
    (227, "%+ d", &[Arg::Int(-1024)], "-1024"),
    (228, "%+ i", &[Arg::Int(1024)], "+1024"),
    (229, "%+ i", &[Arg::Int(-1024)], "-1024"),
    (232, "%+ o", &[Arg::Int(511)], "777"),
    (233, "%+ o", &[Arg::Int(4294966785)], "37777777001"),
    (234, "%+ x", &[Arg::Int(305441741)], "1234abcd"),
    (235, "%+ x", &[Arg::Int(3989525555)], "edcb5433"),
    (236, "%+ X", &[Arg::Int(305441741)], "1234ABCD"),
    (237, "%+ X", &[Arg::Int(3989525555)], "EDCB5433"),
    (238, "%+ c", &[Arg::Char('x')], "x"),
    (239, "%#o", &[Arg::Int(511)], "0777"),
    (240, "%#o", &[Arg::Int(4294966785)], "037777777001"),
    (241, "%#x", &[Arg::Int(305441741)], "0x1234abcd"),
    (242, "%#x", &[Arg::Int(3989525555)], "0xedcb5433"),
    (243, "%#X", &[Arg::Int(305441741)], "0X1234ABCD"),
    (244, "%#X", &[Arg::Int(3989525555)], "0XEDCB5433"),
    (245, "%#o", &[Arg::Int(0)], "0"),
    (246, "%#x", &[Arg::Int(0)], "0"),
    (247, "%#X", &[Arg::Int(0)], "0"),
    (248, "%1s", &[Arg::Str("Hallo heimur")], "Hallo heimur"),
    (249, "%1d", &[Arg::Int(1024)], "1024"),
    (250, "%1d", &[Arg::Int(-1024)], "-1024"),
    (251, "%1i", &[Arg::Int(1024)], "1024"),
    (252, "%1i", &[Arg::Int(-1024)], "-1024"),
    (253, "%1u", &[Arg::Int(1024)], "1024"),
    (254, "%1u", &[Arg::Int(4294966272)], "4294966272"),
    (255, "%1o", &[Arg::Int(511)], "777"),
    (256, "%1o", &[Arg::Int(4294966785)], "37777777001"),
    (257, "%1x", &[Arg::Int(305441741)], "1234abcd"),
    (258, "%1x", &[Arg::Int(3989525555)], "edcb5433"),
    (259, "%1X", &[Arg::Int(305441741)], "1234ABCD"),
    (260, "%1X", &[Arg::Int(3989525555)], "EDCB5433"),
    (261, "%1c", &[Arg::Char('x')], "x"),
    (262, "%20s", &[Arg::Str("Hallo")], "               Hallo"),
    (263, "%20d", &[Arg::Int(1024)], "                1024"),
    (264, "%20d", &[Arg::Int(-1024)], "               -1024"),
    (265, "%20i", &[Arg::Int(1024)], "                1024"),
    (266, "%20i", &[Arg::Int(-1024)], "               -1024"),
    (267, "%20u", &[Arg::Int(1024)], "                1024"),
    (268, "%20u", &[Arg::Int(4294966272)], "          4294966272"),
    (269, "%20o", &[Arg::Int(511)], "                 777"),
    (270, "%20o", &[Arg::Int(4294966785)], "         37777777001"),
    (271, "%20x", &[Arg::Int(305441741)], "            1234abcd"),
    (272, "%20x", &[Arg::Int(3989525555)], "            edcb5433"),
    (273, "%20X", &[Arg::Int(305441741)], "            1234ABCD"),
    (274, "%20X", &[Arg::Int(3989525555)], "            EDCB5433"),
    (275, "%20c", &[Arg::Char('x')], "                   x"),
    (276, "%-20s", &[Arg::Str("Hallo")], "Hallo               "),
    (277, "%-20d", &[Arg::Int(1024)], "1024                "),
    (278, "%-20d", &[Arg::Int(-1024)], "-1024               "),
    (279, "%-20i", &[Arg::Int(1024)], "1024                "),
    (280, "%-20i", &[Arg::Int(-1024)], "-1024               "),
    (281, "%-20u", &[Arg::Int(1024)], "1024                "),
    (
        282,
        "%-20u",
        &[Arg::Int(4294966272)],
        "4294966272          ",
    ),
    (283, "%-20o", &[Arg::Int(511)], "777                 "),
    (
        284,
        "%-20o",
        &[Arg::Int(4294966785)],
        "37777777001         ",
    ),
    (285, "%-20x", &[Arg::Int(305441741)], "1234abcd            "),
    (
        286,
        "%-20x",
        &[Arg::Int(3989525555)],
        "edcb5433            ",
    ),
    (287, "%-20X", &[Arg::Int(305441741)], "1234ABCD            "),
    (
        288,
        "%-20X",
        &[Arg::Int(3989525555)],
        "EDCB5433            ",
    ),
    (289, "%-20c", &[Arg::Char('x')], "x                   "),
    (290, "%020d", &[Arg::Int(1024)], "00000000000000001024"),
    (291, "%020d", &[Arg::Int(-1024)], "-0000000000000001024"),
    (292, "%020i", &[Arg::Int(1024)], "00000000000000001024"),
    (293, "%020i", &[Arg::Int(-1024)], "-0000000000000001024"),
    (294, "%020u", &[Arg::Int(1024)], "00000000000000001024"),
    (
        295,
        "%020u",
        &[Arg::Int(4294966272)],
        "00000000004294966272",
    ),
    (296, "%020o", &[Arg::Int(511)], "00000000000000000777"),
    (
        297,
        "%020o",
        &[Arg::Int(4294966785)],
        "00000000037777777001",
    ),
    (298, "%020x", &[Arg::Int(305441741)], "0000000000001234abcd"),
    (
        299,
        "%020x",
        &[Arg::Int(3989525555)],
        "000000000000edcb5433",
    ),
    (300, "%020X", &[Arg::Int(305441741)], "0000000000001234ABCD"),
    (
        301,
        "%020X",
        &[Arg::Int(3989525555)],
        "000000000000EDCB5433",
    ),
    (302, "%#20o", &[Arg::Int(511)], "                0777"),
    (
        303,
        "%#20o",
        &[Arg::Int(4294966785)],
        "        037777777001",
    ),
    (304, "%#20x", &[Arg::Int(305441741)], "          0x1234abcd"),
    (
        305,
        "%#20x",
        &[Arg::Int(3989525555)],
        "          0xedcb5433",
    ),
    (306, "%#20X", &[Arg::Int(305441741)], "          0X1234ABCD"),
    (
        307,
        "%#20X",
        &[Arg::Int(3989525555)],
        "          0XEDCB5433",
    ),
    (308, "%#020o", &[Arg::Int(511)], "00000000000000000777"),
    (
        309,
        "%#020o",
        &[Arg::Int(4294966785)],
        "00000000037777777001",
    ),
    (
        310,
        "%#020x",
        &[Arg::Int(305441741)],
        "0x00000000001234abcd",
    ),
    (
        311,
        "%#020x",
        &[Arg::Int(3989525555)],
        "0x0000000000edcb5433",
    ),
    (
        312,
        "%#020X",
        &[Arg::Int(305441741)],
        "0X00000000001234ABCD",
    ),
    (
        313,
        "%#020X",
        &[Arg::Int(3989525555)],
        "0X0000000000EDCB5433",
    ),
    (314, "%0-20s", &[Arg::Str("Hallo")], "Hallo               "),
    (315, "%0-20d", &[Arg::Int(1024)], "1024                "),
    (316, "%0-20d", &[Arg::Int(-1024)], "-1024               "),
    (317, "%0-20i", &[Arg::Int(1024)], "1024                "),
    (318, "%0-20i", &[Arg::Int(-1024)], "-1024               "),
    (319, "%0-20u", &[Arg::Int(1024)], "1024                "),
    (
        320,
        "%0-20u",
        &[Arg::Int(4294966272)],
        "4294966272          ",
    ),
    (321, "%-020o", &[Arg::Int(511)], "777                 "),
    (
        322,
        "%-020o",
        &[Arg::Int(4294966785)],
        "37777777001         ",
    ),
    (
        323,
        "%-020x",
        &[Arg::Int(305441741)],
        "1234abcd            ",
    ),
    (
        324,
        "%-020x",
        &[Arg::Int(3989525555)],
        "edcb5433            ",
    ),
    (
        325,
        "%-020X",
        &[Arg::Int(305441741)],
        "1234ABCD            ",
    ),
    (
        326,
        "%-020X",
        &[Arg::Int(3989525555)],
        "EDCB5433            ",
    ),
    (327, "%-020c", &[Arg::Char('x')], "x                   "),
    (
        328,
        "%*s",
        &[Arg::Int(20), Arg::Str("Hallo")],
        "               Hallo",
    ),
    (
        329,
        "%*d",
        &[Arg::Int(20), Arg::Int(1024)],
        "                1024",
    ),
    (
        330,
        "%*d",
        &[Arg::Int(20), Arg::Int(-1024)],
        "               -1024",
    ),
    (
        331,
        "%*i",
        &[Arg::Int(20), Arg::Int(1024)],
        "                1024",
    ),
    (
        332,
        "%*i",
        &[Arg::Int(20), Arg::Int(-1024)],
        "               -1024",
    ),
    (
        333,
        "%*u",
        &[Arg::Int(20), Arg::Int(1024)],
        "                1024",
    ),
    (
        334,
        "%*u",
        &[Arg::Int(20), Arg::Int(4294966272)],
        "          4294966272",
    ),
    (
        335,
        "%*o",
        &[Arg::Int(20), Arg::Int(511)],
        "                 777",
    ),
    (
        336,
        "%*o",
        &[Arg::Int(20), Arg::Int(4294966785)],
        "         37777777001",
    ),
    (
        337,
        "%*x",
        &[Arg::Int(20), Arg::Int(305441741)],
        "            1234abcd",
    ),
    (
        338,
        "%*x",
        &[Arg::Int(20), Arg::Int(3989525555)],
        "            edcb5433",
    ),
    (
        339,
        "%*X",
        &[Arg::Int(20), Arg::Int(305441741)],
        "            1234ABCD",
    ),
    (
        340,
        "%*X",
        &[Arg::Int(20), Arg::Int(3989525555)],
        "            EDCB5433",
    ),
    (
        341,
        "%*c",
        &[Arg::Int(20), Arg::Char('x')],
        "                   x",
    ),
    (342, "%.20s", &[Arg::Str("Hallo heimur")], "Hallo heimur"),
    (343, "%.20d", &[Arg::Int(1024)], "00000000000000001024"),
    (344, "%.20d", &[Arg::Int(-1024)], "-00000000000000001024"),
    (345, "%.20i", &[Arg::Int(1024)], "00000000000000001024"),
    (346, "%.20i", &[Arg::Int(-1024)], "-00000000000000001024"),
    (347, "%.20u", &[Arg::Int(1024)], "00000000000000001024"),
    (
        348,
        "%.20u",
        &[Arg::Int(4294966272)],
        "00000000004294966272",
    ),
    (349, "%.20o", &[Arg::Int(511)], "00000000000000000777"),
    (
        350,
        "%.20o",
        &[Arg::Int(4294966785)],
        "00000000037777777001",
    ),
    (351, "%.20x", &[Arg::Int(305441741)], "0000000000001234abcd"),
    (
        352,
        "%.20x",
        &[Arg::Int(3989525555)],
        "000000000000edcb5433",
    ),
    (353, "%.20X", &[Arg::Int(305441741)], "0000000000001234ABCD"),
    (
        354,
        "%.20X",
        &[Arg::Int(3989525555)],
        "000000000000EDCB5433",
    ),
    (
        355,
        "%20.5s",
        &[Arg::Str("Hallo heimur")],
        "               Hallo",
    ),
    (356, "%20.5d", &[Arg::Int(1024)], "               01024"),
    (357, "%20.5d", &[Arg::Int(-1024)], "              -01024"),
    (358, "%20.5i", &[Arg::Int(1024)], "               01024"),
    (359, "%20.5i", &[Arg::Int(-1024)], "              -01024"),
    (360, "%20.5u", &[Arg::Int(1024)], "               01024"),
    (
        361,
        "%20.5u",
        &[Arg::Int(4294966272)],
        "          4294966272",
    ),
    (362, "%20.5o", &[Arg::Int(511)], "               00777"),
    (
        363,
        "%20.5o",
        &[Arg::Int(4294966785)],
        "         37777777001",
    ),
    (
        364,
        "%20.5x",
        &[Arg::Int(305441741)],
        "            1234abcd",
    ),
    (
        365,
        "%20.10x",
        &[Arg::Int(3989525555)],
        "          00edcb5433",
    ),
    (
        366,
        "%20.5X",
        &[Arg::Int(305441741)],
        "            1234ABCD",
    ),
    (
        367,
        "%20.10X",
        &[Arg::Int(3989525555)],
        "          00EDCB5433",
    ),
    (369, "%020.5d", &[Arg::Int(1024)], "               01024"),
    (370, "%020.5d", &[Arg::Int(-1024)], "              -01024"),
    (371, "%020.5i", &[Arg::Int(1024)], "               01024"),
    (372, "%020.5i", &[Arg::Int(-1024)], "              -01024"),
    (373, "%020.5u", &[Arg::Int(1024)], "               01024"),
    (
        374,
        "%020.5u",
        &[Arg::Int(4294966272)],
        "          4294966272",
    ),
    (375, "%020.5o", &[Arg::Int(511)], "               00777"),
    (
        376,
        "%020.5o",
        &[Arg::Int(4294966785)],
        "         37777777001",
    ),
    (
        377,
        "%020.5x",
        &[Arg::Int(305441741)],
        "            1234abcd",
    ),
    (
        378,
        "%020.10x",
        &[Arg::Int(3989525555)],
        "          00edcb5433",
    ),
    (
        379,
        "%020.5X",
        &[Arg::Int(305441741)],
        "            1234ABCD",
    ),
    (
        380,
        "%020.10X",
        &[Arg::Int(3989525555)],
        "          00EDCB5433",
    ),
    (381, "%.0s", &[Arg::Str("Hallo heimur")], ""),
    (
        382,
        "%20.0s",
        &[Arg::Str("Hallo heimur")],
        "                    ",
    ),
    (383, "%.s", &[Arg::Str("Hallo heimur")], ""),
    (
        384,
        "%20.s",
        &[Arg::Str("Hallo heimur")],
        "                    ",
    ),
    (385, "%20.0d", &[Arg::Int(1024)], "                1024"),
    (386, "%20.d", &[Arg::Int(-1024)], "               -1024"),
    (387, "%20.d", &[Arg::Int(0)], "                    "),
    (388, "%20.0i", &[Arg::Int(1024)], "                1024"),
    (389, "%20.i", &[Arg::Int(-1024)], "               -1024"),
    (390, "%20.i", &[Arg::Int(0)], "                    "),
    (391, "%20.u", &[Arg::Int(1024)], "                1024"),
    (
        392,
        "%20.0u",
        &[Arg::Int(4294966272)],
        "          4294966272",
    ),
    (393, "%20.u", &[Arg::Int(0)], "                    "),
    (394, "%20.o", &[Arg::Int(511)], "                 777"),
    (
        395,
        "%20.0o",
        &[Arg::Int(4294966785)],
        "         37777777001",
    ),
    (396, "%20.o", &[Arg::Int(0)], "                    "),
    (397, "%20.x", &[Arg::Int(305441741)], "            1234abcd"),
    (
        398,
        "%20.0x",
        &[Arg::Int(3989525555)],
        "            edcb5433",
    ),
    (399, "%20.x", &[Arg::Int(0)], "                    "),
    (400, "%20.X", &[Arg::Int(305441741)], "            1234ABCD"),
    (
        401,
        "%20.0X",
        &[Arg::Int(3989525555)],
        "            EDCB5433",
    ),
    (402, "%20.X", &[Arg::Int(0)], "                    "),
    (
        403,
        "% -0+*.*s",
        &[Arg::Int(20), Arg::Int(5), Arg::Str("Hallo heimur")],
        "Hallo               ",
    ),
    (
        404,
        "% -0+*.*d",
        &[Arg::Int(20), Arg::Int(5), Arg::Int(1024)],
        "+01024              ",
    ),
    (
        405,
        "% -0+*.*d",
        &[Arg::Int(20), Arg::Int(5), Arg::Int(-1024)],
        "-01024              ",
    ),
    (
        406,
        "% -0+*.*i",
        &[Arg::Int(20), Arg::Int(5), Arg::Int(1024)],
        "+01024              ",
    ),
    (
        407,
        "% 0-+*.*i",
        &[Arg::Int(20), Arg::Int(5), Arg::Int(-1024)],
        "-01024              ",
    ),
    (
        410,
        "%+ -0*.*o",
        &[Arg::Int(20), Arg::Int(5), Arg::Int(511)],
        "00777               ",
    ),
    (
        411,
        "%+ -0*.*o",
        &[Arg::Int(20), Arg::Int(5), Arg::Int(4294966785)],
        "37777777001         ",
    ),
    (
        412,
        "%+ -0*.*x",
        &[Arg::Int(20), Arg::Int(5), Arg::Int(305441741)],
        "1234abcd            ",
    ),
    (
        413,
        "%+ -0*.*x",
        &[Arg::Int(20), Arg::Int(10), Arg::Int(3989525555)],
        "00edcb5433          ",
    ),
    (
        414,
        "% -+0*.*X",
        &[Arg::Int(20), Arg::Int(5), Arg::Int(305441741)],
        "1234ABCD            ",
    ),
    (
        415,
        "% -+0*.*X",
        &[Arg::Int(20), Arg::Int(10), Arg::Int(3989525555)],
        "00EDCB5433          ",
    ),
];

#[test]
fn every_row_of_the_reference_table_still_holds() {
    let mut checked = 0_usize;
    for (serial, format_string, args, expected) in REFERENCE_TABLE {
        assert_eq!(
            format(format_string, args),
            *expected,
            "reference serial {serial}: {format_string:?}"
        );
        checked += 1;
    }
    assert_eq!(checked, 339);
}

#[test]
fn length_modifiers_are_accepted_and_ignored() {
    // `test_typed_printf` (reference lines 125-151) fits every width and
    // signedness into the one integer conversion: the modifier is dropped and
    // "ll" is added back for all of them (`src/string_formatter.h:274-287`),
    // so `%hhi` and `%i` must print the same value without truncating it.
    let rows: &[(&str, &str, i64, &str)] = &[
        ("%hhi", "%i", i64::from(i8::MIN), "-128"),
        ("%hhi", "%i", i64::from(i8::MAX), "127"),
        ("%hhu", "%u", i64::from(u8::MAX), "255"),
        ("%hi", "%i", i64::from(i16::MIN), "-32768"),
        ("%hi", "%i", i64::from(i16::MAX), "32767"),
        ("%hu", "%u", i64::from(u16::MAX), "65535"),
        ("%i", "%i", i64::from(i32::MIN), "-2147483648"),
        ("%u", "%u", i64::from(u32::MAX), "4294967295"),
        ("%li", "%i", i64::MIN, "-9223372036854775808"),
        ("%lli", "%i", i64::MAX, "9223372036854775807"),
        ("%td", "%d", i64::MIN, "-9223372036854775808"),
        // `size_t`'s maximum is 2^64 - 1, which travels as the same bit pattern.
        ("%zu", "%u", -1, "18446744073709551615"),
        ("%lu", "%u", -1, "18446744073709551615"),
        ("%llu", "%u", -1, "18446744073709551615"),
    ];
    for &(modified, plain, value, expected) in rows {
        assert_eq!(
            format(modified, &[Arg::Int(value)]),
            expected,
            "{modified} of {value}"
        );
        assert_eq!(
            format(plain, &[Arg::Int(value)]),
            expected,
            "{plain} of {value}"
        );
    }
}
