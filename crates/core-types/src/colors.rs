//! Colour definitions: RGB, named colours, and the 256-colour table.
//!
//! Reference: `src/color.h`, `src/color.cpp`.
//!
//! The reference splits its colour layer in two, and this module keeps the
//! split:
//!
//! * The attribute layer — [`NcColor`], [`ColorId`] and the name table — is the
//!   terminal build. A colour there is a packed ncurses attribute: a colour-pair
//!   index plus bold and blink bits. It has no RGB values at all; the terminal
//!   picks the pixels.
//! * The RGB layer — [`Rgb`] and its arithmetic — lives in the graphics build
//!   (`src/sdl_utils.h`, `src/sdl_utils.cpp`). It is the only place in the
//!   reference where colour channels are numbers.
//!
//! Everything here is pure: no game state, no I/O, and every failure is a value.

/// A four-channel colour, one byte per channel.
///
/// Reference: `SDL_Color` as used by `src/sdl_utils.h`. Alpha is carried but
/// most of the reference's arithmetic ignores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rgb {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Rgb {
    /// Builds a colour from its red, green, blue and alpha components.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// The red component.
    #[must_use]
    pub const fn red(self) -> u8 {
        self.r
    }

    /// The green component.
    #[must_use]
    pub const fn green(self) -> u8 {
        self.g
    }

    /// The blue component.
    #[must_use]
    pub const fn blue(self) -> u8 {
        self.b
    }

    /// The alpha component.
    #[must_use]
    pub const fn alpha(self) -> u8 {
        self.a
    }

    /// Whether the colour is pure black, ignoring alpha.
    ///
    /// Reference: `is_black` (`src/sdl_utils.h:27`) — alpha is not consulted, so
    /// a fully transparent black counts as black.
    #[must_use]
    pub const fn is_black(self) -> bool {
        self.r == 0 && self.g == 0 && self.b == 0
    }

    /// The colour's brightness as a single channel: `85 * (r + g + b) >> 8`.
    ///
    /// Reference: `average_pixel_color` (`src/sdl_utils.h:39`). 85/256 is just
    /// under a third, so white averages to 254 and small values truncate to zero.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // Capped at 254: 85 * 3 * 255 >> 8.
    pub const fn average_pixel_color(self) -> u8 {
        ((85 * (self.r as u32 + self.g as u32 + self.b as u32)) >> 8) as u8
    }
}

/// Scales each channel by a percentage, saturating at 255 and keeping alpha.
///
/// Reference: `adjust_color_brightness` (`src/sdl_utils.cpp:34`). Percentages at
/// or below zero give black; 100 is the identity. The division truncates, so a
/// 50% scale of 3 is 1, not 2.
#[must_use]
#[allow(clippy::cast_possible_truncation)] // Clamped by `min` before the cast.
#[allow(clippy::cast_sign_loss)] // The percentage is clamped positive above.
pub fn adjust_color_brightness(color: Rgb, percent: i32) -> Rgb {
    if percent <= 0 {
        return Rgb::new(0, 0, 0, color.alpha());
    }
    if percent == 100 {
        return color;
    }
    let scale = |channel: u8| {
        let scaled = (i32::from(channel) * percent / 100).min(0xFF);
        scaled as u8
    };
    Rgb::new(
        scale(color.red()),
        scale(color.green()),
        scale(color.blue()),
        color.alpha(),
    )
}

/// Blends `second` over `first` by weight, channel by channel, alpha included.
///
/// Reference: `mix_colors` (`src/sdl_utils.cpp:53`). Weights at or below zero
/// return `first` whole; at or above 100 they return `second` whole. In between
/// the blend truncates rather than rounds: a 50/50 blend of 0 and 255 is 127.
#[must_use]
#[allow(clippy::cast_possible_truncation)] // Both weights are clamped to 0..=100.
#[allow(clippy::cast_sign_loss)] // The result is a weighted average of 0..=255.
pub fn mix_colors(first: Rgb, second: Rgb, second_percent: i32) -> Rgb {
    if second_percent <= 0 {
        return first;
    }
    if second_percent >= 100 {
        return second;
    }
    let first_percent = 100 - second_percent;
    let mix = |a: u8, b: u8| {
        let blended = (i32::from(a) * first_percent + i32::from(b) * second_percent) / 100;
        blended.min(0xFF) as u8
    };
    Rgb::new(
        mix(first.red(), second.red()),
        mix(first.green(), second.green()),
        mix(first.blue(), second.blue()),
        mix(first.alpha(), second.alpha()),
    )
}

/// One channel of an interpolation step.
///
/// The reference (`src/sdl_utils.cpp:172-185`) casts a possibly negative float
/// straight to a byte, which is undefined behaviour in C++ and only produces the
/// documented values because the platform wraps modulo 256. `rem_euclid` is that
/// same wrap, made well defined.
#[allow(clippy::cast_possible_truncation)] // Truncation is the behaviour being ported.
#[allow(clippy::cast_precision_loss)] // Channels are 0..=255, exact in f32.
fn interpolated_channel(start: u8, end: u8, percent: f32) -> u8 {
    let delta = i32::from(end) - i32::from(start);
    let scaled = (percent * delta as f32) as i32;
    ((i32::from(start) + scaled.rem_euclid(256)).rem_euclid(256)) as u8
}

/// Interpolates `additional_steps` colours between two ends, inclusive of both.
///
/// Reference: `color_linear_interpolate` (`src/sdl_utils.cpp:169`). Steps are
/// spaced by an f32 fraction, so the arithmetic truncates rather than rounds,
/// and the intermediate alpha is always 255 no matter what the ends carry.
#[must_use]
#[allow(clippy::cast_possible_truncation)] // Step counts this large are not real.
#[allow(clippy::cast_precision_loss)] // Step counts this large are not exact anyway.
pub fn color_linear_interpolate(start: Rgb, end: Rgb, additional_steps: u32) -> Vec<Rgb> {
    let steps = additional_steps.saturating_add(2);
    let mut colors = Vec::with_capacity(steps as usize);
    colors.push(start);
    let step = 1.0_f32 / (steps - 1) as f32;
    for i in 0..steps - 2 {
        let percent = (i + 1) as f32 * step;
        colors.push(Rgb::new(
            interpolated_channel(start.red(), end.red(), percent),
            interpolated_channel(start.green(), end.green(), percent),
            interpolated_channel(start.blue(), end.blue(), percent),
            255,
        ));
    }
    colors.push(end);
    colors
}

/// The blink bit of an ncurses attribute (`src/cursesport.cpp:516`).
pub const A_BLINK: i32 = 0x0000_0800;

/// The bold bit of an ncurses attribute (`src/cursesport.cpp:517`).
pub const A_BOLD: i32 = 0x0000_2000;

/// The mask holding the colour-pair index (`src/cursesport.cpp:518`).
///
/// Nine bits wide at offset 17, so pair indices 0 through 511 survive a
/// round trip and anything above them is silently truncated.
pub const A_COLOR: i32 = 0x03fe_0000;

/// A colour as the terminal build knows it: a packed ncurses attribute.
///
/// Reference: `class nc_color` (`src/color.h:358`). The attribute holds a
/// colour-pair index plus the bold and blink bits; the terminal resolves the
/// pair to actual pixels. This type therefore says nothing about RGB — see
/// [`Rgb`] and [`curses_color_to_rgb`] for the graphics build's view.
///
/// Comparison, ordering and hashing use the attribute alone (`src/color.h:400`,
/// `src/color.h:415`), ignoring the remembered pair index — which is exactly the
/// field the imgui build reads, so the two disagree about what makes a colour
/// distinct.
#[derive(Debug, Clone, Copy, Default)]
pub struct NcColor {
    attribute: i32,
    index: u16,
}

impl NcColor {
    /// Builds a colour from a raw attribute, remembering pair index zero.
    ///
    /// Reference: the private `nc_color( int a )` constructor (`src/color.h:365`).
    #[must_use]
    pub const fn from_attribute(attribute: i32) -> Self {
        Self {
            attribute,
            index: 0,
        }
    }

    /// Builds a colour from a colour-pair index.
    ///
    /// Reference: `nc_color::from_color_pair_index` (`src/cursesport.cpp:520`),
    /// which shifts the index into [`A_COLOR`]. The index is masked to nine bits
    /// first, which is what the reference's mask does anyway but without the
    /// signed overflow the C++ version has for indices above 16383.
    #[must_use]
    pub const fn from_color_pair_index(index: u16) -> Self {
        Self {
            attribute: (index as i32 & 0x1FF) << 17,
            index,
        }
    }

    /// The raw ncurses attribute (`to_int`, `src/color.h:383`).
    #[must_use]
    pub const fn to_int(self) -> i32 {
        self.attribute
    }

    /// The colour-pair index packed into the attribute.
    ///
    /// Reference: `to_color_pair_index` (`src/cursesport.cpp:525`).
    #[must_use]
    pub const fn to_color_pair_index(self) -> i32 {
        (self.attribute & A_COLOR) >> 17
    }

    /// The pair index the colour was built from, separately from the attribute.
    ///
    /// Reference: `get_index` (`src/color.h:386`). Only the ncurses build fills
    /// this in; `src/cursesport.cpp:520` leaves it at zero.
    #[must_use]
    pub const fn index(self) -> u16 {
        self.index
    }

    /// This colour plus [`A_BOLD`].
    ///
    /// Reference: `bold` (`src/color.h:391`).
    #[must_use]
    pub const fn bold(self) -> Self {
        Self {
            attribute: self.attribute | A_BOLD,
            index: self.index,
        }
    }

    /// Whether [`A_BOLD`] is set.
    #[must_use]
    pub const fn is_bold(self) -> bool {
        self.attribute & A_BOLD != 0
    }

    /// This colour plus [`A_BLINK`].
    ///
    /// Reference: `blink` (`src/color.h:394`).
    #[must_use]
    pub const fn blink(self) -> Self {
        Self {
            attribute: self.attribute | A_BLINK,
            index: self.index,
        }
    }

    /// Whether [`A_BLINK`] is set.
    #[must_use]
    pub const fn is_blink(self) -> bool {
        self.attribute & A_BLINK != 0
    }
}

impl PartialEq for NcColor {
    fn eq(&self, other: &Self) -> bool {
        self.attribute == other.attribute
    }
}

impl Eq for NcColor {}

impl core::hash::Hash for NcColor {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.attribute.hash(state);
    }
}

impl Ord for NcColor {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.attribute.cmp(&other.attribute)
    }
}

impl PartialOrd for NcColor {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl ColorId {
    /// The id's position in the declaration order, which is also its row in
    /// [`COLORS`].
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// The attribute a table row builds: its pair plus its bold and blink bits.
fn built_color(def: &ColorDef) -> NcColor {
    let mut color = NcColor::from_color_pair_index(def.pair);
    if def.bold {
        color = color.bold();
    }
    if def.blink {
        color = color.blink();
    }
    color
}

/// The colour a given id stands for with default settings.
///
/// Reference: `color_manager::get` (`src/color.cpp:151`) in its
/// no-custom-colour case.
#[must_use]
pub fn color_value(id: ColorId) -> Option<NcColor> {
    color_def(id).map(built_color)
}

/// The row for a colour id.
///
/// The table is in declaration order, so this is a binary search rather than a
/// scan; `None` only if the id is somehow missing from the table.
#[must_use]
pub fn color_def(id: ColorId) -> Option<&'static ColorDef> {
    COLORS
        .binary_search_by_key(&id, |def| def.id)
        .ok()
        .and_then(|at| COLORS.get(at))
}

/// The id registered under a name, exactly as spelled.
///
/// Reference: `color_manager::name_to_id` (`src/color.cpp:113`), minus the
/// `debugmsg` it emits for a miss.
#[must_use]
pub fn color_from_name(name: &str) -> Option<ColorId> {
    COLORS.iter().find(|def| def.name == name).map(|def| def.id)
}

/// The canonical name of a colour id.
///
/// Reference: `color_manager::id_to_name` (`src/color.cpp:127`).
#[must_use]
pub fn color_name(id: ColorId) -> Option<&'static str> {
    color_def(id).map(|def| def.name)
}
/// Every colour id the reference declares, in declaration order (`enum color_id`, `src/color.h:179`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u16)]
pub enum ColorId {
    /// `def_c_black`
    CBlack = 0,
    /// `def_c_white`
    CWhite = 1,
    /// `def_c_light_gray`
    CLightGray = 2,
    /// `def_c_dark_gray`
    CDarkGray = 3,
    /// `def_c_red`
    CRed = 4,
    /// `def_c_green`
    CGreen = 5,
    /// `def_c_blue`
    CBlue = 6,
    /// `def_c_cyan`
    CCyan = 7,
    /// `def_c_magenta`
    CMagenta = 8,
    /// `def_c_brown`
    CBrown = 9,
    /// `def_c_light_red`
    CLightRed = 10,
    /// `def_c_light_green`
    CLightGreen = 11,
    /// `def_c_light_blue`
    CLightBlue = 12,
    /// `def_c_light_cyan`
    CLightCyan = 13,
    /// `def_c_pink`
    CPink = 14,
    /// `def_c_yellow`
    CYellow = 15,
    /// `def_h_black`
    HBlack = 16,
    /// `def_h_white`
    HWhite = 17,
    /// `def_h_light_gray`
    HLightGray = 18,
    /// `def_h_dark_gray`
    HDarkGray = 19,
    /// `def_h_red`
    HRed = 20,
    /// `def_h_green`
    HGreen = 21,
    /// `def_h_blue`
    HBlue = 22,
    /// `def_h_cyan`
    HCyan = 23,
    /// `def_h_magenta`
    HMagenta = 24,
    /// `def_h_brown`
    HBrown = 25,
    /// `def_h_light_red`
    HLightRed = 26,
    /// `def_h_light_green`
    HLightGreen = 27,
    /// `def_h_light_blue`
    HLightBlue = 28,
    /// `def_h_light_cyan`
    HLightCyan = 29,
    /// `def_h_pink`
    HPink = 30,
    /// `def_h_yellow`
    HYellow = 31,
    /// `def_i_black`
    IBlack = 32,
    /// `def_i_white`
    IWhite = 33,
    /// `def_i_light_gray`
    ILightGray = 34,
    /// `def_i_dark_gray`
    IDarkGray = 35,
    /// `def_i_red`
    IRed = 36,
    /// `def_i_green`
    IGreen = 37,
    /// `def_i_blue`
    IBlue = 38,
    /// `def_i_cyan`
    ICyan = 39,
    /// `def_i_magenta`
    IMagenta = 40,
    /// `def_i_brown`
    IBrown = 41,
    /// `def_i_light_red`
    ILightRed = 42,
    /// `def_i_light_green`
    ILightGreen = 43,
    /// `def_i_light_blue`
    ILightBlue = 44,
    /// `def_i_light_cyan`
    ILightCyan = 45,
    /// `def_i_pink`
    IPink = 46,
    /// `def_i_yellow`
    IYellow = 47,
    /// `def_c_unset`
    CUnset = 48,
    /// `def_c_black_red`
    CBlackRed = 49,
    /// `def_c_white_red`
    CWhiteRed = 50,
    /// `def_c_light_gray_red`
    CLightGrayRed = 51,
    /// `def_c_dark_gray_red`
    CDarkGrayRed = 52,
    /// `def_c_red_red`
    CRedRed = 53,
    /// `def_c_green_red`
    CGreenRed = 54,
    /// `def_c_blue_red`
    CBlueRed = 55,
    /// `def_c_cyan_red`
    CCyanRed = 56,
    /// `def_c_magenta_red`
    CMagentaRed = 57,
    /// `def_c_brown_red`
    CBrownRed = 58,
    /// `def_c_light_red_red`
    CLightRedRed = 59,
    /// `def_c_light_green_red`
    CLightGreenRed = 60,
    /// `def_c_light_blue_red`
    CLightBlueRed = 61,
    /// `def_c_light_cyan_red`
    CLightCyanRed = 62,
    /// `def_c_pink_red`
    CPinkRed = 63,
    /// `def_c_yellow_red`
    CYellowRed = 64,
    /// `def_c_black_white`
    CBlackWhite = 65,
    /// `def_c_dark_gray_white`
    CDarkGrayWhite = 66,
    /// `def_c_light_gray_white`
    CLightGrayWhite = 67,
    /// `def_c_white_white`
    CWhiteWhite = 68,
    /// `def_c_red_white`
    CRedWhite = 69,
    /// `def_c_light_red_white`
    CLightRedWhite = 70,
    /// `def_c_green_white`
    CGreenWhite = 71,
    /// `def_c_light_green_white`
    CLightGreenWhite = 72,
    /// `def_c_brown_white`
    CBrownWhite = 73,
    /// `def_c_yellow_white`
    CYellowWhite = 74,
    /// `def_c_blue_white`
    CBlueWhite = 75,
    /// `def_c_light_blue_white`
    CLightBlueWhite = 76,
    /// `def_c_magenta_white`
    CMagentaWhite = 77,
    /// `def_c_pink_white`
    CPinkWhite = 78,
    /// `def_c_cyan_white`
    CCyanWhite = 79,
    /// `def_c_light_cyan_white`
    CLightCyanWhite = 80,
    /// `def_c_black_green`
    CBlackGreen = 81,
    /// `def_c_dark_gray_green`
    CDarkGrayGreen = 82,
    /// `def_c_light_gray_green`
    CLightGrayGreen = 83,
    /// `def_c_white_green`
    CWhiteGreen = 84,
    /// `def_c_red_green`
    CRedGreen = 85,
    /// `def_c_light_red_green`
    CLightRedGreen = 86,
    /// `def_c_green_green`
    CGreenGreen = 87,
    /// `def_c_light_green_green`
    CLightGreenGreen = 88,
    /// `def_c_brown_green`
    CBrownGreen = 89,
    /// `def_c_yellow_green`
    CYellowGreen = 90,
    /// `def_c_blue_green`
    CBlueGreen = 91,
    /// `def_c_light_blue_green`
    CLightBlueGreen = 92,
    /// `def_c_magenta_green`
    CMagentaGreen = 93,
    /// `def_c_pink_green`
    CPinkGreen = 94,
    /// `def_c_cyan_green`
    CCyanGreen = 95,
    /// `def_c_light_cyan_green`
    CLightCyanGreen = 96,
    /// `def_c_black_yellow`
    CBlackYellow = 97,
    /// `def_c_dark_gray_yellow`
    CDarkGrayYellow = 98,
    /// `def_c_light_gray_yellow`
    CLightGrayYellow = 99,
    /// `def_c_white_yellow`
    CWhiteYellow = 100,
    /// `def_c_red_yellow`
    CRedYellow = 101,
    /// `def_c_light_red_yellow`
    CLightRedYellow = 102,
    /// `def_c_green_yellow`
    CGreenYellow = 103,
    /// `def_c_light_green_yellow`
    CLightGreenYellow = 104,
    /// `def_c_brown_yellow`
    CBrownYellow = 105,
    /// `def_c_yellow_yellow`
    CYellowYellow = 106,
    /// `def_c_blue_yellow`
    CBlueYellow = 107,
    /// `def_c_light_blue_yellow`
    CLightBlueYellow = 108,
    /// `def_c_magenta_yellow`
    CMagentaYellow = 109,
    /// `def_c_pink_yellow`
    CPinkYellow = 110,
    /// `def_c_cyan_yellow`
    CCyanYellow = 111,
    /// `def_c_light_cyan_yellow`
    CLightCyanYellow = 112,
    /// `def_c_black_magenta`
    CBlackMagenta = 113,
    /// `def_c_dark_gray_magenta`
    CDarkGrayMagenta = 114,
    /// `def_c_light_gray_magenta`
    CLightGrayMagenta = 115,
    /// `def_c_white_magenta`
    CWhiteMagenta = 116,
    /// `def_c_red_magenta`
    CRedMagenta = 117,
    /// `def_c_light_red_magenta`
    CLightRedMagenta = 118,
    /// `def_c_green_magenta`
    CGreenMagenta = 119,
    /// `def_c_light_green_magenta`
    CLightGreenMagenta = 120,
    /// `def_c_brown_magenta`
    CBrownMagenta = 121,
    /// `def_c_yellow_magenta`
    CYellowMagenta = 122,
    /// `def_c_blue_magenta`
    CBlueMagenta = 123,
    /// `def_c_light_blue_magenta`
    CLightBlueMagenta = 124,
    /// `def_c_magenta_magenta`
    CMagentaMagenta = 125,
    /// `def_c_pink_magenta`
    CPinkMagenta = 126,
    /// `def_c_cyan_magenta`
    CCyanMagenta = 127,
    /// `def_c_light_cyan_magenta`
    CLightCyanMagenta = 128,
    /// `def_c_black_cyan`
    CBlackCyan = 129,
    /// `def_c_dark_gray_cyan`
    CDarkGrayCyan = 130,
    /// `def_c_light_gray_cyan`
    CLightGrayCyan = 131,
    /// `def_c_white_cyan`
    CWhiteCyan = 132,
    /// `def_c_red_cyan`
    CRedCyan = 133,
    /// `def_c_light_red_cyan`
    CLightRedCyan = 134,
    /// `def_c_green_cyan`
    CGreenCyan = 135,
    /// `def_c_light_green_cyan`
    CLightGreenCyan = 136,
    /// `def_c_brown_cyan`
    CBrownCyan = 137,
    /// `def_c_yellow_cyan`
    CYellowCyan = 138,
    /// `def_c_blue_cyan`
    CBlueCyan = 139,
    /// `def_c_light_blue_cyan`
    CLightBlueCyan = 140,
    /// `def_c_magenta_cyan`
    CMagentaCyan = 141,
    /// `def_c_pink_cyan`
    CPinkCyan = 142,
    /// `def_c_cyan_cyan`
    CCyanCyan = 143,
    /// `def_c_light_cyan_cyan`
    CLightCyanCyan = 144,
}

/// How many colours the reference declares (`num_colors`).
pub const NUM_COLORS: usize = 145;

/// A colour pair as `init_colors` defines it: two base colours from
/// `src/cursesdef.h:93` (black 0, red 1, green 2, yellow 3, blue 4, magenta 5,
/// cyan 6, white 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorPair {
    /// Foreground base colour.
    pub fg: u8,
    /// Background base colour.
    pub bg: u8,
}

/// The colour pairs `init_colors` registers (`src/color.cpp:425`), indexed by
/// pair number. Entry zero is the default pair, which `init_pair` cannot set.
pub const COLOR_PAIRS: [ColorPair; 88] = [
    ColorPair { fg: 0, bg: 0 },   // pair 0: default pair
    ColorPair { fg: 7, bg: 0 },   // 1: white on black
    ColorPair { fg: 1, bg: 0 },   // 2: red on black
    ColorPair { fg: 2, bg: 0 },   // 3: green on black
    ColorPair { fg: 4, bg: 0 },   // 4: blue on black
    ColorPair { fg: 6, bg: 0 },   // 5: cyan on black
    ColorPair { fg: 5, bg: 0 },   // 6: magenta on black
    ColorPair { fg: 3, bg: 0 },   // 7: yellow on black
    ColorPair { fg: 0, bg: 7 },   // 8: black on white
    ColorPair { fg: 0, bg: 1 },   // 9: black on red
    ColorPair { fg: 0, bg: 2 },   // 10: black on green
    ColorPair { fg: 0, bg: 4 },   // 11: black on blue
    ColorPair { fg: 0, bg: 6 },   // 12: black on cyan
    ColorPair { fg: 0, bg: 5 },   // 13: black on magenta
    ColorPair { fg: 0, bg: 3 },   // 14: black on yellow
    ColorPair { fg: 7, bg: 4 },   // 15: white on blue
    ColorPair { fg: 1, bg: 4 },   // 16: red on blue
    ColorPair { fg: 2, bg: 4 },   // 17: green on blue
    ColorPair { fg: 4, bg: 4 },   // 18: blue on blue
    ColorPair { fg: 6, bg: 4 },   // 19: cyan on blue
    ColorPair { fg: 0, bg: 4 },   // 20: black on blue
    ColorPair { fg: 5, bg: 4 },   // 21: magenta on blue
    ColorPair { fg: 3, bg: 4 },   // 22: yellow on blue
    ColorPair { fg: 7, bg: 1 },   // 23: white on red
    ColorPair { fg: 1, bg: 1 },   // 24: red on red
    ColorPair { fg: 2, bg: 1 },   // 25: green on red
    ColorPair { fg: 4, bg: 1 },   // 26: blue on red
    ColorPair { fg: 6, bg: 1 },   // 27: cyan on red
    ColorPair { fg: 5, bg: 1 },   // 28: magenta on red
    ColorPair { fg: 3, bg: 1 },   // 29: yellow on red
    ColorPair { fg: 0, bg: 0 },   // 30: black on black
    ColorPair { fg: 7, bg: 0 },   // 31: white on black
    ColorPair { fg: 0, bg: 7 },   // 32: black on white
    ColorPair { fg: 7, bg: 7 },   // 33: white on white
    ColorPair { fg: 1, bg: 7 },   // 34: red on white
    ColorPair { fg: 2, bg: 7 },   // 35: green on white
    ColorPair { fg: 3, bg: 7 },   // 36: yellow on white
    ColorPair { fg: 4, bg: 7 },   // 37: blue on white
    ColorPair { fg: 5, bg: 7 },   // 38: magenta on white
    ColorPair { fg: 6, bg: 7 },   // 39: cyan on white
    ColorPair { fg: 0, bg: 2 },   // 40: black on green
    ColorPair { fg: 7, bg: 2 },   // 41: white on green
    ColorPair { fg: 1, bg: 2 },   // 42: red on green
    ColorPair { fg: 2, bg: 2 },   // 43: green on green
    ColorPair { fg: 3, bg: 2 },   // 44: yellow on green
    ColorPair { fg: 4, bg: 2 },   // 45: blue on green
    ColorPair { fg: 5, bg: 2 },   // 46: magenta on green
    ColorPair { fg: 6, bg: 2 },   // 47: cyan on green
    ColorPair { fg: 0, bg: 3 },   // 48: black on yellow
    ColorPair { fg: 7, bg: 3 },   // 49: white on yellow
    ColorPair { fg: 1, bg: 3 },   // 50: red on yellow
    ColorPair { fg: 2, bg: 3 },   // 51: green on yellow
    ColorPair { fg: 3, bg: 3 },   // 52: yellow on yellow
    ColorPair { fg: 4, bg: 3 },   // 53: blue on yellow
    ColorPair { fg: 5, bg: 3 },   // 54: magenta on yellow
    ColorPair { fg: 6, bg: 3 },   // 55: cyan on yellow
    ColorPair { fg: 0, bg: 5 },   // 56: black on magenta
    ColorPair { fg: 7, bg: 5 },   // 57: white on magenta
    ColorPair { fg: 1, bg: 5 },   // 58: red on magenta
    ColorPair { fg: 2, bg: 5 },   // 59: green on magenta
    ColorPair { fg: 3, bg: 5 },   // 60: yellow on magenta
    ColorPair { fg: 4, bg: 5 },   // 61: blue on magenta
    ColorPair { fg: 5, bg: 5 },   // 62: magenta on magenta
    ColorPair { fg: 6, bg: 5 },   // 63: cyan on magenta
    ColorPair { fg: 0, bg: 6 },   // 64: black on cyan
    ColorPair { fg: 7, bg: 6 },   // 65: white on cyan
    ColorPair { fg: 1, bg: 6 },   // 66: red on cyan
    ColorPair { fg: 2, bg: 6 },   // 67: green on cyan
    ColorPair { fg: 3, bg: 6 },   // 68: yellow on cyan
    ColorPair { fg: 4, bg: 6 },   // 69: blue on cyan
    ColorPair { fg: 5, bg: 6 },   // 70: magenta on cyan
    ColorPair { fg: 6, bg: 6 },   // 71: cyan on cyan
    ColorPair { fg: 237, bg: 0 }, // 72: dark_gray on black
    ColorPair { fg: 237, bg: 1 }, // 73: dark_gray on red
    ColorPair { fg: 237, bg: 2 }, // 74: dark_gray on green
    ColorPair { fg: 237, bg: 4 }, // 75: dark_gray on blue
    ColorPair { fg: 237, bg: 6 }, // 76: dark_gray on cyan
    ColorPair { fg: 237, bg: 5 }, // 77: dark_gray on magenta
    ColorPair { fg: 237, bg: 3 }, // 78: dark_gray on yellow
    ColorPair { fg: 237, bg: 7 }, // 79: dark_gray on white
    ColorPair { fg: 0, bg: 237 }, // 80: black on dark_gray
    ColorPair { fg: 1, bg: 237 }, // 81: red on dark_gray
    ColorPair { fg: 2, bg: 237 }, // 82: green on dark_gray
    ColorPair { fg: 4, bg: 237 }, // 83: blue on dark_gray
    ColorPair { fg: 6, bg: 237 }, // 84: cyan on dark_gray
    ColorPair { fg: 5, bg: 237 }, // 85: magenta on dark_gray
    ColorPair { fg: 3, bg: 237 }, // 86: yellow on dark_gray
    ColorPair { fg: 7, bg: 237 }, // 87: white on dark_gray
];

/// One colour: its id, its name, the pair it uses, the attribute bits the
/// reference adds to that pair, and the id it inverts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorDef {
    /// The colour's id.
    pub id: ColorId,
    /// The name `name_map` knows it by.
    pub name: &'static str,
    /// The colour pair it draws with.
    pub pair: u16,
    /// Whether `A_BOLD` is added to the pair.
    pub bold: bool,
    /// Whether `A_BLINK` is added to the pair.
    pub blink: bool,
    /// The colour this one inverts to.
    pub invert: ColorId,
}

/// The default colour table, as the 256-colour terminal build defines it
/// (`color_manager::load_default`, `src/color.cpp:221`). Order follows
/// [`ColorId`].
pub const COLORS: &[ColorDef] = &[
    ColorDef {
        id: ColorId::CBlack,
        name: "c_black",
        pair: 30,
        bold: false,
        blink: false,
        invert: ColorId::IBlack,
    },
    ColorDef {
        id: ColorId::CWhite,
        name: "c_white",
        pair: 1,
        bold: true,
        blink: false,
        invert: ColorId::IWhite,
    },
    ColorDef {
        id: ColorId::CLightGray,
        name: "c_light_gray",
        pair: 1,
        bold: false,
        blink: false,
        invert: ColorId::ILightGray,
    },
    ColorDef {
        id: ColorId::CDarkGray,
        name: "c_dark_gray",
        pair: 72,
        bold: false,
        blink: false,
        invert: ColorId::IDarkGray,
    },
    ColorDef {
        id: ColorId::CRed,
        name: "c_red",
        pair: 2,
        bold: false,
        blink: false,
        invert: ColorId::IRed,
    },
    ColorDef {
        id: ColorId::CGreen,
        name: "c_green",
        pair: 3,
        bold: false,
        blink: false,
        invert: ColorId::IGreen,
    },
    ColorDef {
        id: ColorId::CBlue,
        name: "c_blue",
        pair: 4,
        bold: false,
        blink: false,
        invert: ColorId::IBlue,
    },
    ColorDef {
        id: ColorId::CCyan,
        name: "c_cyan",
        pair: 5,
        bold: false,
        blink: false,
        invert: ColorId::ICyan,
    },
    ColorDef {
        id: ColorId::CMagenta,
        name: "c_magenta",
        pair: 6,
        bold: false,
        blink: false,
        invert: ColorId::IMagenta,
    },
    ColorDef {
        id: ColorId::CBrown,
        name: "c_brown",
        pair: 7,
        bold: false,
        blink: false,
        invert: ColorId::IBrown,
    },
    ColorDef {
        id: ColorId::CLightRed,
        name: "c_light_red",
        pair: 2,
        bold: true,
        blink: false,
        invert: ColorId::ILightRed,
    },
    ColorDef {
        id: ColorId::CLightGreen,
        name: "c_light_green",
        pair: 3,
        bold: true,
        blink: false,
        invert: ColorId::ILightGreen,
    },
    ColorDef {
        id: ColorId::CLightBlue,
        name: "c_light_blue",
        pair: 4,
        bold: true,
        blink: false,
        invert: ColorId::ILightBlue,
    },
    ColorDef {
        id: ColorId::CLightCyan,
        name: "c_light_cyan",
        pair: 5,
        bold: true,
        blink: false,
        invert: ColorId::ILightCyan,
    },
    ColorDef {
        id: ColorId::CPink,
        name: "c_pink",
        pair: 6,
        bold: true,
        blink: false,
        invert: ColorId::IPink,
    },
    ColorDef {
        id: ColorId::CYellow,
        name: "c_yellow",
        pair: 7,
        bold: true,
        blink: false,
        invert: ColorId::IYellow,
    },
    ColorDef {
        id: ColorId::HBlack,
        name: "h_black",
        pair: 20,
        bold: false,
        blink: false,
        invert: ColorId::CBlue,
    },
    ColorDef {
        id: ColorId::HWhite,
        name: "h_white",
        pair: 15,
        bold: true,
        blink: false,
        invert: ColorId::CLightBlueWhite,
    },
    ColorDef {
        id: ColorId::HLightGray,
        name: "h_light_gray",
        pair: 15,
        bold: false,
        blink: false,
        invert: ColorId::CBlueWhite,
    },
    ColorDef {
        id: ColorId::HDarkGray,
        name: "h_dark_gray",
        pair: 75,
        bold: false,
        blink: false,
        invert: ColorId::CLightBlue,
    },
    ColorDef {
        id: ColorId::HRed,
        name: "h_red",
        pair: 16,
        bold: false,
        blink: false,
        invert: ColorId::CBlueRed,
    },
    ColorDef {
        id: ColorId::HGreen,
        name: "h_green",
        pair: 17,
        bold: false,
        blink: false,
        invert: ColorId::CBlueGreen,
    },
    ColorDef {
        id: ColorId::HBlue,
        name: "h_blue",
        pair: 20,
        bold: false,
        blink: false,
        invert: ColorId::HBlue,
    },
    ColorDef {
        id: ColorId::HCyan,
        name: "h_cyan",
        pair: 19,
        bold: false,
        blink: false,
        invert: ColorId::CBlueCyan,
    },
    ColorDef {
        id: ColorId::HMagenta,
        name: "h_magenta",
        pair: 21,
        bold: false,
        blink: false,
        invert: ColorId::CBlueMagenta,
    },
    ColorDef {
        id: ColorId::HBrown,
        name: "h_brown",
        pair: 22,
        bold: false,
        blink: false,
        invert: ColorId::CBlueYellow,
    },
    ColorDef {
        id: ColorId::HLightRed,
        name: "h_light_red",
        pair: 16,
        bold: true,
        blink: false,
        invert: ColorId::CLightBlueRed,
    },
    ColorDef {
        id: ColorId::HLightGreen,
        name: "h_light_green",
        pair: 17,
        bold: true,
        blink: false,
        invert: ColorId::CLightBlueGreen,
    },
    ColorDef {
        id: ColorId::HLightBlue,
        name: "h_light_blue",
        pair: 18,
        bold: true,
        blink: false,
        invert: ColorId::HLightBlue,
    },
    ColorDef {
        id: ColorId::HLightCyan,
        name: "h_light_cyan",
        pair: 19,
        bold: true,
        blink: false,
        invert: ColorId::CLightBlueCyan,
    },
    ColorDef {
        id: ColorId::HPink,
        name: "h_pink",
        pair: 21,
        bold: true,
        blink: false,
        invert: ColorId::CLightBlueMagenta,
    },
    ColorDef {
        id: ColorId::HYellow,
        name: "h_yellow",
        pair: 22,
        bold: true,
        blink: false,
        invert: ColorId::CLightBlueYellow,
    },
    ColorDef {
        id: ColorId::IBlack,
        name: "i_black",
        pair: 32,
        bold: false,
        blink: false,
        invert: ColorId::CBlack,
    },
    ColorDef {
        id: ColorId::IWhite,
        name: "i_white",
        pair: 8,
        bold: false,
        blink: true,
        invert: ColorId::CWhite,
    },
    ColorDef {
        id: ColorId::ILightGray,
        name: "i_light_gray",
        pair: 8,
        bold: false,
        blink: false,
        invert: ColorId::CLightGray,
    },
    ColorDef {
        id: ColorId::IDarkGray,
        name: "i_dark_gray",
        pair: 79,
        bold: false,
        blink: true,
        invert: ColorId::CDarkGray,
    },
    ColorDef {
        id: ColorId::IRed,
        name: "i_red",
        pair: 9,
        bold: false,
        blink: false,
        invert: ColorId::CRed,
    },
    ColorDef {
        id: ColorId::IGreen,
        name: "i_green",
        pair: 10,
        bold: false,
        blink: false,
        invert: ColorId::CGreen,
    },
    ColorDef {
        id: ColorId::IBlue,
        name: "i_blue",
        pair: 11,
        bold: false,
        blink: false,
        invert: ColorId::CBlue,
    },
    ColorDef {
        id: ColorId::ICyan,
        name: "i_cyan",
        pair: 12,
        bold: false,
        blink: false,
        invert: ColorId::CCyan,
    },
    ColorDef {
        id: ColorId::IMagenta,
        name: "i_magenta",
        pair: 13,
        bold: false,
        blink: false,
        invert: ColorId::CMagenta,
    },
    ColorDef {
        id: ColorId::IBrown,
        name: "i_brown",
        pair: 14,
        bold: false,
        blink: false,
        invert: ColorId::CBrown,
    },
    ColorDef {
        id: ColorId::ILightRed,
        name: "i_light_red",
        pair: 9,
        bold: false,
        blink: true,
        invert: ColorId::CLightRed,
    },
    ColorDef {
        id: ColorId::ILightGreen,
        name: "i_light_green",
        pair: 10,
        bold: false,
        blink: true,
        invert: ColorId::CLightGreen,
    },
    ColorDef {
        id: ColorId::ILightBlue,
        name: "i_light_blue",
        pair: 11,
        bold: false,
        blink: true,
        invert: ColorId::CLightBlue,
    },
    ColorDef {
        id: ColorId::ILightCyan,
        name: "i_light_cyan",
        pair: 12,
        bold: false,
        blink: true,
        invert: ColorId::CLightCyan,
    },
    ColorDef {
        id: ColorId::IPink,
        name: "i_pink",
        pair: 13,
        bold: false,
        blink: true,
        invert: ColorId::CPink,
    },
    ColorDef {
        id: ColorId::IYellow,
        name: "i_yellow",
        pair: 14,
        bold: false,
        blink: true,
        invert: ColorId::CYellow,
    },
    ColorDef {
        id: ColorId::CUnset,
        name: "c_unset",
        pair: 31,
        bold: false,
        blink: false,
        invert: ColorId::CUnset,
    },
    ColorDef {
        id: ColorId::CBlackRed,
        name: "c_black_red",
        pair: 9,
        bold: true,
        blink: false,
        invert: ColorId::CRed,
    },
    ColorDef {
        id: ColorId::CWhiteRed,
        name: "c_white_red",
        pair: 23,
        bold: true,
        blink: false,
        invert: ColorId::CRedWhite,
    },
    ColorDef {
        id: ColorId::CLightGrayRed,
        name: "c_light_gray_red",
        pair: 23,
        bold: false,
        blink: false,
        invert: ColorId::CLightRedWhite,
    },
    ColorDef {
        id: ColorId::CDarkGrayRed,
        name: "c_dark_gray_red",
        pair: 73,
        bold: false,
        blink: false,
        invert: ColorId::CDarkGrayRed,
    },
    ColorDef {
        id: ColorId::CRedRed,
        name: "c_red_red",
        pair: 9,
        bold: false,
        blink: false,
        invert: ColorId::CRedRed,
    },
    ColorDef {
        id: ColorId::CGreenRed,
        name: "c_green_red",
        pair: 25,
        bold: false,
        blink: false,
        invert: ColorId::CRedGreen,
    },
    ColorDef {
        id: ColorId::CBlueRed,
        name: "c_blue_red",
        pair: 26,
        bold: false,
        blink: false,
        invert: ColorId::HRed,
    },
    ColorDef {
        id: ColorId::CCyanRed,
        name: "c_cyan_red",
        pair: 27,
        bold: false,
        blink: false,
        invert: ColorId::CRedCyan,
    },
    ColorDef {
        id: ColorId::CMagentaRed,
        name: "c_magenta_red",
        pair: 28,
        bold: false,
        blink: false,
        invert: ColorId::CRedMagenta,
    },
    ColorDef {
        id: ColorId::CBrownRed,
        name: "c_brown_red",
        pair: 29,
        bold: false,
        blink: false,
        invert: ColorId::CRedYellow,
    },
    ColorDef {
        id: ColorId::CLightRedRed,
        name: "c_light_red_red",
        pair: 24,
        bold: true,
        blink: false,
        invert: ColorId::CRedRed,
    },
    ColorDef {
        id: ColorId::CLightGreenRed,
        name: "c_light_green_red",
        pair: 25,
        bold: true,
        blink: false,
        invert: ColorId::CLightRedGreen,
    },
    ColorDef {
        id: ColorId::CLightBlueRed,
        name: "c_light_blue_red",
        pair: 26,
        bold: true,
        blink: false,
        invert: ColorId::HLightRed,
    },
    ColorDef {
        id: ColorId::CLightCyanRed,
        name: "c_light_cyan_red",
        pair: 27,
        bold: true,
        blink: false,
        invert: ColorId::CLightRedCyan,
    },
    ColorDef {
        id: ColorId::CPinkRed,
        name: "c_pink_red",
        pair: 28,
        bold: true,
        blink: false,
        invert: ColorId::CLightRedMagenta,
    },
    ColorDef {
        id: ColorId::CYellowRed,
        name: "c_yellow_red",
        pair: 29,
        bold: true,
        blink: false,
        invert: ColorId::CRedYellow,
    },
    ColorDef {
        id: ColorId::CBlackWhite,
        name: "c_black_white",
        pair: 32,
        bold: false,
        blink: false,
        invert: ColorId::CLightGray,
    },
    ColorDef {
        id: ColorId::CDarkGrayWhite,
        name: "c_dark_gray_white",
        pair: 79,
        bold: false,
        blink: false,
        invert: ColorId::CWhite,
    },
    ColorDef {
        id: ColorId::CLightGrayWhite,
        name: "c_light_gray_white",
        pair: 33,
        bold: false,
        blink: false,
        invert: ColorId::CLightGrayWhite,
    },
    ColorDef {
        id: ColorId::CWhiteWhite,
        name: "c_white_white",
        pair: 33,
        bold: true,
        blink: false,
        invert: ColorId::CWhiteWhite,
    },
    ColorDef {
        id: ColorId::CRedWhite,
        name: "c_red_white",
        pair: 34,
        bold: false,
        blink: false,
        invert: ColorId::CWhiteRed,
    },
    ColorDef {
        id: ColorId::CLightRedWhite,
        name: "c_light_red_white",
        pair: 34,
        bold: true,
        blink: false,
        invert: ColorId::CLightGrayRed,
    },
    ColorDef {
        id: ColorId::CGreenWhite,
        name: "c_green_white",
        pair: 35,
        bold: false,
        blink: false,
        invert: ColorId::CLightGrayGreen,
    },
    ColorDef {
        id: ColorId::CLightGreenWhite,
        name: "c_light_green_white",
        pair: 35,
        bold: true,
        blink: false,
        invert: ColorId::CWhiteGreen,
    },
    ColorDef {
        id: ColorId::CBrownWhite,
        name: "c_brown_white",
        pair: 36,
        bold: false,
        blink: false,
        invert: ColorId::CLightGrayYellow,
    },
    ColorDef {
        id: ColorId::CYellowWhite,
        name: "c_yellow_white",
        pair: 36,
        bold: true,
        blink: false,
        invert: ColorId::CWhiteYellow,
    },
    ColorDef {
        id: ColorId::CBlueWhite,
        name: "c_blue_white",
        pair: 37,
        bold: false,
        blink: false,
        invert: ColorId::HLightGray,
    },
    ColorDef {
        id: ColorId::CLightBlueWhite,
        name: "c_light_blue_white",
        pair: 37,
        bold: true,
        blink: false,
        invert: ColorId::HWhite,
    },
    ColorDef {
        id: ColorId::CMagentaWhite,
        name: "c_magenta_white",
        pair: 38,
        bold: false,
        blink: false,
        invert: ColorId::CLightGrayMagenta,
    },
    ColorDef {
        id: ColorId::CPinkWhite,
        name: "c_pink_white",
        pair: 38,
        bold: true,
        blink: false,
        invert: ColorId::CWhiteMagenta,
    },
    ColorDef {
        id: ColorId::CCyanWhite,
        name: "c_cyan_white",
        pair: 39,
        bold: false,
        blink: false,
        invert: ColorId::CLightGrayCyan,
    },
    ColorDef {
        id: ColorId::CLightCyanWhite,
        name: "c_light_cyan_white",
        pair: 39,
        bold: true,
        blink: false,
        invert: ColorId::CWhiteCyan,
    },
    ColorDef {
        id: ColorId::CBlackGreen,
        name: "c_black_green",
        pair: 40,
        bold: false,
        blink: false,
        invert: ColorId::CGreen,
    },
    ColorDef {
        id: ColorId::CDarkGrayGreen,
        name: "c_dark_gray_green",
        pair: 74,
        bold: false,
        blink: false,
        invert: ColorId::CLightGreen,
    },
    ColorDef {
        id: ColorId::CLightGrayGreen,
        name: "c_light_gray_green",
        pair: 41,
        bold: false,
        blink: false,
        invert: ColorId::CGreenWhite,
    },
    ColorDef {
        id: ColorId::CWhiteGreen,
        name: "c_white_green",
        pair: 41,
        bold: true,
        blink: false,
        invert: ColorId::CLightGreenWhite,
    },
    ColorDef {
        id: ColorId::CRedGreen,
        name: "c_red_green",
        pair: 42,
        bold: false,
        blink: false,
        invert: ColorId::CGreenRed,
    },
    ColorDef {
        id: ColorId::CLightRedGreen,
        name: "c_light_red_green",
        pair: 42,
        bold: true,
        blink: false,
        invert: ColorId::CLightGreenRed,
    },
    ColorDef {
        id: ColorId::CGreenGreen,
        name: "c_green_green",
        pair: 43,
        bold: false,
        blink: false,
        invert: ColorId::CGreenGreen,
    },
    ColorDef {
        id: ColorId::CLightGreenGreen,
        name: "c_light_green_green",
        pair: 43,
        bold: true,
        blink: false,
        invert: ColorId::CLightGreenGreen,
    },
    ColorDef {
        id: ColorId::CBrownGreen,
        name: "c_brown_green",
        pair: 44,
        bold: false,
        blink: false,
        invert: ColorId::CGreenYellow,
    },
    ColorDef {
        id: ColorId::CYellowGreen,
        name: "c_yellow_green",
        pair: 44,
        bold: true,
        blink: false,
        invert: ColorId::CLightGreenYellow,
    },
    ColorDef {
        id: ColorId::CBlueGreen,
        name: "c_blue_green",
        pair: 45,
        bold: false,
        blink: false,
        invert: ColorId::HGreen,
    },
    ColorDef {
        id: ColorId::CLightBlueGreen,
        name: "c_light_blue_green",
        pair: 45,
        bold: true,
        blink: false,
        invert: ColorId::HLightGreen,
    },
    ColorDef {
        id: ColorId::CMagentaGreen,
        name: "c_magenta_green",
        pair: 46,
        bold: false,
        blink: false,
        invert: ColorId::CGreenMagenta,
    },
    ColorDef {
        id: ColorId::CPinkGreen,
        name: "c_pink_green",
        pair: 46,
        bold: true,
        blink: false,
        invert: ColorId::CLightGreenMagenta,
    },
    ColorDef {
        id: ColorId::CCyanGreen,
        name: "c_cyan_green",
        pair: 47,
        bold: false,
        blink: false,
        invert: ColorId::CGreenCyan,
    },
    ColorDef {
        id: ColorId::CLightCyanGreen,
        name: "c_light_cyan_green",
        pair: 47,
        bold: true,
        blink: false,
        invert: ColorId::CLightGreenCyan,
    },
    ColorDef {
        id: ColorId::CBlackYellow,
        name: "c_black_yellow",
        pair: 48,
        bold: false,
        blink: false,
        invert: ColorId::CBrown,
    },
    ColorDef {
        id: ColorId::CDarkGrayYellow,
        name: "c_dark_gray_yellow",
        pair: 78,
        bold: false,
        blink: false,
        invert: ColorId::CYellow,
    },
    ColorDef {
        id: ColorId::CLightGrayYellow,
        name: "c_light_gray_yellow",
        pair: 49,
        bold: false,
        blink: false,
        invert: ColorId::CBrownWhite,
    },
    ColorDef {
        id: ColorId::CWhiteYellow,
        name: "c_white_yellow",
        pair: 49,
        bold: true,
        blink: false,
        invert: ColorId::CYellowWhite,
    },
    ColorDef {
        id: ColorId::CRedYellow,
        name: "c_red_yellow",
        pair: 50,
        bold: false,
        blink: false,
        invert: ColorId::CYellowRed,
    },
    ColorDef {
        id: ColorId::CLightRedYellow,
        name: "c_light_red_yellow",
        pair: 50,
        bold: true,
        blink: false,
        invert: ColorId::CYellowRed,
    },
    ColorDef {
        id: ColorId::CGreenYellow,
        name: "c_green_yellow",
        pair: 51,
        bold: false,
        blink: false,
        invert: ColorId::CBrownGreen,
    },
    ColorDef {
        id: ColorId::CLightGreenYellow,
        name: "c_light_green_yellow",
        pair: 51,
        bold: true,
        blink: false,
        invert: ColorId::CYellowGreen,
    },
    ColorDef {
        id: ColorId::CBrownYellow,
        name: "c_brown_yellow",
        pair: 52,
        bold: false,
        blink: false,
        invert: ColorId::CBrownYellow,
    },
    ColorDef {
        id: ColorId::CYellowYellow,
        name: "c_yellow_yellow",
        pair: 52,
        bold: true,
        blink: false,
        invert: ColorId::CYellowYellow,
    },
    ColorDef {
        id: ColorId::CBlueYellow,
        name: "c_blue_yellow",
        pair: 53,
        bold: false,
        blink: false,
        invert: ColorId::HBrown,
    },
    ColorDef {
        id: ColorId::CLightBlueYellow,
        name: "c_light_blue_yellow",
        pair: 53,
        bold: true,
        blink: false,
        invert: ColorId::HYellow,
    },
    ColorDef {
        id: ColorId::CMagentaYellow,
        name: "c_magenta_yellow",
        pair: 54,
        bold: false,
        blink: false,
        invert: ColorId::CBrownMagenta,
    },
    ColorDef {
        id: ColorId::CPinkYellow,
        name: "c_pink_yellow",
        pair: 54,
        bold: true,
        blink: false,
        invert: ColorId::CYellowMagenta,
    },
    ColorDef {
        id: ColorId::CCyanYellow,
        name: "c_cyan_yellow",
        pair: 55,
        bold: false,
        blink: false,
        invert: ColorId::CBrownCyan,
    },
    ColorDef {
        id: ColorId::CLightCyanYellow,
        name: "c_light_cyan_yellow",
        pair: 55,
        bold: true,
        blink: false,
        invert: ColorId::CYellowCyan,
    },
    ColorDef {
        id: ColorId::CBlackMagenta,
        name: "c_black_magenta",
        pair: 56,
        bold: false,
        blink: false,
        invert: ColorId::CMagenta,
    },
    ColorDef {
        id: ColorId::CDarkGrayMagenta,
        name: "c_dark_gray_magenta",
        pair: 77,
        bold: false,
        blink: false,
        invert: ColorId::CPink,
    },
    ColorDef {
        id: ColorId::CLightGrayMagenta,
        name: "c_light_gray_magenta",
        pair: 57,
        bold: false,
        blink: false,
        invert: ColorId::CMagentaWhite,
    },
    ColorDef {
        id: ColorId::CWhiteMagenta,
        name: "c_white_magenta",
        pair: 57,
        bold: true,
        blink: false,
        invert: ColorId::CPinkWhite,
    },
    ColorDef {
        id: ColorId::CRedMagenta,
        name: "c_red_magenta",
        pair: 58,
        bold: false,
        blink: false,
        invert: ColorId::CMagentaRed,
    },
    ColorDef {
        id: ColorId::CLightRedMagenta,
        name: "c_light_red_magenta",
        pair: 58,
        bold: true,
        blink: false,
        invert: ColorId::CPinkRed,
    },
    ColorDef {
        id: ColorId::CGreenMagenta,
        name: "c_green_magenta",
        pair: 59,
        bold: false,
        blink: false,
        invert: ColorId::CMagentaGreen,
    },
    ColorDef {
        id: ColorId::CLightGreenMagenta,
        name: "c_light_green_magenta",
        pair: 59,
        bold: true,
        blink: false,
        invert: ColorId::CPinkGreen,
    },
    ColorDef {
        id: ColorId::CBrownMagenta,
        name: "c_brown_magenta",
        pair: 60,
        bold: false,
        blink: false,
        invert: ColorId::CMagentaYellow,
    },
    ColorDef {
        id: ColorId::CYellowMagenta,
        name: "c_yellow_magenta",
        pair: 60,
        bold: true,
        blink: false,
        invert: ColorId::CPinkYellow,
    },
    ColorDef {
        id: ColorId::CBlueMagenta,
        name: "c_blue_magenta",
        pair: 61,
        bold: false,
        blink: false,
        invert: ColorId::HMagenta,
    },
    ColorDef {
        id: ColorId::CLightBlueMagenta,
        name: "c_light_blue_magenta",
        pair: 61,
        bold: true,
        blink: false,
        invert: ColorId::HPink,
    },
    ColorDef {
        id: ColorId::CMagentaMagenta,
        name: "c_magenta_magenta",
        pair: 62,
        bold: false,
        blink: false,
        invert: ColorId::CMagentaMagenta,
    },
    ColorDef {
        id: ColorId::CPinkMagenta,
        name: "c_pink_magenta",
        pair: 62,
        bold: true,
        blink: false,
        invert: ColorId::CPinkMagenta,
    },
    ColorDef {
        id: ColorId::CCyanMagenta,
        name: "c_cyan_magenta",
        pair: 63,
        bold: false,
        blink: false,
        invert: ColorId::CMagentaCyan,
    },
    ColorDef {
        id: ColorId::CLightCyanMagenta,
        name: "c_light_cyan_magenta",
        pair: 63,
        bold: true,
        blink: false,
        invert: ColorId::CPinkCyan,
    },
    ColorDef {
        id: ColorId::CBlackCyan,
        name: "c_black_cyan",
        pair: 64,
        bold: false,
        blink: false,
        invert: ColorId::CCyan,
    },
    ColorDef {
        id: ColorId::CDarkGrayCyan,
        name: "c_dark_gray_cyan",
        pair: 76,
        bold: false,
        blink: false,
        invert: ColorId::CLightCyan,
    },
    ColorDef {
        id: ColorId::CLightGrayCyan,
        name: "c_light_gray_cyan",
        pair: 65,
        bold: false,
        blink: false,
        invert: ColorId::CCyanWhite,
    },
    ColorDef {
        id: ColorId::CWhiteCyan,
        name: "c_white_cyan",
        pair: 65,
        bold: true,
        blink: false,
        invert: ColorId::CLightCyanWhite,
    },
    ColorDef {
        id: ColorId::CRedCyan,
        name: "c_red_cyan",
        pair: 66,
        bold: false,
        blink: false,
        invert: ColorId::CCyanRed,
    },
    ColorDef {
        id: ColorId::CLightRedCyan,
        name: "c_light_red_cyan",
        pair: 66,
        bold: true,
        blink: false,
        invert: ColorId::CLightCyanRed,
    },
    ColorDef {
        id: ColorId::CGreenCyan,
        name: "c_green_cyan",
        pair: 67,
        bold: false,
        blink: false,
        invert: ColorId::CCyanGreen,
    },
    ColorDef {
        id: ColorId::CLightGreenCyan,
        name: "c_light_green_cyan",
        pair: 67,
        bold: true,
        blink: false,
        invert: ColorId::CLightCyanGreen,
    },
    ColorDef {
        id: ColorId::CBrownCyan,
        name: "c_brown_cyan",
        pair: 68,
        bold: false,
        blink: false,
        invert: ColorId::CCyanYellow,
    },
    ColorDef {
        id: ColorId::CYellowCyan,
        name: "c_yellow_cyan",
        pair: 68,
        bold: true,
        blink: false,
        invert: ColorId::CLightCyanYellow,
    },
    ColorDef {
        id: ColorId::CBlueCyan,
        name: "c_blue_cyan",
        pair: 69,
        bold: false,
        blink: false,
        invert: ColorId::HCyan,
    },
    ColorDef {
        id: ColorId::CLightBlueCyan,
        name: "c_light_blue_cyan",
        pair: 69,
        bold: true,
        blink: false,
        invert: ColorId::HLightCyan,
    },
    ColorDef {
        id: ColorId::CMagentaCyan,
        name: "c_magenta_cyan",
        pair: 70,
        bold: false,
        blink: false,
        invert: ColorId::CCyanMagenta,
    },
    ColorDef {
        id: ColorId::CPinkCyan,
        name: "c_pink_cyan",
        pair: 70,
        bold: true,
        blink: false,
        invert: ColorId::CLightCyanMagenta,
    },
    ColorDef {
        id: ColorId::CCyanCyan,
        name: "c_cyan_cyan",
        pair: 71,
        bold: false,
        blink: false,
        invert: ColorId::CCyanCyan,
    },
    ColorDef {
        id: ColorId::CLightCyanCyan,
        name: "c_light_cyan_cyan",
        pair: 71,
        bold: true,
        blink: false,
        invert: ColorId::CLightCyanCyan,
    },
];

/// The nine colours that differ in a terminal without 256-colour support
/// (`src/color.cpp:409-421`): the same ids, reached through bold on the base
/// eight instead of dedicated pairs 72-79.
pub const DARK_GRAY_FALLBACK: &[ColorDef] = &[
    ColorDef {
        id: ColorId::CDarkGray,
        name: "c_dark_gray",
        pair: 30,
        bold: true,
        blink: false,
        invert: ColorId::IDarkGray,
    },
    ColorDef {
        id: ColorId::HDarkGray,
        name: "h_dark_gray",
        pair: 20,
        bold: true,
        blink: false,
        invert: ColorId::CLightBlue,
    },
    ColorDef {
        id: ColorId::IDarkGray,
        name: "i_dark_gray",
        pair: 32,
        bold: false,
        blink: true,
        invert: ColorId::CDarkGray,
    },
    ColorDef {
        id: ColorId::CDarkGrayRed,
        name: "c_dark_gray_red",
        pair: 9,
        bold: true,
        blink: false,
        invert: ColorId::CDarkGrayRed,
    },
    ColorDef {
        id: ColorId::CDarkGrayWhite,
        name: "c_dark_gray_white",
        pair: 32,
        bold: true,
        blink: false,
        invert: ColorId::CWhite,
    },
    ColorDef {
        id: ColorId::CDarkGrayGreen,
        name: "c_dark_gray_green",
        pair: 40,
        bold: true,
        blink: false,
        invert: ColorId::CLightGreen,
    },
    ColorDef {
        id: ColorId::CDarkGrayYellow,
        name: "c_dark_gray_yellow",
        pair: 48,
        bold: true,
        blink: false,
        invert: ColorId::CYellow,
    },
    ColorDef {
        id: ColorId::CDarkGrayMagenta,
        name: "c_dark_gray_magenta",
        pair: 56,
        bold: true,
        blink: false,
        invert: ColorId::CPink,
    },
    ColorDef {
        id: ColorId::CDarkGrayCyan,
        name: "c_dark_gray_cyan",
        pair: 64,
        bold: true,
        blink: false,
        invert: ColorId::CLightCyan,
    },
];

/// Why a colour name could not be turned into a colour.
///
/// The reference has no error type here: it logs a `debugmsg` and hands back the
/// `c_unset` sentinel (`src/color.cpp:117`). Callers that need the sentinel use
/// `color_value(ColorId::CUnset)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorError {
    /// No colour is registered under that spelling.
    UnknownName,
}

/// Resolves a foreground colour name.
///
/// Reference: `color_from_string` (`src/color.cpp:633`). A name whose second
/// character is not `_` gets `c_` in front, then the deprecated `lt`/`dk`
/// spellings are rewritten to `light_`/`dark_` — in that order, anywhere in the
/// string. Names are case sensitive, there is no `#rrggbb` form, and no `bg_`
/// prefix: backgrounds go through [`bgcolor_from_string`].
///
/// # Errors
///
/// [`ColorError::UnknownName`] for anything the table does not know, which in the
/// reference is the `c_unset` sentinel.
pub fn color_from_string(name: &str) -> Result<NcColor, ColorError> {
    if name.is_empty() {
        return Err(ColorError::UnknownName);
    }
    // The reference tests `substr( 1, 1 ) != "_"`, byte for byte.
    let mut spelled = if name.as_bytes().get(1) == Some(&b'_') {
        name.to_owned()
    } else {
        format!("c_{name}")
    };
    if spelled.contains("lt") {
        spelled = spelled.replace("lt", "light_");
    }
    if spelled.contains("dk") {
        spelled = spelled.replace("dk", "dark_");
    }
    let id = color_from_name(&spelled).ok_or(ColorError::UnknownName)?;
    color_value(id).ok_or(ColorError::UnknownName)
}

/// The id a colour resolves to.
///
/// Reference: `color_manager::color_to_id` (`src/color.cpp:132`). The reference
/// keys its reverse map by attribute and lets the last colour defined win, so
/// where the table holds the same attribute twice — `h_black`/`h_blue`,
/// `i_black`/`c_black_white`, `i_red`/`c_red_red` — the later id is the answer
/// and the earlier name never comes back.
#[must_use]
pub fn color_to_id(color: NcColor) -> Option<ColorId> {
    let attribute = color.to_int();
    COLORS
        .iter()
        .rev()
        .find(|def| built_color(def).to_int() == attribute)
        .map(|def| def.id)
}

/// The canonical name to print for a colour.
///
/// Reference: `string_from_color` (`src/color.cpp:671`). The `c_unset` name is
/// reported as `c_white`, so `string_from_color(c_unset)` is `"c_white"` and the
/// name round trip is not stable through [`color_from_string`] — the *value*
/// round trip is.
#[must_use]
pub fn string_from_color(color: NcColor) -> &'static str {
    let name = color_to_id(color).and_then(color_name).unwrap_or("c_unset");
    if name == "c_unset" { "c_white" } else { name }
}

/// Resolves a background colour name.
///
/// Reference: `bgcolor_from_string` (`src/color.cpp:689`). The prefix is always
/// `i_`, the deprecated `lt`/`dk` spellings are rewritten the same way as for
/// foregrounds, and a name the table does not know yields `i_white` — not the
/// `c_unset` sentinel a foreground would get.
#[must_use]
pub fn bgcolor_from_string(name: &str) -> NcColor {
    let mut spelled = format!("i_{name}");
    if spelled.contains("lt") {
        spelled = spelled.replace("lt", "light_");
    }
    if spelled.contains("dk") {
        spelled = spelled.replace("dk", "dark_");
    }
    // `i_white` is always in the table, so the default is unreachable; the crate
    // forbids panicking, so it stays a value.
    color_from_name(&spelled)
        .and_then(color_value)
        .or_else(|| color_value(ColorId::IWhite))
        .unwrap_or_default()
}

/// What a `<color_...>` tag turned out to be.
///
/// Reference: `struct color_tag_parse_result` (`src/color.h:513`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagParse {
    /// An opening tag naming a usable colour.
    Open(NcColor),
    /// `</color>`, which closes the current colour.
    Close,
    /// Anything else: not a colour tag, or a colour tag naming nothing usable.
    NonColor,
}

/// Parses one `<color_...>` tag.
///
/// Reference: `get_color_from_tag` (`src/color.cpp:716`). Only a leading `<` can
/// start a tag, `</color>` closes, and an opening tag must spell `<color_` and
/// end its name at the first `>`. A name that resolves to `c_unset` — including
/// the name `unset` itself — is reported as [`TagParse::NonColor`].
#[must_use]
pub fn get_color_from_tag(tag: &str) -> TagParse {
    if tag.is_empty() || !tag.starts_with('<') {
        return TagParse::NonColor;
    }
    if tag.starts_with("</color>") {
        return TagParse::Close;
    }
    if !tag.starts_with("<color_") {
        return TagParse::NonColor;
    }
    let Some(close) = tag.find('>') else {
        return TagParse::NonColor;
    };
    let Some(name) = tag.get(7..close) else {
        return TagParse::NonColor;
    };
    match color_from_string(name) {
        Ok(color) if Some(color) != color_value(ColorId::CUnset) => TagParse::Open(color),
        _ => TagParse::NonColor,
    }
}

/// The tag that opens the given colour.
///
/// Reference: `get_tag_from_color` (`src/color.cpp:741`).
#[must_use]
pub fn get_tag_from_color(color: NcColor) -> String {
    format!("<color_{}>", string_from_color(color))
}

/// Wraps text in the given colour's tags, one wrap per line.
///
/// Reference: `colorize` (`src/color.cpp:746`). Newlines stay outside the tags,
/// and a trailing newline leaves a wrapped empty last line behind.
#[must_use]
pub fn colorize(text: &str, color: NcColor) -> String {
    let tag = get_tag_from_color(color);
    if text.find('\n').is_none() {
        return format!("{tag}{text}</color>");
    }
    let mut out = String::new();
    let mut start = 0;
    while let Some(found) = text.get(start..).and_then(|rest| rest.find('\n')) {
        let end = start + found;
        out.push_str(&tag);
        out.push_str(text.get(start..end).unwrap_or(""));
        out.push_str("</color>\n");
        start = end + 1;
    }
    out.push_str(&tag);
    out.push_str(text.get(start..).unwrap_or(""));
    out.push_str("</color>");
    out
}

/// The backgrounds a colour can be highlighted against.
///
/// Reference: `enum hl_enum` (`src/color.h:345`). Row zero is the plain
/// highlight, whose background name is empty and which reaches the `h_` family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Highlight {
    /// Black on blue — the plain highlight.
    Blue,
    /// On red: monsters on fire.
    Red,
    /// On white.
    White,
    /// On green.
    Green,
    /// On yellow.
    Yellow,
    /// On magenta.
    Magenta,
    /// On cyan.
    Cyan,
}

impl Highlight {
    /// The suffix the highlight cache appends, `None` for the blue highlight
    /// because that one borrows the `h_` family instead.
    const fn suffix(self) -> Option<&'static str> {
        match self {
            Self::Blue => None,
            Self::Red => Some("red"),
            Self::White => Some("white"),
            Self::Green => Some("green"),
            Self::Yellow => Some("yellow"),
            Self::Magenta => Some("magenta"),
            Self::Cyan => Some("cyan"),
        }
    }
}

/// The cached highlight of a colour, if the reference caches one.
///
/// Reference: the highlight loop in `color_manager::finalize`
/// (`src/color.cpp:89`). Only plain `c_` names qualify — `unset`, the `h_`/`i_`
/// families and the already-paired `c_x_y` names have no highlight, and callers
/// substitute their own default for `None`.
#[must_use]
pub fn highlight(color: NcColor, background: Highlight) -> Option<NcColor> {
    let def = color_def(color_to_id(color)?)?;
    let root = def.name.strip_prefix("c_")?;
    let pairs = root.matches('_').count()
        - usize::from(root.contains("light_"))
        - usize::from(root.contains("dark_"));
    if root == "unset" || pairs > 0 {
        return None;
    }
    let target = match background.suffix() {
        None => format!("h_{root}"),
        Some(suffix) => format!("{}_{suffix}", def.name),
    };
    color_from_name(&target).and_then(color_value)
}

/// The plain (blue) highlight of a colour, falling back to `h_white`.
///
/// Reference: `hilite` (`src/color.cpp:550`).
#[must_use]
pub fn hilite(color: NcColor) -> NcColor {
    highlight(color, Highlight::Blue)
        .or_else(|| color_value(ColorId::HWhite))
        .unwrap_or_default()
}

/// The red-background form of a colour, falling back to `c_white_red`.
///
/// Reference: `red_background` (`src/color.cpp:556`).
#[must_use]
pub fn red_background(color: NcColor) -> NcColor {
    highlight(color, Highlight::Red)
        .or_else(|| color_value(ColorId::CWhiteRed))
        .unwrap_or_default()
}

/// The white-background form of a colour, falling back to `c_black_white`.
///
/// Reference: `white_background` (`src/color.cpp:562`).
#[must_use]
pub fn white_background(color: NcColor) -> NcColor {
    highlight(color, Highlight::White)
        .or_else(|| color_value(ColorId::CBlackWhite))
        .unwrap_or_default()
}

/// The green-background form of a colour, falling back to `c_black_green`.
///
/// Reference: `green_background` (`src/color.cpp:568`).
#[must_use]
pub fn green_background(color: NcColor) -> NcColor {
    highlight(color, Highlight::Green)
        .or_else(|| color_value(ColorId::CBlackGreen))
        .unwrap_or_default()
}

/// The yellow-background form of a colour, falling back to `c_black_yellow`.
///
/// Reference: `yellow_background` (`src/color.cpp:574`).
#[must_use]
pub fn yellow_background(color: NcColor) -> NcColor {
    highlight(color, Highlight::Yellow)
        .or_else(|| color_value(ColorId::CBlackYellow))
        .unwrap_or_default()
}

/// The magenta-background form of a colour, falling back to `c_black_magenta`.
///
/// Reference: `magenta_background` (`src/color.cpp:580`).
#[must_use]
pub fn magenta_background(color: NcColor) -> NcColor {
    highlight(color, Highlight::Magenta)
        .or_else(|| color_value(ColorId::CBlackMagenta))
        .unwrap_or_default()
}

/// The cyan-background form of a colour, falling back to `c_black_cyan`.
///
/// Reference: `cyan_background` (`src/color.cpp:586`).
#[must_use]
pub fn cyan_background(color: NcColor) -> NcColor {
    highlight(color, Highlight::Cyan)
        .or_else(|| color_value(ColorId::CBlackCyan))
        .unwrap_or_default()
}

/// The colour's invert, falling back to `c_pink`.
///
/// Reference: `invert_color` (`src/color.cpp:544`). The fallback is dead in
/// practice: every table row inverts to a row with a non-zero attribute, and an
/// unknown colour resolves through `c_unset`, which inverts to itself.
#[must_use]
pub fn invert_color(color: NcColor) -> NcColor {
    // An unknown colour is resolved as `c_unset` (`color_to_id` does the same),
    // not treated as a miss, which is why `c_unset` comes back for it.
    let id = color_to_id(color).unwrap_or(ColorId::CUnset);
    color_def(id)
        .and_then(|def| color_value(def.invert))
        .or_else(|| color_value(ColorId::CPink))
        .unwrap_or_default()
}

/// The sixteen palette names `color_loader` reads, in file order.
///
/// Reference: `color_loader::main_color_names` (`src/color_loader.h:27`) and the
/// keys of `data/raw/colors.json`.
pub const PALETTE_NAMES: [&str; 16] = [
    "BLACK", "RED", "GREEN", "BROWN", "BLUE", "MAGENTA", "CYAN", "GRAY", "DGRAY", "LRED", "LGREEN",
    "YELLOW", "LBLUE", "LMAGENTA", "LCYAN", "WHITE",
];

/// The RGB values behind [`PALETTE_NAMES`].
///
/// Reference: `data/raw/colors.json`, which the graphics build loads through
/// `color_loader` and stores opaquely (`src/sdltiles.cpp:3895` sets alpha to 0xFF).
///
/// The first eight follow the `base_color` codes of `src/cursesdef.h:93` — but
/// note the names disagree with that enum: its `yellow` is the palette's `BROWN`
/// and its `white` is the palette's `GRAY`. Bold selects the upper eight, which
/// is where the enum's `yellow` and `white` actually live.
pub const PALETTE: [Rgb; 16] = [
    Rgb::new(0, 0, 0, 255),
    Rgb::new(255, 0, 0, 255),
    Rgb::new(0, 110, 0, 255),
    Rgb::new(97, 56, 28, 255),
    Rgb::new(10, 10, 220, 255),
    Rgb::new(139, 58, 98, 255),
    Rgb::new(0, 150, 180, 255),
    Rgb::new(150, 150, 150, 255),
    Rgb::new(99, 99, 99, 255),
    Rgb::new(255, 150, 150, 255),
    Rgb::new(0, 255, 0, 255),
    Rgb::new(255, 255, 0, 255),
    Rgb::new(100, 100, 255, 255),
    Rgb::new(254, 0, 254, 255),
    Rgb::new(0, 240, 255, 255),
    Rgb::new(255, 255, 255, 255),
];

/// The RGB colour the graphics build draws a terminal colour with.
///
/// Reference: `curses_color_to_SDL` (`src/sdl_utils.cpp:151`). The pair's
/// foreground wins unless it is black, in which case the background does; bold
/// then shifts the index by eight into the bright half of the palette.
///
/// This is the whole of the terminal-versus-graphics difference: the terminal
/// build sends the attribute to curses and never computes RGB, and the graphics
/// build recovers RGB from the pair alone — so an inverted colour and its plain
/// counterpart have identical foregrounds, and `dark_gray`, which
/// `src/cursesdef.h:90` codes as 237, would index past this sixteen-entry
/// palette if any pair ever used it (none do: pairs 80-87 are registered by
/// `init_colors` but no named colour reaches them).
#[must_use]
pub fn curses_color_to_rgb(color: NcColor) -> Option<Rgb> {
    let pair_index = usize::try_from(color.to_color_pair_index()).ok()?;
    let pair = COLOR_PAIRS.get(pair_index)?;
    let mut palette_index = if pair.fg != 0 {
        usize::from(pair.fg)
    } else {
        usize::from(pair.bg)
    };
    if color.is_bold() {
        palette_index += PALETTE_NAMES.len() / 2;
    }
    PALETTE.get(palette_index).copied()
}

/// Greyscale, keeping black black and never letting a lit pixel go dark.
///
/// Reference: `color_pixel_grayscale` (`src/sdl_utils.cpp:80`).
#[must_use]
#[allow(clippy::cast_possible_truncation)] // The shift caps the result at 255.
pub fn color_pixel_grayscale(color: Rgb) -> Rgb {
    if color.is_black() {
        return color;
    }
    // Every channel is promoted to an int in the reference; `u8` arithmetic would
    // overflow here, so the whole sum is done in `u32`.
    let average = u32::from(color.average_pixel_color());
    let level = ((average * 5) >> 3).max(1);
    Rgb::new(level as u8, level as u8, level as u8, color.alpha())
}

/// Darkens to roughly a third, clamping the floor at one.
///
/// Reference: `color_pixel_darken` (`src/sdl_utils.cpp:120`).
#[must_use]
#[allow(clippy::cast_possible_truncation)] // 85 * 255 >> 8 is 84.
pub fn color_pixel_darken(color: Rgb) -> Rgb {
    if color.is_black() {
        return color;
    }
    let dim = |channel: u8| {
        let scaled = (85 * u32::from(channel)) >> 8;
        if scaled > 1 { scaled as u8 } else { 1 }
    };
    Rgb::new(
        dim(color.red()),
        dim(color.green()),
        dim(color.blue()),
        color.alpha(),
    )
}

/// Shifts the average towards green, as night vision does.
///
/// Reference: `color_pixel_nightvision` (`src/sdl_utils.cpp:90`).
#[must_use]
#[allow(clippy::cast_possible_truncation)] // Both bounds are clamped to 255.
pub fn color_pixel_nightvision(color: Rgb) -> Rgb {
    let average = u32::from(color.average_pixel_color());
    let boosted = ((average * (((average * 3) >> 2) + 64)) >> 8) + 16;
    let level = if boosted < 0xFF { boosted } else { 0xFF } as u8;
    Rgb::new(level >> 2, level, level >> 3, color.alpha())
}

/// Shifts the average towards green and lifts it, as overexposure does.
///
/// Reference: `color_pixel_overexposed` (`src/sdl_utils.cpp:102`).
#[must_use]
#[allow(clippy::cast_possible_truncation)] // The result is clamped to 255.
pub fn color_pixel_overexposed(color: Rgb) -> Rgb {
    let average = u32::from(color.average_pixel_color());
    let lifted = 64 + ((average * ((average >> 2) + 0xC0)) >> 8);
    let level = if lifted < 0xFF { lifted } else { 0xFF } as u8;
    Rgb::new(level >> 2, level, level >> 3, color.alpha())
}

/// Blends between two colours by a gamma-corrected brightness weight.
///
/// Reference: `color_pixel_mixer` (`src/sdl_utils.cpp:134`). Unlike the rest of
/// the RGB arithmetic this one rounds, and caps the weight at 100.
#[must_use]
#[allow(clippy::cast_possible_truncation)] // The weight is capped at 100.
#[allow(clippy::cast_sign_loss)] // The weight is non-negative by construction.
pub fn color_pixel_mixer(color: Rgb, gamma: f32, dark: Rgb, light: Rgb) -> Rgb {
    if color.is_black() {
        return color;
    }
    let brightness = f32::from(color.average_pixel_color()) / 255.0;
    let weight = (brightness.powf(gamma) * 150.0).round().min(100.0);
    mix_colors(dark, light, weight as i32)
}

/// The light sepia filter.
///
/// Reference: `color_pixel_sepia_light` (`src/sdl_utils.h:66`).
#[must_use]
pub fn color_pixel_sepia_light(color: Rgb) -> Rgb {
    color_pixel_mixer(
        color,
        1.6,
        Rgb::new(39, 23, 19, color.alpha()),
        Rgb::new(241, 220, 163, color.alpha()),
    )
}
