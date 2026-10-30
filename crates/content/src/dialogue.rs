//! Dialogue conditions, read from the data.
//!
//! Reference: `conditional_t::conditional_t` (`condition.cpp:2660-2760`), which reads one JSON object as a condition -
//! `and`/`or`/`not` composing others, and one leaf per verb (`u_has_effect`, `u_has_trait`, and a hundred more). The port
//! reads the leaves it can evaluate and reports the ones it cannot, because a condition quietly answered "false" is a
//! behaviour change in every effect-on-condition that uses it.

use cdda_json::Value;

/// A condition the port can evaluate.
#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    /// The creature wears an effect, at least as strongly as stated.
    ///
    /// Reference: `conditional_fun::f_has_effect` (`condition.cpp:980-996`): the effect must be there, and the stated
    /// intensity must be no more than the one worn. An intensity of zero means "any".
    HasEffect {
        /// The effect's id, e.g. `effect_bleed`.
        effect: String,
        /// The least intensity that counts, as the data states it.
        intensity: i32,
    },
    /// Every one of these holds.
    And(Vec<Condition>),
    /// At least one of these holds.
    Or(Vec<Condition>),
    /// This one does not hold.
    Not(Box<Condition>),
    /// An arithmetic expression, which holds when its result is not zero.
    ///
    /// Reference: `conditional_t`'s `math` member (`condition.cpp:194-196`), read into a `math_exp` and evaluated as a
    /// boolean.
    Math(Expression),
}

/// An arithmetic expression, as the data writes one.
///
/// Reference: `math_exp` (`math_parser.cpp`), whose JSON form is an array: a fully parenthesised `[left, op, right]` triple,
/// with numbers, nested arrays and `scope('name')` variable calls for leaves. The array's own nesting carries the order of
/// operations, so there is no precedence table to get wrong.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// A number.
    Number(f64),
    /// A variable: `u_val('name')` or `global_val('name')`.
    Variable {
        /// Which namespace, as the call spells it: `u_val` or `global_val`.
        scope: String,
        /// The variable's name.
        name: String,
    },
    /// Two expressions and the operator between them.
    Binary {
        /// One of `+ - * / %` and the comparisons `== != < <= > >=`.
        op: Operator,
        /// The left side.
        left: Box<Expression>,
        /// The right side.
        right: Box<Expression>,
    },
}

/// An operator an expression may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    /// Add.
    Add,
    /// Subtract.
    Subtract,
    /// Multiply.
    Multiply,
    /// Divide.
    Divide,
    /// Remainder.
    Remainder,
    /// Equal.
    Equal,
    /// Not equal.
    NotEqual,
    /// Less than.
    Less,
    /// At most.
    AtMost,
    /// Greater than.
    Greater,
    /// At least.
    AtLeast,
}

impl Operator {
    /// The operator a spelled operator names, or nothing when the port has no rule for it.
    #[must_use]
    pub fn named(spelling: &str) -> Option<Self> {
        Some(match spelling {
            "+" => Self::Add,
            "-" => Self::Subtract,
            "*" => Self::Multiply,
            "/" => Self::Divide,
            "%" => Self::Remainder,
            "==" => Self::Equal,
            "!=" => Self::NotEqual,
            "<" => Self::Less,
            "<=" => Self::AtMost,
            ">" => Self::Greater,
            ">=" => Self::AtLeast,
            _ => return None,
        })
    }
}

/// Reads a condition, or nothing when the port has no rule for it.
///
/// A `None` is not "false": it is "the port cannot answer", and a caller that treats the two the same would silently give
/// every unread condition the weakest possible meaning.
#[must_use]
pub fn read_condition(value: &Value) -> Option<Condition> {
    let Value::Object(object) = value else {
        return None;
    };
    if let Some(Value::Array(entries)) = object.get("and") {
        return read_all(entries).map(Condition::And);
    }
    if let Some(Value::Array(entries)) = object.get("or") {
        return read_all(entries).map(Condition::Or);
    }
    if let Some(inner) = object.get("not") {
        return read_condition(inner).map(|condition| Condition::Not(Box::new(condition)));
    }
    if let Some(math) = object.get("math") {
        return read_expression(math).map(Condition::Math);
    }
    let Some(Value::Str(effect)) = object.get("u_has_effect") else {
        return None;
    };
    // A condition that names a body part cannot be answered by the port's effects, which are the creature's rather than a
    // part's: refusing it is honest, and answering it as though the part did not matter would be wrong every time it did.
    if object.get("bodypart").is_some() {
        return None;
    }
    let intensity = match object.get("intensity") {
        Some(Value::Int(number)) => i32::try_from(*number).ok()?,
        Some(Value::Float(number)) => {
            #[allow(clippy::cast_possible_truncation)]
            let as_whole = *number as i32;
            as_whole
        }
        Some(_) => return None,
        None => 0,
    };
    Some(Condition::HasEffect {
        effect: effect.clone(),
        intensity,
    })
}

/// Reads an expression, or nothing when the port has no rule for its shape.
#[must_use]
pub fn read_expression(value: &Value) -> Option<Expression> {
    match value {
        Value::Int(number) => {
            #[allow(clippy::cast_precision_loss)]
            let as_float = *number as f64;
            Some(Expression::Number(as_float))
        }
        Value::Float(number) => Some(Expression::Number(*number)),
        Value::Str(text) => read_leaf(text),
        Value::Array(parts) if parts.len() == 3 => {
            let [left, spelling, right] = parts.as_slice() else {
                return None;
            };
            let Value::Str(spelling) = spelling else {
                return None;
            };
            Some(Expression::Binary {
                op: Operator::named(spelling)?,
                left: Box::new(read_expression(left)?),
                right: Box::new(read_expression(right)?),
            })
        }
        _ => None,
    }
}

/// Reads a leaf that is written as text: a variable call, or nothing.
///
/// Reference: the `scope('name')` form the math parser's lexer reads, where the scope is the variable's type - `u_val`,
/// `npc_val`, `global_val`, `context_val` or `var_val` (`dialogue_helpers.h:33-40`).
fn read_leaf(text: &str) -> Option<Expression> {
    let open = text.find('(')?;
    let scope = &text[..open];
    let rest = text.get(open + 1..)?.strip_suffix(')')?;
    let name = rest.trim().trim_matches('\'').trim_matches('"');
    if name.is_empty() || !matches!(scope, "u_val" | "npc_val" | "global_val") {
        return None;
    }
    Some(Expression::Variable {
        scope: scope.to_owned(),
        name: name.to_owned(),
    })
}

/// Reads a list of conditions, or nothing when any of them is one the port cannot read.
fn read_all(entries: &[Value]) -> Option<Vec<Condition>> {
    entries.iter().map(read_condition).collect()
}
