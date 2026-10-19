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

use cdda_core_types::text::{Arg, string_format};
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
