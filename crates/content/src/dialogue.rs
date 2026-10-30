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
    /// The creature carries enough of a thing.
    ///
    /// Reference: `conditional_fun::f_has_items` (`condition.cpp:836-865`). The object must name an `item` and state either a
    /// `count` or a `charges`; with neither, the reference answers a flat false rather than asking the inventory anything.
    HasItems {
        /// The item's id, as the data writes it.
        item: StringSource,
        /// How many must be carried, when the data states a count.
        count: Option<i32>,
        /// How many charges must be carried, when the data states charges.
        charges: Option<i32>,
    },
    /// At least two of these strings are the same.
    ///
    /// Reference: `conditional_fun::f_compare_string` (`condition.cpp:1900-1920`), which is not a comparison of two values
    /// but a search for a repeat: it walks the list and holds as soon as a value it has already seen comes round again.
    CompareString(Vec<StringSource>),
    /// An arithmetic expression, which holds when its result is not zero.
    ///
    /// Reference: `conditional_t`'s `math` member (`condition.cpp:194-196`), read into a `math_exp` and evaluated as a
    /// boolean.
    Math(Expression),
}

/// How many arguments a math function takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arity {
    /// Exactly this many.
    Exactly(usize),
    /// This many or more, which is how `min` and `max` are written.
    AtLeast(usize),
}

/// The math functions the port knows, and the arity of each.
///
/// Reference: `math_parser_func.h:141-163`. `rng` and `rand` are deliberately absent: they draw dice, and an expression that
/// draws dice cannot be evaluated by a reader that holds the store by reference. They come when the dialogue machine can
/// take the rolls, the way the spell effects already do.
#[must_use]
pub fn math_function(name: &str) -> Option<Arity> {
    Some(match name {
        "abs" | "floor" | "trunc" | "ceil" | "round" | "sqrt" | "log" | "sin" | "cos" | "tan" | "celsius"
        | "fahrenheit" | "from_celsius" | "from_fahrenheit" => Arity::Exactly(1),
        "clamp" => Arity::Exactly(3),
        "min" | "max" => Arity::AtLeast(1),
        _ => return None,
    })
}

/// Somewhere a string comes from: written out, or named by a variable.
///
/// Reference: `str_or_var` (`dialogue_helpers.h`), which is a string or an object naming a variable type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringSource {
    /// A string the data writes out.
    Literal(String),
    /// A variable, read at the moment the question is asked.
    Variable {
        /// Which namespace, as the data spells it.
        scope: String,
        /// The variable's name.
        name: String,
    },
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
    /// A call to one of the reference's math functions.
    ///
    /// Reference: the `math_func` table (`math_parser_func.h:141-163`), which fixes each function's name and the number of
    /// arguments it takes - `abs`, `floor`, `ceil`, `round`, `sqrt`, `log`, the trigonometric three, `clamp`, the variadic
    /// `min` and `max`, and the four temperature conversions. A name that is not in that table is not a function.
    Call {
        /// The function's name.
        name: String,
        /// Its arguments, in the order the data writes them.
        arguments: Vec<Expression>,
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
    if let Some(Value::Object(requirement)) = object.get("u_has_items") {
        return read_has_items(requirement);
    }
    if let Some(Value::Array(values)) = object.get("compare_string") {
        return values
            .iter()
            .map(read_string_source)
            .collect::<Option<Vec<_>>>()
            .map(Condition::CompareString);
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

/// Reads a `u_has_items` requirement, or nothing when the port cannot ask the question it states.
fn read_has_items(requirement: &cdda_json::Object) -> Option<Condition> {
    let item = read_string_source(requirement.get("item")?)?;
    let count = read_whole_number(requirement.get("count"));
    let charges = read_whole_number(requirement.get("charges"));
    if count.is_none() && charges.is_none() {
        // Reference: an object that states neither answers false, and the reference builds that closure without asking the
        // inventory. Refusing here would be a different answer, so the port keeps the false.
        return None;
    }
    Some(Condition::HasItems {
        item,
        count,
        charges,
    })
}

/// A whole number the data states, or nothing when it states something the port cannot read as one.
fn read_whole_number(value: Option<&Value>) -> Option<i32> {
    match value? {
        Value::Int(number) => i32::try_from(*number).ok(),
        _ => None,
    }
}

/// Reads one source of a string, or nothing when it is a shape the port cannot resolve.
#[must_use]
pub fn read_string_source(value: &Value) -> Option<StringSource> {
    match value {
        Value::Str(text) => Some(StringSource::Literal(text.clone())),
        Value::Object(object) => {
            for scope in ["u_val", "n_val", "npc_val", "global_val", "var_val", "context_val"] {
                if let Some(Value::Str(name)) = object.get(scope) {
                    return Some(StringSource::Variable {
                        scope: scope.to_owned(),
                        name: name.clone(),
                    });
                }
            }
            None
        }
        _ => None,
    }
}

/// Reads an expression from a string, as the data's `math` members actually write them.
///
/// Reference: the reference lexes these strings (`math_parser_lex.cpp`) and parses them with operator precedence
/// Reads an expression from a string, as the data's `math` members actually write them.
///
/// Reference: the reference lexes these strings (`math_parser_lex.cpp`) and parses them by operator precedence
/// (`math_parser_impl.h`). This is a precedence-climbing parser over the same surface: numbers, `scope('name')` calls, bare
/// identifiers as the dialogue's own variables, function calls from the math table, parentheses, unary minus, and the
/// arithmetic and comparison operators.
#[must_use]
pub fn read_text_expression(text: &str) -> Option<Expression> {
    let tokens = tokenize(text)?;
    let (expression, rest) = parse_precedence(&tokens, 0)?;
    if rest.is_empty() {
        Some(expression)
    } else {
        None
    }
}

/// One token of an expression's text.
#[derive(Debug, Clone, PartialEq)]
enum Token {
    /// A number.
    Number(f64),
    /// A name, whether a function's or a variable's.
    Name(String),
    /// A quoted string, which is how a variable's name is written inside a call.
    Str(String),
    /// An operator, as spelled.
    Operator(String),
    /// `(`.
    Open,
    /// `)`.
    Close,
    /// `,`, which separates a call's arguments.
    Comma,
}

/// Splits an expression's text, or nothing when a character has no place in one.
fn tokenize(text: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let trimmed = rest.trim_start();
        for (spelling, token) in [("(", Token::Open), (")", Token::Close), (",", Token::Comma)] {
            if let Some(after) = trimmed.strip_prefix(spelling) {
                tokens.push(token);
                rest = after;
                break;
            }
        }
        if rest.trim_start().len() != trimmed.len() {
            continue;
        }
        let (token, length) = read_operator(trimmed)
            .or_else(|| read_quoted(trimmed))
            .or_else(|| read_number(trimmed))
            .or_else(|| read_name(trimmed))?;
        tokens.push(token);
        rest = trimmed.get(length..)?;
    }
    Some(tokens)
}

/// The operator at the front of the text, longest spelling first so `<=` is not read as `<`.
fn read_operator(text: &str) -> Option<(Token, usize)> {
    for spelling in ["<=", ">=", "==", "!=", "+", "-", "*", "/", "%", "<", ">"] {
        if text.starts_with(spelling) {
            return Some((Token::Operator(spelling.to_owned()), spelling.len()));
        }
    }
    None
}

/// The quoted string at the front of the text, either quote character allowed.
fn read_quoted(text: &str) -> Option<(Token, usize)> {
    let quote = text.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let after = text.get(1..)?;
    let close = after.find(quote)?;
    Some((Token::Str(after.get(..close)?.to_owned()), close + 2))
}

/// The number at the front of the text.
fn read_number(text: &str) -> Option<(Token, usize)> {
    let end = text
        .char_indices()
        .find(|(_, character)| !character.is_ascii_digit() && *character != '.')
        .map_or(text.len(), |(at, _)| at);
    if end == 0 {
        return None;
    }
    let number: f64 = text.get(..end)?.parse().ok()?;
    Some((Token::Number(number), end))
}

/// The name at the front of the text.
fn read_name(text: &str) -> Option<(Token, usize)> {
    let end = text
        .char_indices()
        .find(|(_, character)| !character.is_alphanumeric() && *character != '_')
        .map_or(text.len(), |(at, _)| at);
    if end == 0 {
        return None;
    }
    Some((Token::Name(text.get(..end)?.to_owned()), end))
}

/// How tightly an operator binds: comparisons loosest, then addition, then multiplication.
fn binding(spelling: &str) -> Option<u8> {
    Some(match spelling {
        "<" | "<=" | ">" | ">=" | "==" | "!=" => 1,
        "+" | "-" => 2,
        "*" | "/" | "%" => 3,
        _ => return None,
    })
}

/// Parses the lowest-precedence layer still above `minimum`, answering the expression and what is left.
fn parse_precedence(tokens: &[Token], minimum: u8) -> Option<(Expression, &[Token])> {
    let (mut left, mut rest) = parse_primary(tokens)?;
    while let Some(Token::Operator(spelling)) = rest.first() {
        let Some(binds) = binding(spelling) else {
            break;
        };
        if binds < minimum {
            break;
        }
        let spelling = spelling.clone();
        let (right, after) = parse_precedence(rest.get(1..)?, binds + 1)?;
        left = Expression::Binary {
            op: Operator::named(&spelling)?,
            left: Box::new(left),
            right: Box::new(right),
        };
        rest = after;
    }
    Some((left, rest))
}

/// The tokens before the parenthesis that closes this one, and what comes after it.
fn split_arguments(tokens: &[Token]) -> Option<(&[Token], &[Token])> {
    let mut depth = 0usize;
    for (at, token) in tokens.iter().enumerate() {
        match token {
            Token::Open => depth += 1,
            Token::Close if depth == 0 => {
                return Some((tokens.get(..at)?, tokens.get(at + 1..)?));
            }
            Token::Close => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Splits a call's arguments at its top-level commas and reads each as an expression.
fn read_arguments(tokens: &[Token]) -> Option<Vec<Expression>> {
    if tokens.is_empty() {
        return Some(Vec::new());
    }
    let mut arguments = Vec::new();
    let mut depth = 0usize;
    let mut from = 0usize;
    for (at, token) in tokens.iter().enumerate() {
        match token {
            Token::Open => depth += 1,
            Token::Close => depth = depth.checked_sub(1)?,
            Token::Comma if depth == 0 => {
                let (expression, rest) = parse_precedence(tokens.get(from..at)?, 0)?;
                if !rest.is_empty() {
                    return None;
                }
                arguments.push(expression);
                from = at + 1;
            }
            _ => {}
        }
    }
    let (expression, rest) = parse_precedence(tokens.get(from..)?, 0)?;
    if !rest.is_empty() {
        return None;
    }
    arguments.push(expression);
    Some(arguments)
}

/// Parses one value: a number, a variable, a function call, a parenthesised expression, or a negation.
fn parse_primary(tokens: &[Token]) -> Option<(Expression, &[Token])> {
    match tokens.first()? {
        Token::Number(number) => Some((Expression::Number(*number), tokens.get(1..)?)),
        Token::Name(name) => {
            let after = tokens.get(1..)?;
            let Some(after_open) = after.strip_prefix(&[Token::Open]) else {
                // A bare name is the dialogue's own variable: the reference looks it up in the dialogue's context first.
                let expression = Expression::Variable {
                    scope: "var_val".to_owned(),
                    name: name.clone(),
                };
                return Some((expression, after));
            };
            let (argument_tokens, rest) = split_arguments(after_open)?;
            if matches!(
                name.as_str(),
                "u_val" | "n_val" | "npc_val" | "global_val" | "var_val" | "context_val"
            ) {
                let [Token::Str(variable)] = argument_tokens else {
                    return None;
                };
                let expression = Expression::Variable {
                    scope: name.clone(),
                    name: variable.clone(),
                };
                return Some((expression, rest));
            }
            let arity = math_function(name)?;
            let arguments = read_arguments(argument_tokens)?;
            match arity {
                Arity::Exactly(wanted) if arguments.len() == wanted => {}
                Arity::AtLeast(wanted) if arguments.len() >= wanted => {}
                _ => return None,
            }
            Some((
                Expression::Call {
                    name: name.clone(),
                    arguments,
                },
                rest,
            ))
        }
        Token::Open => {
            let (inner, rest) = parse_precedence(tokens.get(1..)?, 0)?;
            let after = rest.strip_prefix(&[Token::Close])?;
            Some((inner, after))
        }
        Token::Operator(spelling) if spelling == "-" => {
            let (inner, rest) = parse_primary(tokens.get(1..)?)?;
            Some((
                Expression::Binary {
                    op: Operator::Subtract,
                    left: Box::new(Expression::Number(0.0)),
                    right: Box::new(inner),
                },
                rest,
            ))
        }
        _ => None,
    }
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
        Value::Str(text) => read_leaf(text).or_else(|| read_text_expression(text)),
        // The data's own shape: `"math"` holds a one-element array wrapping a string expression, and the array form
        // `[left, op, right]` is the fully parenthesised one the port also accepts.
        Value::Array(parts) if parts.len() == 1 => read_expression(parts.first()?),
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
    if name.is_empty()
        || !matches!(
            scope,
            "u_val" | "n_val" | "npc_val" | "global_val" | "var_val" | "context_val"
        )
    {
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

/// An operation a `math` effect performs on a variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentOp {
    /// `x = e`
    Set,
    /// `x += e`
    Add,
    /// `x -= e`
    Subtract,
    /// `x *= e`
    Multiply,
    /// `x /= e`
    Divide,
}

/// A `math` effect's statement: a variable and the expression that changes it.
///
/// Reference: the `math` effect (`npctalk_funcs.cpp`) reads one string per entry, and the string is an **assignment** - the
/// data writes `"void_spider_killed = 1"`, not a bare expression. The port had an expression parser and nothing that could
/// read the `name = ...` half, which is why 304 effect entries were refused.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    /// Where the value is written.
    pub target: AssignmentTarget,
    /// What is done to it.
    pub op: AssignmentOp,
    /// The value, worked out against the dialogue.
    pub expression: Expression,
}

/// Where a `math` statement writes its result.
///
/// The data uses both spellings: `"x = 1"` names a bare variable, and `"u_val('x') = 1"` names the variable the way the
/// expression language does. They are the same thing - the reference's `read_var_info` resolves the second form to the same
/// variable type the first implies (`condition.cpp:408-437`) - so the port keeps one target type with the scope recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignmentTarget {
    /// A bare name, which the dialogue looks up in its own context.
    Bare(String),
    /// A variable named the way the expression language names one: `u_val('x')`.
    Variable {
        /// The variable's type, as the data spells it.
        scope: String,
        /// The variable's name.
        name: String,
    },
}

impl AssignmentTarget {
    /// The variable's name, whichever way it was written.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Bare(name) | Self::Variable { name, .. } => name,
        }
    }

    /// The scope the target names, or nothing for a bare name.
    #[must_use]
    pub fn scope(&self) -> Option<&str> {
        match self {
            Self::Bare(_) => None,
            Self::Variable { scope, .. } => Some(scope.as_str()),
        }
    }
}

/// Reads a `math` statement, or nothing when it is not an assignment the port can perform.
#[must_use]
pub fn read_assignment(text: &str) -> Option<Assignment> {
    let (name_part, op, rest) = split_assignment(text)?;
    let target = match read_text_expression(name_part.trim())? {
        Expression::Variable { scope, name } => AssignmentTarget::Variable { scope, name },
        // A destination must be a variable: a number, a call or an operator chain is an expression, and a `math` statement
        // whose left-hand side is an expression has no place to put its answer.
        Expression::Number(_) | Expression::Call { .. } | Expression::Binary { .. } => return None,
    };
    Some(Assignment {
        target,
        op,
        expression: read_text_expression(rest)?,
    })
}

/// Splits `name OP expression` at its top-level assignment operator, longest operator first.
fn split_assignment(text: &str) -> Option<(&str, AssignmentOp, &str)> {
    let mut depth = 0i32;
    let bytes = text.as_bytes();
    let mut at = 0usize;
    while at < bytes.len() {
        let character = bytes.get(at).copied()?;
        match character {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'=' if depth == 0 => {
                let (op, width) = match (at.checked_sub(1).and_then(|before| bytes.get(before)), bytes.get(at + 1)) {
                    (Some(b'+'), _) => (AssignmentOp::Add, 2),
                    (Some(b'-'), _) => (AssignmentOp::Subtract, 2),
                    (Some(b'*'), _) => (AssignmentOp::Multiply, 2),
                    (Some(b'/'), _) => (AssignmentOp::Divide, 2),
                    (_, Some(b'=')) => return None,
                    _ => (AssignmentOp::Set, 1),
                };
                let before_op = match op {
                    AssignmentOp::Set => at,
                    _ => at.checked_sub(1)?,
                };
                return Some((
                    text.get(..before_op)?,
                    op,
                    text.get(at + width..)?,
                ));
            }
            _ => {}
        }
        at += 1;
    }
    None
}
