//! The condition tree weather types are gated on.
//!
//! Reference: `dialogue_condition` (`condition.cpp`) restricted to what the weather data uses - `and`, `or`, `not`, a named condition, and `math`. Every weather type in
//! `data/json/weather_type.json` is built from these five, and nothing else in the file appears.
//!
//! A condition that cannot be evaluated is **false**, which is what the reference does with a broken dialogue expression: it logs and moves on. The difference from a silent
//! zero is that [`Condition::holds`] answers the question asked, while the error is available to anything that wants it through [`Condition::evaluate`].

use crate::math_expr::{Context, MathError, Value, evaluate};
use cdda_json::{Object, Value as Json, parse};

/// One condition.
#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    /// Always holds - the data's `true`.
    Always,
    /// Never holds - the data's `false`.
    Never,
    /// Every member must hold.
    All(Vec<Condition>),
    /// One member must hold.
    Any(Vec<Condition>),
    /// The member must not hold.
    Not(Box<Condition>),
    /// A named condition the context answers: `is_day` is the only one the weather uses.
    Named(String),
    /// An expression.
    Math(String),
}

impl Condition {
    /// Reads one condition from JSON text.
    ///
    /// # Errors
    ///
    /// Returns the reason the text is not a condition the port understands.
    pub fn from_json_bytes(json: &[u8]) -> Result<Condition, String> {
        read(&parse(json).map_err(|error| error.to_string())?)
    }

    /// Reads one condition from an already-parsed JSON value.
    ///
    /// # Errors
    ///
    /// Returns the reason the value is not a condition the port understands.
    pub fn from_value(value: &Json) -> Result<Condition, String> {
        read(value)
    }

    /// Whether the condition holds, with an evaluation failure counting as false - which is what the reference does.
    #[must_use]
    pub fn holds(&self, context: &dyn Context) -> bool {
        self.evaluate(context).unwrap_or(false)
    }

    /// Whether the condition holds, keeping the reason it could not be decided.
    ///
    /// # Errors
    ///
    /// Returns the evaluation failure: a malformed expression, or a name the context does not know.
    pub fn evaluate(&self, context: &dyn Context) -> Result<bool, MathError> {
        match self {
            Condition::Always => Ok(true),
            Condition::Never => Ok(false),
            Condition::All(members) => {
                for member in members {
                    if !member.evaluate(context)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Condition::Any(members) => {
                for member in members {
                    if member.evaluate(context)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Condition::Not(member) => Ok(!member.evaluate(context)?),
            Condition::Named(name) => match context.number(&format!("condition:{name}")) {
                Some(value) => Ok(value != 0.0),
                None => Err(MathError::UnknownName {
                    name: format!("condition:{name}"),
                }),
            },
            Condition::Math(expression) => match evaluate(expression, context)? {
                Value::Number(value) => Ok(value != 0.0),
                Value::Text(text) => Err(MathError::Type {
                    message: format!("{text:?} is not a truth value"),
                }),
            },
        }
    }
}

fn read(value: &Json) -> Result<Condition, String> {
    match value {
        Json::Bool(true) => Ok(Condition::Always),
        Json::Bool(false) => Ok(Condition::Never),
        Json::Str(name) => Ok(Condition::Named(name.clone())),
        Json::Array(members) => Ok(Condition::All(
            members.iter().map(read).collect::<Result<_, _>>()?,
        )),
        Json::Object(object) => read_object(object),
        other => Err(format!("{other:?} is not a condition")),
    }
}

fn read_object(object: &Object) -> Result<Condition, String> {
    if let Some(members) = list(object, "and")? {
        return Ok(Condition::All(members));
    }
    if let Some(members) = list(object, "or")? {
        return Ok(Condition::Any(members));
    }
    if let Some(member) = object.get("not") {
        return Ok(Condition::Not(Box::new(read(member)?)));
    }
    if let Some(math) = object.get("math") {
        return read_math(math);
    }
    Err("a condition the port does not understand".to_string())
}

fn list(object: &Object, key: &str) -> Result<Option<Vec<Condition>>, String> {
    match object.get(key) {
        Some(Json::Array(members)) => Ok(Some(members.iter().map(read).collect::<Result<_, _>>()?)),
        Some(_) => Err(format!("{key} holds a list")),
        None => Ok(None),
    }
}

fn read_math(value: &Json) -> Result<Condition, String> {
    match value {
        Json::Str(expression) => Ok(Condition::Math(expression.clone())),
        Json::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                match item {
                    Json::Str(text) => parts.push(text.clone()),
                    other => return Err(format!("{other:?} is not part of a formula")),
                }
            }
            // Several strings are a formula with statements, which the weather data never uses; one string is the whole expression.
            if parts.len() == 1 {
                parts.pop().map_or_else(
                    || Err("an empty formula".to_string()),
                    |only| Ok(Condition::Math(only)),
                )
            } else {
                Err(format!(
                    "a formula of {} statements is not ported",
                    parts.len()
                ))
            }
        }
        other => Err(format!("{other:?} is not a formula")),
    }
}
