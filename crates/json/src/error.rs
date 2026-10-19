//! Parse errors, shaped like the reference's.

use std::fmt;

/// Where and why a parse failed.
///
/// The reference formats every error as `line:column: message`, replacing the
/// position with `EOF` once the cursor sits on the terminating NUL
/// (`src/third-party/flatbuffers/idl_parser.cpp:126-145`); both the position and
/// the wording are reproduced here so failures can be compared directly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// 1-based line number of the cursor.
    pub line: usize,
    /// Byte offset of the cursor within the line, as the reference reports it.
    pub column: usize,
    /// Whether the cursor had reached the end of input.
    pub at_eof: bool,
    /// The reference's message text, without the position prefix.
    pub message: String,
}

impl ParseError {
    /// The message as the reference would print it, position prefix included.
    #[must_use]
    pub fn text(&self) -> String {
        if self.at_eof {
            format!("EOF: error: {}", self.message)
        } else {
            format!("{}:{}: error: {}", self.line, self.column, self.message)
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text())
    }
}

impl std::error::Error for ParseError {}
