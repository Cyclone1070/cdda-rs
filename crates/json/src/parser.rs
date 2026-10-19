//! The reader.
//!
//! The reference does not parse game data with its own hand-written reader any
//! more: `flexbuffer_cache::parse_json_to_flexbuffer_` hands the text to the
//! vendored flatbuffers parser configured with `strict_json = true` and
//! `use_flexbuffers = true` (`src/flexbuffer_cache.cpp:59-70`), and the
//! hand-written `TextJsonIn` is only used afterwards, to produce a nicer message
//! when that parse failed (`src/flexbuffer_cache.cpp:70-126`). Acceptance rules
//! therefore come from flatbuffers' lexer and value parser, and that is what
//! this module reproduces.
//!
//! Input is a byte slice, not a `&str`: the reference reads raw bytes and
//! validates UTF-8 only inside strings (`idl_parser.cpp:474-477`), and a NUL
//! byte acts as the end of input (`idl_parser.cpp:344-347`), because the parser
//! is handed a NUL-terminated buffer.

use crate::error::ParseError;
use crate::value::{Object, Value};

/// Parses one JSON document, with the reference's acceptance rules.
///
/// Only the **first** value is read: the reference never checks for input after
/// it (`idl_parser.cpp:3077-3086` parses a single value and returns), so
/// `{} {}`, `1 2` and `{"a":1}xyz` are all accepted and yield the first value.
/// Trailing whitespace and comments are simply never looked at.
///
/// # Errors
///
/// Returns the reference's message and position for input it rejects.
pub fn parse(input: &[u8]) -> Result<Value, ParseError> {
    let mut parser = Parser::new(input);
    parser.parse_document()
}

/// A lexer token, named as the reference names it for error messages
/// (`idl_parser.cpp:267-275`).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    /// `kTokenEof`.
    Eof,
    /// A single-character token, held as its byte.
    Char(u8),
    /// `kTokenStringConstant`.
    Str,
    /// `kTokenIntegerConstant`.
    Int,
    /// `kTokenFloatConstant`.
    Float,
    /// `kTokenIdentifier`, with its text.
    Ident(Vec<u8>),
}

impl Token {
    /// The token's name as the reference prints it
    /// (`idl_parser.cpp:282-299`).
    fn name(&self) -> String {
        match self {
            Self::Eof => "end of file".to_string(),
            Self::Char(c) => char::from(*c).to_string(),
            Self::Str => "string constant".to_string(),
            Self::Int => "integer constant".to_string(),
            Self::Float => "float constant".to_string(),
            Self::Ident(text) => describe_bytes(text),
        }
    }
}

/// How many values the reference lets nest: `FLATBUFFERS_MAX_PARSING_DEPTH`
/// (`src/third-party/flatbuffers/idl.h:41-42`).
const MAX_PARSING_DEPTH: usize = 64;

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
    line: usize,
    line_start: usize,
    /// The current token, and the literal text it was lexed from.
    token: Token,
    attribute: Vec<u8>,
}

impl<'a> Parser<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            pos: 0,
            line: 1,
            line_start: 0,
            token: Token::Eof,
            attribute: Vec::new(),
        }
    }

    fn parse_document(&mut self) -> Result<Value, ParseError> {
        self.skip_byte_order_mark()?;
        self.next_token()?;
        if self.token == Token::Eof {
            return Err(self.error("input file is empty"));
        }
        self.parse_value(0)
    }

    /// Skips a UTF-8 byte order mark, which the reference allows once, before
    /// the first token (`idl_parser.cpp:3113-3121`, `idl_parser.cpp:319-329`).
    fn skip_byte_order_mark(&mut self) -> Result<(), ParseError> {
        if self.byte_at(0) != 0xef {
            return Ok(());
        }
        self.pos += 1;
        if self.current() != 0xbb {
            return Err(self.error("invalid utf-8 byte order mark"));
        }
        self.pos += 1;
        if self.current() != 0xbf {
            return Err(self.error("invalid utf-8 byte order mark"));
        }
        self.pos += 1;
        Ok(())
    }

    fn byte_at(&self, at: usize) -> u8 {
        self.input.get(at).copied().unwrap_or(0)
    }

    fn current(&self) -> u8 {
        self.byte_at(self.pos)
    }

    /// Marks a line start the way `MarkNewLine` does
    /// (`idl_parser.cpp:351-354`, `idl.h:705-708`).
    fn new_line(&mut self) {
        self.line += 1;
        self.line_start = self.pos;
    }

    /// True once the cursor sits on the terminating NUL, which is when the
    /// reference prints `EOF` instead of a position (`idl_parser.cpp:137-142`).
    fn at_end(&self) -> bool {
        self.current() == 0
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        ParseError {
            line: self.line,
            column: self.pos.saturating_sub(self.line_start),
            at_eof: self.at_end(),
            message: message.into(),
        }
    }

    fn token_error(&self) -> ParseError {
        self.error(format!(
            "cannot parse value starting with: {}",
            self.token.name()
        ))
    }

    fn expect(&mut self, token: &Token) -> Result<(), ParseError> {
        if self.token != *token {
            return Err(self.error(format!(
                "expecting: {} instead got: {}",
                token.name(),
                self.token.name()
            )));
        }
        self.next_token()
    }

    /// Reads the next token, mirroring `Parser::Next`
    /// (`idl_parser.cpp:335-573`).
    fn next_token(&mut self) -> Result<(), ParseError> {
        self.attribute.clear();
        // `seen_newline` starts out true only when nothing has been consumed
        // yet, and is set when a newline is skipped in this call
        // (`idl_parser.cpp:337`, `idl_parser.cpp:351-354`).
        let mut seen_newline = self.pos == 0;
        loop {
            let c = self.current();
            self.pos += 1;
            match c {
                0 => {
                    self.pos -= 1;
                    self.token = Token::Eof;
                    return Ok(());
                }
                b' ' | b'\r' | b'\t' => {}
                b'\n' => {
                    self.new_line();
                    seen_newline = true;
                }
                b'{' | b'}' | b'(' | b')' | b'[' | b']' | b',' | b':' | b';' | b'=' => {
                    self.token = Token::Char(c);
                    return Ok(());
                }
                b'"' | b'\'' => return self.lex_string(c),
                b'/' => {
                    if let Some(()) = self.lex_comment(seen_newline)? {
                        continue;
                    }
                    return self.lex_word(c);
                }
                _ => return self.lex_word(c),
            }
        }
    }

    /// The comment cases of the lexer (`idl_parser.cpp:481-503`).
    ///
    /// Returns `Some(())` when a comment was consumed (the caller keeps
    /// lexing), or `None` when the slash was not a comment at all and has to be
    /// lexed as an ordinary character.
    fn lex_comment(&mut self, seen_newline: bool) -> Result<Option<()>, ParseError> {
        if self.current() == b'/' {
            self.pos += 1;
            let start = self.pos;
            while !matches!(self.current(), 0 | b'\n' | b'\r') {
                self.pos += 1;
            }
            // A third slash makes it a documentation comment
            // (`idl_parser.cpp:485-490`).
            if self.byte_at(start) == b'/' && !seen_newline {
                return Err(self.error("a documentation comment should be on a line on its own"));
            }
            return Ok(Some(()));
        }
        if self.current() == b'*' {
            self.pos += 1;
            while !(self.current() == b'*' && self.byte_at(self.pos + 1) == b'/') {
                // `MarkNewLine` here points the line start *at* the newline,
                // because the cursor is not advanced first
                // (`idl_parser.cpp:496`).
                if self.current() == b'\n' {
                    self.line += 1;
                    self.line_start = self.pos;
                }
                if self.current() == 0 {
                    return Err(self.error("end of file in comment"));
                }
                self.pos += 1;
            }
            self.pos += 2;
            return Ok(Some(()));
        }
        Ok(None)
    }

    /// A quoted string, mirroring the string case of `Parser::Next`
    /// (`idl_parser.cpp:365-480`). Escapes are handled when the cursor meets a
    /// backslash; raw control bytes are refused by the reference itself
    /// (`idl_parser.cpp:370-371`), which is also how an unterminated string is
    /// caught, because the terminating NUL is one of them.
    fn lex_string(&mut self, quote: u8) -> Result<(), ParseError> {
        let mut bytes = Vec::new();
        let mut high_surrogate: Option<u32> = None;
        while self.current() != quote {
            let current = self.current();
            if current < 0x20 {
                return Err(self.error("illegal character in string constant"));
            }
            if current == b'\\' {
                self.pos += 1;
                let escape = self.current();
                if high_surrogate.is_some() && escape != b'u' {
                    return Err(self.error("illegal Unicode sequence (unpaired high surrogate)"));
                }
                if let Some(decoded) = simple_escape(escape) {
                    bytes.push(decoded);
                    self.pos += 1;
                    continue;
                }
                match escape {
                    // `\xNN` is not a JSON escape, but the reference accepts it
                    // (`idl_parser.cpp:416-423`).
                    b'x' => {
                        self.pos += 1;
                        let value = self.parse_hex_num(2)?;
                        bytes.push(value.to_le_bytes().first().copied().unwrap_or(0));
                    }
                    b'u' => {
                        self.pos += 1;
                        let value = self.parse_hex_num(4)?;
                        match value {
                            0xD800..=0xDBFF => {
                                if high_surrogate.is_some() {
                                    return Err(self.error(
                                        "illegal Unicode sequence (multiple high surrogates)",
                                    ));
                                }
                                high_surrogate = Some(value);
                            }
                            0xDC00..=0xDFFF => {
                                let Some(high) = high_surrogate.take() else {
                                    return Err(self.error(
                                        "illegal Unicode sequence (unpaired low surrogate)",
                                    ));
                                };
                                let code_point =
                                    0x10000 + ((high & 0x03FF) << 10) + (value & 0x03FF);
                                push_utf8(&mut bytes, code_point);
                            }
                            _ => {
                                if high_surrogate.is_some() {
                                    return Err(self.error(
                                        "illegal Unicode sequence (unpaired high surrogate)",
                                    ));
                                }
                                push_utf8(&mut bytes, value);
                            }
                        }
                    }
                    _ => return Err(self.error("unknown escape code in string constant")),
                }
                continue;
            }
            if high_surrogate.is_some() {
                return Err(self.error("illegal Unicode sequence (unpaired high surrogate)"));
            }
            bytes.push(current);
            self.pos += 1;
        }
        if high_surrogate.is_some() {
            return Err(self.error("illegal Unicode sequence (unpaired high surrogate)"));
        }
        self.pos += 1;
        self.attribute = bytes;
        self.token = Token::Str;
        Ok(())
    }

    /// Exactly `nibbles` hexadecimal digits (`idl_parser.cpp:307-317`).
    fn parse_hex_num(&mut self, nibbles: usize) -> Result<u32, ParseError> {
        let mut value = 0_u32;
        for i in 0..nibbles {
            let Some(digit) = char::from(self.byte_at(self.pos + i)).to_digit(16) else {
                return Err(self.error(format!(
                    "escape code must be followed by {nibbles} hex digits"
                )));
            };
            value = value * 16 + digit;
        }
        self.pos += nibbles;
        Ok(value)
    }

    /// The reference's default lexer branch: identifiers and numbers
    /// (`idl_parser.cpp:504-570`).
    fn lex_word(&mut self, c: u8) -> Result<(), ParseError> {
        let start = self.pos - 1;
        let has_sign = c == b'+' || c == b'-';

        if is_identifier_start(c) || (has_sign && is_identifier_start(self.current())) {
            while is_identifier_start(self.current()) || is_digit(self.current()) {
                self.pos += 1;
            }
            let text = self.slice(start, self.pos).to_vec();
            // A signed word is a *string constant* in the reference, which is
            // how `-inf` and `+nan` come out as strings
            // (`idl_parser.cpp:508-514`).
            self.token = if has_sign {
                Token::Str
            } else {
                Token::Ident(text.clone())
            };
            self.attribute = text;
            return Ok(());
        }

        let mut dot_lvl: i32 = i32::from(c != b'.');
        if dot_lvl == 0 && !is_digit(self.current()) {
            self.token = Token::Char(c);
            return Ok(());
        }
        if is_digit(c) || has_sign || dot_lvl == 0 {
            let mut cc = c;
            let mut start_digits = if is_digit(c) { start } else { self.pos };
            if !is_digit(c) && is_digit(self.current()) {
                start_digits = self.pos;
                cc = self.current();
                self.pos += 1;
            }
            // A hexadecimal float cannot start with `.`
            // (`idl_parser.cpp:528-530`).
            let use_hex = dot_lvl != 0 && cc == b'0' && is_alpha_char(self.current(), b'X');
            if use_hex {
                self.pos += 1;
                start_digits = self.pos;
            }
            loop {
                if use_hex {
                    while is_xdigit(self.current()) {
                        self.pos += 1;
                    }
                } else {
                    while is_digit(self.current()) {
                        self.pos += 1;
                    }
                }
                if self.current() == b'.' {
                    self.pos += 1;
                    dot_lvl -= 1;
                    if dot_lvl >= 0 {
                        continue;
                    }
                }
                break;
            }
            if dot_lvl >= 0 && self.pos > start_digits {
                if use_hex && dot_lvl == 0 {
                    start_digits = self.pos;
                }
                if (use_hex && is_alpha_char(self.current(), b'P'))
                    || is_alpha_char(self.current(), b'E')
                {
                    dot_lvl = 0;
                    self.pos += 1;
                    if matches!(self.current(), b'+' | b'-') {
                        self.pos += 1;
                    }
                    start_digits = self.pos;
                    while is_digit(self.current()) {
                        self.pos += 1;
                    }
                    if self.current() == b'.' {
                        self.pos += 1;
                        dot_lvl = -1;
                    }
                }
            }
            if dot_lvl >= 0 && self.pos > start_digits {
                let text = self.slice(start, self.pos).to_vec();
                self.attribute = text;
                self.token = if dot_lvl != 0 {
                    Token::Int
                } else {
                    Token::Float
                };
                return Ok(());
            }
            return Err(self.error(format!("invalid number: {}", self.slice_text(start))));
        }

        Err(self.error(format!("illegal character: {}", describe_byte(c))))
    }

    fn slice(&self, from: usize, to: usize) -> &'a [u8] {
        self.input.get(from..to).unwrap_or_default()
    }

    /// The text from `from` up to the cursor, for error messages.
    fn slice_text(&self, from: usize) -> String {
        describe_bytes(self.slice(from, self.pos))
    }

    /// Parses one value. `depth` is the number of values already on the stack,
    /// mirroring `Parser::ParseDepthGuard` (`idl_parser.cpp:163-186`): the
    /// counter is incremented first, so the message names `depth + 1`.
    fn parse_value(&mut self, depth: usize) -> Result<Value, ParseError> {
        if depth >= MAX_PARSING_DEPTH {
            return Err(self.error(format!("maximum parsing depth {} reached", depth + 1)));
        }
        match &self.token {
            Token::Char(b'{') => self.parse_object(depth + 1),
            Token::Char(b'[') => self.parse_array(depth + 1),
            Token::Ident(text) => match text.as_slice() {
                b"true" => {
                    self.next_token()?;
                    Ok(Value::Bool(true))
                }
                b"false" => {
                    self.next_token()?;
                    Ok(Value::Bool(false))
                }
                b"null" => {
                    self.next_token()?;
                    Ok(Value::Null)
                }
                _ => Err(self.token_error()),
            },
            Token::Str => {
                let value = self.string_value()?;
                self.next_token()?;
                Ok(Value::Str(value))
            }
            Token::Int => {
                let value = integer_value(&self.attribute);
                self.next_token()?;
                Ok(Value::Int(value))
            }
            Token::Float => {
                let value = float_value(&self.attribute);
                self.next_token()?;
                Ok(Value::Float(value))
            }
            _ => Err(self.token_error()),
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.expect(&Token::Char(b'{'))?;
        let mut object = Object::new();
        let mut duplicated = false;
        loop {
            // `strict_json` narrows the empty-object exception to the case with
            // no members yet, which is what rejects `{"a":1,}`
            // (`idl_parser.cpp:1191-1192`).
            if object.is_empty() && self.token == Token::Char(b'}') {
                break;
            }
            if self.token != Token::Str {
                return Err(self.error(format!(
                    "expecting: string constant instead got: {}",
                    self.token.name()
                )));
            }
            let key = self.string_value()?;
            self.next_token()?;
            self.expect(&Token::Char(b':'))?;
            let value = self.parse_value(depth)?;
            if !object.insert(key, value) {
                duplicated = true;
            }
            if self.token == Token::Char(b'}') {
                break;
            }
            self.expect(&Token::Char(b','))?;
        }
        self.expect(&Token::Char(b'}'))?;
        if duplicated {
            // flexbuffers notices the collision while sorting the finished map
            // (`flexbuffers.h:1143-1149`), so the error is raised here, after
            // the closing brace, exactly like `idl_parser.cpp:3032-3034`.
            return Err(self.error("FlexBuffers map has duplicate keys"));
        }
        Ok(Value::Object(object))
    }

    /// The text of the current string token.
    ///
    /// The reference validates UTF-8 only for strings that held an escape or a
    /// byte outside printable ASCII (`idl_parser.cpp:474-477`), which is every
    /// string that could fail: printable ASCII is always valid UTF-8, so
    /// validating unconditionally accepts and rejects exactly the same
    /// strings.
    fn string_value(&self) -> Result<String, ParseError> {
        std::str::from_utf8(&self.attribute)
            .map(ToString::to_string)
            .map_err(|_| self.error("illegal UTF-8 sequence"))
    }

    fn parse_array(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.expect(&Token::Char(b'['))?;
        let mut items = Vec::new();
        loop {
            // `strict_json` keeps the empty-element exception for the first
            // element only, which is what rejects `[1,]`
            // (`idl_parser.cpp:1382-1394`).
            if items.is_empty() && self.token == Token::Char(b']') {
                break;
            }
            items.push(self.parse_value(depth)?);
            if self.token == Token::Char(b']') {
                break;
            }
            self.expect(&Token::Char(b','))?;
        }
        self.expect(&Token::Char(b']'))?;
        Ok(Value::Array(items))
    }
}

/// The integer a token denotes: decimal only, and out of range becomes 0.
///
/// The reference calls `StringToInt`, which parses base 10 and returns 0 when
/// the text does not convert or when `strtoll` overflowed
/// (`idl_parser.cpp:3051`, `util.h:303-327`, `util.h:407-410`). So `0x10` and
/// `12345678901234567890123` are both accepted, and both mean 0.
fn integer_value(text: &[u8]) -> i64 {
    std::str::from_utf8(text)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// The double a token denotes, with C `strtod` semantics in the C locale.
///
/// Decimal text goes through Rust's parser, which accepts the forms `strtod`
/// accepts for the shapes the lexer can produce here (`1.`, `.5`, `1e+5`) and,
/// like `strtod`, answers infinity on overflow and zero on underflow. Hex
/// floats (`0x1p3`) have no Rust equivalent and are converted here; unlike
/// `strtod` that conversion is not guaranteed correctly rounded once the
/// mantissa exceeds an integer, which no game data needs.
fn float_value(text: &[u8]) -> f64 {
    if let Some(hex) = hex_float(text) {
        return hex;
    }
    std::str::from_utf8(text)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0)
}

/// Converts a C99 hexadecimal float such as `0x1p3` (`8.0`), or `None` when the
/// text is not one.
fn hex_float(text: &[u8]) -> Option<f64> {
    let (negative, rest) = match text.split_first() {
        Some((b'-', rest)) => (true, rest),
        Some((b'+', rest)) => (false, rest),
        _ => (false, text),
    };
    let digits = rest
        .strip_prefix(b"0x".as_slice())
        .or_else(|| rest.strip_prefix(b"0X".as_slice()))?;
    let (mantissa, exponent) = match digits.iter().position(|b| *b == b'p' || *b == b'P') {
        Some(at) => (
            digits.get(..at)?,
            std::str::from_utf8(digits.get(at + 1..)?)
                .ok()?
                .parse()
                .ok()?,
        ),
        None => (digits, 0_i32),
    };
    let (whole, fraction) = match mantissa.iter().position(|b| *b == b'.') {
        Some(at) => (mantissa.get(..at)?, mantissa.get(at + 1..)?),
        None => (mantissa, &[][..]),
    };
    let mut value = 0.0_f64;
    for b in whole.iter().chain(fraction) {
        value = value * 16.0 + f64::from(char::from(*b).to_digit(16)?);
    }
    value *= 16.0_f64.powi(-i32::try_from(fraction.len()).unwrap_or(i32::MAX));
    value *= 2.0_f64.powi(exponent);
    Some(if negative { -value } else { value })
}

/// The escapes that stand for a single byte (`idl_parser.cpp:379-415`).
fn simple_escape(escape: u8) -> Option<u8> {
    match escape {
        b'n' => Some(b'\n'),
        b't' => Some(b'\t'),
        b'r' => Some(b'\r'),
        b'b' => Some(0x08),
        b'f' => Some(0x0c),
        b'"' => Some(b'"'),
        b'\'' => Some(b'\''),
        b'\\' => Some(b'\\'),
        b'/' => Some(b'/'),
        _ => None,
    }
}

/// Appends a code point as UTF-8, the way `ToUTF8` does
/// (`util.h:493-514`).
fn push_utf8(out: &mut Vec<u8>, code_point: u32) {
    match char::from_u32(code_point) {
        Some(c) => {
            let mut buffer = [0_u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
        }
        None => out.push(code_point.to_le_bytes().first().copied().unwrap_or(0)),
    }
}

fn is_digit(c: u8) -> bool {
    c.is_ascii_digit()
}

fn is_xdigit(c: u8) -> bool {
    c.is_ascii_hexdigit()
}

/// `is_alpha_char(c, alpha)`: case-insensitive ASCII compare
/// (`util.h:60-64`).
fn is_alpha_char(c: u8, alpha: u8) -> bool {
    (c & 0xDF) == (alpha & 0xDF)
}

fn is_identifier_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

/// Renders bytes the way the reference's messages do.
pub(crate) fn describe_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|b| describe_byte(*b)).collect()
}

/// A byte as the reference prints it: printable ASCII as itself, anything else
/// as the *signed* value of the byte (`idl_parser.cpp:566-570`).
fn describe_byte(b: u8) -> String {
    if (0x20..=0x7e).contains(&b) {
        char::from(b).to_string()
    } else {
        format!("code: {}", b.cast_signed())
    }
}
