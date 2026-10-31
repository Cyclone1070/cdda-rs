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
    /// A condition written as a bare name, which the reference resolves to an argument-less condition.
    ///
    /// Reference: `read_condition` (`condition.cpp:515-540`) hands a string to `conditional_t( std::string_view type )`
    /// (`condition.cpp:2783-2810`), which looks the name up among the **simple** parsers - the conditions that take no argument.
    /// The port accepts the three whose answers it can already give and refuses the rest, so the census stays honest about which
    /// names it can read.
    Simple {
        /// The name the guard uses.
        name: String,
    },
    /// The sky is doing one of these things.
    ///
    /// Reference: `f_is_weather` (`condition.cpp:1812-1818`).
    IsWeather {
        /// The weather ids any of which answers true.
        wanted: Vec<String>,
    },
    /// The player has taken a mission.
    ///
    /// Reference: `f_u_has_mission` (`condition.cpp:709-721`), which walks the avatar's active missions for a matching type id.
    HasMission {
        /// The mission's type id.
        mission: String,
    },
    /// A die rolled against a chance: true `one_in( n )` of the time.
    ///
    /// Reference: `f_one_in_chance` (`condition.cpp:1760-1766`) hands the value to `one_in` (`src/rng.cpp:196-199`), which is
    /// `chance <= 1 || rng( 0, chance - 1 ) == 0` - so a chance of one or less always holds.
    OneInChance(Box<Expression>),
    /// A roll against `x` in `y`.
    ///
    /// Reference: `f_x_in_y_chance` (`condition.cpp:1801-1810`), whose object holds `x` and `y`, answered by `x_in_y`
    /// (`src/rng.cpp:206-209`) as `rng_float( 0, 1 ) <= x / y`.
    XInYChance {
        /// The numerator.
        x: Box<Expression>,
        /// The denominator.
        y: Box<Expression>,
    },
    /// A creature carries a trait.
    ///
    /// Reference: `f_has_trait` (`condition.cpp:580-586`), which asks `has_trait( trait_id( name ) )` of the actor the member
    /// names - `u_has_trait` for the speaker, `npc_has_trait` for the listener.
    HasTrait {
        /// The trait's id.
        trait_name: String,
        /// Whether the question is about the listener rather than the speaker.
        listener: bool,
    },
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
    /// The speaker stands on an overmap tile of a named terrain.
    ///
    /// Reference: `f_at_om_location` (`condition.cpp:1023-1046`), which reads the speaker's overmap tile and compares its terrain id - with
    /// direction and connections stripped (`oter_no_dir_or_connections`) - against the string the data writes.
    AtOmLocation {
        /// The terrain id the data writes.
        place: StringSource,
        /// Whether it asks about the listener rather than the speaker.
        listener: bool,
    },
    /// Every one of these variables is set in the dialogue's context.
    ///
    /// Reference: `f_expects_vars` (`condition.cpp:1111-1131`), which answers false when any named value is missing from the
    /// invocation's context - it is how a program states what its caller must hand it.
    ExpectsVars {
        /// The names that must be present.
        names: Vec<StringSource>,
    },
    /// The date is in a named season.
    ///
    /// Reference: `f_is_season` (`condition.cpp:1250-1260`), which compares `season_of_year( calendar::turn )` against one of the four
    /// names `"spring"`, `"summer"`, `"autumn"`, `"winter"`.
    IsSeason {
        /// The season's name, as the data writes it.
        season: StringSource,
    },
    /// Another program's own condition, asked in place.
    ///
    /// Reference: `f_test_eoc` (`condition.cpp:1949-1961`) looks the named effect-on-condition up and evaluates **its condition** - it
    /// does not run the program, so nothing it would do happens.
    TestEoc {
        /// The program's id, as the data writes it.
        id: StringSource,
    },
    /// A creature holds at least one of these traits.
    ///
    /// Reference: `f_has_any_trait` (`condition.cpp:563-578`), which walks the list and holds as soon as one is held.
    HasAnyTrait {
        /// The trait names, any one of which answers true.
        traits: Vec<StringSource>,
        /// Whether it asks about the listener rather than the speaker.
        listener: bool,
    },
    /// A creature is under at least one of these effects.
    ///
    /// Reference: `f_has_any_effect` (`condition.cpp:953-...`), the list form of `u_has_effect`, holding as soon as one is active.
    HasAnyEffect {
        /// The effect ids, any one of which answers true.
        effects: Vec<StringSource>,
        /// Whether it asks about the listener rather than the speaker.
        listener: bool,
    },
    /// A creature carries at least one of an item, by stack or by charge.
    ///
    /// Reference: `f_has_item` (`condition.cpp:826-834`) answers `charges_of( id ) > 0 || has_amount( id, 1 )`, so a partly used
    /// stack of a charge item still counts.
    /// Whether a creature has a bionic installed.
    ///
    /// Reference: `f_has_bionics` (`condition.cpp:940-951`): the member names a bionic id, or the word `ANY`, which holds when the creature has
    /// any bionic at all or a full power store.
    HasBionics {
        /// The bionic's id, or `ANY`.
        bionic: StringSource,
        /// Whether the question is about the listener rather than the speaker.
        listener: bool,
    },
    /// Whether a creature is carrying an item.
    ///
    /// Reference: `f_has_item` (`condition.cpp`), which asks the talker's inventory for the id.
    HasItem {
        /// The item's id, as the data writes it.
        item: StringSource,
        /// Whether it asks about the listener rather than the speaker.
        listener: bool,
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
        // `time` takes a duration **string**, and parses like any other one-argument function: the argument's shape is checked
        // when it is evaluated, not when it is read.
        | "fahrenheit" | "from_celsius" | "from_fahrenheit" | "time"
        // `rand( n )` (`math_parser_func.h:57-60`) is one argument too; it is refused at run time only when the caller has no dice.
        | "rand" => Arity::Exactly(1),
        "clamp" => Arity::Exactly(3),
        // Reference: `rng( lo, hi )` (`math_parser_func.h:141-163`).
        "rng" => Arity::Exactly(2),
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
    /// A quoted text used as an argument, which only the functions that take one can read.
    ///
    /// Reference: `time( '1 minutes' )` passes a **duration string**, and the reference hands it to the duration parser rather than to
    /// arithmetic. The port keeps the literal as text so it survives parsing, and refuses it anywhere a number is wanted.
    Text(String),
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
    // Reference: a bare name is an argument-less condition (`condition.cpp:2783-2810`), and the port reads the three it can answer.
    if let Value::Str(name) = value {
        return matches!(
            name.as_str(),
            "has_beta" | "u_is_avatar" | "u_is_character"
        )
        .then(|| Condition::Simple { name: name.clone() });
    }
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
    for (member, listener) in [("u_at_om_location", false), ("npc_at_om_location", true)] {
        if let Some(place) = object.get(member) {
            return Some(Condition::AtOmLocation {
                place: read_string_source(place)?,
                listener,
            });
        }
    }
    if let Some(Value::Array(names)) = object.get("expects_vars") {
        return Some(Condition::ExpectsVars {
            names: read_string_sources(names)?,
        });
    }
    if let Some(season) = object.get("is_season") {
        return Some(Condition::IsSeason {
            season: read_string_source(season)?,
        });
    }
    if let Some(guard) = read_world_guard(object) {
        return Some(guard);
    }
    // Reference: `f_has_item` (`condition.cpp:826-834`), the sibling of `u_has_items` that asks only "any at all".
    if let Some(bionic) = read_bionic_condition(object) {
        return Some(bionic);
    }
    for (member, listener) in [("u_has_item", false), ("npc_has_item", true)] {
        if let Some(item) = object.get(member) {
            return Some(Condition::HasItem {
                item: read_string_source(item)?,
                listener,
            });
        }
    }
    // Reference: `f_u_has_mission` (`condition.cpp:709-721`) answers about the **avatar's** active missions, not the speaker's.
    if let Some(Value::Str(mission)) = object.get("u_has_mission") {
        return Some(Condition::HasMission {
            mission: mission.clone(),
        });
    }
    // Reference: `f_has_trait` (`condition.cpp:580-586`) - the actor simply has to carry the trait. Measured against the data,
    // this is the second biggest guard the port could not answer (36 programs).
    for (member, listener) in [("u_has_trait", false), ("npc_has_trait", true)] {
        if let Some(Value::Str(trait_name)) = object.get(member) {
            return Some(Condition::HasTrait {
                trait_name: trait_name.clone(),
                listener,
            });
        }
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



/// Reads the guards that ask about the world rather than a creature: the weather and the avatar's missions.
///
/// Reference: `f_is_weather` (`condition.cpp:1812-1818`) and `f_u_has_mission` (`condition.cpp:709-721`).
fn read_world_guard(object: &cdda_json::Object) -> Option<Condition> {
    // Reference: `f_is_weather` (`condition.cpp:1812-1818`) compares the happening weather's id with the one the guard names. The data
    // writes both a bare string and a list of them, and the reference's `get_str_or_var` reads a list as "any of these".
    if let Some(named) = object.get("is_weather") {
        let wanted = match named {
            Value::Str(id) => vec![id.clone()],
            Value::Array(ids) => ids
                .iter()
                .map(|id| match id {
                    Value::Str(id) => Some(id.clone()),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?,
            _ => return None,
        };
        return Some(Condition::IsWeather { wanted });
    }
    // Reference: `f_one_in_chance` (`condition.cpp:1760-1766`).
    if let Some(chance) = object.get("one_in_chance") {
        return Some(Condition::OneInChance(Box::new(read_expression(chance)?)));
    }
    // Reference: `f_x_in_y_chance` (`condition.cpp:1801-1810`), whose member is the object holding `x` and `y`.
    if let Some(Value::Object(chance)) = object.get("x_in_y_chance") {
        return Some(Condition::XInYChance {
            x: Box::new(read_expression(chance.get("x")?)?),
            y: Box::new(read_expression(chance.get("y")?)?),
        });
    }
    if let Some(id) = object.get("test_eoc") {
        return Some(Condition::TestEoc {
            id: read_string_source(id)?,
        });
    }
    if let Some(list) = read_list_guard(object) {
        return Some(list);
    }
    None
}


/// Reads the two guards that ask about a bionic, `u_has_bionics` and `npc_has_bionics`.
///
/// Reference: `f_has_bionics` (`condition.cpp:940-951`).
fn read_bionic_condition(object: &cdda_json::Object) -> Option<Condition> {
    for (member, listener) in [("u_has_bionics", false), ("npc_has_bionics", true)] {
        if let Some(bionic) = object.get(member) {
            return Some(Condition::HasBionics {
                bionic: read_string_source(bionic)?,
                listener,
            });
        }
    }
    None
}

/// Reads the two guards whose member is a list of names: `u_has_any_trait` and `u_has_any_effect`.
///
/// Reference: `f_has_any_trait` (`condition.cpp:563-578`) and `f_has_any_effect` (`condition.cpp:953`), the list forms of the
/// single-name guards, which hold as soon as one name matches.
fn read_list_guard(object: &cdda_json::Object) -> Option<Condition> {
    // Reference: `f_has_any_trait` (`condition.cpp:563-578`) and `f_has_any_effect` (`condition.cpp:953`) - the list forms of the two
    // single-name guards, whose members are arrays of names.
    for (member, listener) in [("u_has_any_trait", false), ("npc_has_any_trait", true)] {
        if let Some(Value::Array(names)) = object.get(member) {
            return Some(Condition::HasAnyTrait {
                traits: read_string_sources(names)?,
                listener,
            });
        }
    }
    for (member, listener) in [("u_has_any_effect", false), ("npc_has_any_effect", true)] {
        if let Some(Value::Array(names)) = object.get(member) {
            return Some(Condition::HasAnyEffect {
                effects: read_string_sources(names)?,
                listener,
            });
        }
    }
    None
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
pub fn read_string_sources(values: &[Value]) -> Option<Vec<StringSource>> {
    values.iter().map(read_string_source).collect()
}

/// Reads one string, or where it comes from.
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
            // A single quoted literal is a text argument, not an expression: `time( '1 minutes' )`.
            let (argument_tokens, rest) = split_arguments(after_open)?;
            if let [Token::Str(text)] = argument_tokens
                && math_function(name).is_some()
            {
                {
                    return Some((
                        Expression::Call {
                            name: name.clone(),
                            arguments: vec![Expression::Text(text.clone())],
                        },
                        rest,
                    ));
                }
            }
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

/// Reads an expression, which the data may also wrap in a `math` object.
///
/// The dialogue writes a value that needs computing as `{"math": ["…"]}` - the same shape a condition uses for its expressions - so a member
/// like `intensity` can be either a number and an expression or an object holding one.
fn read_math_expression(value: &Value) -> Option<Expression> {
    if let Value::Object(object) = value
        && let Some(Value::Array(statements)) = object.get("math")
    {
        let mut joined = String::new();
        for statement in statements {
            let Value::Str(text) = statement else {
                return None;
            };
            joined.push_str(text);
        }
        return read_text_expression(&joined);
    }
    read_expression(value)
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
        Expression::Number(_)
        | Expression::Text(_)
        | Expression::Call { .. }
        | Expression::Binary { .. } => return None,
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

/// How long an added effect lasts.
///
/// Reference: `f_add_effect` (`npctalk.cpp:3426-3450`) and `get_duration_or_var`, which accept a duration string, a number of turns, an array
/// of two ends, or `PERMANENT`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EffectDuration {
    /// It lasts until something removes it.
    Permanent,
    /// A fixed number of turns, or the reference's default when the data states none.
    Turns(Option<i64>),
    /// A roll between two ends, inclusive.
    Range(i64, i64),
}

/// An effect a dialogue program can run.
///
/// Reference: `talk_effect_fun_t`'s table (`npctalk.cpp:8039-8135`), which maps each member name to a function. The port keeps
/// the same shape - a name, a function - and grows one name at a time, counting what it can run against the data.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// A `math` statement list: one or more assignments to dialogue variables.
    Math(Vec<Assignment>),
    /// A message to the player.
    Message(String),
    /// Run one list of effects or another, depending on a condition.
    ///
    /// Reference: `f_if` (`npctalk.cpp:6977-6993`): the `if` member is a condition, `then` is the list run when it holds, and `else`
    /// is the list run when it does not - absent, it runs nothing. 100 of the data's effect entries are one of these, and each is
    /// worth more than itself because it can hold any other effect.
    If {
        /// What decides which list runs.
        condition: Box<Condition>,
        /// The effects run when the condition holds.
        then: Vec<Effect>,
        /// The effects run when it does not.
        otherwise: Vec<Effect>,
    },
    /// Give a creature an effect.
    ///
    /// Reference: `f_add_effect` (`npctalk.cpp:3426-3450`). The duration is a time string, the word `PERMANENT`, or absent - in
    /// which case it is 1000 turns. The intensity defaults to zero, which the effect's own rule raises to one.
    AddEffect {
        /// The effect's id.
        effect: String,
        /// How long it lasts.
        duration: EffectDuration,
        /// How strong it is, as an expression the reference evaluates when the effect is applied.
        intensity: Option<Expression>,
        /// Whether it goes to the listener rather than the speaker.
        listener: bool,
    },
    /// Take an effect away from a creature.
    ///
    /// Reference: `f_remove_effect` (`npctalk.cpp`), which calls `remove_effect` with the effect's own body part.
    RemoveEffect {
        /// The effect's id.
        effect: String,
        /// Whether it comes from the listener rather than the speaker.
        listener: bool,
    },
    /// Give a trait to a creature.
    ///
    /// Reference: `f_add_trait` (`npctalk.cpp:3498-3508`), which calls `set_mutation`. Its `variant` member is refused rather than
    /// dropped: a variant changes which mutations are legal to take next, and the port has no mutation graph to change.
    LearnTrait {
        /// The trait's id.
        trait_name: String,
        /// Whether it goes to the listener rather than the speaker.
        listener: bool,
    },
    /// Take a trait away from a creature.
    ///
    /// Reference: `f_remove_trait` (`npctalk.cpp:3536-3543`), which calls `unset_mutation`.
    LoseTrait {
        /// The trait's id.
        trait_name: String,
        /// Whether it goes from the listener rather than the speaker.
        listener: bool,
    },
    /// Open a conversation, or run the branch that says it could not be opened.
    ///
    /// Reference: `f_open_dialogue` (`npctalk.cpp:5476-5502`): only the **avatar** can open one, so a speaker that is not the avatar runs
    /// `false_eocs` and stops; otherwise the conversation opens and `true_eocs` run. The port runs the branch rule exactly and does not open a
    /// conversation, because there is no dialogue interface to open (P10).
    OpenDialogue {
        /// The topic to open, when the data names one.
        topic: Option<StringSource>,
        /// The programs to run when the conversation opened.
        then: Vec<ProgramRef>,
        /// The programs to run when it did not.
        otherwise: Vec<ProgramRef>,
    },
    /// Run one of several programs, chosen by weight.
    ///
    /// Reference: `f_weighted_list_eocs` (`npctalk.cpp:6950-6975`): each entry is a `[ program, weight ]` pair, the weights are evaluated and one
    /// program is picked by them - so an entry with weight zero is never picked, and the heavier an entry the likelier it is.
    WeightedList(Vec<(ProgramRef, Expression)>),
    /// Say a line picked from a snippet category.
    ///
    /// Reference: `f_message` (`npctalk.cpp:5312-5405`) with `snippet: true`: the member names a **category**, not a message, and the line is
    /// picked from that category by weight. With `same_snippet` the pick is remembered so the same line repeats.
    Snippet {
        /// The snippet category to pick from.
        category: StringSource,
        /// Whether the pick is remembered rather than made afresh.
        same: bool,
        /// Whether it goes to the listener's log rather than the speaker's.
        listener: bool,
    },
    /// Give a creature a morale point.
    ///
    /// Reference: `f_add_morale` (`npctalk.cpp:7177-7195`), whose defaults are a one-hour duration and a half-hour decay start.
    AddMorale {
        /// The morale type's id, which is what stacks.
        kind: StringSource,
        /// How much it is worth before decay.
        bonus: Expression,
        /// The cap it may reach.
        max_bonus: Expression,
        /// How long it lasts.
        duration: EffectDuration,
        /// The age at which it starts to fade.
        decay_start: EffectDuration,
        /// Whether the cap applies even at zero.
        capped: bool,
        /// Whether it goes to the listener rather than the speaker.
        listener: bool,
    },
    /// Put a location into a dialogue variable.
    ///
    /// Reference: `f_location_variable` (`npctalk.cpp:4194`). The port reads the plain form - the actor's own square - and refuses the
    /// searching forms (`target_params`, `terrain`/`furniture`/`field`/`monster`/`npc`/`species`/`trap`/`zone`, radii): those walk the overmap
    /// and the map for the nearest match, which nothing in the port can do yet.
    LocationVariable {
        /// The variable's scope, as the data spells it: `u_val`, `npc_val`, `global_val`.
        scope: String,
        /// The variable's name.
        name: String,
        /// Whether the location is the listener's rather than the speaker's.
        listener: bool,
    },
    /// Run the last case whose value is at most the switch's value.
    ///
    /// Reference: `f_switch` (`npctalk.cpp:6998-7020`): each case is tried in order, and **every** case whose number the switch value
    /// reaches replaces the chosen one - so the *last* matching case runs, not the first, and the cases need not be sorted.
    Switch {
        /// The value being switched on.
        on: Expression,
        /// The cases, in the data's order.
        cases: Vec<(Expression, Vec<Effect>)>,
    },
    /// Run another effect-on-condition, by id or as a program written where it is used.
    ///
    /// Reference: `f_run_eocs` (`npctalk.cpp`) - the entry is a name, an array of names, or an object with `run_eocs` and
    /// modifiers. The port reads the two plain forms: one name, or a list of them.
    RunEoc(Vec<ProgramRef>),
}

impl Effect {
    /// The member the data spells this effect with, for a report about one.
    ///
    /// The names are the data's own - `math`, `u_message`, `run_eocs`, `u_add_effect`, `u_learn_trait` - so a report can be read beside the
    /// JSON it came from without a translation table. The two the reference spells with a shared name have the name the reader uses.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Math(_) => "math",
            Self::Message(_) | Self::Snippet { .. } => "u_message",
            Self::If { .. } => "if",
            Self::AddEffect { .. } => "u_add_effect",
            Self::RemoveEffect { .. } => "u_lose_effect",
            Self::LearnTrait { .. } => "u_add_trait",
            Self::LoseTrait { .. } => "u_lose_trait",
            Self::RunEoc(_) => "run_eocs",
            Self::Switch { .. } => "switch",
            Self::LocationVariable { .. } => "u_location_variable",
            Self::WeightedList(_) => "weighted_list_eocs",
            Self::OpenDialogue { .. } => "open_dialogue",
            Self::AddMorale { .. } => "u_add_morale",
        }
    }
}

/// What a `run_eocs` entry names.
///
/// Reference: `load_eoc_vector_id_and_var` (`npctalk.cpp:512-541`) accepts a **name** or an **object**, and an object without an `id`
/// is an **inline** effect-on-condition loaded on the spot (`load_inline_eoc`). The port keeps both shapes; an object carrying
/// `variables` is refused, because its context values live in a scope the port has not ported.
#[derive(Debug, Clone, PartialEq)]
pub enum ProgramRef {
    /// An effect-on-condition the data defines elsewhere.
    Named {
        /// The program's id.
        id: String,
        /// The context values the call hands it, as `variables`.
        variables: Vec<(String, StringSource)>,
    },
    /// A program written where it is used.
    Inline {
        /// The program.
        program: Box<Program>,
        /// The context values the call hands it, as `variables`.
        variables: Vec<(String, StringSource)>,
    },
}

/// One effect-on-condition, as the data defines it.
///
/// Reference: `effect_on_condition.cpp:119-133`. A named one has an `id`, a `condition` and an `effect` list; the port keeps
/// the same three parts, and refuses a definition any of whose parts it cannot read rather than running half a program.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    /// Its id, which is how another effect names it.
    pub id: String,
    /// What must hold before it runs, when it states a condition.
    pub condition: Option<Condition>,
    /// What it does when it runs.
    pub effects: Vec<Effect>,
}

/// Reads an effect-on-condition definition, or nothing when the port cannot run it whole.
#[must_use]
pub fn read_program(value: &Value) -> Option<Program> {
    let Value::Object(object) = value else {
        return None;
    };
    let Some(Value::Str(id)) = object.get("id") else {
        return None;
    };
    let condition = match object.get("condition") {
        None => None,
        Some(condition) => Some(read_condition(condition)?),
    };
    let entries = match object.get("effect") {
        Some(Value::Array(entries)) => entries.as_slice(),
        // An effect-on-condition with no `effect` still has a condition to ask; the reference runs nothing, which the port
        // reads as an empty list rather than refusing the definition.
        None => &[],
        _ => return None,
    };
    let effects = entries
        .iter()
        .map(read_effect)
        .collect::<Option<Vec<_>>>()?;
    Some(Program {
        id: id.clone(),
        condition,
        effects,
    })
}

/// Reads one effect entry of an effect-on-condition, or nothing when the port has no rule for it.
///
/// A named entry (`"EOC_something"`) is a call to another effect-on-condition, which the port does not run yet; an object is
/// read through the member that names the effect.
#[must_use]
pub fn read_effect(value: &Value) -> Option<Effect> {
    // Reference: a bare name in an effect list runs that effect-on-condition.
    if let Value::Str(name) = value {
        return Some(Effect::RunEoc(vec![ProgramRef::Named {
            id: name.clone(),
            variables: Vec::new(),
        }]));
    }
    let Value::Object(object) = value else {
        return None;
    };
    if let Some(condition) = object.get("if") {
        let then = read_effect_list(object.get("then"))?;
        // An absent `else` runs nothing, which the reference allows; a present one that the port cannot read refuses the whole.
        let otherwise = match object.get("else") {
            None => Vec::new(),
            Some(entries) => read_effect_list(Some(entries))?,
        };
        return Some(Effect::If {
            condition: Box::new(read_condition(condition)?),
            then,
            otherwise,
        });
    }
    if let Some(names) = object.get("run_eocs") {
        let entries = match names {
            Value::Array(entries) => entries.as_slice(),
            other => std::slice::from_ref(other),
        };
        let programs = entries
            .iter()
            .map(read_program_ref)
            .collect::<Option<Vec<_>>>()?;
        return Some(Effect::RunEoc(programs));
    }
    if let Some(morale) = read_add_morale(object) {
        return Some(morale);
    }
    if let Some(dialogue) = read_open_dialogue(object) {
        return Some(dialogue);
    }
    if let Some(weighted) = read_weighted_list(object) {
        return Some(weighted);
    }
    if let Some(location) = read_location_variable(object) {
        return Some(location);
    }
    if let Some(switched) = read_switch(object) {
        return Some(switched);
    }
    if let Some(Value::Array(statements)) = object.get("math") {
        // Reference: `eoc_math::from_json` (`condition.cpp:2494-2502`) **concatenates** the array's strings and parses the result as
        // one statement - `combined.append( objects.get_string( i ) )` with nothing between them, then one `defer_math`. So
        // `["x", " *= ", "3"]` and `["x *= 3"]` are the same effect, and a port that read one statement per element would refuse
        // every split one. The array is a token list, not a program.
        let mut joined = String::new();
        for statement in statements {
            let Value::Str(text) = statement else {
                return None;
            };
            joined.push_str(text);
        }
        return read_assignment(&joined).map(|one| Effect::Math(vec![one]));
    }
    if matches!(object.get("snippet"), Some(Value::Bool(true))) {
        return read_snippet(object);
    }
    if let Some(changed) = read_effect_change(object) {
        return Some(changed);
    }
    // Reference: `f_add_trait` and `f_remove_trait` (`npctalk.cpp:3498-3543`). A `variant` member is a mutation-graph change the port
    // has no graph for, so an entry that states one is refused whole.
    for (member, listener, learn) in [
        ("u_add_trait", false, true),
        ("npc_add_trait", true, true),
        ("u_lose_trait", false, false),
        ("npc_lose_trait", true, false),
    ] {
        if let Some(Value::Str(trait_name)) = object.get(member) {
            if learn && object.get("variant").is_some() {
                return None;
            }
            return Some(if learn {
                Effect::LearnTrait {
                    trait_name: trait_name.clone(),
                    listener,
                }
            } else {
                Effect::LoseTrait {
                    trait_name: trait_name.clone(),
                    listener,
                }
            });
        }
    }
    for member in ["u_message", "npc_message"] {
        if let Some(value) = object.get(member) {
            return read_message(value).map(Effect::Message);
        }
    }
    None
}

/// Reads a list of effects, refusing the whole list when any entry in it is refused.
///
/// Reference: `talk_effect_t::load_effect` (`npctalk.cpp`), which reads an array or a single entry. The port reads an array, and an
/// entry that is not an array is refused rather than read as a one-element list - the data always writes arrays here, so a
/// non-array is a shape the port has not seen rather than a shape it should guess at.
fn read_effect_list(value: Option<&Value>) -> Option<Vec<Effect>> {
    // Reference: `talk_effect_t::load_effect` (`npctalk.cpp`) reads an array **or a single entry**, so `"then": {...}` is a one-entry list
    // rather than a shape the port should refuse. The data writes both.
    match value {
        Some(Value::Array(entries)) => entries.iter().map(read_effect).collect(),
        Some(single) => Some(vec![read_effect(single)?]),
        None => None,
    }
}







/// Reads a message whose member names a **snippet category** rather than the text itself.
///
/// Reference: `f_message` (`npctalk.cpp:5312-5405`): with `snippet: true` the member is a category of text, the line is picked from it by weight,
/// and `same_snippet` remembers the pick so the same line repeats.
fn read_snippet(object: &cdda_json::Object) -> Option<Effect> {
    for member in ["u_message", "npc_message"] {
        let Some(value) = object.get(member) else {
            continue;
        };
        return Some(Effect::Snippet {
            category: read_string_source(value)?,
            same: matches!(object.get("same_snippet"), Some(Value::Bool(true))),
            listener: member == "npc_message",
        });
    }
    None
}

/// Reads a morale point being added, which may name its type with a variable and compute its numbers.
///
/// Reference: `f_add_morale` (`npctalk.cpp:7177-7195`): the type is a `str_or_var`, the bonus and cap are `dbl_or_var`, and the two times
/// default to an hour and half an hour.
fn read_add_morale(object: &cdda_json::Object) -> Option<Effect> {
    for (member, listener) in [("u_add_morale", false), ("npc_add_morale", true)] {
        let Some(kind) = object.get(member) else {
            continue;
        };
        return Some(Effect::AddMorale {
            kind: read_string_source(kind)?,
            bonus: read_math_expression(object.get("bonus")?)?,
            max_bonus: read_math_expression(object.get("max_bonus")?)?,
            duration: read_effect_duration(object.get("duration"), EffectDuration::Turns(Some(3600)))?,
            decay_start: read_effect_duration(object.get("decay_start"), EffectDuration::Turns(Some(1800)))?,
            capped: matches!(object.get("capped"), Some(Value::Bool(true))),
            listener,
        });
    }
    None
}

/// Reads a duration member: a string, a number of turns, two ends, or the default the caller names.
fn read_effect_duration(value: Option<&Value>, fallback: EffectDuration) -> Option<EffectDuration> {
    match value {
        None => Some(fallback),
        Some(Value::Str(text)) if text == "PERMANENT" => Some(EffectDuration::Permanent),
        Some(Value::Str(text)) => Some(EffectDuration::Turns(Some(crate::constructions::parse_duration(text)))),
        Some(Value::Int(turns)) => Some(EffectDuration::Turns(Some(*turns))),
        Some(Value::Float(turns)) => {
            #[allow(clippy::cast_possible_truncation)]
            let whole = *turns as i64;
            Some(EffectDuration::Turns(Some(whole)))
        }
        Some(Value::Array(ends)) => {
            let rolls = ends
                .iter()
                .map(|end| match end {
                    Value::Int(turns) => Some(*turns),
                    Value::Str(text) => Some(crate::constructions::parse_duration(text)),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            let [shortest, longest] = rolls.as_slice() else {
                return None;
            };
            Some(EffectDuration::Range(*shortest, *longest))
        }
        Some(_) => None,
    }
}

/// Reads an `open_dialogue`, which may name a topic or carry its own branches.
///
/// Reference: `f_open_dialogue` (`npctalk.cpp:5476-5502`), whose member is a topic string or an object holding `topic`, `true_eocs` and
/// `false_eocs`.
fn read_open_dialogue(object: &cdda_json::Object) -> Option<Effect> {
    let member = object.get("open_dialogue")?;
    if let Value::Str(topic) = member {
        return Some(Effect::OpenDialogue {
            topic: Some(StringSource::Literal(topic.clone())),
            then: Vec::new(),
            otherwise: Vec::new(),
        });
    }
    let Value::Object(inner) = member else {
        return None;
    };
    let topic = match inner.get("topic") {
        Some(value) => Some(read_string_source(value)?),
        None => None,
    };
    Some(Effect::OpenDialogue {
        topic,
        then: read_program_refs(inner.get("true_eocs"))?,
        otherwise: read_program_refs(inner.get("false_eocs"))?,
    })
}

/// Reads a `true_eocs`/`false_eocs` list, which is a name, an array of them, or nothing.
fn read_program_refs(value: Option<&Value>) -> Option<Vec<ProgramRef>> {
    match value {
        None => Some(Vec::new()),
        Some(Value::Array(entries)) => entries.iter().map(read_program_ref).collect(),
        Some(other) => Some(vec![read_program_ref(other)?]),
    }
}

/// Reads a `weighted_list_eocs`, whose entries are `[ program, weight ]` pairs.
///
/// Reference: `f_weighted_list_eocs` (`npctalk.cpp:6950-6975`), where the program may be a name or an inline effect-on-condition.
fn read_weighted_list(object: &cdda_json::Object) -> Option<Effect> {
    let Some(Value::Array(entries)) = object.get("weighted_list_eocs") else {
        return None;
    };
    let weighted = entries
        .iter()
        .map(|entry| {
            let Value::Array(pair) = entry else {
                return None;
            };
            let [program, weight] = pair.as_slice() else {
                return None;
            };
            Some((read_program_ref(program)?, read_expression(weight)?))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Effect::WeightedList(weighted))
}

/// Reads a `switch`, whose cases each hold effects of their own.
///
/// Reference: `f_switch` (`npctalk.cpp:6998-7020`): the cases are tried in the data's order and the **last** one the value reaches runs.
fn read_switch(object: &cdda_json::Object) -> Option<Effect> {
    if let Some(on) = object.get("switch") {
        let Some(Value::Array(cases)) = object.get("cases") else {
            return None;
        };
        let cases = cases
            .iter()
            .map(|case| {
                let Value::Object(case) = case else {
                    return None;
                };
                Some((
                    read_expression(case.get("case")?)?,
                    read_effect_list(case.get("effect"))?,
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        return Some(Effect::Switch {
            on: read_expression(on)?,
            cases,
        });
    }
    None
}

/// Reads the plain form of a location variable, refusing the searching forms.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4194`). The searching forms (`target_params`, a `terrain`/`furniture`/`field`/`monster`/`npc`/
/// `species`/`trap`/`zone` list, radii) walk the overmap and the map for the nearest match, which nothing in the port can do yet - and answering
/// them with the actor's own square would be a wrong answer that looked right.
fn read_location_variable(object: &cdda_json::Object) -> Option<Effect> {
    /// The members that turn the effect into a search.
    const SEARCHING: [&str; 11] = [
        "target_params",
        "terrain",
        "furniture",
        "field",
        "monster",
        "npc",
        "species",
        "trap",
        "zone",
        "min_radius",
        "max_radius",
    ];
    for (member, listener) in [("u_location_variable", false), ("npc_location_variable", true)] {
        let Some(Value::Object(target)) = object.get(member) else {
            continue;
        };
        if SEARCHING.iter().any(|key| object.get(key).is_some()) {
            return None;
        }
        let (scope, name) = target.members().iter().find_map(|(scope, value)| match value {
            Value::Str(name) => Some((scope.clone(), name.clone())),
            _ => None,
        })?;
        return Some(Effect::LocationVariable {
            scope,
            name,
            listener,
        });
    }
    None
}

/// Reads a call's `variables`, which become the context values the program sees.
///
/// Reference: `f_run_eocs` (`npctalk.cpp:6332-6338`), which reads each member into the invocation's context map.
fn read_variables(value: Option<&Value>) -> Option<Vec<(String, StringSource)>> {
    match value {
        None => Some(Vec::new()),
        Some(Value::Object(variables)) => variables
            .members()
            .iter()
            .map(|(name, source)| Some((name.clone(), read_string_source(source)?)))
            .collect(),
        Some(_) => None,
    }
}

/// Reads one `run_eocs` entry: a name, or a program written inline.
///
/// Reference: `load_eoc_vector_id_and_var` (`npctalk.cpp:512-541`). An object with `variables` carries context values, which reach a
/// program through a scope the port has not ported; refusing it is honest, and reading it while dropping the variables would run a
/// different program from the one the data wrote.
fn read_program_ref(value: &Value) -> Option<ProgramRef> {
    match value {
        Value::Str(name) => Some(ProgramRef::Named {
            id: name.clone(),
            variables: Vec::new(),
        }),
        Value::Object(object) => {
            let variables = read_variables(object.get("variables"))?;
            // An entry naming an `id` runs that program; anything else is a program written here.
            if let Some(Value::Str(id)) = object.get("id") {
                return Some(ProgramRef::Named {
                    id: id.clone(),
                    variables,
                });
            }
            let effects = read_effect_list(object.get("effect"))?;
            let condition = match object.get("condition") {
                None => None,
                Some(condition) => Some(read_condition(condition)?),
            };
            Some(ProgramRef::Inline {
                program: Box::new(Program {
                    id: String::new(),
                    condition,
                    effects,
                }),
                variables,
            })
        }
        _ => None,
    }
}

/// Reads an effect a program adds to or removes from a creature.
///
/// Reference: `f_add_effect` (`npctalk.cpp:3426-3450`). A duration of `PERMANENT` keeps the effect until something removes it, and
/// an absent one is 1000 turns. A `target_part` other than `bp_null` names a body part the port does not model, so an entry that
/// states one is refused rather than applied to the whole creature.
fn read_effect_change(object: &cdda_json::Object) -> Option<Effect> {
// Reference: `f_add_effect` (`npctalk.cpp:3426-3450`). A duration of `PERMANENT` keeps the effect until something removes it,
// and an absent one is 1000 turns. A `target_part` other than `bp_null` names a body part the port does not model, so an entry
// that states one is refused rather than applied to the whole creature.
for (member, listener, learn) in [
    ("u_add_effect", false, true),
    ("npc_add_effect", true, true),
    ("u_remove_effect", false, false),
    ("npc_remove_effect", true, false),
] {
    if let Some(Value::Str(effect)) = object.get(member) {
        if let Some(Value::Str(part)) = object.get("target_part")
            && part != "bp_null"
        {
            return None;
        }
        if learn {
            // Reference: `get_duration_or_var` (`npctalk.cpp:3426-3450`) accepts a duration **string**, a number of turns, or an
            // array of two of either, which the reference rolls between. The port reads all three; a roll is made at run time.
            let duration = match object.get("duration") {
                Some(Value::Str(text)) if text == "PERMANENT" => EffectDuration::Permanent,
                Some(Value::Str(text)) => EffectDuration::Turns(Some(crate::constructions::parse_duration(text))),
                Some(Value::Int(turns)) => EffectDuration::Turns(Some(*turns)),
                Some(Value::Float(turns)) => {
                    #[allow(clippy::cast_possible_truncation)]
                    let whole = *turns as i64;
                    EffectDuration::Turns(Some(whole))
                }
                // An array is a range: the reference rolls a duration between the two ends.
                Some(Value::Array(ends)) => {
                    let rolls = ends
                        .iter()
                        .map(|end| match end {
                            Value::Int(turns) => Some(*turns),
                            Value::Str(text) => Some(crate::constructions::parse_duration(text)),
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()?;
                    let [shortest, longest] = rolls.as_slice() else {
                        return None;
                    };
                    EffectDuration::Range(*shortest, *longest)
                }
                Some(_) => return None,
                None => EffectDuration::Turns(Some(1000)),
            };
            // Reference: `get_dbl_or_var( jo, "intensity" )` - an intensity may be an expression, evaluated when the effect is applied.
            let intensity = match object.get("intensity") {
                Some(value) => Some(read_math_expression(value)?),
                None => None,
            };
            return Some(Effect::AddEffect {
                effect: effect.clone(),
                duration,
                intensity,
                listener,
            });
        }
        return Some(Effect::RemoveEffect {
            effect: effect.clone(),
            listener,
        });
    }
}
    None
}

/// Reads a message's text, or nothing when it is written in a shape the port cannot render.
///
/// Reference: `f_message` (`npctalk.cpp:5312-5331`) takes a translation - a string, or an object with `str` - and separately
/// takes `snippet`, which names a snippet the port has no table for. A snippet is refused rather than shown as its own id.
#[must_use]
pub fn read_message(value: &Value) -> Option<String> {
    match value {
        Value::Str(text) => Some(text.clone()),
        Value::Object(object) => match object.get("str") {
            Some(Value::Str(text)) => Some(text.clone()),
            _ => None,
        },
        _ => None,
    }
}
