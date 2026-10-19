//! Colour values: RGB arithmetic, ncurses attributes, names and tags.
//!
//! Reference: `src/color.h`, `src/color.cpp` for the attribute and naming
//! layer, `src/sdl_utils.h`/`src/sdl_utils.cpp` for the RGB arithmetic (the
//! graphics build is the only one that has RGB values at all), and
//! `data/raw/colors.json` for the palette those RGB values come from.

use std::cmp::Ordering;
use std::collections::HashSet;

use cdda_core_types::colors::{
    A_BLINK, A_BOLD, COLOR_PAIRS, COLORS, ColorError, ColorId, DARK_GRAY_FALLBACK, NUM_COLORS,
    NcColor, Rgb, TagParse, adjust_color_brightness, bgcolor_from_string, color_def,
    color_from_name, color_from_string, color_linear_interpolate, color_name, color_pixel_darken,
    color_pixel_grayscale, color_pixel_nightvision, color_pixel_overexposed,
    color_pixel_sepia_light, color_value, colorize, curses_color_to_rgb, cyan_background,
    get_color_from_tag, get_tag_from_color, green_background, hilite, invert_color,
    magenta_background, mix_colors, red_background, string_from_color, white_background,
    yellow_background,
};

#[test]
fn rgb_is_built_from_four_components() {
    let colour = Rgb::new(1, 2, 3, 4);
    assert_eq!(colour.red(), 1);
    assert_eq!(colour.green(), 2);
    assert_eq!(colour.blue(), 3);
    assert_eq!(colour.alpha(), 4);
    // Components are bytes; the constructor must not normalise or swap them.
    assert_eq!(Rgb::new(255, 0, 128, 255).green(), 0);
}

#[test]
fn blackness_ignores_alpha_and_the_average_truncates() {
    // `is_black` (`sdl_utils.h:27`) looks at r, g and b only.
    assert!(Rgb::new(0, 0, 0, 0).is_black());
    assert!(Rgb::new(0, 0, 0, 255).is_black());
    assert!(!Rgb::new(0, 0, 1, 255).is_black());
    assert!(!Rgb::new(0, 1, 0, 255).is_black());
    assert!(!Rgb::new(1, 0, 0, 255).is_black());

    // `average_pixel_color` is `85 * (r + g + b) >> 8` (`sdl_utils.h:39`):
    // 85/256 is a shade under one third, so full white averages to 254, not 255.
    assert_eq!(Rgb::new(255, 255, 255, 255).average_pixel_color(), 254);
    assert_eq!(Rgb::new(0, 0, 0, 255).average_pixel_color(), 0);
    // The shift truncates: one unit of red rounds away to nothing.
    assert_eq!(Rgb::new(1, 0, 0, 255).average_pixel_color(), 0);
    assert_eq!(Rgb::new(255, 0, 0, 255).average_pixel_color(), 84);
}

#[test]
fn brightness_scales_channels_and_saturates_at_255() {
    let colour = Rgb::new(100, 150, 255, 255);
    // 255 * 150 / 100 = 382, clamped to 255; the others scale exactly.
    assert_eq!(
        adjust_color_brightness(colour, 150),
        Rgb::new(150, 225, 255, 255)
    );
    // 100% is the identity, alpha included.
    assert_eq!(adjust_color_brightness(colour, 100), colour);
    // Zero and below are black, but keep the alpha.
    assert_eq!(adjust_color_brightness(colour, 0), Rgb::new(0, 0, 0, 255));
    assert_eq!(adjust_color_brightness(colour, -10), Rgb::new(0, 0, 0, 255));
    // Integer division truncates: 3 * 50 / 100 is 1, and 1 * 50 / 100 is 0.
    assert_eq!(
        adjust_color_brightness(Rgb::new(3, 1, 0, 7), 50),
        Rgb::new(1, 0, 0, 7)
    );
}

#[test]
fn mixing_truncates_and_mixes_alpha_too() {
    let transparent_black = Rgb::new(0, 0, 0, 0);
    let white = Rgb::new(255, 255, 255, 255);
    // (0 * 50 + 255 * 50) / 100 = 127: truncated, not rounded to 128.
    assert_eq!(
        mix_colors(transparent_black, white, 50),
        Rgb::new(127, 127, 127, 127)
    );
    // 1% of 255 is 2.55, truncated to 2.
    assert_eq!(
        mix_colors(transparent_black, white, 1),
        Rgb::new(2, 2, 2, 2)
    );
    // The ends are returned whole, alpha included: no arithmetic is done.
    assert_eq!(mix_colors(transparent_black, white, 0), transparent_black);
    assert_eq!(mix_colors(transparent_black, white, 100), white);
    assert_eq!(mix_colors(transparent_black, white, -5), transparent_black);
    assert_eq!(mix_colors(transparent_black, white, 150), white);
}

#[test]
fn interpolating_reproduces_the_documented_white_to_blue_example() {
    // The reference documents this exact case in `src/sdl_utils.h:96`.
    let white = Rgb::new(255, 255, 255, 255);
    let blue = Rgb::new(0, 0, 255, 255);
    assert_eq!(
        color_linear_interpolate(white, blue, 2),
        vec![
            white,
            Rgb::new(170, 170, 255, 255),
            Rgb::new(85, 85, 255, 255),
            blue,
        ]
    );
    // With no steps in between, only the two ends come back, unchanged.
    assert_eq!(color_linear_interpolate(white, blue, 0), vec![white, blue]);
}

#[test]
fn interpolating_truncates_and_rewrites_alpha() {
    // 1.5 truncates to 1 rather than rounding up to 2.
    assert_eq!(
        color_linear_interpolate(Rgb::new(0, 0, 0, 0), Rgb::new(3, 3, 3, 3), 1),
        vec![
            Rgb::new(0, 0, 0, 0),
            Rgb::new(1, 1, 1, 255),
            Rgb::new(3, 3, 3, 3),
        ]
    );
}

#[test]
fn attribute_packs_the_pair_index_with_bold_and_blink() {
    // Values from `src/cursesport.cpp:516`: pair index in bits 17..25, blink at
    // 0x800, bold at 0x2000.
    let red = NcColor::from_color_pair_index(2);
    assert_eq!(red.to_int(), 2 << 17);
    assert_eq!(red.to_color_pair_index(), 2);
    assert!(!red.is_bold());
    assert!(!red.is_blink());

    let bold = red.bold();
    assert_eq!(bold.to_int(), (2 << 17) | A_BOLD);
    assert_eq!(bold.to_color_pair_index(), 2);
    assert!(bold.is_bold());
    assert!(!bold.is_blink());
    // Adding the same attribute twice changes nothing.
    assert_eq!(bold.bold(), bold);

    let blinking = red.blink();
    assert_eq!(blinking.to_int(), (2 << 17) | A_BLINK);
    assert!(blinking.is_blink());
    assert!(!blinking.is_bold());
    assert_eq!(red.bold().blink(), red.blink().bold());

    // The pair index field is nine bits wide, so 511 is the last that survives.
    assert_eq!(
        NcColor::from_color_pair_index(511).to_color_pair_index(),
        511
    );
    assert_eq!(NcColor::from_color_pair_index(512).to_color_pair_index(), 0);
}

#[test]
fn equality_hashing_and_order_look_at_the_attribute_only() {
    // `src/color.h:400` compares `attribute_value` and nothing else, so a colour
    // built from a pair index and one built from raw bits are the same colour
    // even though they remember different pair indices.
    let from_pair = NcColor::from_color_pair_index(2);
    let from_bits = NcColor::from_attribute(2 << 17);
    assert_ne!(from_pair.index(), from_bits.index());
    assert_eq!(from_pair, from_bits);
    assert_eq!(from_pair.cmp(&from_bits), Ordering::Equal);

    let mut seen = HashSet::new();
    seen.insert(from_pair);
    seen.insert(from_bits);
    assert_eq!(seen.len(), 1);

    assert!(NcColor::from_color_pair_index(1) < NcColor::from_color_pair_index(2));
    assert_eq!(NcColor::default().to_int(), 0);
}

#[test]
fn table_covers_every_colour_the_reference_declares() {
    // `num_colors` is 145: 16 c_, 16 h_, 16 i_, unset, and six 16-entry
    // foreground/background families.
    assert_eq!(COLORS.len(), NUM_COLORS);
    assert_eq!(NUM_COLORS, 145);
    // The table is in declaration order, so an id doubles as its row number.
    assert!(COLORS.iter().enumerate().all(|(i, d)| d.id.index() == i));
    // Every name is distinct: no duplicate spells an id twice.
    assert_eq!(
        COLORS.iter().map(|d| d.name).collect::<HashSet<_>>().len(),
        NUM_COLORS
    );
    // Every colour names an invert that exists, and every pair is registered.
    assert!(COLORS.iter().all(|d| color_def(d.invert).is_some()));
    assert!(
        COLORS
            .iter()
            .all(|d| COLOR_PAIRS.get(usize::from(d.pair)).is_some())
    );
    assert_eq!(COLOR_PAIRS.len(), 88);
}

#[test]
fn table_rows_match_the_reference() {
    // Light gray shares white's pair and differs only by the bold bit, which is
    // why the two look different on screen.
    assert_eq!(
        color_def(ColorId::CWhite).map(|d| (d.pair, d.bold, d.invert)),
        Some((1, true, ColorId::IWhite))
    );
    assert_eq!(
        color_def(ColorId::CLightGray).map(|d| (d.pair, d.bold)),
        Some((1, false))
    );
    assert_eq!(
        color_value(ColorId::CWhite),
        Some(NcColor::from_color_pair_index(1).bold())
    );
    assert_eq!(
        color_value(ColorId::CLightGray),
        Some(NcColor::from_color_pair_index(1))
    );

    assert_eq!(color_def(ColorId::CRed).map(|d| d.pair), Some(2));
    assert_eq!(color_def(ColorId::HRed).map(|d| d.pair), Some(16));
    assert_eq!(color_def(ColorId::CUnset).map(|d| d.pair), Some(31));
    // Inverted colours blink instead of bolding.
    assert_eq!(
        color_def(ColorId::IWhite).map(|d| (d.pair, d.blink, d.bold)),
        Some((8, true, false))
    );

    // Dark gray is the one family that depends on the terminal: pairs 72-79 on a
    // 256-colour terminal, the bold base eight otherwise.
    assert_eq!(color_def(ColorId::CDarkGray).map(|d| d.pair), Some(72));
    assert_eq!(DARK_GRAY_FALLBACK.len(), 9);
    assert_eq!(DARK_GRAY_FALLBACK.first().map(|d| d.pair), Some(30));
    assert_eq!(DARK_GRAY_FALLBACK.first().map(|d| d.bold), Some(true));

    assert_eq!(color_from_name("c_red"), Some(ColorId::CRed));
    assert_eq!(color_from_name("nope"), None);
    assert_eq!(color_name(ColorId::CUnset), Some("c_unset"));
}

#[test]
fn colour_names_accept_the_reference_spellings() {
    // Bare names get the `c_` prefix.
    assert_eq!(
        color_from_string("red"),
        Ok(color_value(ColorId::CRed).unwrap_or_default())
    );
    // Explicit prefixes are kept as written.
    assert_eq!(color_from_string("c_red"), color_from_string("red"));
    assert_eq!(
        color_from_string("h_red"),
        Ok(color_value(ColorId::HRed).unwrap_or_default())
    );
    assert_eq!(
        color_from_string("i_white"),
        Ok(color_value(ColorId::IWhite).unwrap_or_default())
    );
    // The deprecated short forms are rewritten to `light_` and `dark_`.
    assert_eq!(color_from_string("ltgray"), color_from_string("light_gray"));
    assert_eq!(color_from_string("dkgray"), color_from_string("dark_gray"));
    assert_eq!(
        color_from_string("dkgray"),
        color_from_string("c_dark_gray")
    );
    // Pair names work with or without the prefix.
    assert_eq!(
        color_from_string("light_blue_red"),
        color_from_string("c_light_blue_red")
    );
    // `unset` is a colour with a name of its own, pair 31.
    assert_eq!(
        color_from_string("unset"),
        Ok(NcColor::from_color_pair_index(31))
    );
}

#[test]
fn colour_names_reject_anything_the_reference_does_not_know() {
    // The reference answers every one of these with `c_unset` (plus a debugmsg).
    assert_eq!(color_from_string(""), Err(ColorError::UnknownName));
    assert_eq!(color_from_string("#ff0000"), Err(ColorError::UnknownName));
    assert_eq!(
        color_from_string("nosuchcolour"),
        Err(ColorError::UnknownName)
    );
    // Names are case sensitive.
    assert_eq!(color_from_string("Red"), Err(ColorError::UnknownName));
    // The one-letter note codes are not colour names.
    assert_eq!(color_from_string("r"), Err(ColorError::UnknownName));
    // A stray underscore is not a prefix.
    assert_eq!(color_from_string("z_red"), Err(ColorError::UnknownName));
}

/// The value of a colour id, for comparisons.
fn value(id: ColorId) -> NcColor {
    color_value(id).unwrap_or_default()
}

#[test]
fn every_colour_but_unset_names_a_value_that_maps_back() {
    for def in COLORS {
        if def.id == ColorId::CUnset {
            continue;
        }
        let colour = value(def.id);
        assert_eq!(
            color_from_string(string_from_color(colour)),
            Ok(colour),
            "{} did not round-trip",
            def.name
        );
    }
}

#[test]
fn unset_and_unknown_colours_print_as_white() {
    // `string_from_color` (`src/color.cpp:671`) answers "c_unset" with "c_white".
    assert_eq!(string_from_color(value(ColorId::CUnset)), "c_white");
    // A default-constructed colour is in no table, so it takes the same path.
    assert_eq!(string_from_color(NcColor::default()), "c_white");
    assert_eq!(string_from_color(value(ColorId::CRed)), "c_red");
}

#[test]
fn three_names_are_not_round_trip_stable() {
    // The reference's table has three pairs of colours with identical attributes,
    // and its reverse map keeps the last one. Values round-trip; names do not.
    assert_eq!(string_from_color(value(ColorId::HBlack)), "h_blue");
    assert_eq!(string_from_color(value(ColorId::IBlack)), "c_black_white");
    assert_eq!(string_from_color(value(ColorId::IRed)), "c_red_red");
    assert_eq!(value(ColorId::HBlack), value(ColorId::HBlue));
    assert_eq!(value(ColorId::IBlack), value(ColorId::CBlackWhite));
    assert_eq!(value(ColorId::IRed), value(ColorId::CRedRed));
}

#[test]
fn backgrounds_prefix_i_and_fall_back_to_inverted_white() {
    // `bgcolor_from_string` (`src/color.cpp:689`) always adds the `i_` prefix and
    // answers a miss with i_white, not with unset.
    assert_eq!(bgcolor_from_string("red"), value(ColorId::IRed));
    assert_eq!(bgcolor_from_string("dkgray"), value(ColorId::IDarkGray));
    assert_eq!(bgcolor_from_string("nosuch"), value(ColorId::IWhite));
    assert_eq!(bgcolor_from_string(""), value(ColorId::IWhite));
    // There is no `bg_` prefix anywhere in the reference: a whole name is used,
    // so `c_red` becomes the unknown `i_c_red`.
    assert_eq!(bgcolor_from_string("c_red"), value(ColorId::IWhite));
}

#[test]
fn colour_tags_parse_into_three_outcomes() {
    assert_eq!(
        get_color_from_tag("<color_red>"),
        TagParse::Open(value(ColorId::CRed))
    );
    assert_eq!(get_color_from_tag("</color>"), TagParse::Close);
    assert_eq!(get_color_from_tag("<color_nosuch>"), TagParse::NonColor);
    assert_eq!(get_color_from_tag("just text"), TagParse::NonColor);
    // An unterminated tag is not a tag.
    assert_eq!(get_color_from_tag("<color_red"), TagParse::NonColor);
    assert_eq!(get_color_from_tag(""), TagParse::NonColor);
    assert_eq!(get_color_from_tag("<b>"), TagParse::NonColor);
    // `unset` parses as a colour but is not an openable one.
    assert_eq!(get_color_from_tag("<color_unset>"), TagParse::NonColor);
}

#[test]
fn tags_and_colourized_text_round_trip() {
    assert_eq!(get_tag_from_color(value(ColorId::CRed)), "<color_c_red>");
    // Unset has no tag of its own; it borrows white's.
    assert_eq!(
        get_tag_from_color(value(ColorId::CUnset)),
        "<color_c_white>"
    );
    assert_eq!(
        colorize("hi", value(ColorId::CRed)),
        "<color_c_red>hi</color>"
    );
    // Each line is wrapped separately and the newline stays outside the tags.
    assert_eq!(
        colorize("a\nb", value(ColorId::CRed)),
        "<color_c_red>a</color>\n<color_c_red>b</color>"
    );
    // A trailing newline produces a wrapped empty last line.
    assert_eq!(
        colorize("a\n", value(ColorId::CRed)),
        "<color_c_red>a</color>\n<color_c_red></color>"
    );
}

#[test]
fn highlighting_reaches_the_blue_and_background_families() {
    // `finalize` (`src/color.cpp:55`) caches `h_<name>` for the blue highlight and
    // `<name>_<bg>` for the coloured backgrounds, but only for plain `c_` names.
    assert_eq!(hilite(value(ColorId::CRed)), value(ColorId::HRed));
    assert_eq!(hilite(value(ColorId::CWhite)), value(ColorId::HWhite));
    // A `light_`/`dark_` prefix does not count as a pair suffix of its own.
    assert_eq!(hilite(value(ColorId::CLightRed)), value(ColorId::HLightRed));
    // Pair colours have no highlight, so the fallback applies.
    assert_eq!(hilite(value(ColorId::CRedRed)), value(ColorId::HWhite));
    assert_eq!(hilite(NcColor::default()), value(ColorId::HWhite));

    assert_eq!(
        red_background(value(ColorId::CRed)),
        value(ColorId::CRedRed)
    );
    assert_eq!(
        white_background(value(ColorId::CWhite)),
        value(ColorId::CWhiteWhite)
    );
    assert_eq!(
        green_background(value(ColorId::CBlack)),
        value(ColorId::CBlackGreen)
    );
    assert_eq!(
        yellow_background(value(ColorId::CYellow)),
        value(ColorId::CYellowYellow)
    );
    assert_eq!(
        magenta_background(value(ColorId::CPink)),
        value(ColorId::CPinkMagenta)
    );
    assert_eq!(
        cyan_background(value(ColorId::CCyan)),
        value(ColorId::CCyanCyan)
    );
    // A pair colour falls back to the default background colour for that pair.
    assert_eq!(
        red_background(value(ColorId::CRedRed)),
        value(ColorId::CWhiteRed)
    );
}

#[test]
fn inversion_follows_the_table_and_unset_inverts_to_itself() {
    assert_eq!(invert_color(value(ColorId::CWhite)), value(ColorId::IWhite));
    assert_eq!(invert_color(value(ColorId::IWhite)), value(ColorId::CWhite));
    assert_eq!(invert_color(value(ColorId::HRed)), value(ColorId::CBlueRed));
    // An unknown colour resolves through `c_unset`, which inverts to itself, so
    // the reference's `c_pink` fallback is unreachable through this path.
    assert_eq!(invert_color(NcColor::default()), value(ColorId::CUnset));
}

#[test]
fn graphics_palette_maps_pairs_to_rgb() {
    // `curses_color_to_SDL` (`src/sdl_utils.cpp:151`) picks the foreground palette
    // entry, or the background one when the foreground is black, then adds eight
    // for bold.
    assert_eq!(
        curses_color_to_rgb(value(ColorId::CRed)),
        Some(Rgb::new(255, 0, 0, 255))
    );
    assert_eq!(
        curses_color_to_rgb(value(ColorId::CLightRed)),
        Some(Rgb::new(255, 150, 150, 255))
    );
    // White and light gray differ only by the bold bit, so one is GRAY and the
    // other WHITE.
    assert_eq!(
        curses_color_to_rgb(value(ColorId::CLightGray)),
        Some(Rgb::new(150, 150, 150, 255))
    );
    assert_eq!(
        curses_color_to_rgb(value(ColorId::CWhite)),
        Some(Rgb::new(255, 255, 255, 255))
    );
    // A black foreground yields to the background colour.
    assert_eq!(
        curses_color_to_rgb(value(ColorId::CBlackGreen)),
        Some(Rgb::new(0, 110, 0, 255))
    );
    assert_eq!(
        curses_color_to_rgb(value(ColorId::CBlack)),
        Some(Rgb::new(0, 0, 0, 255))
    );
    // An inverted colour is only a pair swap, so its foreground RGB is the same
    // as the plain colour's: the graphics build loses the inversion here.
    assert_eq!(
        curses_color_to_rgb(value(ColorId::IRed)),
        curses_color_to_rgb(value(ColorId::CRed))
    );
    // Pair indices no colour uses have no palette entry.
    assert_eq!(
        curses_color_to_rgb(NcColor::from_color_pair_index(500)),
        None
    );
}

#[test]
fn pixel_filters_truncate_and_never_go_black() {
    // The graphics build's tile filters (`src/sdl_utils.cpp:80-126`) all work in
    // integers and clamp their floors to one, so a lit pixel never turns black.
    assert_eq!(
        color_pixel_grayscale(Rgb::new(128, 128, 64, 255)),
        Rgb::new(66, 66, 66, 255)
    );
    assert_eq!(
        color_pixel_darken(Rgb::new(200, 100, 50, 7)),
        Rgb::new(66, 33, 16, 7)
    );
    assert_eq!(
        color_pixel_nightvision(Rgb::new(100, 100, 100, 255)),
        Rgb::new(17, 69, 8, 255)
    );
    assert_eq!(
        color_pixel_overexposed(Rgb::new(100, 100, 100, 255)),
        Rgb::new(36, 147, 18, 255)
    );
    // Pure black short-circuits, alpha and all.
    assert_eq!(
        color_pixel_grayscale(Rgb::new(0, 0, 0, 5)),
        Rgb::new(0, 0, 0, 5)
    );
    assert_eq!(
        color_pixel_darken(Rgb::new(0, 0, 0, 9)),
        Rgb::new(0, 0, 0, 9)
    );
}

#[test]
fn the_sepia_mixer_rounds_its_gamma_step() {
    // `color_pixel_mixer` (`src/sdl_utils.cpp:134`) rounds `pow` to the nearest
    // step weight instead of truncating it: 127/255 ** 1.6 * 150 is 49.19.
    assert_eq!(
        color_pixel_sepia_light(Rgb::new(128, 128, 128, 255)),
        Rgb::new(137, 119, 89, 255)
    );
}
