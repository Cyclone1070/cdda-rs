//! The writer, a port of `JsonOut`.
//!
//! `JsonOut` is a streaming writer whose layout depends on two flags the caller
//! passes at each container — `pretty_print` and a per-container `wrap`
//! (`src/json.h:735-772`) — and whose separators, indentation, escaping and
//! number formatting are reproduced here line by line:
//!
//! * indentation is two spaces per level (`src/json.cpp:2251-2254`);
//! * separators wrap after the outermost levels and pad inside them
//!   (`json.cpp:2256-2300`);
//! * floats are printed with `std::fixed` and `std::showpoint` and the default
//!   precision of 6, in the classic locale (`json.cpp:2226-2237`), so `1.5`
//!   prints as `1.500000`;
//! * strings escape only what `JsonOut::write` escapes (`json.cpp:2363-2404`).
//!
//! Output order is byte-reproducible because an [`Object`](crate::Object) stores
//! its members in the parser's key order.

use crate::value::Value;

/// A JSON writer with the reference's layout rules.
#[derive(Clone, Debug)]
pub struct Writer {
    output: String,
    pretty: bool,
    indent_level: usize,
    need_separator: bool,
    need_wrap: Vec<bool>,
}

impl Writer {
    /// A writer that either pretty-prints or stays compact.
    #[must_use]
    pub const fn new(pretty: bool) -> Self {
        Self {
            output: String::new(),
            pretty,
            indent_level: 0,
            need_separator: false,
            need_wrap: Vec::new(),
        }
    }

    /// The text written so far.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.output
    }

    /// The finished text.
    #[must_use]
    pub fn into_string(self) -> String {
        self.output
    }

    /// Opens an object. `wrap` forces the container's contents onto their own
    /// lines, exactly like `JsonOut::start_object` (`json.cpp:2316-2325`).
    pub fn start_object(&mut self, wrap: bool) {
        self.write_separator();
        self.output.push('{');
        self.need_wrap.push(wrap);
        self.start_pretty();
        self.need_separator = false;
    }

    /// Closes an object (`json.cpp:2327-2333`).
    pub fn end_object(&mut self) {
        self.end_pretty();
        self.need_wrap.pop();
        self.output.push('}');
        self.need_separator = true;
    }

    /// Opens an array (`json.cpp:2335-2344`).
    pub fn start_array(&mut self, wrap: bool) {
        self.write_separator();
        self.output.push('[');
        self.need_wrap.push(wrap);
        self.start_pretty();
        self.need_separator = false;
    }

    /// Closes an array (`json.cpp:2346-2352`).
    pub fn end_array(&mut self) {
        self.end_pretty();
        self.need_wrap.pop();
        self.output.push(']');
        self.need_separator = true;
    }

    /// Writes a member name and its separator (`json.cpp:2422-2426`).
    pub fn member(&mut self, name: &str) {
        self.write_str(name);
        self.output.push(':');
        if self.pretty {
            self.output.push(' ');
        }
        self.need_separator = false;
    }

    /// Writes `null` (`json.cpp:2354-2361`).
    pub fn write_null(&mut self) {
        self.write_separator();
        self.output.push_str("null");
        self.need_separator = true;
    }

    /// Writes a boolean, spelled out as the reference's `boolalpha` stream does
    /// (`json.cpp:2226-2237`).
    pub fn write_bool(&mut self, value: bool) {
        self.write_separator();
        self.output.push_str(if value { "true" } else { "false" });
        self.need_separator = true;
    }

    /// Writes an integer in decimal.
    pub fn write_int(&mut self, value: i64) {
        self.write_separator();
        self.output.push_str(&value.to_string());
        self.need_separator = true;
    }

    /// Writes a float the way the reference's stream does: fixed notation, six
    /// decimals, and the C++ spellings `inf`, `-inf` and `nan`
    /// (`json.cpp:2226-2237`).
    pub fn write_float(&mut self, value: f64) {
        self.write_separator();
        self.output.push_str(&format_float(value));
        self.need_separator = true;
    }

    /// Writes a quoted, escaped string (`json.cpp:2363-2404`).
    pub fn write_str(&mut self, value: &str) {
        self.write_separator();
        self.output.push('"');
        for character in value.chars() {
            let Ok(byte) = u8::try_from(character) else {
                // Bytes above ASCII are copied through unchanged
                // (`json.cpp:2377-2379`, the final `else`).
                self.output.push(character);
                continue;
            };
            match byte {
                b'"' => self.output.push_str("\\\""),
                b'\\' => self.output.push_str("\\\\"),
                // Escaped characters that have a short form.
                0x08 => self.output.push_str("\\b"),
                0x0c => self.output.push_str("\\f"),
                b'\n' => self.output.push_str("\\n"),
                b'\r' => self.output.push_str("\\r"),
                b'\t' => self.output.push_str("\\t"),
                // Other control bytes become `\u00XY`, with an upper-case hex
                // digit in the low nibble (`json.cpp:2393-2403`).
                other if other < 0x20 => {
                    self.output.push_str("\\u00");
                    self.output.push(if other < 0x10 { '0' } else { '1' });
                    self.output.push(hex_digit(other & 0x0f));
                }
                // `/` is deliberately *not* escaped (`json.cpp:2377-2379`).
                other => self.output.push(char::from(other)),
            }
        }
        self.output.push('"');
        self.need_separator = true;
    }

    /// Writes a whole value.
    ///
    /// Containers are opened with `wrap = false`, which is the reference's
    /// default argument for `start_object`/`start_array`
    /// (`src/json.h:754`, `src/json.h:760`); callers that want the formatter
    /// tool's layout drive the streaming methods instead.
    pub fn write_value(&mut self, value: &Value) {
        match value {
            Value::Null => self.write_null(),
            Value::Bool(flag) => self.write_bool(*flag),
            Value::Int(number) => self.write_int(*number),
            Value::Float(number) => self.write_float(*number),
            Value::Str(text) => self.write_str(text),
            Value::Array(items) => {
                self.start_array(false);
                for item in items {
                    self.write_value(item);
                }
                self.end_array();
            }
            Value::Object(object) => {
                self.start_object(false);
                for (key, item) in object.members() {
                    self.member(key);
                    self.write_value(item);
                }
                self.end_object();
            }
        }
    }

    /// `JsonOut::write_separator` (`json.cpp:2256-2272`).
    fn write_separator(&mut self) {
        if !self.need_separator {
            return;
        }
        self.output.push(',');
        if self.pretty {
            if self.indent_level < 2 || self.need_wrap.last().copied().unwrap_or(false) {
                self.output.push('\n');
                self.write_indent();
            } else {
                self.output.push(' ');
            }
        }
        self.need_separator = false;
    }

    /// `JsonOut::start_pretty` (`json.cpp:2285-2298`).
    fn start_pretty(&mut self) {
        if !self.pretty {
            return;
        }
        self.indent_level += 1;
        if self.indent_level < 2 || self.need_wrap.last().copied().unwrap_or(false) {
            self.output.push('\n');
            self.write_indent();
        } else {
            self.output.push(' ');
        }
    }

    /// `JsonOut::end_pretty` (`json.cpp:2300-2314`).
    fn end_pretty(&mut self) {
        if !self.pretty {
            return;
        }
        self.indent_level -= 1;
        if self.indent_level < 1 || self.need_wrap.last().copied().unwrap_or(false) {
            self.output.push('\n');
            self.write_indent();
        } else {
            self.output.push(' ');
        }
    }

    /// `JsonOut::write_indent`: two spaces per level (`json.cpp:2251-2254`).
    fn write_indent(&mut self) {
        for _ in 0..self.indent_level * 2 {
            self.output.push(' ');
        }
    }
}

/// A float as `std::ostream` prints it with `std::fixed`, `std::showpoint` and
/// precision 6 (`json.cpp:2226-2237`).
///
/// Rust agrees with the C++ stream on every value except NaN, which C++ spells
/// `nan` and Rust spells `NaN`, so that one case is spelled out here.
fn format_float(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    format!("{value:.6}")
}

/// An upper-case hexadecimal digit (`json.cpp:2398-2402`).
const fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'A' + nibble - 10) as char,
    }
}
