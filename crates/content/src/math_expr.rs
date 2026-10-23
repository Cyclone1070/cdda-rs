//! The expression language the weather conditions are written in.
//!
//! Reference: the subset of `math_parser.cpp` that `data/json/weather_type.json` actually uses - numbers, single-quoted strings, `+ - * / %`, the six comparisons,
//! parentheses, and calls. The dialogue machine will need far more of that parser later; weather needs this much, and needs it exact, so it is written where it can
//! be read and gated on its own.
//!
//! Names are resolved through a [`Context`]: the expression language has no variables of its own, and every name in a weather condition is either a field of the
//! point being described (`weather('pressure')`) or a function (`from_fahrenheit`, and the data-defined `dew_point_factor`). An unresolved name is an error, never a
//! zero - a silent zero would make every condition false and every sky clear.

use std::fmt;

/// A value an expression can produce.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Everything the weather conditions compare is a number, including the results of comparisons.
    Number(f64),
    /// A string, which in practice is a field name being handed to `weather`.
    Text(String),
}

impl Value {
    /// The number this value is, if it is one.
    #[must_use]
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(value) => Some(*value),
            Value::Text(_) => None,
        }
    }
}

/// Why an expression could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MathError {
    /// The text is not a well-formed expression.
    Syntax {
        /// The expression as written.
        expression: String,
        /// Where in it the parser gave up, counting characters from the start.
        position: usize,
        /// What it expected.
        message: String,
    },
    /// A name that is neither a number nor a function the context knows.
    UnknownName {
        /// The name as written.
        name: String,
    },
    /// A value of the wrong kind reached an operator: comparing a string to a number, or negating one.
    Type {
        /// What went wrong.
        message: String,
    },
}

impl fmt::Display for MathError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MathError::Syntax { expression, position, message } => {
                write!(out, "in {expression:?} at {position}: {message}")
            }
            MathError::UnknownName { name } => write!(out, "unknown name {name:?}"),
            MathError::Type { message } => write!(out, "type error: {message}"),
        }
    }
}

impl std::error::Error for MathError {}

/// What the expression's names mean.
///
/// Both methods answer `None` for a name they do not know, and the evaluator turns that into [`MathError::UnknownName`]. Answering zero instead would be the one
/// mistake that makes a broken condition look like a working one.
pub trait Context {
    /// A bare name's value.
    fn number(&self, name: &str) -> Option<f64>;

    /// A call's value. Arguments arrive already evaluated, in order.
    ///
    /// The error is a `Result` rather than an `Option` so that a context built on another context - the data-defined functions are exactly that - can pass a failure
    /// through instead of turning it into "unknown name" for the outermost call. That distinction is the difference between an error that says `log` is missing and
    /// one that blames the function that used it.
    ///
    /// # Errors
    ///
    /// Returns [`MathError::UnknownName`] by default, which is what a context with no functions should say.
    fn call(&self, name: &str, _arguments: &[Value]) -> Result<Value, MathError> {
        Err(MathError::UnknownName { name: name.to_string() })
    }
}

/// Evaluates one expression.
///
/// # Errors
///
/// Returns [`MathError`] when the expression is malformed, names something the context does not know, or applies an operator to a string.
pub fn evaluate(expression: &str, context: &dyn Context) -> Result<Value, MathError> {
    let tokens = tokenize(expression)?;
    let mut parser = Parser { expression, tokens, at: 0, context };
    let value = parser.comparison()?;
    if let Some(token) = parser.peek() {
        return Err(parser.syntax(token.position, "an operator or the end of the expression".to_string()));
    }
    Ok(value)
}

/// One token, with where it starts so an error can point at it.
#[derive(Debug, Clone, PartialEq)]
struct Token {
    kind: Kind,
    position: usize,
}

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Number(f64),
    Text(String),
    Name(String),
    Symbol(&'static str),
}

const SYMBOLS: [&str; 14] = ["<=", ">=", "==", "!=", "<", ">", "+", "-", "*", "/", "%", "(", ")", ","];

/// Splits an expression into tokens.
///
/// The indexing below is all guarded by the `at < bytes.len()` conditions right beside it; the expression is ASCII by construction (a non-ASCII byte is refused as
/// punctuation it does not recognise).
#[allow(clippy::indexing_slicing)]
fn tokenize(expression: &str) -> Result<Vec<Token>, MathError> {
    let mut tokens = Vec::new();
    let bytes = expression.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        if byte.is_ascii_whitespace() {
            at += 1;
            continue;
        }
        if byte.is_ascii_digit() || (byte == b'.' && bytes.get(at + 1).is_some_and(u8::is_ascii_digit)) {
            let start = at;
            while at < bytes.len() && (bytes[at].is_ascii_digit() || bytes[at] == b'.') {
                at += 1;
            }
            let text = &expression[start..at];
            let Ok(value) = text.parse::<f64>() else {
                return Err(MathError::Syntax {
                    expression: expression.to_string(),
                    position: start,
                    message: format!("{text:?} is not a number"),
                });
            };
            tokens.push(Token { kind: Kind::Number(value), position: start });
            continue;
        }
        if byte == b'\'' {
            let start = at;
            at += 1;
            let mut text = String::new();
            while at < bytes.len() && bytes[at] != b'\'' {
                text.push(bytes[at] as char);
                at += 1;
            }
            if at >= bytes.len() {
                return Err(MathError::Syntax {
                    expression: expression.to_string(),
                    position: start,
                    message: "unterminated string".to_string(),
                });
            }
            at += 1;
            tokens.push(Token { kind: Kind::Text(text), position: start });
            continue;
        }
        if byte.is_ascii_alphabetic() || byte == b'_' {
            let start = at;
            while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                at += 1;
            }
            tokens.push(Token { kind: Kind::Name(expression[start..at].to_string()), position: start });
            continue;
        }
        let rest = &expression[at..];
        let Some(symbol) = SYMBOLS.iter().find(|symbol| rest.starts_with(**symbol)) else {
            let ch = rest.chars().next().unwrap_or(' ');
            return Err(MathError::Syntax {
                expression: expression.to_string(),
                position: at,
                message: format!("{ch:?} is not part of an expression"),
            });
        };
        tokens.push(Token { kind: Kind::Symbol(symbol), position: at });
        at += symbol.len();
    }
    Ok(tokens)
}

struct Parser<'a> {
    expression: &'a str,
    tokens: Vec<Token>,
    at: usize,
    context: &'a dyn Context,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn symbol(&self) -> Option<&'static str> {
        match self.peek() {
            Some(Token { kind: Kind::Symbol(symbol), .. }) => Some(symbol),
            _ => None,
        }
    }

    fn syntax(&self, position: usize, message: String) -> MathError {
        MathError::Syntax { expression: self.expression.to_string(), position, message }
    }

    /// The comparisons, lowest precedence, all left-associative like the reference's.
    fn comparison(&mut self) -> Result<Value, MathError> {
        let mut left = self.additive()?;
        while let Some(symbol @ ("<" | "<=" | ">" | ">=" | "==" | "!=")) = self.symbol() {
            self.at += 1;
            let right = self.additive()?;
            let (Value::Number(left_number), Value::Number(right_number)) = (&left, &right) else {
                return Err(MathError::Type { message: format!("{symbol} compares numbers") });
            };
            // Floating-point equality is deliberate here: the reference's `==` compares the numbers as written, and a margin would make `weather('windpower') == 0`
            // true for a wind of a billionth, which is a different weather.
            #[allow(clippy::float_cmp)]
            let holds = match symbol {
                "<" => left_number < right_number,
                "<=" => left_number <= right_number,
                ">" => left_number > right_number,
                ">=" => left_number >= right_number,
                "==" => left_number == right_number,
                _ => left_number != right_number,
            };
            left = Value::Number(if holds { 1.0 } else { 0.0 });
        }
        Ok(left)
    }

    fn additive(&mut self) -> Result<Value, MathError> {
        let mut left = self.multiplicative()?;
        while let Some(symbol @ ("+" | "-")) = self.symbol() {
            self.at += 1;
            let right = self.multiplicative()?;
            let (Value::Number(left_number), Value::Number(right_number)) = (&left, &right) else {
                return Err(MathError::Type { message: format!("{symbol} adds numbers") });
            };
            left = Value::Number(if symbol == "+" { left_number + right_number } else { left_number - right_number });
        }
        Ok(left)
    }

    fn multiplicative(&mut self) -> Result<Value, MathError> {
        let mut left = self.unary()?;
        while let Some(symbol @ ("*" | "/" | "%")) = self.symbol() {
            self.at += 1;
            let right = self.unary()?;
            let (Value::Number(left_number), Value::Number(right_number)) = (&left, &right) else {
                return Err(MathError::Type { message: format!("{symbol} multiplies numbers") });
            };
            left = Value::Number(match symbol {
                "*" => left_number * right_number,
                "/" => left_number / right_number,
                _ => left_number % right_number,
            });
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Value, MathError> {
        if self.symbol() == Some("-") {
            self.at += 1;
            let value = self.unary()?;
            let Value::Number(number) = value else {
                return Err(MathError::Type { message: "negation takes a number".to_string() });
            };
            return Ok(Value::Number(-number));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Value, MathError> {
        let Some(token) = self.peek().cloned() else {
            return Err(self.syntax(self.expression.len(), "a value".to_string()));
        };
        self.at += 1;
        match token.kind {
            Kind::Number(value) => Ok(Value::Number(value)),
            Kind::Text(text) => Ok(Value::Text(text)),
            Kind::Symbol("(") => {
                let value = self.comparison()?;
                match self.symbol() {
                    Some(")") => {
                        self.at += 1;
                        Ok(value)
                    }
                    _ => Err(self.syntax(token.position, "a closing parenthesis".to_string())),
                }
            }
            Kind::Name(name) => {
                if self.symbol() == Some("(") {
                    self.at += 1;
                    let mut arguments = Vec::new();
                    if self.symbol() == Some(")") {
                        self.at += 1;
                    } else {
                        loop {
                            arguments.push(self.comparison()?);
                            match self.symbol() {
                                Some(",") => self.at += 1,
                                Some(")") => {
                                    self.at += 1;
                                    break;
                                }
                                _ => return Err(self.syntax(token.position, "a comma or a closing parenthesis".to_string())),
                            }
                        }
                    }
                    return self.context.call(&name, &arguments);
                }
                self.context.number(&name).map(Value::Number).ok_or(MathError::UnknownName { name })
            }
            Kind::Symbol(other) => Err(self.syntax(token.position, format!("a value, not {other:?}"))),
        }
    }
}
