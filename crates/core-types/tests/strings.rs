//! String utilities and the message-translation lookup, ported test-first.
//!
//! Specification is the reference's own tests:
//! `references/Cataclysm-DDA/tests/string_test.cpp` (150 lines) and
//! `tests/fold_string_test.cpp` (108 lines), plus the implementation files
//! `src/translations.h`, `src/translation_manager_impl.cpp`, `src/output.cpp`
//! (`trim_by_length`, `split_by_color`, `remove_color_tags`, `foldstring`,
//! `word_rewrap`), `src/catacharset.cpp` (`UTF8_getch`, `utf8_width`,
//! `cursorx_to_position`), `src/wcwidth.cpp` (`mk_wcwidth`), `src/unicode.cpp`
//! (`is_cjk_or_emoji`, `u32_to_lowercase`) and `src/cata_utility.h`
//! (`str_cat`, `str_append`, `string_split`, `string_starts_with`).
//!
//! There is no `string_utils.h` / `string_utils.cpp` in the pinned checkout;
//! those helpers live in the files listed above.

use cdda_core_types::strings::{
    Catalogue, contextual_key, cursorx_to_position, foldstring, get_tag_positions,
    lowercase_first_letter, mk_wcwidth, n_gettext, npgettext, pgettext, remove_color_tags,
    rm_prefix, split_by_color, str_append, str_cat, string_empty_or_whitespace, string_ends_with,
    string_join, string_split, string_starts_with, to_lowercase_char, translate, trim_by_length,
    trim_by_length_checked, uppercase_first_letter, utf8_getch, utf8_width,
};

/// `mk_wcwidth` is Markus Kuhn's wcwidth, `src/wcwidth.cpp:94-180`.
///
/// ASCII is one column, CJK/Hangul/fullwidth are two, combining marks and
/// format characters are zero, and C0/C1 controls are -1 exactly as the
/// reference's table and `1 + (ucs >= 0x1100 && ...)` expression produce.
#[test]
fn utf8_width_counts_columns_like_the_reference() {
    // Latin, digits and ASCII punctuation are one column each.
    assert_eq!(utf8_width("A"), 1);
    assert_eq!(utf8_width("ABC"), 3);
    assert_eq!(utf8_width("MRE "), 4);

    // CJK Unified Ideographs are two columns: the reference's own expectations
    // in string_test.cpp:59-67 only hold if `主` is 2 wide.
    assert_eq!(utf8_width("主"), 2);
    assert_eq!(utf8_width("主菜"), 4);
    // Fullwidth forms (U+FF08, the fullwidth left parenthesis) are 2 as well.
    assert_eq!(utf8_width("（"), 2);
    // The horizontal ellipsis the reference appends is one column
    // (string_test.cpp:56 expects "A…" to fill a width-2 slot).
    assert_eq!(utf8_width("\u{2026}"), 1);
    // Cyrillic is narrow: string_test.cpp:122 expects "У…" in a width-2 slot.
    assert_eq!(utf8_width("У"), 1);
    assert_eq!(utf8_width("У меню"), 6);

    // Combining marks are zero width (wcwidth.cpp interval { 0x0300, 0x036F }).
    assert_eq!(utf8_width("a\u{0301}"), 1);
    // A Hangul Jamo initial consonant is two columns.
    assert_eq!(utf8_width("\u{1100}"), 2);

    // The empty string has no width.
    assert_eq!(utf8_width(""), 0);
}

/// `mk_wcwidth` on its own, including the reference's negative returns.
///
/// `utf8_width` adds `mk_wcwidth` without clamping (`catacharset.cpp:117`), so
/// a control character *subtracts* a column: `utf8_width("\n")` is -1. That is
/// the reference's behaviour, reproduced here rather than fixed.
#[test]
fn mk_wcwidth_reports_controls_and_zero_width_characters() {
    assert_eq!(mk_wcwidth(0), 0);
    assert_eq!(mk_wcwidth('A' as u32), 1);
    assert_eq!(mk_wcwidth(0x4E3B), 2);
    assert_eq!(mk_wcwidth(0x0301), 0);

    // C0 controls and DEL/C1 controls are negative.
    assert_eq!(mk_wcwidth(0x0A), -1);
    assert_eq!(mk_wcwidth(0x7F), -1);
    assert_eq!(mk_wcwidth(0x9F), -1);

    // And utf8_width does not clamp them.
    assert_eq!(utf8_width("\n"), -1);
    assert_eq!(utf8_width("a\nb"), 1);

    // The reference rejects U+FFFE/U+FFFF (and surrogates) as
    // UNKNOWN_UNICODE *before* mk_wcwidth is reached, so utf8_width skips them
    // entirely and they add nothing.
    assert_eq!(utf8_width("\u{FFFE}"), 0);
}

/// `UTF8_getch` (`src/catacharset.cpp:20-102`) is a byte decoder, not a
/// validator: it always consumes at least one byte, and every form it dislikes
/// comes back as `UNKNOWN_UNICODE` after the bytes it swallowed.
#[test]
fn utf8_getch_consumes_bytes_and_rejects_like_the_reference() {
    // Well-formed sequences consume exactly their own bytes.
    assert_eq!(utf8_getch(b"A"), (Some(0x41), 1));
    assert_eq!(utf8_getch("é".as_bytes()), (Some(0xE9), 2));
    assert_eq!(utf8_getch("У".as_bytes()), (Some(0x423), 2));
    assert_eq!(utf8_getch("主".as_bytes()), (Some(0x4E3B), 3));
    assert_eq!(utf8_getch("😀".as_bytes()), (Some(0x1F600), 4));
    assert_eq!(utf8_getch(b""), (None, 0));

    // A stray continuation byte is rejected but still consumes one byte.
    assert_eq!(utf8_getch(b"\x80X"), (None, 1));
    // 0xFE never starts a sequence the reference accepts; one byte is consumed.
    assert_eq!(utf8_getch(b"\xFEX"), (None, 1));
    // Truncated sequence: the two bytes are consumed, then UNKNOWN.
    assert_eq!(utf8_getch(&[0xE4, 0xB8]), (None, 2));
    // A bad continuation abandons the sequence *at* that byte without
    // consuming it (catacharset.cpp:82-85 breaks before advancing), so only the
    // lead byte is accounted for.
    assert_eq!(utf8_getch(&[0xE4, 0x41, 0x41]), (None, 1));

    // Overlong encodings are rejected: C0 80 (overlong NUL) and F0 80 80 80.
    assert_eq!(utf8_getch(&[0xC0, 0x80]), (None, 2));
    assert_eq!(utf8_getch(&[0xF0, 0x80, 0x80, 0x80]), (None, 4));

    // Surrogates and the two non-characters are rejected explicitly
    // (catacharset.cpp:95-99): ED A0 80 is U+D800, EF BF BE is U+FFFE.
    assert_eq!(utf8_getch(&[0xED, 0xA0, 0x80]), (None, 3));
    assert_eq!(utf8_getch(&[0xEF, 0xBF, 0xBE]), (None, 3));
}

/// `cursorx_to_position` (`src/catacharset.cpp:161-192`) converts a column to a
/// byte offset. It returns the byte offset *after* the character that overshot
/// the column, while its `prevpos` output is the offset of the last character
/// that still fit -- `trim_by_length` reads `prevpos`, not the return value.
#[test]
fn cursorx_to_position_returns_bytes_for_columns() {
    // Plain ASCII: one byte per column.
    assert_eq!(cursorx_to_position("ABC", 0, -1), (0, 0));
    assert_eq!(cursorx_to_position("ABC", 1, -1), (1, 1));
    assert_eq!(cursorx_to_position("ABC", 3, -1), (3, 3));
    // Past the end: the offset stops at the end of the string.
    assert_eq!(cursorx_to_position("ABC", 99, -1), (3, 3));

    // Wide characters: the byte offset overshoots the column offset, and
    // prevpos is where the last whole character ended (string_test.cpp:61
    // needs prevpos == 2 for "AB…" from "AB文字" at width 4).
    assert_eq!(cursorx_to_position("AB文字", 3, -1), (5, 2));
    assert_eq!(cursorx_to_position("AB文字", 4, -1), (5, 5));
    assert_eq!(cursorx_to_position("AB文字", 6, -1), (8, 8));

    // maxlen clamps the byte offset, as the reference's `maxlen` argument does.
    assert_eq!(cursorx_to_position("AB文字", 99, 4), (2, 2));

    // An empty string has nowhere to move.
    assert_eq!(cursorx_to_position("", 5, -1), (0, 0));
}

/// `string_test.cpp:19-31`, Case 1: `remove_color_tags` deletes `<color_...>`
/// and `</color>` but is deliberately forgiving about tags that have no pair.
#[test]
fn remove_color_tags_matches_reference_case_1() {
    // The four assertions of string_test.cpp:22-29, in order.
    assert_eq!(
        remove_color_tags("<color_red>TestString</color>"),
        "TestString"
    );
    assert_eq!(
        remove_color_tags("TestStringWithoutOpeningColorTag</color>"),
        "TestStringWithoutOpeningColorTag"
    );
    assert_eq!(
        remove_color_tags("<color_yellow>TestStringWithoutClosingColorTag"),
        "TestStringWithoutClosingColorTag"
    );
    assert_eq!(
        remove_color_tags("<color_green>Test</color>StringWithMultiple<color_light_gray>ColorTags"),
        "TestStringWithMultipleColorTags"
    );
    // A string with no tags at all is returned unchanged.
    assert_eq!(remove_color_tags("plain"), "plain");
    assert_eq!(remove_color_tags(""), "");
}

/// `split_by_color` and `get_tag_positions` (`src/output.cpp:187-214`,
/// `:1542-1556`), plus `rm_prefix` (`:2712-2721`).
///
/// `split_by_color` cuts *at* every tag, so each segment begins with a tag and
/// the last one runs to the end; a string that starts with a tag therefore
/// yields an empty first segment, which `trim_by_length` has to skip.
#[test]
fn split_by_color_and_get_tag_positions_follow_the_reference() {
    assert_eq!(
        get_tag_positions("<color_red>TestString</color>"),
        vec![0, 21]
    );
    assert_eq!(
        get_tag_positions("<color_green>Test</color>StringWithMultiple<color_light_gray>ColorTags"),
        vec![0, 17, 43]
    );
    // Only the two colour forms are tags; <info> is ordinary text.
    assert_eq!(get_tag_positions("<info>text</info>"), Vec::<usize>::new());

    assert_eq!(
        split_by_color("<color_red>TestString</color>"),
        vec!["", "<color_red>TestString", "</color>"]
    );
    assert_eq!(split_by_color("no tags"), vec!["no tags"]);
    assert_eq!(
        split_by_color("Press [<color_yellow>?</color>] to change keybindings."),
        vec![
            "Press [",
            "<color_yellow>?",
            "</color>] to change keybindings."
        ]
    );

    // rm_prefix removes everything up to and including the first c2, but only
    // when the string really starts with c1 and some c2 exists.
    assert_eq!(
        rm_prefix("<color_red>TestString</color>", '<', '>'),
        "TestString</color>"
    );
    assert_eq!(rm_prefix("no tag", '<', '>'), "no tag");
    assert_eq!(rm_prefix("<noclosing", '<', '>'), "<noclosing");
    assert_eq!(rm_prefix("", '<', '>'), "");
}

/// `string_test.cpp:56-70`: a string wider than the limit gets an ellipsis, one
/// that fits is returned untouched, and the empty string stays empty.
#[test]
fn trim_by_length_trims_and_passes_short_strings_through() {
    assert_eq!(trim_by_length("ABC", 2), "A\u{2026}");
    assert_eq!(trim_by_length("ABC", 3), "ABC");
    assert_eq!(trim_by_length("ABCDEF", 4), "ABC\u{2026}");
    // A string that already fits is returned unchanged, tags and all.
    assert_eq!(trim_by_length("", 5), "");
}

/// `string_test.cpp:74-82`: a non-positive width is a programming error. The
/// reference logs through `debugmsg` and returns an empty string; the port
/// reports the same message through `trim_by_length_checked` and keeps the
/// infallible `trim_by_length` as the silent path, exactly as the reference's
/// return value behaves.
#[test]
fn trim_by_length_reports_invalid_width_like_the_reference() {
    // The reference's exact debugmsg text, including the double space.
    assert_eq!(
        trim_by_length_checked("", 0).err().map(|e| e.to_string()),
        Some("Unable to trim string '' to width 0.  Returning empty string.".to_owned())
    );
    assert_eq!(
        trim_by_length_checked("test string", 0)
            .err()
            .map(|e| e.to_string()),
        Some("Unable to trim string 'test string' to width 0.  Returning empty string.".to_owned())
    );
    assert_eq!(
        trim_by_length_checked("test string", -2)
            .err()
            .map(|e| e.to_string()),
        Some(
            "Unable to trim string 'test string' to width -2.  Returning empty string.".to_owned()
        )
    );
    // The silent wrapper still returns the empty string.
    assert_eq!(trim_by_length("test string", 0), "");
    assert_eq!(trim_by_length("test string", -2), "");
}

/// `string_test.cpp:59-67`: wide characters occupy two columns, so a limit that
/// lands inside one backs off to the previous character boundary.
#[test]
fn trim_by_length_handles_wide_characters() {
    assert_eq!(
        trim_by_length("AB\u{6587}\u{5B57}", 6),
        "AB\u{6587}\u{5B57}"
    );
    assert_eq!(
        trim_by_length("AB\u{6587}\u{5B57}", 5),
        "AB\u{6587}\u{2026}"
    );
    assert_eq!(trim_by_length("AB\u{6587}\u{5B57}", 4), "AB\u{2026}");

    let mre = "MRE \u{4E3B}\u{83DC}\u{FF08}\u{9E21}\u{8089}\u{610F}\u{5927}\u{5229}\u{9999}\u{849C}\u{6C99}\u{53F8}\u{901A}\u{5FC3}\u{7C89}\u{FF09}\u{FF08}\u{65B0}\u{9C9C}\u{FF09}";
    assert_eq!(trim_by_length(mre, 5), "MRE \u{2026}");
    assert_eq!(trim_by_length(mre, 6), "MRE \u{2026}");
    assert_eq!(
        trim_by_length(mre, 36),
        "MRE \u{4E3B}\u{83DC}\u{FF08}\u{9E21}\u{8089}\u{610F}\u{5927}\u{5229}\u{9999}\u{849C}\u{6C99}\u{53F8}\u{901A}\u{5FC3}\u{7C89}\u{2026}"
    );
}

/// `string_test.cpp:89-93`: trimming one colour segment at a time. As the
/// reference's own comment says, a tag only appears on the ellipsis when at
/// least one character of the tagged segment was printed.
#[test]
fn trim_by_length_trims_at_colour_tag_breaks() {
    let keybinding_hint = "Press [<color_yellow>?</color>] to change keybindings.";
    assert_eq!(trim_by_length(keybinding_hint, 6), "Press\u{2026}");
    assert_eq!(trim_by_length(keybinding_hint, 7), "Press \u{2026}");
    assert_eq!(trim_by_length(keybinding_hint, 8), "Press [\u{2026}");
    assert_eq!(
        trim_by_length(keybinding_hint, 9),
        "Press [<color_yellow>?</color>\u{2026}"
    );

    let jelly = "Gather <color_light_blue>80 cattail stalks</color> from the swamp and bring \
                 them back to <color_light_red>learn how to craft cattail jelly</color>.  ";
    assert_eq!(
        trim_by_length(jelly, 10),
        "Gather <color_light_blue>80\u{2026}</color>"
    );
    assert_eq!(
        trim_by_length(jelly, 63),
        "Gather <color_light_blue>80 cattail stalks</color> from the swamp and bring them back to\u{2026}"
    );
    assert_eq!(trim_by_length(jelly, 98), jelly);
}

/// `string_test.cpp:105-125`: wide characters inside colour tags, a fully
/// tagged string, and Cyrillic (which is narrow but multi-byte, so every offset
/// here is a byte offset into a two-byte-per-character string).
#[test]
fn trim_by_length_handles_tagged_wide_and_cyrillic_text() {
    let tagged_mre = "MRE <color_green>\u{4E3B}\u{83DC}</color>\u{FF08}\u{9E21}\u{8089}\u{610F}\u{5927}\u{5229}\u{9999}\u{849C}\u{6C99}\u{53F8}\u{901A}\u{5FC3}\u{7C89}\u{FF09}\u{FF08}\u{65B0}\u{9C9C}\u{FF09}";
    assert_eq!(trim_by_length(tagged_mre, 5), "MRE \u{2026}");
    assert_eq!(trim_by_length(tagged_mre, 6), "MRE \u{2026}");
    assert_eq!(
        trim_by_length(tagged_mre, 7),
        "MRE <color_green>\u{4E3B}\u{2026}</color>"
    );
    assert_eq!(
        trim_by_length(tagged_mre, 8),
        "MRE <color_green>\u{4E3B}\u{2026}</color>"
    );
    assert_eq!(
        trim_by_length(tagged_mre, 9),
        "MRE <color_green>\u{4E3B}\u{83DC}</color>\u{2026}"
    );

    // A string that is entirely inside a colour tag: the ellipsis inherits it,
    // because the opening segment is empty and skipped.
    assert_eq!(
        trim_by_length("<color_white_green>\u{4F11}\u{9592}\u{5340}</color>", 1),
        "<color_white_green>\u{2026}</color>"
    );
    assert_eq!(
        trim_by_length("<color_white_green>\u{4F11}\u{9592}\u{5340}</color>", 3),
        "<color_white_green>\u{4F11}\u{2026}</color>"
    );

    let cyrillic = "\u{423} \u{43C}\u{435}\u{43D}\u{44E} <color_light_cyan>\u{417}\u{431}\u{438}\u{440}\u{430}\u{43D}\u{43D}\u{44F}</color> \u{43D}\u{430}\u{442}\u{438}\u{441}\u{43D}\u{456}\u{442}\u{44C}:";
    assert_eq!(trim_by_length(cyrillic, 2), "\u{423}\u{2026}");
    assert_eq!(
        trim_by_length(cyrillic, 9),
        "\u{423} \u{43C}\u{435}\u{43D}\u{44E} <color_light_cyan>\u{417}\u{2026}</color>"
    );
    assert_eq!(
        trim_by_length(cyrillic, 16),
        "\u{423} \u{43C}\u{435}\u{43D}\u{44E} <color_light_cyan>\u{417}\u{431}\u{438}\u{440}\u{430}\u{43D}\u{43D}\u{44F}</color>\u{2026}"
    );
}

/// `fold_string_test.cpp:24-56`, cases 1 and 2: the empty line, the lorem
/// ipsum wrap, and CJK wrapping (where a break is allowed between any two
/// ideographs).
#[test]
fn foldstring_wraps_lorem_ipsum_and_chinese() {
    // fold_string_test.cpp:24-40.
    assert_eq!(
        foldstring(
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit.  Pellentesque a.",
            17
        ),
        vec![
            "Lorem ipsum dolor",
            "sit amet, ",
            "consectetur ",
            "adipiscing elit. ",
            "Pellentesque a.",
        ]
    );

    // fold_string_test.cpp:42-56.
    assert_eq!(
        foldstring(
            "\u{6625}\u{54E5}\u{7EAF}\u{7237}\u{4EEC}\u{FF0C}\u{94C1}\u{8840}\u{771F}\u{6C49}\u{5B50}\u{3002}\u{4EBA}\u{6C11}\u{597D}\u{5144}\u{5F1F}\u{FF0C}\u{7236}\u{4EB2}\u{597D}\u{513F}\u{5B50}\u{3002}",
            19
        ),
        vec![
            "\u{6625}\u{54E5}\u{7EAF}\u{7237}\u{4EEC}\u{FF0C}\u{94C1}\u{8840}\u{771F}",
            "\u{6C49}\u{5B50}\u{3002}\u{4EBA}\u{6C11}\u{597D}\u{5144}\u{5F1F}\u{FF0C}",
            "\u{7236}\u{4EB2}\u{597D}\u{513F}\u{5B50}\u{3002}",
        ]
    );
}

/// `fold_string_test.cpp:58-90`, cases 3 and 4: mixed Latin/CJK text, and the
/// colour tags that must be re-opened and re-closed on every line.
#[test]
fn foldstring_wraps_mixed_language_and_reopens_colour_tags() {
    // Case 3, fold_string_test.cpp:58-75.
    assert_eq!(
        foldstring(
            "Cataclysm-DDA\u{662F}Github\u{4E0A}\u{7684}\u{4E00}\u{4E2A}\u{5F00}\u{6E90}\u{6E38}\u{620F}\u{9879}\u{76EE}\u{FF0C}\u{76EE}\u{524D}\u{5DF2}\u{6709}\u{8D85}\u{8FC7}16000\u{4E2A}PR.",
            13
        ),
        vec![
            "Cataclysm-DDA",
            "\u{662F}Github\u{4E0A}\u{7684}",
            "\u{4E00}\u{4E2A}\u{5F00}\u{6E90}\u{6E38}\u{620F}",
            "\u{9879}\u{76EE}\u{FF0C}\u{76EE}\u{524D}\u{5DF2}",
            "\u{6709}\u{8D85}\u{8FC7}16000\u{4E2A}",
            "PR.",
        ]
    );

    // Case 4, fold_string_test.cpp:77-90.
    assert_eq!(
        foldstring(
            "<color_red>Lorem ipsum dolor sit amet, <color_green>consectetur adipiscing elit</color>. Pellentesque a.</color>",
            18
        ),
        vec![
            "<color_red>Lorem ipsum dolor </color>",
            "<color_red>sit amet, </color>",
            "<color_red><color_green>consectetur </color></color>",
            "<color_red><color_green>adipiscing elit</color>. </color>",
            "<color_red>Pellentesque a.</color>",
        ]
    );
}

/// `fold_string_test.cpp:92-107`, case 5: a word longer than the width is cut
/// at the width rather than allowed to overflow; the reference's "at least one
/// character per line" fallback is what makes `Hippopotomonstrosesq` exactly 20
/// columns.
#[test]
fn foldstring_breaks_words_longer_than_the_width() {
    assert_eq!(
        foldstring(
            "You gain a mutation called Hippopotomonstrosesquippedaliophobia!",
            20
        ),
        vec![
            "You gain a mutation ",
            "called ",
            "Hippopotomonstrosesq",
            "uippedaliophobia!",
        ]
    );
}

/// `string_test.cpp:129-138`: `str_cat` concatenates without the intermediate
/// allocation of `a + b + c`. The C++ version is variadic and takes rvalues;
/// the port takes a slice of `&str`, so the moved-`std::string` case becomes a
/// repeated borrow, which observes the same result.
#[test]
fn str_cat_concatenates_its_arguments() {
    assert_eq!(str_cat(&[" "]), " ");
    assert_eq!(str_cat(&["a", "b", "c"]), "abc");
    // The reference's `str_cat( a, std::move( b ), a )` with a = "a", b = "b".
    let a = String::from("a");
    let b = String::from("b");
    assert_eq!(str_cat(&[&a, &b, &a]), "aba");
}

/// `string_test.cpp:140-150`: `str_append` appends in place and returns the
/// root, so calls can be chained; the reference's four calls accumulate into
/// `"a bcbcaba"`.
#[test]
fn str_append_accumulates_into_the_root() {
    let mut root = String::from("a");
    assert_eq!(str_append(&mut root, &[" "]), "a ");
    assert_eq!(str_append(&mut root, &["b", "c"]), "a bc");
    assert_eq!(str_append(&mut root, &["b", "c"]), "a bcbc");
    let a = String::from("a");
    let b = String::from("b");
    assert_eq!(str_append(&mut root, &[&a, &b, &a]), "a bcbcaba");
    assert_eq!(root, "a bcbcaba");
}

/// `string_split` (`src/cata_utility.cpp:743-760`) and `string_join`
/// (`src/cata_utility.h:511-524`), which is `ostringstream`-based in the
/// reference and therefore renders elements with `operator<<`.
#[test]
fn string_split_and_join_round_trip() {
    // An empty string yields no elements at all, not one empty element.
    assert_eq!(string_split("", ','), Vec::<String>::new());
    assert_eq!(string_split("a", ','), vec!["a"]);
    assert_eq!(string_split("a,b,c", ','), vec!["a", "b", "c"]);
    // std::getline keeps interior empty fields, and the reference appends a
    // final empty element when the string ends with the delimiter.
    assert_eq!(string_split("a,,b", ','), vec!["a", "", "b"]);
    assert_eq!(string_split("a,b,", ','), vec!["a", "b", ""]);
    assert_eq!(string_split(",a", ','), vec!["", "a"]);

    assert_eq!(string_join(&[] as &[String], ","), "");
    assert_eq!(string_join(&["a"], ","), "a");
    assert_eq!(string_join(&["a", "b", "c"], "-"), "a-b-c");
    assert_eq!(string_join(&["a", "b"], ""), "ab");
}

/// `string_starts_with` / `string_ends_with` (`src/cata_utility.h:466-478`)
/// and `string_empty_or_whitespace` (`src/cata_utility.cpp:704-714`).
///
/// The reference's whitespace test goes through `utf8_to_wstr` and `iswspace`,
/// i.e. the **C locale**: only space, `\t`, `\n`, `\v`, `\f` and `\r` count, and
/// a non-breaking space is *not* whitespace. That is reproduced here rather
/// than replaced with Unicode `char::is_whitespace`; see the findings in the
/// module documentation.
#[test]
fn prefix_suffix_tests_and_whitespace_predicate() {
    assert!(string_starts_with("abcdef", "abc"));
    assert!(string_starts_with("abc", ""));
    assert!(!string_starts_with("abc", "abcd"));
    assert!(string_ends_with("abcdef", "def"));
    assert!(string_ends_with("abc", ""));
    assert!(!string_ends_with("abc", "zabc"));

    assert!(string_empty_or_whitespace(""));
    assert!(string_empty_or_whitespace(" \t\n"));
    assert!(!string_empty_or_whitespace(" x "));
    // C-locale `iswspace`: U+00A0 (NBSP) is not whitespace, and neither is
    // U+3000 (ideographic space).
    assert!(!string_empty_or_whitespace("\u{A0}"));
    assert!(!string_empty_or_whitespace("\u{3000}"));
}

/// `u32_to_lowercase`, `src/unicode.cpp:75-96`: a hand-written table, not the
/// Unicode database.
///
/// Reproduced exactly, including its two oddities: U+00D7 (`×`) sits inside the
/// `0xc0..=0x00de` "accented latin" span and therefore becomes U+00F7 (`÷`),
/// and U+0130 (`İ`) is in the even/odd dotted-I range and becomes U+0131 (`ı`).
#[test]
fn u32_to_lowercase_matches_the_reference_table() {
    // ASCII A-Z.
    assert_eq!(to_lowercase_char('A'), 'a');
    assert_eq!(to_lowercase_char('Z'), 'z');
    assert_eq!(to_lowercase_char('a'), 'a');
    // Accented Latin, and Cyrillic А-Я.
    assert_eq!(to_lowercase_char('\u{C0}'), '\u{E0}');
    assert_eq!(to_lowercase_char('\u{410}'), '\u{430}');
    assert_eq!(to_lowercase_char('\u{42F}'), '\u{44F}');
    // Cyrillic Ё-Џ shift by 0x50, not 0x20.
    assert_eq!(to_lowercase_char('\u{401}'), '\u{451}');
    assert_eq!(to_lowercase_char('\u{178}'), '\u{FF}');
    // Even/odd dotted pairs.
    assert_eq!(to_lowercase_char('\u{100}'), '\u{101}');
    assert_eq!(to_lowercase_char('\u{101}'), '\u{101}');
    assert_eq!(to_lowercase_char('\u{139}'), '\u{13A}');
    assert_eq!(to_lowercase_char('\u{4C1}'), '\u{4C2}');

    // The two documented inconsistencies, kept as the reference has them.
    assert_eq!(to_lowercase_char('\u{D7}'), '\u{F7}'); // × becomes ÷
    assert_eq!(to_lowercase_char('\u{130}'), '\u{131}'); // İ becomes ı
    // Outside the table nothing happens, even for real lowercase forms.
    assert_eq!(to_lowercase_char('\u{3A3}'), '\u{3A3}'); // Greek Σ unchanged
}

/// `uppercase_first_letter` / `lowercase_first_letter`,
/// `src/output.cpp:2692-2706`.
///
/// These go through `towupper`/`towlower`, i.e. the **C locale**, so they only
/// fold ASCII -- unlike `u32_to_lowercase` in the same library, which folds
/// Cyrillic and accented Latin. On an empty string the reference writes
/// `wstr[0]`, which is out of bounds for `std::wstring`; the port returns the
/// string unchanged.
#[test]
fn first_letter_case_folding_is_ascii_only() {
    assert_eq!(lowercase_first_letter("ABC"), "aBC");
    assert_eq!(uppercase_first_letter("abc"), "Abc");
    assert_eq!(lowercase_first_letter(""), "");
    assert_eq!(uppercase_first_letter(""), "");
    // Already the right case: unchanged.
    assert_eq!(lowercase_first_letter("aBC"), "aBC");
    assert_eq!(uppercase_first_letter("Abc"), "Abc");

    // C locale: non-ASCII is left alone, while to_lowercase_char would fold it.
    assert_eq!(lowercase_first_letter("\u{C9}cole"), "\u{C9}cole");
    assert_eq!(to_lowercase_char('\u{C9}'), '\u{E9}');
    assert_eq!(
        uppercase_first_letter("\u{43F}\u{440}\u{438}"),
        "\u{43F}\u{440}\u{438}"
    );
}

/// `translations.h:87-95`, the `!LOCALIZE` branch and
/// `translation_manager_impl.cpp:168-176`: with no catalogue loaded, `_( msgid )`
/// is the msgid itself. The reference expresses this as the process-wide
/// `TranslationManager` singleton; the port takes the catalogue as a parameter,
/// and `None` is the no-catalogue state.
#[test]
fn translate_without_a_catalogue_returns_the_msgid() {
    assert_eq!(translate(None, "Hello"), "Hello");
    assert_eq!(translate(None, ""), "");
    // Not just ASCII: the return is the same bytes, untouched.
    assert_eq!(translate(None, "\u{4E3B}\u{83DC}"), "\u{4E3B}\u{83DC}");
    assert_eq!(translate(None, "100% sure"), "100% sure");
}

/// `pgettext`, `translations.h:70-75` + `translation_manager_impl.cpp:197-210`.
///
/// The lookup key is `context + "\u{4}" + msgid` (`ConstructContextualQuery`),
/// and on a miss the *msgid* comes back -- the context is never returned. With
/// no catalogue every lookup misses.
#[test]
fn pgettext_without_a_catalogue_returns_the_msgid() {
    assert_eq!(pgettext(None, "verb", "Open"), "Open");
    assert_eq!(pgettext(None, "", "Open"), "Open");
    // The same msgid under two contexts is still just the msgid.
    assert_eq!(pgettext(None, "noun", "Open"), "Open");
    // The key really is context + 0x04 + msgid, which is what a catalogue has
    // to be keyed by; see the catalogue tests.
    assert_eq!(contextual_key("verb", "Open"), "verb\u{4}Open");
}

/// `n_gettext`, `translations.h:61-68` + `translation_manager_impl.cpp:178-192`.
///
/// With no catalogue the reference falls back to `n == 1 ? msgid : msgid_plural`
/// -- the exact comparison, so zero takes the plural form (unlike some gettext
/// implementations, which special-case zero). `npgettext` behaves the same way
/// on a miss.
#[test]
fn n_gettext_selects_the_plural_form_by_exact_comparison() {
    assert_eq!(n_gettext(None, "one apple", "many apples", 1), "one apple");
    assert_eq!(
        n_gettext(None, "one apple", "many apples", 0),
        "many apples"
    );
    assert_eq!(
        n_gettext(None, "one apple", "many apples", 2),
        "many apples"
    );
    assert_eq!(
        n_gettext(None, "one apple", "many apples", 1_000_000),
        "many apples"
    );

    // npgettext: same selection, context ignored when there is no catalogue.
    assert_eq!(
        npgettext(None, "fruit", "one apple", "many apples", 1),
        "one apple"
    );
    assert_eq!(
        npgettext(None, "fruit", "one apple", "many apples", 5),
        "many apples"
    );
}

/// A catalogue hit replaces the msgid; a miss still yields the msgid. The
/// catalogue is per-instance, so two languages can coexist.
///
/// Reference: `Translate`/`TranslateWithContext`, and `Catalogue::insert`
/// corresponds to a `.mo` entry.
#[test]
fn a_loaded_catalogue_translates_hits_and_passes_misses_through() {
    let mut german = Catalogue::with_plural_forms("nplurals=2; plural=(n != 1);");
    german.insert("Open", &["\u{D6}ffnen"]);
    german.insert("apple", &["Apfel", "\u{C4}pfel"]);
    german.insert(&contextual_key("verb", "Open"), &["\u{F6}ffnen"]);

    // Plain hit and plain miss.
    assert_eq!(translate(Some(&german), "Open"), "\u{D6}ffnen");
    assert_eq!(translate(Some(&german), "Close"), "Close");
    // A contextual hit only answers for its own context, and the bare msgid
    // is not shadowed by the contextual entry.
    assert_eq!(pgettext(Some(&german), "verb", "Open"), "\u{F6}ffnen");
    assert_eq!(pgettext(Some(&german), "noun", "Open"), "Open");
    assert_eq!(translate(Some(&german), "Open"), "\u{D6}ffnen");

    // The plural rule from the header decides which form `n_gettext` returns.
    assert_eq!(n_gettext(Some(&german), "apple", "apples", 1), "Apfel");
    assert_eq!(n_gettext(Some(&german), "apple", "apples", 2), "\u{C4}pfel");

    // An empty catalogue is indistinguishable from no catalogue.
    let empty = Catalogue::with_plural_forms("nplurals=2; plural=(n != 1);");
    assert_eq!(translate(Some(&empty), "Open"), "Open");
}

/// A three-form language rule, verbatim from a real Russian `.po` header.
///
/// This is the part of the reference the `plural=` expression exists for: which
/// form `n` selects is language data, evaluated by
/// `TranslationPluralRulesEvaluator`, not a hard-coded `n == 1` test.
#[test]
fn a_three_form_language_rule_selects_by_expression() {
    const RUSSIAN: &str = "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : \
                           n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);";
    let mut catalogue = Catalogue::with_plural_forms(RUSSIAN);
    catalogue.insert(
        "\u{43A}\u{43D}\u{438}\u{433}\u{430}", // "книга"
        &[
            "\u{43A}\u{43D}\u{438}\u{433}\u{430}", // 1
            "\u{43A}\u{43D}\u{438}\u{433}\u{438}", // 2..4
            "\u{43A}\u{43D}\u{438}\u{433}",        // the rest
        ],
    );
    let book = "\u{43A}\u{43D}\u{438}\u{433}\u{430}";
    let forms = [
        "\u{43A}\u{43D}\u{438}\u{433}\u{430}",
        "\u{43A}\u{43D}\u{438}\u{433}\u{438}",
        "\u{43A}\u{43D}\u{438}\u{433}",
    ];
    let expected = [
        (1_u64, 0_usize),
        (2, 1),
        (4, 1),
        (5, 2),
        (0, 2),
        (11, 2),
        (21, 0),
        (22, 1),
        (102, 1),
        (111, 2),
        (1_004, 1),
    ];
    for (n, form) in expected {
        assert_eq!(
            n_gettext(Some(&catalogue), book, book, n),
            forms.get(form).copied().unwrap_or(""),
            "n = {n}"
        );
    }
}

/// `GetTranslatedStringPlural`, `src/translation_document.cpp:...`: when the
/// rule picks a form the entry does not carry, the reference logs
/// "Plural forms expression evaluated out-of-bound" and returns **form 0**.
///
/// A malformed header is rejected rather than guessed at, mirroring the
/// evaluator's parse errors.
#[test]
fn plural_selection_falls_back_to_the_first_form() {
    let mut catalogue = Catalogue::with_plural_forms("nplurals=2; plural=(n != 1);");
    // A singular-only translation of a msgid also used as a plural msgid.
    catalogue.insert("apple", &["Apfel"]);
    assert_eq!(n_gettext(Some(&catalogue), "apple", "apples", 1), "Apfel");
    // Form 1 does not exist, so form 0 is used instead of panicking.
    assert_eq!(n_gettext(Some(&catalogue), "apple", "apples", 7), "Apfel");

    // A header the evaluator cannot parse is an error, not a silent default.
    assert!(Catalogue::try_with_plural_forms("nplurals=2; plural=(n !=);").is_err());
    assert!(Catalogue::try_with_plural_forms("plural=(n != 1);").is_err());
    assert!(Catalogue::try_with_plural_forms("nplurals=2; plural=(n & 1);").is_err());
    // And the fallback constructor still yields a usable catalogue.
    assert_eq!(
        Catalogue::with_plural_forms("nonsense")
            .plural_rule()
            .form_count(),
        2
    );
}

/// Undo the one escaping `tools/gen_msgids.py` applies to a corpus line.
fn unescape_corpus_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('\\') | None => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    out
}

/// Every message id `tools/gen_msgids.py` extracted from the C++ source, fed
/// back through the lookup layer with no catalogue loaded, must come out
/// unchanged.
///
/// This is the end-to-end check on both halves: the extractor's output format
/// and the lookup layer's identity behaviour. It also pins the corpus's
/// determinism contract -- sorted, unique, one entry per line -- because the
/// file is compared byte for byte by the tool's own re-run.
#[test]
fn every_extracted_msgid_round_trips_unchanged() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/text/msgids.txt");
    let corpus = std::fs::read_to_string(path).unwrap_or_default();
    assert!(
        !corpus.is_empty(),
        "corpus/text/msgids.txt is missing or empty; run python3 tools/gen_msgids.py"
    );

    let mut bare = 0_usize;
    let mut contextual = 0_usize;
    let mut previous: Option<&str> = None;
    for raw in corpus.lines() {
        assert!(!raw.is_empty(), "corpus contains a blank line");
        if let Some(prev) = previous {
            assert!(prev < raw, "corpus is not sorted and unique at {raw:?}");
        }
        previous = Some(raw);

        if let Some((context, msgid)) = raw.split_once('\t') {
            let context = unescape_corpus_line(context);
            let msgid = unescape_corpus_line(msgid);
            assert_eq!(
                pgettext(None, &context, &msgid),
                msgid,
                "contextual msgid did not round-trip: {context:?} / {msgid:?}"
            );
            contextual += 1;
        } else {
            let msgid = unescape_corpus_line(raw);
            assert_eq!(translate(None, &msgid), msgid, "msgid did not round-trip");
            // Plural call sites pass the same msgid as both forms, so the
            // unchanged passthrough holds for n == 1 and n != 1 alike.
            assert_eq!(n_gettext(None, &msgid, &msgid, 1), msgid);
            assert_eq!(n_gettext(None, &msgid, &msgid, 2), msgid);
            bare += 1;
        }
    }

    // Sanity bound: the reference has thousands of translatable strings, so a
    // truncated corpus must fail rather than silently pass.
    assert!(
        bare > 9_000 && contextual > 500,
        "corpus looks truncated: {bare} bare, {contextual} contextual"
    );
}
