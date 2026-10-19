//! printf-compatible formatting.
//!
//! Reference: `src/string_formatter.h` (the parser, the `convert` overloads and
//! `string_format`), `src/string_formatter.cpp` (the cursor helpers and the error
//! text) and `tests/string_formatter_test.cpp` (the expectations; all 339 active
//! rows of its table are reproduced in `crates/core-types/tests/text.rs`).
//!
//! # What is ported
//!
//! `cata::string_formatter` walks the format string one conversion at a time and
//! formats each argument separately (`src/string_formatter.h:341-370`). That is
//! what makes it type-safe: it converts the argument to the type the conversion
//! asks for, instead of letting `printf` read a `double` out of an `int`.
//!
//! A conversion is read in this order (`src/string_formatter.h:357-368`): a
//! positional argument index (`%1$s`), the flags `#0 +'I-`
//! (`src/string_formatter.cpp:28-34`), a field width or `*`, a precision or `.*`
//! (`src/string_formatter.cpp:72-102`), a length modifier (`l`, `ll`, `h`,
//! `hh`, `z`, `t`, all of which are dropped: `src/string_formatter.h:274-287`),
//! and one conversion character:
//!
//! | conversion | the argument becomes | arguments accepted |
//! |---|---|---|
//! | `%c` | `int` (`:291-292`) | integer, `char` |
//! | `%d`, `%i` | `signed long long int` (`:294-297`) | integer, `char` |
//! | `%o`, `%u`, `%x`, `%X` | `unsigned long long int` (`:298-304`) | integer, `char`, wrapped the way C wraps them |
//! | `%f`, `%F`, `%e`, `%E`, `%g`, `%G`, `%a`, `%A` | `double` (`:305-314`) | floating point **only** |
//! | `%s` | `const char *` (`:318-320`) | string, `char`, integer, floating point |
//! | `%p` | `void *` (`:315-317`) | pointer |
//!
//! `%%` is the only escape; a `%` followed by anything else is an unsupported
//! conversion (`:352-355`, `:321-322`). A `bool` argument is accepted by none of
//! the conversions: the reference excludes `bool` from `is_numeric` (`:54-58`)
//! and its catch-all `convert` fails a `static_assert` (`:161-164`), which is a
//! *compile* error there and a returned error here.
//!
//! # Errors: throw and assert on one side, a value on the other
//!
//! The reference reports every problem by throwing a `std::runtime_error`
//! carrying `"{message} at: \"{prefix}|{suffix}\""` (`src/string_formatter.cpp:104-109`),
//! and its `string_format` *catches that throw and returns the message as the
//! formatted string* (`src/string_formatter.h:426-436`,
//! `handle_string_format_error`, `src/string_formatter.cpp:111-118`). Both halves
//! are kept, as values:
//!
//! - [`string_format`] returns `Err(FormatError)` where the reference throws —
//!   the throwing form, which is what the reference's own tests call
//!   (`tests/string_formatter_test.cpp:17-22`). [`FormatError::kind`] is the
//!   error as data, and [`fmt::Display`] renders the reference's text.
//! - [`string_format_or_error`] is the swallowing form: the error text *is* the
//!   result, exactly as the reference's wrapper behaves.
//!
//! The reference's one `assert` in this code (`src/string_formatter.cpp:55`) is
//! unreachable — its own comment says so — and needs no counterpart.
//!
//! # Out of scope
//!
//! Translation lookup and plural selection (`src/translations.h`), colour tags,
//! and text folding and wrapping. A `translation` argument in the reference is
//! `std::string`-like to the formatter (`src/string_formatter.h:127-132`), so it
//! reaches this port as [`Arg::Str`] holding text that has already been looked up.

use core::fmt;

/// One argument to be formatted.
///
/// The reference decides what an argument means from its C++ type
/// (`src/string_formatter.h:54-81`), and it recognises exactly these kinds:
/// integers including `char` (but not `bool`), floating point, `std::string` /
/// `std::string_view` / C strings, `translation`, `cata_path` and pointers.
/// Rust has no overloading, so the same set is spelled out as a value. `bool` is
/// in the list only so that it can be *rejected*: the reference excludes it from
/// `is_numeric` (`src/string_formatter.h:54-58`) and no conversion accepts it,
/// which makes passing one a compile error (`src/string_formatter.h:161-164`).
#[derive(Debug, Clone, PartialEq)]
pub enum Arg<'a> {
    /// Any integer type. The reference converts every integer to `long long`
    /// (`src/string_formatter.h:283-304`), so one variant covers all widths and
    /// signedness; `u64` values beyond `i64::MAX` travel as the same bit pattern.
    Int(i64),
    /// Any floating point type, converted to `double` (`src/string_formatter.h:305-314`).
    Float(f64),
    /// `std::string`, `std::string_view` or a C string.
    Str(&'a str),
    /// `char`, which is distinct from the integers because `%s` prints it as a
    /// one-character string (`src/string_formatter.h:147-152`).
    Char(char),
    /// A pointer, printed by `%p` (`src/string_formatter.h:315-317`). Rust knows
    /// no `void *`, so the address is carried as a number; `0` is null.
    Pointer(u64),
    /// `bool`: accepted by no conversion (see the type docs).
    Bool(bool),
}

/// The kind of an [`Arg`], for error reports.
///
/// The reference names the argument's C++ type through `demangle(typeid(T).name())`
/// in its message (`src/string_formatter.h:165-166`); a Rust caller cannot be
/// told an `int` from a `long long`, because [`Arg::Int`] is both, so the names
/// here are the kinds [`Arg`] actually distinguishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgKind {
    /// [`Arg::Int`].
    Integer,
    /// [`Arg::Float`].
    FloatingPoint,
    /// [`Arg::Str`].
    String,
    /// [`Arg::Char`].
    Character,
    /// [`Arg::Pointer`].
    Pointer,
    /// [`Arg::Bool`].
    Bool,
}

impl ArgKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Integer => "integer",
            Self::FloatingPoint => "floating point",
            Self::String => "string",
            Self::Character => "character",
            Self::Pointer => "pointer",
            Self::Bool => "bool",
        }
    }
}

impl Arg<'_> {
    /// Which kind this argument is.
    #[must_use]
    pub const fn kind(&self) -> ArgKind {
        match self {
            Self::Int(_) => ArgKind::Integer,
            Self::Float(_) => ArgKind::FloatingPoint,
            Self::Str(_) => ArgKind::String,
            Self::Char(_) => ArgKind::Character,
            Self::Pointer(_) => ArgKind::Pointer,
            Self::Bool(_) => ArgKind::Bool,
        }
    }
}

/// What a conversion asked the argument to become.
///
/// Each variant is one `RT` of `get_nth_arg_as<RT>` (`src/string_formatter.h:290-320`):
/// `%c` asks for `int`, `%d`/`%i` for `signed long long int`, `%o`/`%u`/`%x`/`%X`
/// for `unsigned long long int`, the float conversions for `double`, `%s` for
/// `const char *` and `%p` for `void *`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conversion {
    /// `%c`: converted to `int` (`src/string_formatter.h:291-292`).
    Character,
    /// `%d`, `%i`: converted to `signed long long int` (`src/string_formatter.h:294-297`).
    SignedInteger,
    /// `%o`, `%u`, `%x`, `%X`: converted to `unsigned long long int`
    /// (`src/string_formatter.h:298-304`).
    UnsignedInteger,
    /// `%f`, `%F`, `%e`, `%E`, `%g`, `%G`, `%a`, `%A`: converted to `double`
    /// (`src/string_formatter.h:305-314`).
    FloatingPoint,
    /// `%p`: converted to `void *` (`src/string_formatter.h:315-317`).
    Pointer,
    /// `%s`: converted to `const char *` (`src/string_formatter.h:318-320`).
    String,
}

impl Conversion {
    fn as_str(self) -> &'static str {
        match self {
            // The names are the reference's own `RT`s, which is what its
            // "Tried to convert ... to ..." message prints.
            Self::Character => "int",
            Self::SignedInteger => "signed long long int",
            Self::UnsignedInteger => "unsigned long long int",
            Self::FloatingPoint => "double",
            Self::Pointer => "void *",
            Self::String => "const char *",
        }
    }
}

/// Why a format string could not be formatted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatErrorKind {
    /// A conversion character the reference does not implement
    /// (`src/string_formatter.h:321-322`). Also what a format that ends in a
    /// lone `%` produces, because the parser reads the end of the format as a
    /// null character (`src/string_formatter.cpp:8-11`).
    UnsupportedConversion(char),
    /// The argument cannot become what the conversion asked for: the reference's
    /// catch-all `convert` overload (`src/string_formatter.h:157-168`), whose
    /// message is `"Tried to convert argument of type T to RT, which is not
    /// possible"`.
    UnsupportedArgument {
        /// What kind the argument was.
        argument: ArgKind,
        /// What the conversion wanted.
        target: Conversion,
    },
    /// A `*` width or precision with a positional argument index but no `$`.
    /// The reference's message calls it "field precision" even when it is the
    /// width that was being read (`src/string_formatter.cpp:80`).
    ExpectedDollar,
    /// The conversion asked for an argument that was not supplied
    /// (`src/string_formatter.h:256-259`): `"Requested argument N but input has
    /// only M"`.
    MissingArgument {
        /// The zero-based argument index the conversion asked for.
        requested: usize,
        /// How many arguments were supplied.
        available: usize,
    },
}

impl FormatErrorKind {
    /// The reference's message for this kind, before the position is appended
    /// (`src/string_formatter.cpp:104-109`).
    fn message(&self) -> String {
        match self {
            Self::UnsupportedConversion(conversion) => {
                format!("Unsupported format conversion: {conversion}")
            }
            Self::UnsupportedArgument { argument, target } => format!(
                "Tried to convert argument of type {} to {}, which is not possible",
                argument.as_str(),
                target.as_str()
            ),
            Self::ExpectedDollar => "expected '$' after field precision".to_owned(),
            Self::MissingArgument {
                requested,
                available,
            } => format!("Requested argument {requested} but input has only {available}"),
        }
    }
}

/// A failed [`string_format`]: what went wrong, and where in the format.
///
/// Reference: `string_formatter::throw_error` (`src/string_formatter.cpp:104-109`),
/// which throws `std::runtime_error` with the message
/// `"{msg} at: \"{prefix}|{suffix}\""` where the prefix is the part of the format
/// already consumed. The kind is a value rather than a string so callers can
/// match on it; [`fmt::Display`] renders the reference's exact text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatError {
    kind: FormatErrorKind,
    format: String,
    position: usize,
}

impl FormatError {
    fn new(kind: FormatErrorKind, format: &str, position: usize) -> Self {
        Self {
            kind,
            format: format.to_owned(),
            position,
        }
    }

    /// What went wrong, as a value that can be matched on.
    #[must_use]
    pub fn kind(&self) -> &FormatErrorKind {
        &self.kind
    }

    /// Byte offset into the format string where parsing stopped.
    #[must_use]
    pub fn position(&self) -> usize {
        self.position
    }

    /// The format string that failed.
    #[must_use]
    pub fn format(&self) -> &str {
        &self.format
    }
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The two halves of the format are split at the current position, with
        // the reference's `|` marker between them (`string_formatter.cpp:106-108`).
        let prefix = self.format.get(..self.position).unwrap_or_default();
        let suffix = self.format.get(self.position..).unwrap_or_default();
        write!(f, "{} at: \"{prefix}|{suffix}\"", self.kind.message())
    }
}

impl core::error::Error for FormatError {}

/// The flags a conversion may carry.
///
/// Reference: `read_flags` (`src/string_formatter.cpp:28-34`), whose set of
/// recognised characters is `"#0 +'I-"`, stored here as a set rather than as
/// seven separate booleans. The two locale flags are accepted and kept because
/// the reference forwards them to `sprintf`, but this port has no locale: `'`
/// (grouping) and `I` (locale digits) never change the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Flags(u8);

impl Flags {
    /// `#`: alternate form, `0x` for `%x`, a leading `0` for `%o`.
    const ALTERNATE: u8 = 1 << 0;
    /// `0`: pad with zeroes instead of spaces.
    const ZERO: u8 = 1 << 1;
    /// ` `: a space stands where a sign would be.
    const SPACE: u8 = 1 << 2;
    /// `+`: always print a sign.
    const PLUS: u8 = 1 << 3;
    /// `-`: left-justify within the field width.
    const LEFT: u8 = 1 << 4;
    /// `'`: group digits, which needs a locale and so does nothing here.
    const GROUP: u8 = 1 << 5;
    /// `I`: use the locale's digits, which likewise does nothing here.
    const LOCALE_DIGITS: u8 = 1 << 6;

    /// Every flag character the reference recognises (`src/string_formatter.cpp:30`).
    const CHARACTERS: &'static str = "#0 +'I-";

    /// Reads and consumes flag characters (`src/string_formatter.cpp:28-34`).
    fn read(cursor: &mut Cursor<'_>) -> Self {
        let mut flags = Self::default();
        while let Some(flag) = Self::CHARACTERS
            .chars()
            .find(|flag| *flag == cursor.get_current_input())
        {
            cursor.consume_next_input();
            flags.0 |= match flag {
                '#' => Self::ALTERNATE,
                '0' => Self::ZERO,
                ' ' => Self::SPACE,
                '+' => Self::PLUS,
                '-' => Self::LEFT,
                '\'' => Self::GROUP,
                'I' => Self::LOCALE_DIGITS,
                _ => 0,
            };
        }
        flags
    }

    /// Whether every flag in `other` is set in `self`.
    const fn has(self, other: u8) -> bool {
        self.0 & other != 0
    }
}

/// The sign `printf` prints for `value`, honouring `+` and ` `.
fn float_sign(value: f64, flags: Flags) -> &'static str {
    if value.is_sign_negative() {
        "-"
    } else if flags.has(Flags::PLUS) {
        "+"
    } else if flags.has(Flags::SPACE) {
        " "
    } else {
        ""
    }
}

/// The digits of `value` in fixed point, without a sign.
///
/// The digits themselves come straight from Rust's own `{:.p$}` formatting,
/// which rounds the exact binary value the same way `printf` does (halves to
/// even; verified against glibc for every row of the reference's table). What
/// is added here is what `printf` does and Rust does not: `inf`/`nan` spelled
/// in the case the conversion asked for, and the `#` flag's forced decimal
/// point (`%#.0f` of 1 is `1.`).
fn fixed_magnitude(
    value: f64,
    precision: Option<usize>,
    alternate: bool,
    upper_case: bool,
) -> String {
    let magnitude = value.abs();
    let mut digits = if magnitude.is_nan() {
        if upper_case { "NAN" } else { "nan" }.to_owned()
    } else if magnitude.is_infinite() {
        if upper_case { "INF" } else { "inf" }.to_owned()
    } else {
        let precision = precision.unwrap_or(6);
        format!("{magnitude:.precision$}")
    };
    if alternate && !digits.contains('.') && magnitude.is_finite() {
        digits.push('.');
    }
    digits
}

/// The digits of `value` in hexadecimal float style, without a sign.
///
/// `%a` prints `0x`, one leading digit, the hexadecimal fraction and a binary
/// exponent (`0x1.8p+0` for 1.5). The thirteen fraction digits are the exact
/// low 52 bits of the double, so nothing is rounded unless a precision asks for
/// fewer digits, in which case `printf` rounds them and lets a carry turn the
/// leading digit into a 2 rather than renormalising (`%.1a` of 1.96875 is
/// `0x2.0p+0`, checked against glibc).
/// The low 52 bits of a double: the fraction, without the implicit leading bit.
const FRACTION_MASK: u64 = (1 << 52) - 1;

fn hexadecimal_magnitude(
    value: f64,
    precision: Option<usize>,
    alternate: bool,
    upper_case: bool,
) -> String {
    let magnitude = value.abs();
    if !magnitude.is_finite() {
        return fixed_magnitude(magnitude, None, false, upper_case);
    }
    let bits = magnitude.to_bits();
    let biased = u32::try_from((bits >> 52) & 0x7ff).unwrap_or(0);
    let fraction = bits & FRACTION_MASK;
    let is_zero = biased == 0 && fraction == 0;
    // `printf` prints subnormals in the normalised `1.f` form too, with the
    // binary exponent of the leading bit (`%a` of 1e-310 is `0x1.2688b70e62bp-1030`).
    let (leading, exponent) = if is_zero {
        (0_u64, 0_i32)
    } else if biased == 0 {
        (1_u64, i32::try_from(fraction.ilog2()).unwrap_or(0) - 1074)
    } else {
        (1_u64, i32::try_from(biased).unwrap_or(0) - 1023)
    };
    // A normalised fraction always has its leading bit implicit, so a subnormal
    // loses it when it is turned into the `1.f` form.
    let fraction = if biased == 0 && !is_zero {
        (fraction << (52 - fraction.ilog2())) & FRACTION_MASK
    } else {
        fraction
    };

    let (leading, digits) = match precision {
        // Round the low bits away, halves to even, carrying into the leading digit.
        Some(0) => {
            // glibc drops an exact half rather than rounding it up (`%.1a` of
            // 1.96875, which is exactly 0x1.f8p+0, prints `0x1.fp+0`), so the
            // comparison is strict; the standard leaves the choice open.
            let half = 1_u64 << 51;
            let round_up = fraction > half;
            (leading + u64::from(round_up), String::new())
        }
        Some(precision) if precision < 13 => {
            let kept_bits = 4 * u32::try_from(precision).unwrap_or(0);
            let dropped = 52 - kept_bits;
            let kept = fraction >> dropped;
            let rest = fraction & ((1 << dropped) - 1);
            let half = 1_u64 << (dropped - 1);
            let round_up = rest > half;
            let kept = kept + u64::from(round_up);
            if kept >> kept_bits == 1 {
                // The fraction rounded past `f` and put the carry in the leading digit.
                (leading + 1, "0".repeat(precision))
            } else {
                (leading, format!("{kept:0precision$x}"))
            }
        }
        // Thirteen digits are all the bits a double has; more are zeroes.
        Some(precision) => {
            let mut text = format!("{fraction:013x}");
            text.extend(core::iter::repeat_n('0', precision.saturating_sub(13)));
            (leading, text)
        }
        // No precision: the exact thirteen digits with the trailing zeros dropped,
        // and none at all when they are all zero (`%a` of 1.0 is `0x1p+0`).
        None => {
            let text = format!("{fraction:013x}");
            let trimmed = text.trim_end_matches('0');
            (leading, trimmed.to_owned())
        }
    };
    let mut output = String::from(if upper_case { "0X" } else { "0x" });
    // The leading digit is 0, 1 or 2, so a single hexadecimal digit always fits.
    output.push(char::from_digit(u32::try_from(leading).unwrap_or(0), 16).unwrap_or('0'));
    if !digits.is_empty() || alternate {
        output.push('.');
        output.push_str(&digits);
    }
    output.push(if upper_case { 'P' } else { 'p' });
    output.push(if exponent < 0 { '-' } else { '+' });
    output.push_str(&exponent.unsigned_abs().to_string());
    if upper_case {
        output.make_ascii_uppercase();
    }
    output
}

/// The digits of `value` in exponent style, without a sign.
///
/// Rust's `{:.p$e}` already rounds like `printf` but writes the exponent the
/// way Rust does (`1.2346e3`); `printf` writes at least two exponent digits and
/// always a sign (`1.2346e+03`).
fn exponential_magnitude(
    value: f64,
    precision: usize,
    alternate: bool,
    upper_case: bool,
) -> String {
    let magnitude = value.abs();
    if !magnitude.is_finite() {
        return fixed_magnitude(magnitude, None, false, upper_case);
    }
    let formatted = format!("{magnitude:.precision$e}");
    let (mantissa, exponent) = formatted
        .split_once('e')
        .map_or((formatted.as_str(), "0"), |(mantissa, exponent)| {
            (mantissa, exponent)
        });
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let (letter, sign) = match (upper_case, exponent < 0) {
        (false, false) => ('e', '+'),
        (false, true) => ('e', '-'),
        (true, false) => ('E', '+'),
        (true, true) => ('E', '-'),
    };
    let mut mantissa = mantissa.to_owned();
    if alternate && !mantissa.contains('.') {
        mantissa.push('.');
    }
    format!("{mantissa}{letter}{sign}{:02}", exponent.unsigned_abs())
}

/// The decimal exponent `printf` compares against the precision to choose
/// between `%e` and `%f` style in `%g` (the `P > X >= -4` rule).
fn decimal_exponent(magnitude: f64) -> i32 {
    if magnitude <= 0.0 || !magnitude.is_finite() {
        return 0;
    }
    format!("{magnitude:e}")
        .split_once('e')
        .and_then(|(_, exponent)| exponent.parse().ok())
        .unwrap_or(0)
}

/// The digits of `value` in `%g` style, without a sign.
fn general_magnitude(
    value: f64,
    precision: Option<usize>,
    alternate: bool,
    upper_case: bool,
) -> String {
    let magnitude = value.abs();
    if !magnitude.is_finite() {
        return fixed_magnitude(magnitude, None, false, upper_case);
    }
    // `%g` reads a precision of zero as one (`printf`).
    let precision = precision.unwrap_or(6).max(1);
    let exponent = decimal_exponent(magnitude);
    let mut digits = if i32::try_from(precision).unwrap_or(i32::MAX) > exponent && exponent >= -4 {
        // `%g` gives `%f` a precision of `P - 1 - X`, where `X` is the decimal
        // exponent and may be negative (`%.3g` of 0.0001234 asks for 6).
        let decimals = usize::try_from(
            i64::try_from(precision.saturating_sub(1)).unwrap_or(i64::MAX) - i64::from(exponent),
        )
        .unwrap_or(0);
        format!("{magnitude:.decimals$}")
    } else {
        exponential_magnitude(magnitude, precision - 1, alternate, upper_case)
    };
    if !alternate {
        digits = strip_trailing_zeros(&digits);
    }
    digits
}

/// Removes the fractional zeros `printf` drops without `#`, in either style,
/// and the point along with them when nothing follows it (`%g` of 8.6 is `8.6`,
/// not `8.60000`).
fn strip_trailing_zeros(text: &str) -> String {
    let (mantissa, suffix) = match text.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => {
            // Keep whichever letter the conversion asked for (`%G` -> `E`).
            let letter = if text.contains('E') { 'E' } else { 'e' };
            (mantissa, format!("{letter}{exponent}"))
        }
        None => (text, String::new()),
    };
    if !mantissa.contains('.') {
        return text.to_owned();
    }
    let trimmed = mantissa.trim_end_matches('0').trim_end_matches('.');
    format!("{trimmed}{suffix}")
}

/// Cuts `text` to at most `limit` bytes, never inside a character.
///
/// `printf` counts the precision of `%s` in bytes; a byte count that lands inside
/// a multi-byte character cannot be reproduced in a Rust `str`, so the cut is
/// moved back to the previous character boundary.
fn truncate_to_bytes(text: &str, limit: usize) -> &str {
    if limit >= text.len() {
        return text;
    }
    let mut end = limit;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.get(..end).unwrap_or_default()
}

/// Zero-pads `digits` to `precision` and, at zero, suppresses a value of zero
/// entirely (`%.0d` of 0 is empty, serial 104; `%20.d` of 0 is all spaces,
/// serial 387).
fn pad_to_precision(digits: &str, precision: Option<usize>) -> String {
    match precision {
        Some(0) if digits == "0" => String::new(),
        Some(precision) => {
            let padding = precision.saturating_sub(digits.chars().count());
            format!("{}{digits}", "0".repeat(padding))
        }
        None => digits.to_owned(),
    }
}

/// A cursor over the format string.
///
/// Mirrors the reference's `consume_next_input` / `get_current_input` /
/// `consume_next_input_if` trio (`src/string_formatter.cpp:8-26`), including the
/// null character that stands in for the end of the format.
struct Cursor<'f> {
    format: &'f str,
    position: usize,
}

impl<'f> Cursor<'f> {
    const fn new(format: &'f str) -> Self {
        Self {
            format,
            position: 0,
        }
    }

    /// The next character, or a null character at the end of the format
    /// (`src/string_formatter.cpp:8-11`).
    fn consume_next_input(&mut self) -> char {
        let current = self.get_current_input();
        if current != '\0' {
            self.position += current.len_utf8();
        }
        current
    }

    /// How far into the format the cursor has read.
    const fn position(&self) -> usize {
        self.position
    }

    /// The next character without consuming it (`src/string_formatter.cpp:13-16`).
    fn get_current_input(&self) -> char {
        self.format
            .get(self.position..)
            .and_then(|rest| rest.chars().next())
            .unwrap_or('\0')
    }

    /// Consumes the next character if it is `wanted` (`src/string_formatter.cpp:18-26`).
    fn consume_next_input_if(&mut self, wanted: char) -> bool {
        if self.get_current_input() == wanted {
            self.consume_next_input();
            true
        } else {
            false
        }
    }
    /// Whether the next character is a decimal digit (`has_digit`,
    /// `src/string_formatter.cpp:36-40`).
    fn has_digit(&self) -> bool {
        self.get_current_input().is_ascii_digit()
    }

    /// Consumes decimal digits and returns them as a number
    /// (`parse_integer`, `src/string_formatter.cpp:62-70`). Returns 0 when the
    /// next character is not a digit. The reference does not check for overflow
    /// ("TODO: Check for overflow") and neither does this.
    fn parse_integer(&mut self) -> usize {
        let mut result: usize = 0;
        while self.has_digit() {
            let digit = self.consume_next_input().to_digit(10).unwrap_or(0);
            result = result * 10 + digit as usize;
        }
        result
    }
}

/// Formats `args` according to `format`, printf-style.
///
/// This is the reference's `throwing_string_format` (the throwing form used by
/// its own tests, `tests/string_formatter_test.cpp:17-22`) and the body of
/// `string_formatter::parse` (`src/string_formatter.h:341-370`).
///
/// # Errors
///
/// Returns [`FormatError`] wherever the reference throws: a conversion the
/// parser does not support, an argument that cannot be converted to what the
/// conversion asks for, or a conversion that refers to an argument that was not
/// supplied.
pub fn string_format(format: &str, args: &[Arg<'_>]) -> Result<String, FormatError> {
    Formatter::new(format, args).parse()
}

/// Where a field width or precision comes from.
///
/// Reference: `read_number_or_argument_index` (`src/string_formatter.cpp:72-88`),
/// which either copies the digits it read into the rebuilt `sprintf` format or
/// returns the index of the argument holding the number.
#[derive(Debug, Clone, Copy)]
enum Amount {
    /// Digits in the format itself.
    Fixed(usize),
    /// The argument at this index (`*`).
    FromArgument(usize),
}

/// Which digits an integer conversion writes.
///
/// Reference: `read_conversion` (`src/string_formatter.h:293-304`), which picks
/// `%lli` for `%d`/`%i` and `%llu`, `%llo`, `%llx` or `%llX` for the unsigned
/// conversions, and therefore never prints a sign for those, whatever `+` or
/// ` ` asks for (serials 204-209).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Base {
    /// `%d`, `%i`, `%u`: decimal.
    Decimal,
    /// `%o`: octal.
    Octal,
    /// `%x`: lowercase hexadecimal.
    LowerHex,
    /// `%X`: uppercase hexadecimal.
    UpperHex,
}

/// The parser state: the reference's `string_formatter` members
/// (`src/string_formatter.h:184-199`).
struct Formatter<'f, 'a> {
    /// Complete format string (`src/string_formatter.h:187`).
    format: &'f str,
    /// Used during parsing to denote the next character to be parsed (`:190`).
    cursor: Cursor<'f>,
    /// The arguments, indexed as `printf` would (`:199`).
    args: &'a [Arg<'a>],
    /// The formatted output string (`:193`).
    output: String,
    /// The *index* (not number) of the next argument to be formatted when the
    /// conversion does not name one (`src/string_formatter.h:199`, `367`).
    next_argument: usize,
}

impl<'f, 'a> Formatter<'f, 'a> {
    fn new(format: &'f str, args: &'a [Arg<'a>]) -> Self {
        Self {
            format,
            cursor: Cursor::new(format),
            args,
            output: String::with_capacity(format.len()),
            next_argument: 0,
        }
    }

    /// `string_formatter::parse` (`src/string_formatter.h:341-370`).
    fn parse(&mut self) -> Result<String, FormatError> {
        loop {
            let c = self.cursor.consume_next_input();
            // The reference loops `while( const char c = consume_next_input() )`, so a
            // null character — the end of the format, or a literal one in it — ends
            // the loop (`src/string_formatter.h:347`).
            if c == '\0' {
                return Ok(core::mem::take(&mut self.output));
            }
            if c != '%' {
                self.output.push(c);
                continue;
            }
            if self.cursor.consume_next_input_if('%') {
                self.output.push('%');
                continue;
            }
            self.parse_conversion()?;
        }
    }

    /// Reads and applies one conversion (`read_conversion`,
    /// `src/string_formatter.h:272-324`).
    fn parse_conversion(&mut self) -> Result<(), FormatError> {
        // A positional argument index comes before the flags
        // (`src/string_formatter.h:357-358`).
        let named_index = self.read_argument_index();
        // Flags come before the field width, which comes before the precision,
        // which comes before the conversion itself (`src/string_formatter.h:357-368`).
        let mut flags = Flags::read(&mut self.cursor);
        let (width, negative_width) = self.resolve_width()?;
        if negative_width {
            // A negative width means "left justify this far" (`printf`): the
            // reference rebuilds the format as `%-3s` by appending "-3"
            // (`src/string_formatter.h:359-362`), so serial 144 pads on the right.
            flags = Flags(flags.0 | Flags::LEFT);
        }
        let precision = self.resolve_precision()?;
        // Which argument this conversion formats: the named one if it named one,
        // otherwise the next in sequence (`src/string_formatter.h:367`). The star
        // arguments above have already advanced the sequence.
        let argument_index = match named_index {
            Some(index) => index,
            None => self.take_argument_index(),
        };
        self.read_length_modifier();
        // The conversion character is consumed before it is looked at (`:288-290`),
        // so an unknown one is reported *after* it, and a format that ends here
        // reports the null character the cursor returns at the end.
        let conversion = self.cursor.consume_next_input();
        match conversion {
            'c' => {
                let value = self.character(argument_index)?;
                self.push_character(value, flags, width);
                Ok(())
            }
            'd' | 'i' => {
                let value = self.signed_integer(argument_index)?;
                self.push_signed(value, flags, width, precision);
                Ok(())
            }
            'o' | 'u' | 'x' | 'X' => {
                let value = self.unsigned_integer(argument_index)?;
                let base = match conversion {
                    'o' => Base::Octal,
                    'x' => Base::LowerHex,
                    'X' => Base::UpperHex,
                    _ => Base::Decimal,
                };
                self.push_unsigned(value, base, flags, width, precision);
                Ok(())
            }
            'f' | 'F' => {
                let value = self.floating_point(argument_index)?;
                self.push_fixed(value, flags, width, precision, conversion == 'F');
                Ok(())
            }
            'e' | 'E' => {
                let value = self.floating_point(argument_index)?;
                self.push_exponential(value, flags, width, precision, conversion == 'E');
                Ok(())
            }
            'g' | 'G' => {
                let value = self.floating_point(argument_index)?;
                self.push_general(value, flags, width, precision, conversion == 'G');
                Ok(())
            }
            'a' | 'A' => {
                let value = self.floating_point(argument_index)?;
                self.push_hexadecimal(value, flags, width, precision, conversion == 'A');
                Ok(())
            }
            'p' => {
                let value = self.pointer(argument_index)?;
                self.push_pointer(value, flags, width);
                Ok(())
            }
            's' => {
                let text = self.string(argument_index)?;
                self.push_string(&text, flags, width, precision);
                Ok(())
            }
            _ => Err(self.error(FormatErrorKind::UnsupportedConversion(conversion))),
        }
    }

    /// Reads a `%N$` positional argument index (`read_argument_index`,
    /// `src/string_formatter.cpp:42-60`).
    ///
    /// The reference looks for the end of the digits with
    /// `find_first_not_of( "012345678" )`, a set that is missing `9`. An index
    /// whose second digit is a `9` therefore fails that lookahead and is *not*
    /// treated as positional at all: `%19$s` is a string with width 19. That is
    /// reproduced here, because the width it falls back to is what the reference
    /// parses too.
    fn read_argument_index(&mut self) -> Option<usize> {
        let first = self.cursor.get_current_input();
        if !first.is_ascii_digit() || first == '0' {
            return None;
        }
        let start = self.cursor.position() + first.len_utf8();
        let following = self
            .format
            .get(start..)
            .unwrap_or_default()
            .chars()
            .find(|character| !"012345678".contains(*character));
        if following != Some('$') {
            return None;
        }
        let index = self.cursor.parse_integer().saturating_sub(1);
        self.cursor.consume_next_input_if('$');
        Some(index)
    }

    /// The argument this conversion formats when it names none: the reference's
    /// `current_argument_index++` (`src/string_formatter.h:367`).
    fn take_argument_index(&mut self) -> usize {
        let index = self.next_argument;
        self.next_argument += 1;
        index
    }

    /// Reads the field width, or the argument it comes from
    /// (`read_width`, `src/string_formatter.cpp:90-93`).
    fn read_width(&mut self) -> Result<Option<Amount>, FormatError> {
        self.read_number_or_argument_index()
    }

    /// Reads the precision (`read_precision`, `src/string_formatter.cpp:95-102`).
    /// A `.` with no digits after it means a precision of zero, which is how
    /// `%20.d` (serial 386) behaves like `%20.0d`.
    fn read_precision(&mut self) -> Result<Option<Amount>, FormatError> {
        if !self.cursor.consume_next_input_if('.') {
            return Ok(None);
        }
        Ok(Some(
            self.read_number_or_argument_index()?
                .unwrap_or(Amount::Fixed(0)),
        ))
    }

    /// The common body of the two (`read_number_or_argument_index`,
    /// `src/string_formatter.cpp:72-88`).
    ///
    /// `*` takes the number from the next argument — or, with a positional
    /// index, from the argument named before the `$`, in which case a missing
    /// `$` is an error.
    fn read_number_or_argument_index(&mut self) -> Result<Option<Amount>, FormatError> {
        if self.cursor.consume_next_input_if('*') {
            if !self.cursor.has_digit() {
                return Ok(Some(Amount::FromArgument(self.take_argument_index())));
            }
            let index = self.cursor.parse_integer().saturating_sub(1);
            if !self.cursor.consume_next_input_if('$') {
                return Err(self.error(FormatErrorKind::ExpectedDollar));
            }
            return Ok(Some(Amount::FromArgument(index)));
        }
        if self.cursor.has_digit() {
            return Ok(Some(Amount::Fixed(self.cursor.parse_integer())));
        }
        Ok(None)
    }

    /// Turns a width into a number, reading an argument if it is a `*`
    /// (`src/string_formatter.h:359-362`). The second value is whether the
    /// argument was negative, which means left justification.
    fn resolve_width(&mut self) -> Result<(Option<usize>, bool), FormatError> {
        match self.read_width()? {
            None => Ok((None, false)),
            Some(Amount::Fixed(width)) => Ok((Some(width), false)),
            Some(Amount::FromArgument(index)) => {
                let value = self.int_value(index)?;
                Ok((
                    Some(usize::try_from(value.unsigned_abs()).unwrap_or(0)),
                    value < 0,
                ))
            }
        }
    }

    /// Turns a precision into a number, reading an argument if it is a `*`
    /// (`src/string_formatter.h:363-366`). A negative precision counts as no
    /// precision at all, as `printf` defines it.
    fn resolve_precision(&mut self) -> Result<Option<usize>, FormatError> {
        match self.read_precision()? {
            None => Ok(None),
            Some(Amount::Fixed(precision)) => Ok(Some(precision)),
            Some(Amount::FromArgument(index)) => {
                let value = self.int_value(index)?;
                Ok(usize::try_from(value).ok())
            }
        }
    }

    /// Consumes and drops the length modifier (`l`, `ll`, `h`, `hh`, `z`, `t`).
    ///
    /// The reference strips them all and adds `ll` back for every integer
    /// conversion (`src/string_formatter.h:274-287`), so `%hhi`, `%i` and `%lli`
    /// accept the same arguments and print the same text; the width, which is
    /// what the modifier would otherwise be confused with, has already been read.
    fn read_length_modifier(&mut self) {
        if self.cursor.consume_next_input_if('l') {
            self.cursor.consume_next_input_if('l');
        } else if self.cursor.consume_next_input_if('h') {
            self.cursor.consume_next_input_if('h');
        } else if !self.cursor.consume_next_input_if('z') {
            self.cursor.consume_next_input_if('t');
        }
    }

    /// A signed decimal with its sign (`%d` / `%i`,
    /// `src/string_formatter.h:293-297`).
    fn push_signed(
        &mut self,
        value: i64,
        flags: Flags,
        width: Option<usize>,
        precision: Option<usize>,
    ) {
        // `%+d` forces a sign and `% d` forces a space, but a negative value
        // always keeps its own `-` (`printf` rules, and serials 11 and 13).
        let sign = if value < 0 {
            "-"
        } else if flags.has(Flags::PLUS) {
            "+"
        } else if flags.has(Flags::SPACE) {
            " "
        } else {
            ""
        };
        let digits = value.unsigned_abs().to_string();
        let digits = pad_to_precision(&digits, precision);
        self.push_field(sign, &digits, flags, width, precision.is_none());
    }

    /// An unsigned conversion in its base (`%o` / `%u` / `%x` / `%X`,
    /// `src/string_formatter.h:298-304`).
    ///
    /// `#` prints `0x`/`0X` in front of a nonzero hexadecimal value and a `0` in
    /// front of an octal one that does not already start with a zero; a zero
    /// value has nothing to mark (serials 245-247).
    fn push_unsigned(
        &mut self,
        value: u64,
        base: Base,
        flags: Flags,
        width: Option<usize>,
        precision: Option<usize>,
    ) {
        let digits = match base {
            Base::Decimal => value.to_string(),
            Base::Octal => format!("{value:o}"),
            Base::LowerHex => format!("{value:x}"),
            Base::UpperHex => format!("{value:X}"),
        };
        // The precision comes first: for octal the alternate form then depends on
        // whether the padded digits already start with a zero (serial 106).
        let digits = pad_to_precision(&digits, precision);
        let prefix = if flags.has(Flags::ALTERNATE) {
            match base {
                Base::Octal if !digits.starts_with('0') => "0",
                Base::LowerHex if value != 0 => "0x",
                Base::UpperHex if value != 0 => "0X",
                Base::Octal | Base::Decimal | Base::LowerHex | Base::UpperHex => "",
            }
        } else {
            ""
        };
        self.push_field(prefix, &digits, flags, width, precision.is_none());
    }

    /// Writes a prefix and digits into the field.
    ///
    /// The `0` flag pads the field to the width — after the prefix, and only when
    /// `-` has not asked for left justification and no precision was given, which
    /// is why serials 20, 30 and 369 do not zero-pad (`printf` field-width rules).
    fn push_field(
        &mut self,
        prefix: &str,
        digits: &str,
        flags: Flags,
        width: Option<usize>,
        zero_allowed: bool,
    ) {
        let content = prefix.chars().count() + digits.chars().count();
        let field = width.unwrap_or(0).saturating_sub(content);
        if flags.has(Flags::LEFT) {
            self.output.push_str(prefix);
            self.output.push_str(digits);
            self.output.extend(core::iter::repeat_n(' ', field));
        } else if flags.has(Flags::ZERO) && zero_allowed {
            self.output.push_str(prefix);
            self.output.extend(core::iter::repeat_n('0', field));
            self.output.push_str(digits);
        } else {
            self.output.extend(core::iter::repeat_n(' ', field));
            self.output.push_str(prefix);
            self.output.push_str(digits);
        }
    }

    /// The argument at `index`, or the reference's "requested argument" error
    /// (`src/string_formatter.h:256-259`).
    fn argument(&self, index: usize) -> Result<&'a Arg<'a>, FormatError> {
        self.args.get(index).ok_or_else(|| {
            self.error(FormatErrorKind::MissingArgument {
                requested: index,
                available: self.args.len(),
            })
        })
    }

    /// `get_nth_arg_as<signed long long int>` (`src/string_formatter.h:294-297`):
    /// any integer, including `char` (`:83-94`).
    fn signed_integer(&self, index: usize) -> Result<i64, FormatError> {
        match self.argument(index)? {
            Arg::Int(value) => Ok(*value),
            Arg::Char(value) => Ok(i64::from(u32::from(*value))),
            other => Err(self.unsupported_argument(other, Conversion::SignedInteger)),
        }
    }

    /// `get_nth_arg_as<unsigned long long int>` (`src/string_formatter.h:298-304`):
    /// any integer, including `char`, converted the way C converts it, which is
    /// why `%x` of -100 prints the 64-bit two's complement (serial 105).
    fn unsigned_integer(&self, index: usize) -> Result<u64, FormatError> {
        match self.argument(index)? {
            Arg::Int(value) => Ok(value.cast_unsigned()),
            Arg::Char(value) => Ok(u64::from(u32::from(*value))),
            other => Err(self.unsupported_argument(other, Conversion::UnsignedInteger)),
        }
    }

    /// `get_nth_arg_as<const char *>` for `%s` (`src/string_formatter.h:318-320`).
    ///
    /// The reference accepts a string as it is, a `char` as a one-character
    /// string, and a number by running `std::to_string` on it
    /// (`src/string_formatter.h:139-152`) — which is why `%s` of 5 is "5" and not
    /// a string conversion of any kind: the *text* is never scanned for
    /// conversions again (serial 169).
    fn string(&self, index: usize) -> Result<String, FormatError> {
        match self.argument(index)? {
            Arg::Str(text) => Ok((*text).to_owned()),
            Arg::Char(value) => Ok(value.to_string()),
            Arg::Int(value) => Ok(value.to_string()),
            Arg::Float(value) => Ok(format!(
                "{}{}",
                float_sign(*value, Flags::default()),
                fixed_magnitude(*value, None, false, false)
            )),
            other => Err(self.unsupported_argument(other, Conversion::String)),
        }
    }

    /// `%s`: the text, cut to the precision, padded to the width with spaces.
    /// `0` does nothing here, because `printf` gives strings no zero-padding.
    fn push_string(
        &mut self,
        text: &str,
        flags: Flags,
        width: Option<usize>,
        precision: Option<usize>,
    ) {
        let text = precision.map_or(text, |limit| truncate_to_bytes(text, limit));
        self.push_field("", text, flags, width, false);
    }

    /// `get_nth_arg_as<double>` (`src/string_formatter.h:305-314`).
    ///
    /// Only a floating point argument converts: `convert<double>` is enabled for
    /// `is_floating_point<RT> && is_numeric<T> && !is_integer<T>`
    /// (`src/string_formatter.h:95-101`), so `%f` of an *integer* falls through to
    /// the catch-all and throws (`:157-168`) — the opposite of what a `printf`
    /// user would expect, and of the comment at `:418-419`.
    fn floating_point(&self, index: usize) -> Result<f64, FormatError> {
        match self.argument(index)? {
            Arg::Float(value) => Ok(*value),
            other => Err(self.unsupported_argument(other, Conversion::FloatingPoint)),
        }
    }

    /// `%f` / `%F` (`printf`'s fixed point style).
    fn push_fixed(
        &mut self,
        value: f64,
        flags: Flags,
        width: Option<usize>,
        precision: Option<usize>,
        upper_case: bool,
    ) {
        let sign = float_sign(value, flags);
        let digits = fixed_magnitude(value, precision, flags.has(Flags::ALTERNATE), upper_case);
        // `printf` does not zero-pad an infinity or a NaN.
        self.push_field(sign, &digits, flags, width, value.is_finite());
    }

    /// `%e` / `%E` (`printf`'s exponent style).
    fn push_exponential(
        &mut self,
        value: f64,
        flags: Flags,
        width: Option<usize>,
        precision: Option<usize>,
        upper_case: bool,
    ) {
        let sign = float_sign(value, flags);
        let digits = exponential_magnitude(
            value,
            precision.unwrap_or(6),
            flags.has(Flags::ALTERNATE),
            upper_case,
        );
        self.push_field(sign, &digits, flags, width, value.is_finite());
    }

    /// `%g` / `%G`: `%e` or `%f`, whichever `printf` picks for the exponent, with
    /// trailing zeros dropped unless `#`.
    fn push_general(
        &mut self,
        value: f64,
        flags: Flags,
        width: Option<usize>,
        precision: Option<usize>,
        upper_case: bool,
    ) {
        let sign = float_sign(value, flags);
        let digits = general_magnitude(value, precision, flags.has(Flags::ALTERNATE), upper_case);
        self.push_field(sign, &digits, flags, width, value.is_finite());
    }

    /// `%a` / `%A` (`printf`'s hexadecimal float style).
    fn push_hexadecimal(
        &mut self,
        value: f64,
        flags: Flags,
        width: Option<usize>,
        precision: Option<usize>,
        upper_case: bool,
    ) {
        let sign = float_sign(value, flags);
        let digits =
            hexadecimal_magnitude(value, precision, flags.has(Flags::ALTERNATE), upper_case);
        self.push_field(sign, &digits, flags, width, value.is_finite());
    }

    /// `get_nth_arg_as<void *>` for `%p` (`src/string_formatter.h:315-317`).
    fn pointer(&self, index: usize) -> Result<u64, FormatError> {
        match self.argument(index)? {
            Arg::Pointer(value) => Ok(*value),
            other => Err(self.unsupported_argument(other, Conversion::Pointer)),
        }
    }

    /// `%p`: the address in lowercase hex behind a `0x`, zero-padded after the
    /// prefix when `0` asks for it (glibc's behaviour, which the standard leaves
    /// undefined).
    fn push_pointer(&mut self, value: u64, flags: Flags, width: Option<usize>) {
        self.push_field("0x", &format!("{value:x}"), flags, width, true);
    }

    /// `get_nth_arg_as<int>` for `%c` (`src/string_formatter.h:291-292`): any
    /// integer, including `char`; `printf` then takes its low byte.
    fn character(&self, index: usize) -> Result<char, FormatError> {
        let value = self.int_value(index)?;
        Ok(char::from(
            u8::try_from(value.cast_unsigned() & 0xff).unwrap_or(0),
        ))
    }

    /// `get_nth_arg_as<int>`: any integer, including `char`. Used by `%c` and by
    /// a `*` width or precision, which the reference reads with the same call
    /// (`src/string_formatter.h:360`, `364`).
    fn int_value(&self, index: usize) -> Result<i64, FormatError> {
        match self.argument(index)? {
            Arg::Int(value) => Ok(*value),
            Arg::Char(value) => Ok(i64::from(u32::from(*value))),
            other => Err(self.unsupported_argument(other, Conversion::Character)),
        }
    }

    /// `%c`: one character, padded with spaces (`printf` gives `%c` no
    /// zero-padding, so `0` and the precision are ignored).
    fn push_character(&mut self, value: char, flags: Flags, width: Option<usize>) {
        self.push_field("", &value.to_string(), flags, width, false);
    }

    /// The reference's catch-all `convert` overload (`src/string_formatter.h:157-168`).
    fn unsupported_argument(&self, argument: &Arg<'_>, target: Conversion) -> FormatError {
        self.error(FormatErrorKind::UnsupportedArgument {
            argument: argument.kind(),
            target,
        })
    }

    /// `string_formatter::throw_error` (`src/string_formatter.cpp:104-109`), at
    /// the current position.
    fn error(&self, kind: FormatErrorKind) -> FormatError {
        FormatError::new(kind, self.format, self.cursor.position())
    }
}

/// `string_format` (`src/string_formatter.h:426-436`): the formatter whose
/// failures come back *as* the result string.
#[must_use]
pub fn string_format_or_error(format: &str, args: &[Arg<'_>]) -> String {
    string_format(format, args).unwrap_or_else(|error| error.to_string())
}
