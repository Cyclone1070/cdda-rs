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
    /// Whether a creature is standing under the sky.
    ///
    /// Reference: `f_is_outside` (`condition.cpp:2650`) with `is_creature_outside` (`weather.cpp:76`), which asks the map about the creature's own square.
    IsOutside {
        /// Whether it is the listener rather than the speaker.
        listener: bool,
    },
    /// Whether a named overmap terrain is within a range of the actor.
    ///
    /// Reference: `f_near_om_location` (`condition.cpp:1074`) for `u_near_om_location` and `npc_near_om_location`: every overmap tile within `range` is
    /// compared against the name, and the guard holds on the first match.
    NearOmLocation {
        /// The terrain name to look for.
        location: StringSource,
        /// How far to look, in overmap tiles.
        range: i32,
        /// Whether it is the listener rather than the speaker.
        listener: bool,
    },
    /// Whether anything a creature wears carries a flag.
    ///
    /// Reference: `f_has_worn_with_flag` (`condition.cpp`), which asks `worn_with_flag` - a flag on a **worn item's type**, not on the creature.
    HasWornWithFlag {
        /// The flag's name.
        flag: String,
        /// Whether it is the listener rather than the speaker.
        listener: bool,
    },
    /// Whether a creature carries a flag.
    ///
    /// Reference: `f_has_flag` (`condition.cpp:2529`): a creature's flag, which its mutations and worn items give it.
    HasFlag {
        /// The flag's name, as the data writes it (`GRAB`, `FLIES`).
        flag: String,
        /// Whether it is the listener rather than the speaker.
        listener: bool,
    },
    /// Whether a creature can see.
    ///
    /// Reference: `talker_character_const::can_see` (`talker_character.cpp:917`): not blind, and either awake or bearing `SEESLEEP`. The data writes
    /// this as a bare string - `"condition": "u_can_see"` - because the rule takes no value.
    CanSee {
        /// Whether it is the listener rather than the speaker.
        listener: bool,
    },
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
    /// The player knows a recipe.
    ///
    /// Reference: `f_u_know_recipe` (`condition.cpp:2034-2042`): `knows_recipe` is a membership test on `learned_recipes`, and the reference asks it of the
    /// **player** whoever the dialogue is with.
    KnowsRecipe {
        /// The recipe's id.
        recipe: String,
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
        /// Whether it asks about the listener rather than the speaker.
        ///
        /// The data spells this two ways - `u_has_effect` and `npc_has_effect` - and uses the second **263 times**, so a reader that knows only the first
        /// refuses a quarter of the data's guard on whom an effect sits.
        listener: bool,
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
        // `time_since( x )` (`math_parser_diag.cpp:1173`) takes the point to count from: a name (`cataclysm`, `midnight`, `noon`) or a turn.
        | "time_since"
        // `skill( 'bashing' )` (`math_parser_diag.cpp:961`) answers a character's level in a named skill.
        | "skill" | "npc_skill" | "u_skill"
        // Reference: `{ "spell_level", { "un", 1, spell_level_eval, spell_level_ass } }` and its `spell_exp` sibling (`math_parser_diag.cpp:1734-1737`).
        | "spell_level" | "u_spell_level" | "npc_spell_level" | "spell_exp" | "u_spell_exp" | "npc_spell_exp"
        // Reference: `{ "field_strength", { "ung", 1, field_strength_eval, {}, { "location" } } }` (`math_parser_diag.cpp:1703`) - one field name.
        | "field_strength" | "u_field_strength" | "npc_field_strength"
        // Reference: `{ "has_flag", { "un", 1, has_flag_eval } }` (`math_parser_diag.cpp:1707`) - one flag name, which the data often writes as a variable.
        | "has_flag" | "u_has_flag" | "npc_has_flag"
        // Reference: `{ "addiction_intensity", { "un", 1, addiction_intensity_eval } }` and `addiction_turns` (`math_parser_diag.cpp:1680-1681`) - one id each.
        | "addiction_intensity" | "u_addiction_intensity" | "npc_addiction_intensity"
        | "addiction_turns" | "u_addiction_turns" | "npc_addiction_turns"
        // `vitamin( 'mutagen' )` (`math_parser_diag.cpp:1584`) answers how much of one vitamin the body holds, and zero for one it holds none of.
        | "vitamin" | "u_vitamin" | "npc_vitamin"
        // `rand( n )` (`math_parser_func.h:57-60`) is one argument too; it is refused at run time only when the caller has no dice.
        | "rand" => Arity::Exactly(1),
        // The creature's own numbers: `u_hp` takes a body part the port does not model, so it is read with none or with one and answered from the
        // whole creature. `u_effect_intensity` and `has_var` take the name they ask about.
        // `u_hp` and its siblings take a body part the port does not model, so they are read with none or with one. The four opinion numbers
        // (`math_parser_diag.cpp:1408-1440`) work the same way: the name's prefix says whose opinion it is (`n_npc_value()` asks the NPC's), and the data
        // sometimes also passes a scope the prefix already said.
        // Reference: `{ "hp_max", { "un", 1, hp_max_eval } }` (`math_parser_diag.cpp:1714`) takes one body part; the port answers from the whole creature, so the
        // part is optional here for the same reason it is on `u_hp`.
        "hp_max" | "u_hp_max" | "npc_hp_max"
        | "u_hp" | "npc_hp" | "u_effect_intensity" | "npc_effect_intensity" | "has_var" | "moon_phase"
        | "npc_value" | "npc_anger" | "npc_trust" | "npc_fear" | "n_npc_value" | "n_npc_anger" | "n_npc_trust"
        | "n_npc_fear" | "u_npc_value" | "u_npc_anger" | "u_npc_trust" | "u_npc_fear"
        // Reference: `{ "monsters_nearby", { "ung", -1, ... } }` (`math_parser_diag.cpp:1719`) - any number of monster ids, plus keywords.
        | "u_monsters_nearby" | "npc_monsters_nearby" | "monsters_nearby"
        // Reference: `{ "characters_nearby", { "ung", 0, ... } }` (`math_parser_diag.cpp:736`) - no positional arguments at all, only keywords.
        | "u_characters_nearby" | "npc_characters_nearby" | "characters_nearby" => Arity::AtLeast(0),
        "clamp" => Arity::Exactly(3),
        // Reference: `rng( lo, hi )` (`math_parser_func.h:141-163`).
        // Reference: `{ "distance", { "g", 2, distance_eval } }` (`math_parser_diag.cpp:1690`) - two points, each a named actor or a stored location.
        "rng" | "distance" => Arity::Exactly(2),
        // Reference: `pain_eval` takes no positional arguments; it is scoped (`u_pain`/`npc_pain`) and reads a `type` keyword.
        // Reference: `{ "health", { "un", 0, health_eval, health_ass } }` (`math_parser_diag.cpp:1693`) - no arguments, and the scope says whose.
        // Reference: `{ "calories", { "un", 0, calories_eval, calories_ass, { "format", "dont_affect_weariness" } } }` (`math_parser_diag.cpp:1753`).
        "u_pain" | "npc_pain" | "health" | "u_health" | "npc_health" | "calories" | "u_calories" | "npc_calories" => {
            Arity::Exactly(0)
        }

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
    /// A pair of numbers the data wrote as an array, which means a random draw between them.
    ///
    /// Reference: `get_dbl_or_var` (`condition.cpp:207-226`) - a member written as a two-element array sets `pair`, and `dbl_or_var::evaluate` draws
    /// `rng( min, max )` instead of answering `min`. So `"bonus": [ -15, -30 ]` is a nightmare's morale hit drawn between two values, not a list.
    Pair {
        /// The low end of the draw, as written.
        low: f64,
        /// The high end of the draw, as written.
        high: f64,
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
        /// The named arguments the data passes after the positional ones, as `'unit': 'hour'`.
        keywords: Vec<(String, Expression)>,
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
    /// Logical not, written `!` before its one operand: the port keeps it as a binary with no left side, so a truth test is one node rather than a second kind
    /// of expression. Reference: the math parser's `!`, whose answer is one when the value is zero and zero otherwise.
    Not,
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
            "!" => Self::Not,
            "<" => Self::Less,
            "<=" => Self::AtMost,
            ">" => Self::Greater,
            ">=" => Self::AtLeast,
            _ => return None,
        })
    }
}

/// Reads whether the actor carries a trait.
///
/// Reference: `f_has_trait` (`condition.cpp:580-586`) - the actor simply has to carry it, with `u_has_trait` asking the speaker and `npc_has_trait` the listener.
fn read_has_trait(object: &cdda_json::Object) -> Option<Condition> {
    for (member, listener) in [("u_has_trait", false), ("npc_has_trait", true)] {
        if let Some(Value::Str(trait_name)) = object.get(member) {
            return Some(Condition::HasTrait {
                trait_name: trait_name.clone(),
                listener,
            });
        }
    }
    None
}

/// Reads whether the player knows a recipe.
///
/// Reference: `f_u_know_recipe` (`condition.cpp:2034-2042`): the reference asks the **player**, not the scoped actor, because recipes were not part of the talker
/// interface, and the port keeps that rule rather than answering for a listener the reference would not have asked.
fn read_knows_recipe(object: &cdda_json::Object) -> Option<Condition> {
    if let Some(Value::Str(recipe)) = object.get("u_know_recipe") {
        return Some(Condition::KnowsRecipe {
            recipe: recipe.clone(),
        });
    }
    None
}

/// Reads a condition, or nothing when the port has no rule for it.
///
/// Reads a condition written as a bare name, which the reference calls an argument-less condition.
///
/// Reference: `condition.cpp:2783-2810` looks a bare name up among the **simple** parsers. The port answers the ones it has a rule for and refuses the
/// rest, so the census stays honest about which names it can read. `u_can_see` takes no value and is therefore bare far more often than not.
#[must_use]
fn read_bare_condition(name: &str) -> Option<Condition> {
    match name {
        "u_can_see" => return Some(Condition::CanSee { listener: false }),
        "npc_can_see" => return Some(Condition::CanSee { listener: true }),
        // Some argument-less guards are written as **bare names** inside an `or`, not as members: `"u_is_outside"` with no value (`haunting.json`).
        "u_is_outside" | "npc_is_outside" => {
            return Some(Condition::IsOutside {
                listener: name == "npc_is_outside",
            });
        }
        _ => {}
    }
    // Reference: `conditional_t( std::string_view )` (`condition.cpp:2783-2810`) looks a bare name up among the **argument-less** parsers, so every one of them is a
    // condition the data may write as a string. `is_day` and `is_night` are the two the port can answer from the calendar alone.
    matches!(
        name,
        "has_beta" | "u_is_avatar" | "u_is_character" | "is_day" | "is_night"
    )
    .then(|| Condition::Simple {
        name: name.to_owned(),
    })
}

/// Reads a condition that asks whether a creature wears an effect, in either scope.
///
/// Reference: `conditional_fun::f_has_effect` (`condition.cpp:980-996`): the effect must be there, and the stated intensity must be no more than the one worn. An
/// intensity of zero means "any". The data spells the scope two ways, `u_has_effect` and `npc_has_effect` (263 uses), and the second asks about the listener.
fn read_has_effect(object: &cdda_json::Object) -> Option<Condition> {
    let (effect, listener) = match (object.get("u_has_effect"), object.get("npc_has_effect")) {
        (Some(Value::Str(effect)), _) => (effect, false),
        (_, Some(Value::Str(effect))) => (effect, true),
        _ => return None,
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
        listener,
    })
}

/// Reads a condition, or nothing when the port has no rule for it.
///
/// A `None` is not "false": it is "the port cannot answer", and a caller that treats the two the same would silently give
/// every unread condition the weakest possible meaning.
#[must_use]
pub fn read_condition(value: &Value) -> Option<Condition> {
    // Reference: a bare name is an argument-less condition (`condition.cpp:2783-2810`).
    if let Value::Str(name) = value {
        return read_bare_condition(name);
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
    if let Some(outside) = read_outside_condition(object) { return Some(outside); }
    if let Some(nearby) = read_near_om_condition(object) {
        return Some(nearby);
    }
    if let Some(flag) = read_flag_condition(object) {
        return Some(flag);
    }
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
    if let Some(known) = read_knows_recipe(object) {
        return Some(known);
    }
    // Reference: `f_has_trait` (`condition.cpp:580-586`) - the actor simply has to carry the trait. Measured against the data,
    // this is the second biggest guard the port could not answer (36 programs).
    if let Some(trait_condition) = read_has_trait(object) {
        return Some(trait_condition);
    }
    read_has_effect(object)
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

/// Reads the two guards that ask about a creature flag, `u_has_flag` and `npc_has_flag`.
///
/// Reference: `f_has_flag` (`condition.cpp:2529`): the member names the flag, and a **missing** flag is an answer, not a refusal - which is what the
/// data leans on when it asks a plain survivor about `GRAB_FILTER`.
/// Reads the two guards that ask whether a named overmap terrain is nearby.
///
/// Reference: `f_near_om_location` (`condition.cpp:1074`): the member names the terrain and a sibling `range` says how far to look, defaulting to **1**.
/// The two camp names (`FACTION_CAMP_ANY`, `FACTION_CAMP_START`) are questions about camps rather than about terrain, so they are refused here.
fn read_near_om_condition(object: &cdda_json::Object) -> Option<Condition> {
    for (member, listener) in [("u_near_om_location", false), ("npc_near_om_location", true)] {
        let Some(value) = object.get(member) else {
            continue;
        };
        let location = read_string_source(value)?;
        if let StringSource::Literal(named) = &location
            && (named == "FACTION_CAMP_ANY" || named == "FACTION_CAMP_START")
        {
            return None;
        }
        let range = match object.get("range") {
            None => 1,
            Some(Value::Int(number)) => i32::try_from(*number).ok()?,
            Some(_) => return None,
        };
        return Some(Condition::NearOmLocation {
            location,
            range,
            listener,
        });
    }
    None
}

/// Reads the two guards that ask whether a creature is under the sky.
///
/// Reference: `f_is_outside` (`condition.cpp:2650`), which takes no argument - the question is about the creature's own square.
fn read_outside_condition(object: &cdda_json::Object) -> Option<Condition> {
    for (member, listener) in [("u_is_outside", false), ("npc_is_outside", true)] {
        if object.get(member).is_some() {
            return Some(Condition::IsOutside { listener });
        }
    }
    None
}

fn read_flag_condition(object: &cdda_json::Object) -> Option<Condition> {
    for (member, listener) in [("u_has_worn_with_flag", false), ("npc_has_worn_with_flag", true)] {
        if let Some(cdda_json::Value::Str(flag)) = object.get(member) {
            return Some(Condition::HasWornWithFlag {
                flag: flag.clone(),
                listener,
            });
        }
    }
    for (member, listener) in [("u_has_flag", false), ("npc_has_flag", true)] {
        if let Some(cdda_json::Value::Str(flag)) = object.get(member) {
            return Some(Condition::HasFlag {
                flag: flag.clone(),
                listener,
            });
        }
    }
    None
}

/// Reads the three members that forget a dialogue variable.
///
/// Reference: `f_remove_var` (`npctalk.cpp:3656`). The scopes are the variable namespaces the port's resolver knows (`u_val`, `npc_val`, `global_val`), not
/// the member prefixes: the member spells whose variable it is, and the resolver is what turns that into an actor.
/// How many of something: one number, or a range to draw from.
///
/// Reference: `get_dbl_or_var` (`npctalk.cpp`), whose array form `[ a, b ]` is a **range** the game draws between, not a list of two choices.
#[derive(Debug, Clone, PartialEq)]
pub enum CountRange {
    /// The same number every time.
    One(i32),
    /// A number drawn between the two ends, both included.
    Between(i32, i32),
    /// A number the data computes, as a `math` expression the game evaluates where it is used.
    ///
    /// Reference: `get_dbl_or_var` (`npctalk.cpp`) takes a number, an array of two ends, **or** a `math` expression - and the spawn data uses the third for a
    /// count that depends on the world, such as `rand(2) + 1` hallucinations.
    Expression(Box<Expression>),
}

/// Reads the two members that spawn monsters.
///
/// Reference: `f_spawn_monster` (`npctalk.cpp:7371-7420`): `min_radius` defaults to **1** and `max_radius` to **10**, both counts default to zero, and the
/// members the port cannot carry out - `group`, `target_var`, `lifespan`, the spawn messages, `mon_variables`, and the outdoor flags - are refused rather
/// than ignored.
fn read_spawn_monster(object: &cdda_json::Object) -> Option<Effect> {
    for (member, _) in [("u_spawn_monster", false), ("npc_spawn_monster", true)] {
        let Some(monster) = object.get(member) else {
            continue;
        };
        for refused in [
            "single_target",
            "target_var",
            "mon_variables",
            "open_air_allowed",
            "friendly",
            "summoner_is_alpha",
            "summoner_is_beta",
        ] {
            if object.get(refused).is_some() {
                return None;
            }
        }
        let monster = read_string_source(monster)?;
        let real_count = read_count(object.get("real_count"))?;
        let hallucination_count = read_count(object.get("hallucination_count"))?;
        let radius = |key: &str, fallback: i32| match object.get(key) {
            None => Some(fallback),
            Some(Value::Int(number)) => i32::try_from(*number).ok(),
            Some(_) => None,
        };
        // Reference: `get_duration_or_var( jo, "lifespan", false, 0_seconds )` - a duration string, and zero means "no summon time".
        // Reference: `get_duration_or_var( jo, "lifespan", false, 0_seconds )`. A string or a number is one duration; an array is the two ends of a range the
        // game draws between (`"lifespan": [ "5 seconds", "10 seconds" ]` in `spells.json`), not a list of choices.
        let lifespan = match object.get("lifespan") {
            None => None,
            Some(value) => Some(read_duration_range(value)?),
        };
        let flag = |key: &str| matches!(object.get(key), Some(Value::Bool(true)));
        let (outdoor_only, indoor_only) = (flag("outdoor_only"), flag("indoor_only"));
        // Reference: `f_spawn_monster` throws when both are set, because no square is both under the sky and under a roof.
        if outdoor_only && indoor_only {
            return None;
        }
        let message = |key: &str| match object.get(key) {
            None => Some(None),
            Some(value) => Some(Some(read_string_source(value)?)),
        };
        // Reference: `group` (`npctalk.cpp:7390`) says the member names a group the game picks a monster from, not a type.
        let group = flag("group");
        // Reference: `get_dbl_or_var( jo, "target_range", false, 0 )` - zero by default, and it only matters when the member names no monster at all.
        let target_range = match object.get("target_range") {
            None => None,
            Some(Value::Int(range)) => Some(i32::try_from(*range).ok()?),
            Some(_) => return None,
        };
        return Some(Effect::SpawnMonster {
            monster,
            group,
            target_range,
            real_count,
            hallucination_count,
            min_radius: radius("min_radius", 1)?,
            max_radius: radius("max_radius", 10)?,
            lifespan,
            outdoor_only,
            indoor_only,
            spawn_message: message("spawn_message")?,
            spawn_message_plural: message("spawn_message_plural")?,
        });
    }
    None
}

/// Reads one duration, or the two ends of a range to draw between, in turns.
fn read_duration_range(value: &Value) -> Option<(i64, i64)> {
    let one = |value: &Value| match value {
        Value::Str(text) => Some(crate::constructions::parse_duration(text)),
        Value::Int(turns) => turns.checked_abs(),
        _ => None,
    };
    match value {
        Value::Array(ends) => {
            let [lower, upper] = ends.as_slice() else {
                return None;
            };
            let (lower, upper) = (one(lower)?, one(upper)?);
            Some((lower.min(upper), lower.max(upper)))
        }
        other => {
            let turns = one(other)?;
            (turns > 0).then_some((turns, turns))
        }
    }
}

/// Reads a count that may be one number, a `[ lower, upper ]` range, or absent.
fn read_count(value: Option<&Value>) -> Option<CountRange> {
    match value {
        None => Some(CountRange::One(0)),
        Some(Value::Int(number)) => i32::try_from(*number).ok().map(CountRange::One),
        Some(Value::Array(ends)) => {
            let [lower, upper] = ends.as_slice() else {
                return None;
            };
            let (Value::Int(lower), Value::Int(upper)) = (lower, upper) else {
                return None;
            };
            let (lower, upper) = (i32::try_from(*lower).ok()?, i32::try_from(*upper).ok()?);
            Some(CountRange::Between(lower.min(upper), lower.max(upper)))
        }
        Some(Value::Object(object)) if object.get("math").is_some() => Some(CountRange::Expression(Box::new(
            read_expression(object.get("math")?)?,
        ))),
        Some(_) => None,
    }
}

/// What `u_add_var` stores: one text, or a list to draw one from.
#[derive(Debug, Clone, PartialEq)]
pub enum VarValue {
    /// The single `value` the data wrote.
    One(StringSource),
    /// The `possible_values` to draw from.
    Many(Vec<StringSource>),
}

/// Reads the effects that act on what a creature carries or is: monsters it spawns, and the dialogue variables it holds.
fn read_carried_effect(object: &cdda_json::Object) -> Option<Effect> {
    read_spawn_monster(object)
        .or_else(|| read_run_npc_eocs(object))
        .or_else(|| read_run_monster_eocs(object))
        .or_else(|| read_learn_recipe(object))
        .or_else(|| read_assign_mission(object))
        .or_else(|| read_teleport(object))
        .or_else(|| read_variable_effect(object))
}

/// Reads `u_run_npc_eocs` and its listener spelling.
///
/// Reference: `f_run_npc_eocs` (`npctalk.cpp:6552-6600`). The programs are names (`load_eoc_vector`), `unique_ids` narrows the people, `npc_range` limits the
/// distance and `local` says whose square that distance counts from. `z_min`, `z_max` and `npc_must_see` are members the port has no rule for yet.
fn read_run_npc_eocs(object: &cdda_json::Object) -> Option<Effect> {
    for (member, local) in [("u_run_npc_eocs", true), ("npc_run_npc_eocs", false)] {
        let Some(value) = object.get(member) else {
            continue;
        };
        for refused in ["z_min", "z_max", "npc_must_see"] {
            if object.get(refused).is_some() {
                return None;
            }
        }
        let Value::Array(entries) = value else {
            return None;
        };
        // Reference: `load_eoc_vector` reads each entry with `load_eoc`, which takes a name **or** a program written in place - and the data uses the second form
        // for the guard programs, so reading only names leaves every one of them refused.
        let programs = entries.iter().map(read_program_ref).collect::<Option<Vec<_>>>()?;
        let unique_ids = match object.get("unique_ids") {
            None => Vec::new(),
            Some(Value::Array(ids)) => ids
                .iter()
                .map(read_string_source)
                .collect::<Option<Vec<_>>>()?,
            Some(_) => return None,
        };
        let npc_range = match object.get("npc_range") {
            None => None,
            Some(Value::Int(range)) => Some(i32::try_from(*range).ok()?),
            Some(_) => return None,
        };
        return Some(Effect::RunNpcEocs {
            programs,
            unique_ids,
            npc_range,
            local,
        });
    }
    None
}

/// Reads the members that set or forget a dialogue variable, which come as a pair in the reference's own table (`npctalk.cpp:8016-8017`).
fn read_variable_effect(object: &cdda_json::Object) -> Option<Effect> {
    read_add_var(object).or_else(|| read_lose_var(object))
}

/// Reads what `u_add_var` stores.
///
/// `time: true` stores the clock, so a value beside it is not read - the reference does not read one either. `possible_values` wins over `value`, and an
/// empty list is a variable with nothing to hold.
fn read_add_var_value(object: &cdda_json::Object, timed: bool) -> Option<VarValue> {
    if timed {
        return Some(VarValue::One(StringSource::Literal(String::new())));
    }
    if let Some(Value::Array(choices)) = object.get("possible_values") {
        let drawn: Vec<StringSource> = choices
            .iter()
            .map(read_string_source)
            .collect::<Option<Vec<_>>>()?;
        return (!drawn.is_empty()).then_some(VarValue::Many(drawn));
    }
    Some(VarValue::One(read_string_source(object.get("value")?)?))
}

/// Reads the three members that set a dialogue variable.
///
/// Reference: `f_add_var` (`npctalk.cpp:3589-3610`): `possible_values` wins over `value` when both are there, `time: true` stores the clock rather than
/// either, and a member with neither a value nor `time` is a variable with nothing to hold.
fn read_add_var(object: &cdda_json::Object) -> Option<Effect> {
    for (member, scope) in [
        ("u_add_var", "u_val"),
        ("npc_add_var", "npc_val"),
        ("global_add_var", "global_val"),
    ] {
        let Some(Value::Str(name)) = object.get(member) else {
            continue;
        };
        let timed = matches!(object.get("time"), Some(Value::Bool(true)));
        let value = read_add_var_value(object, timed)?;
        return Some(Effect::AddVar {
            name: name.clone(),
            scope: scope.to_owned(),
            value,
            timed,
        });
    }
    None
}

fn read_lose_var(object: &cdda_json::Object) -> Option<Effect> {
    for (member, scope) in [
        ("u_lose_var", "u_val"),
        ("npc_lose_var", "npc_val"),
        ("global_lose_var", "global_val"),
    ] {
        if let Some(Value::Str(name)) = object.get(member) {
            return Some(Effect::LoseVar {
                name: name.clone(),
                scope: scope.to_owned(),
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
    /// `:`, which separates a named argument's name from its value.
    Colon,
}

/// Splits an expression's text, or nothing when a character has no place in one.
fn tokenize(text: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let trimmed = rest.trim_start();
        for (spelling, token) in [
            ("!=", Token::Operator("!=".to_owned())),
            ("!", Token::Operator("!".to_owned())),
            ("(", Token::Open),
            (")", Token::Close),
            (",", Token::Comma),
            (":", Token::Colon),
        ] {
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

/// Splits a call's arguments at its top-level commas and reads each as an expression, telling the positional ones from the named ones.
///
/// Reference: `math_parser` takes a call's arguments positionally and then its named ones (`'radius': 10`, `'unit': 'hour'`), and a named argument's name is
/// always a quoted string followed by a colon.
/// A call's arguments: the positional ones first, then the named ones.
type CallArguments = (Vec<Expression>, Vec<(String, Expression)>);

fn read_arguments(tokens: &[Token]) -> Option<CallArguments> {
    if tokens.is_empty() {
        return Some((Vec::new(), Vec::new()));
    }
    let mut arguments = Vec::new();
    let mut keywords = Vec::new();
    let mut depth = 0usize;
    let mut from = 0usize;
    for (at, token) in tokens.iter().enumerate() {
        match token {
            Token::Open => depth += 1,
            Token::Close => depth = depth.checked_sub(1)?,
            Token::Comma if depth == 0 => {
                read_one_argument(tokens.get(from..at)?, &mut arguments, &mut keywords)?;
                from = at + 1;
            }
            _ => {}
        }
    }
    read_one_argument(tokens.get(from..)?, &mut arguments, &mut keywords)?;
    Some((arguments, keywords))
}

/// Reads one argument, which is either a value or a `'name': value` pair.
fn read_one_argument(
    tokens: &[Token],
    arguments: &mut Vec<Expression>,
    keywords: &mut Vec<(String, Expression)>,
) -> Option<()> {
    if let [Token::Str(name), Token::Colon, rest @ ..] = tokens
        && !rest.is_empty()
    {
        // A keyword's value may be a quoted string, which is a text value rather than a variable's name: `'unit': 'hour'`.
        let value = if let [Token::Str(text)] = rest {
            Expression::Text(text.clone())
        } else {
            let (value, left) = parse_precedence(rest, 0)?;
            if !left.is_empty() {
                return None;
            }
            value
        };
        keywords.push((name.clone(), value));
        return Some(());
    }
    // A positional quoted string is a text argument rather than a variable's name: `time_since('midnight', 'unit': 'hour')`.
    if let [Token::Str(text)] = tokens {
        arguments.push(Expression::Text(text.clone()));
        return Some(());
    }
    let (expression, rest) = parse_precedence(tokens, 0)?;
    if !rest.is_empty() {
        return None;
    }
    arguments.push(expression);
    Some(())
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
            // A single quoted literal is a text argument rather than a variable's name: `time( '1 minutes' )`.
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
            // Reference: `math_parser` accepts **any** function name while parsing and fails when it evaluates, which is how the data's own functions
            // (`jmath_function`) are called - they are registered in the reference's table at load, not known to its reader. A name neither table knows
            // therefore parses here and refuses when it is answered, rather than refusing the whole expression at read time.
            let arity = math_function(name).unwrap_or(Arity::AtLeast(0));
            // Reference: `math_parser` reads a call's arguments positionally and then its **named** ones (`'radius': 10`), which any function may take
            // and each decides whether it understands. A name the function does not know is refused when it is evaluated, not dropped here.
            let (arguments, keywords) = read_arguments(argument_tokens)?;
            match arity {
                Arity::Exactly(wanted) if arguments.len() == wanted => {}
                Arity::AtLeast(wanted) if arguments.len() >= wanted => {}
                _ => return None,
            }
            Some((
                Expression::Call {
                    name: name.clone(),
                    arguments,
                    keywords,
                },
                rest,
            ))
        }
        Token::Open => {
            let (inner, rest) = parse_precedence(tokens.get(1..)?, 0)?;
            let after = rest.strip_prefix(&[Token::Close])?;
            Some((inner, after))
        }
        // A `!` before its operand is the reference's logical not, and the data writes it in guards such as `!has_var(flag)`.
        Token::Operator(spelling) if spelling == "!" => {
            let (inner, rest) = parse_primary(tokens.get(1..)?)?;
            Some((
                Expression::Binary {
                    op: Operator::Not,
                    left: Box::new(Expression::Number(0.0)),
                    right: Box::new(inner),
                },
                rest,
            ))
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
        let joined = joined_statement(statements)?;
        return read_text_expression(&joined);
    }
    read_expression(value)
}

/// Reads an expression, or nothing when the port has no rule for its shape.
///
/// Set `CDDA_SHOW_MATH` to print every expression the port refuses, which is how the census finds the shapes it is missing rather than guessing at them.
#[must_use]
pub fn read_expression(value: &Value) -> Option<Expression> {
    let read = read_expression_inner(value);
    if read.is_none() && std::env::var("CDDA_SHOW_MATH").is_ok() {
        eprintln!("refused math: {value:?}");
    }
    read
}

/// The reader itself, so one place can report a refusal.
#[must_use]
fn read_expression_inner(value: &Value) -> Option<Expression> {
    match value {
        Value::Int(number) => {
            #[allow(clippy::cast_precision_loss)]
            let as_float = *number as f64;
            Some(Expression::Number(as_float))
        }
        Value::Float(number) => Some(Expression::Number(*number)),
        Value::Str(text) => read_leaf(text).or_else(|| read_text_expression(text)),
        // A variable written as an object: `{ "global_val": "name" }` where a number is expected. The reference reads this form anywhere an expression is
        // taken (`get_dbl_or_var`), so an expression that is an object with a scope name is that variable, and one holding `math` is that expression.
        Value::Object(object) => {
            if let Some(inner) = object.get("math") {
                return read_expression_inner(inner);
            }
            for scope in ["u_val", "n_val", "npc_val", "global_val", "var_val", "context_val"] {
                if let Some(Value::Str(name)) = object.get(scope) {
                    return Some(Expression::Variable {
                        scope: scope.to_owned(),
                        name: name.clone(),
                    });
                }
            }
            None
        }
        // The data's own shape: `"math"` holds a one-element array wrapping a string expression, and the array form
        // `[left, op, right]` is the fully parenthesised one the port also accepts.
        Value::Array(parts) if parts.len() == 1 => read_expression_inner(parts.first()?),
        // A pair: two numbers where a number is expected, which the reference draws between (`dbl_or_var`'s `pair`).
        Value::Array(parts) if parts.len() == 2 => {
            let [low, high] = parts.as_slice() else {
                return None;
            };
            let Expression::Number(low) = read_expression_inner(low)? else {
                return None;
            };
            let Expression::Number(high) = read_expression_inner(high)? else {
                return None;
            };
            Some(Expression::Pair { low, high })
        }
        Value::Array(parts) if parts.len() == 3 => {
            let [left, spelling, right] = parts.as_slice() else {
                return None;
            };
            let Value::Str(spelling) = spelling else {
                return None;
            };
            Some(Expression::Binary {
                op: Operator::named(spelling)?,
                left: Box::new(read_expression_inner(left)?),
                right: Box::new(read_expression_inner(right)?),
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
#[derive(Debug, Clone, PartialEq)]
pub enum AssignmentTarget {
    /// A bare name, which the dialogue looks up in its own context.
    Bare(String),
    /// A function call whose answer is written back: `u_vitamin('x') = 5`.
    ///
    /// Reference: the math parser's setters (`vitamin_ass`, `hp_ass`, `skill_ass` in `math_parser_diag.cpp`) are registered beside its getters, so a body may
    /// write to what it can read. The port's destinations are **stores**, not variables, so the target carries the call rather than a name.
    Call {
        /// The function's name, as the data spells it.
        name: String,
        /// Its arguments, unread until the write happens.
        arguments: Vec<Expression>,
    },
    /// A variable named the way the expression language names it: `u_val('x')`.
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
            Self::Bare(name) | Self::Variable { name, .. } | Self::Call { name, .. } => name,
        }
    }

    /// The scope the target names, or nothing for a bare name.
    #[must_use]
    pub fn scope(&self) -> Option<&str> {
        match self {
            Self::Bare(_) | Self::Call { .. } => None,
            Self::Variable { scope, .. } => Some(scope.as_str()),
        }
    }
}

/// The effects recognised by the member they carry rather than by a name: casting a spell, spawning an NPC, and playing a sound effect.
fn read_unnamed_effect(object: &cdda_json::Object) -> Option<Effect> {
    read_cast_spell(object)
        .or_else(|| read_spawn_npc(object))
        .or_else(|| read_sound_effect(object))
        .or_else(|| read_spawn_item(object))
        .or_else(|| read_consume_item(object))
        .or_else(|| read_set_string_var(object))
        .or_else(|| read_copy_var(object))
}

/// Reads a `sound_effect` entry.
///
/// Reference: `f_sound_effect` (`npctalk.cpp:5531-5560`): the member is the variant, `id` is the sound, `volume` is a loudness with `-1` meaning eighty, and
/// `outdoor_event` limits it to the sky. The read effect is complete; running it changes no game state, because a sound effect is audio and the port has none.
fn read_sound_effect(object: &cdda_json::Object) -> Option<Effect> {
    let variant = object.get("sound_effect")?;
    let id = object.get("id")?;
    Some(Effect::PlaySoundEffect {
        variant: read_string_source(variant)?,
        id: read_string_source(id)?,
        volume: match object.get("volume") {
            None => CountRange::One(-1),
            Some(value) => read_count(Some(value))?,
        },
        outdoor_event: matches!(object.get("outdoor_event"), Some(Value::Bool(true))),
    })
}

/// Reads a `copy_var` entry.
///
/// Reference: `f_copy_var` (`npctalk.cpp:5797-5807`): the member is the source `var_info` and `target_var` the destination, both objects naming a scope and a name.
fn read_copy_var(object: &cdda_json::Object) -> Option<Effect> {
    let Value::Object(from) = object.get("copy_var")? else {
        return None;
    };
    let Value::Object(to) = object.get("target_var")? else {
        return None;
    };
    let (from_scope, from_name) = named_variable(from)?;
    let (to_scope, to_name) = named_variable(to)?;
    Some(Effect::CopyVar {
        from_scope,
        from_name,
        to_scope,
        to_name,
    })
}

/// Reads a `set_string_var` entry.
///
/// Reference: `f_set_string_var` (`npctalk.cpp:5809-5866`): the member may be a string, an array of strings one of which is drawn, or a variable reference; the
/// `target_var` object names where the text is stored.
fn read_set_string_var(object: &cdda_json::Object) -> Option<Effect> {
    let written = object.get("set_string_var")?;
    if ["i18n", "parse_tags", "string_input"].iter().any(|key| object.get(key).is_some()) {
        return None;
    }
    let Value::Object(target) = object.get("target_var")? else {
        return None;
    };
    let (scope, name) = named_variable(target)?;
    let value = match written {
        Value::Array(choices) => {
            let mut many = Vec::new();
            for choice in choices {
                many.push(read_string_source(choice)?);
            }
            VarValue::Many(many)
        }
        other => VarValue::One(read_string_source(other)?),
    };
    Some(Effect::SetStringVar {
        value,
        scope,
        name,
    })
}

/// Reads a `u_consume_item` / `npc_consume_item` entry.
///
/// Reference: `f_consume_item` (`npctalk.cpp:3935-3941`): `charges` defaults to 0, and `count` to **1**, or to **0** when the entry has an integer `charges` - the
/// same twist `f_spawn_item` has, because a stack's size is its charges.
fn read_consume_item(object: &cdda_json::Object) -> Option<Effect> {
    for (member, listener) in [("u_consume_item", false), ("npc_consume_item", true)] {
        let Some(item) = object.get(member) else {
            continue;
        };
        let has_charges = matches!(object.get("charges"), Some(Value::Int(_)));
        return Some(Effect::ConsumeItem {
            item: read_string_source(item)?,
            count: match object.get("count") {
                None => CountRange::One(i32::from(!has_charges)),
                Some(value) => read_count(Some(value))?,
            },
            charges: match object.get("charges") {
                None => CountRange::One(0),
                Some(value) => read_count(Some(value))?,
            },
            popup: matches!(object.get("popup"), Some(Value::Bool(true))),
            listener,
        });
    }
    None
}

/// Reads a `u_spawn_item` / `map_spawn_item` entry.
///
/// Reference: `f_spawn_item` (`npctalk.cpp:3771-3800`): `count` is `get_dbl_or_var( jo, "count", false, 1 )`, **except** when the entry has an integer `charges`,
/// where the same call passes a default of **0**. Entries that place the item with `loc` or force it into a hand are refused whole.
fn read_spawn_item(object: &cdda_json::Object) -> Option<Effect> {
    for member in ["u_spawn_item", "map_spawn_item"] {
        let Some(item) = object.get(member) else {
            continue;
        };
        for unsupported in ["loc", "force_equip", "use_item_group", "container"] {
            if object.get(unsupported).is_some() {
                return None;
            }
        }
        let has_charges = matches!(object.get("charges"), Some(Value::Int(_)));
        let charges = match object.get("charges") {
            None => CountRange::One(0),
            Some(value) => read_count(Some(value))?,
        };
        return Some(Effect::SpawnItem {
            item: read_string_source(item)?,
            count: match object.get("count") {
                None => CountRange::One(i32::from(!has_charges)),
                Some(value) => read_count(Some(value))?,
            },
            charges,
            suppress_message: matches!(object.get("suppress_message"), Some(Value::Bool(true))),
        });
    }
    None
}

/// Reads a `u_spawn_npc` / `npc_spawn_npc` entry.
///
/// Reference: `f_spawn_npc` (`npctalk.cpp:7575-7620`), whose counts and radii are `dbl_or_var`s - a number, a pair or a `math` expression - which is exactly
/// [`CountRange`]. Members the port has no rule for (`open_air_allowed`, `target_var`) refuse the entry whole rather than being dropped silently.
fn read_spawn_npc(object: &cdda_json::Object) -> Option<Effect> {
    for (member, listener) in [("u_spawn_npc", false), ("npc_spawn_npc", true)] {
        let Some(class) = object.get(member) else {
            continue;
        };
        for unsupported in ["open_air_allowed", "target_var", "outdoor_only", "indoor_only", "lifespan"] {
            if object.get(unsupported).is_some() {
                return None;
            }
        }
        let mut traits = Vec::new();
        match object.get("traits") {
            None => {}
            Some(Value::Array(entries)) => {
                for entry in entries {
                    traits.push(read_string_source(entry)?);
                }
            }
            Some(_) => return None,
        }
        return Some(Effect::SpawnNpc {
            class: read_string_source(class)?,
            unique_id: match object.get("unique_id") {
                None => None,
                Some(value) => Some(read_string_source(value)?),
            },
            traits,
            real_count: read_count(object.get("real_count"))?,
            hallucination_count: read_count(object.get("hallucination_count"))?,
            // Reference: the radii are `get_dbl_or_var( jo, "min_radius", false, 1 )` and `( jo, "max_radius", false, 10 )` - defaults **one** and **ten**.
            min_radius: match object.get("min_radius") {
                None => CountRange::One(1),
                Some(value) => read_count(Some(value))?,
            },
            max_radius: match object.get("max_radius") {
                None => CountRange::One(10),
                Some(value) => read_count(Some(value))?,
            },
            listener,
        });
    }
    None
}

/// Reads a `u_cast_spell` / `npc_cast_spell` entry.
///
/// Reference: `f_cast_spell` (`npctalk.cpp:5414-5440`) reads a `fake_spell`, which the data writes either as a bare id or as `{ "id": ..., "hit_self": true }`.
fn read_cast_spell(object: &cdda_json::Object) -> Option<Effect> {
    let (spell, listener) = match (object.get("u_cast_spell"), object.get("npc_cast_spell")) {
        (Some(value), _) => (value, false),
        (_, Some(value)) => (value, true),
        _ => return None,
    };
    let (spell, hit_self) = match spell {
        Value::Str(id) => (id.clone(), false),
        Value::Object(inside) => {
            let Value::Str(id) = inside.get("id")? else {
                return None;
            };
            let hit_self = matches!(inside.get("hit_self"), Some(Value::Bool(true)));
            (id.clone(), hit_self)
        }
        _ => return None,
    };
    Some(Effect::CastSpell {
        spell,
        hit_self,
        listener,
    })
}

/// Joins a `math` array's pieces into one statement.
///
/// Reference: `eoc_math::from_json` (`condition.cpp:2494-2502`) appends the pieces with nothing between them. The data splits at operator boundaries -
/// `["u_pain()", "-=", "5"]` - and a port that glued them into `u_pain()-=5` could not tell the operator from the name, so the pieces are joined with a space.
fn joined_statement(statements: &[Value]) -> Option<String> {
    let mut joined = String::new();
    for statement in statements {
        let Value::Str(text) = statement else {
            return None;
        };
        if !joined.is_empty() {
            joined.push(' ');
        }
        joined.push_str(text);
    }
    Some(joined)
}

/// Reads a `math` statement, or nothing when it is not an assignment the port can perform.
#[must_use]
pub fn read_assignment(text: &str) -> Option<Assignment> {
    let (name_part, op, rest) = split_assignment(text)?;
    let target = match read_text_expression(name_part.trim())? {
        Expression::Variable { scope, name } => AssignmentTarget::Variable { scope, name },
        // A destination must be a variable: a number, a call or an operator chain is an expression, and a `math` statement
        // whose left-hand side is an expression has no place to put its answer.
        Expression::Call { name, arguments, .. } => AssignmentTarget::Call { name, arguments },
        Expression::Number(_) | Expression::Text(_) | Expression::Pair { .. } | Expression::Binary { .. } => return None,
    };
    Some(Assignment {
        target,
        op,
        expression: read_text_expression(rest)?,
    })
}

/// Splits `name OP expression` at its top-level assignment operator, longest operator first.
fn split_assignment(text: &str) -> Option<(&str, AssignmentOp, &str)> {
    // A postfix step: `"u_val('focus')--"` is that variable minus one, and the data writes it that way when one is all it wants to change.
    if let Some(name_part) = text.strip_suffix("--") {
        return Some((name_part.trim(), AssignmentOp::Subtract, "1"));
    }
    if let Some(name_part) = text.strip_suffix("++") {
        return Some((name_part.trim(), AssignmentOp::Add, "1"));
    }
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

/// What sort of sound an effect makes.
///
/// Reference: `sounds::sound_t` (`sounds.h`), whose names the data writes out: `background`, `speech`, `alarm`, `order`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundKind {
    /// Noise with no words: a door banging, a distant engine.
    Background,
    /// The sky.
    Weather,
    /// Music playing from something.
    Music,
    /// Something moving.
    Movement,
    /// Somebody talking.
    Speech,
    /// Something electronic talking: a robot, a radio.
    ElectronicSpeech,
    /// Work being done.
    Activity,
    /// Work being done that breaks things.
    DestructiveActivity,
    /// Something that tells everyone nearby to look.
    Alarm,
    /// Fighting.
    Combat,
    /// Something loud enough to be noticed.
    Alert,
    /// Something a leader tells its followers.
    Order,
}

/// How long an added effect lasts.
///
/// Reference: `f_add_effect` (`npctalk.cpp:3426-3450`) and `get_duration_or_var`, which accept a duration string, a number of turns, an array
/// of two ends, or `PERMANENT`.
#[derive(Debug, Clone, PartialEq)]
pub enum EffectDuration {
    /// It lasts until something removes it.
    Permanent,
    /// A fixed number of turns, or the reference's default when the data states none.
    Turns(Option<i64>),
    /// A roll between two ends, inclusive.
    Range(i64, i64),
    /// A length the dialogue works out when the effect is applied - `{"math": ["rand(345600) + 86400"]}` in the data.
    Computed(crate::dialogue::Expression),
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
    /// Teach the actor a recipe, or take one away.
    ///
    /// Reference: `f_learn_recipe` (`npctalk.cpp:5250-5258`) takes one recipe id and hands it to the actor's `learn_recipe`; `f_forget_recipe` (`:5260`) is the
    /// same member with the opposite meaning. Both are read here because the difference is one boolean, not one rule.
    LearnRecipe {
        /// The recipe to teach or to take away.
        recipe: StringSource,
        /// Whether the effect takes the recipe away rather than teaching it.
        forget: bool,
    },
    /// Plays a sound effect: a named sound and one of its variants.
    ///
    /// Reference: `f_sound_effect` (`npctalk.cpp:5531-5560`): the member is the **variant**, `id` names the sound, and `volume` is a loudness defaulting to
    /// `-1`, which the reference reads as **80**. Only the player hears it, and no game state changes - the port reads the entry and plays nothing, because it has
    /// no audio, and says so rather than pretending.
    PlaySoundEffect {
        /// The variant of the named sound.
        variant: StringSource,
        /// The sound's own id.
        id: StringSource,
        /// How loud, with `-1` meaning the reference's own eighty.
        volume: CountRange,
        /// Whether it is only heard outdoors.
        outdoor_event: bool,
    },
    /// Copies one variable's value into another.
    ///
    /// Reference: `f_copy_var` (`npctalk.cpp:5797-5807`): both ends are `var_info`s, the value is read from the first and written to the second, and nothing is
    /// drawn or interpreted on the way. A variable the source does not hold copies its **absence**, which is why the runner asks the store rather than defaulting.
    CopyVar {
        /// The variable read.
        from_scope: String,
        /// Its name.
        from_name: String,
        /// The variable written.
        to_scope: String,
        /// Its name.
        to_name: String,
    },
    /// Stores a text into a named variable, drawing it from a list when the data gives one.
    ///
    /// Reference: `f_set_string_var` (`npctalk.cpp:5809-5866`): the member is a string, a **list to draw one from**, or a variable to copy, and `target_var` says
    /// where it goes. `i18n` (translated strings), `parse_tags` (substituting tags into the text) and `string_input` (asking the player to type) are refused, because
    /// each is a rule the port has not written.
    SetStringVar {
        /// The text, or the list to draw one from.
        value: VarValue,
        /// The variable's type, as the data spells it.
        scope: String,
        /// The variable's name.
        name: String,
    },
    /// Takes items out of what a creature carries.
    ///
    /// Reference: `f_consume_item` (`npctalk.cpp:3931-3985`): `charges` defaults to **0** and `count` to **1** - to **0** when the entry gives `charges` - and the
    /// effect consumes charges when only charges were asked for, whole stacks when a count was, and says something when the creature has neither.
    ConsumeItem {
        /// The item's id.
        item: StringSource,
        /// How many whole stacks.
        count: CountRange,
        /// How many charges out of them.
        charges: CountRange,
        /// Whether a message is shown about it.
        popup: bool,
        /// Whether it is the listener who does the consuming.
        listener: bool,
    },
    /// Puts items on the actor's square, or near it.
    ///
    /// Reference: `f_spawn_item` (`npctalk.cpp:3771-3800`): the member names the item, `count` says how many and defaults to **1** - but to **0** when the entry
    /// also gives `charges`, because a stack's size is then its charges - `container` wraps it, and `loc` says where. The port reads the item, the count and the
    /// charges, and refuses the entries that place it somewhere else, because a location variable is a second store the port reads only for `u_location_variable`.
    SpawnItem {
        /// The item's id.
        item: StringSource,
        /// How many.
        count: CountRange,
        /// How many charges each one holds, when the data gives charges instead of a count.
        charges: CountRange,
        /// Whether anything is shown to say it appeared.
        suppress_message: bool,
    },
    /// Brings NPCs of a class into being around the actor.
    ///
    /// Reference: `f_spawn_npc` (`npctalk.cpp:7575-7620`): the member names an NPC **class**, `real_count` and `hallucination_count` say how many, the radii say
    /// where, and each one is given the `traits` the data lists. The counts default to **zero** and the radii to **one and ten**.
    SpawnNpc {
        /// The NPC class's id.
        class: StringSource,
        /// The id the spawn is remembered by, when the data gives one.
        unique_id: Option<StringSource>,
        /// Traits each one is given.
        traits: Vec<StringSource>,
        /// How many are really there.
        real_count: CountRange,
        /// How many are only imagined.
        hallucination_count: CountRange,
        /// The nearest square one may appear on.
        min_radius: CountRange,
        /// The furthest square one may appear on.
        max_radius: CountRange,
        /// Whether it is the listener who does the spawning.
        listener: bool,
    },
    /// Casts a spell at a target, or at the caster.
    ///
    /// Reference: `f_cast_spell` (`npctalk.cpp:5414-5440`), which reads a `fake_spell` - an `id` and a `hit_self` flag - and casts it through the dialogue's actor.
    /// The port **reads** the effect and refuses to run it: casting needs the spell's own effect table, and answering "done" for a spell nobody cast would be a
    /// silent lie the census would count as success.
    CastSpell {
        /// The spell's id.
        spell: String,
        /// Whether it lands on the caster rather than on a target.
        hit_self: bool,
        /// Whether it is the listener who casts.
        listener: bool,
    },
    /// An entry that does nothing, because every member it had was a comment.
    ///
    /// Reference: a member named `//`, or any name starting with `//`, is a **comment** and no loader reads it (`json.h:1500`, `json.cpp:153`). An effect
    /// object holding nothing else therefore loads as a function that does nothing, which is what the data writes when an entry is disabled by hand.
    Nothing,
    /// Run a program on every nearby monster.
    ///
    /// Reference: `f_run_monster_eocs` (`npctalk.cpp:6590-6700`): a list of programs, the `mtype_ids` that narrow which monsters, `monster_range` for how far
    /// from the actor, and the z filters and `monster_must_see`. The port reads the programs and the two narrowing members; the z filters and the sight test are
    /// refused rather than ignored.
    RunMonsterEocs {
        /// The programs to run, one after another, on each monster.
        programs: Vec<ProgramRef>,
        /// The monster types to run on, all of them when empty.
        mtype_ids: Vec<StringSource>,
        /// How far from the actor a monster may be, or nothing for every monster on the actor's level.
        monster_range: Option<i32>,
    },
    /// Run a program on every nearby person.
    ///
    /// Reference: `f_run_npc_eocs` (`npctalk.cpp:6552-6600`): a list of programs, the `unique_ids` that narrow which people, `local` for whose square the range
    /// counts from, and `npc_range` for how far. The z filters and `npc_must_see` are refused rather than ignored.
    RunNpcEocs {
        /// The programs to run, one after another, on each person: names, or programs written where they are used.
        programs: Vec<ProgramRef>,
        /// The ids that pick out the people, all of them when empty.
        unique_ids: Vec<StringSource>,
        /// How far from the actor a person may be, or nothing for everyone on the actor's level.
        npc_range: Option<i32>,
        /// Whether the range counts from the speaker rather than from the world.
        local: bool,
    },
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
    /// Offer a menu of programs and run the one the player picks.
    ///
    /// Reference: `f_run_eoc_selector` (`npctalk.cpp:6385`): the member is a list of programs, `names` and `descriptions` label them, and
    /// `allow_cancel` says whether walking away is an option. The port asks a **queued choice** rather than drawing a menu, because the port is
    /// headless: a scripted playtest or the UI both answer the same queue.
    RunEocSelector {
        /// The programs on the menu.
        options: Vec<ProgramRef>,
        /// What each one is called, in the same order.
        names: Vec<StringSource>,
        /// Whether the player may walk away without choosing.
        allow_cancel: bool,
    },
    /// Store the nearest overmap tile of a terrain in a dialogue variable.
    ///
    /// Reference: `f_location_variable` (`npctalk.cpp:4194`) with a `target_params` naming an overmap terrain: the search runs over the overmap and the
    /// variable holds the square it found.
    LocationVariableSearch {
        /// The variable's type, as the data spells it.
        scope: String,
        /// The variable's name.
        name: String,
        /// The overmap terrain to look for.
        om_terrain: String,
        /// How far out to search, in overmap tiles.
        search_range: i32,
        /// How near a tile may be and still count; closer ones are skipped.
        min_distance: i32,
        /// The furniture to look for inside the tile, when the data names one.
        ///
        /// Reference: `f_location_variable` (`npctalk.cpp:4200-4248`) accepts **one** of terrain, furniture, field, monster, species, trap, zone or npc as its
        /// search target, and a second one is an error. The port reads the furniture case and refuses to run it, because its squares carry a furniture
        /// **code** rather than the id this names - a store gap, not a reader gap.
        furniture: Option<String>,
        /// How near the drawn square may be to the tile found; zero when the data draws none.
        min_radius: i32,
        /// How far from the tile found the square may be drawn; zero when the data draws none.
        max_radius: i32,
        /// The map level to search on, when the data names one.
        z: Option<i32>,
        /// Whether the square goes to the listener's variables rather than the speaker's.
        listener: bool,
    },
    /// Set a dialogue variable.
    ///
    /// Reference: `f_add_var` (`npctalk.cpp:3589`): the member names the variable, `value` gives it a text, `possible_values` gives it a choice of
    /// texts and one is drawn, and `time: true` gives it the current turn instead of any value the data wrote.
    AddVar {
        /// The variable's name.
        name: String,
        /// Which actor's variables it belongs to, in the namespaces the resolver knows.
        scope: String,
        /// What to store: one text, or a list to draw from.
        value: VarValue,
        /// Whether the variable gets the clock instead of a value from the data.
        timed: bool,
    },
    /// Spawn monsters around the actor.
    ///
    /// Reference: `f_spawn_monster` (`npctalk.cpp:7371-7420`) for `u_spawn_monster` and `npc_spawn_monster`: a type, how many are real, how many are
    /// hallucinations, and the ring around the actor they appear in.
    SpawnMonster {
        /// The monster type to make - or, with `group`, the group to draw one from.
        monster: StringSource,
        /// Whether `monster` names a **monster group** rather than a type.
        group: bool,
        /// How far to look for a creature to copy, when `monster` names none.
        target_range: Option<i32>,
        /// How many real ones, which the data may give as a single number or as a range to draw from.
        real_count: CountRange,
        /// How many hallucinations, drawn the same way.
        hallucination_count: CountRange,
        /// How near the actor they may appear.
        min_radius: i32,
        /// How far from the actor they may appear.
        max_radius: i32,
        /// How long a summon lasts, in turns: one number, or the two ends of a range to draw between. Absent or zero means it stays.
        lifespan: Option<(i64, i64)>,
        /// Whether the spawn must be under the sky, or must not be.
        outdoor_only: bool,
        /// Whether the spawn must be under a roof, or must not be.
        indoor_only: bool,
        /// The line the player reads when they see it happen.
        spawn_message: Option<StringSource>,
        /// The line for when more than one appears.
        spawn_message_plural: Option<StringSource>,
    },
    /// Store a square found by looking around the actor.
    ///
    /// Reference: `f_location_variable` (`npctalk.cpp`) draws up to 25 squares within `max_radius` and takes the first at least `min_radius` away.
    LocationVariableRadius {
        /// The variable's type, as the data spells it.
        scope: String,
        /// The variable's name.
        name: String,
        /// How near a square may be and still count.
        min_radius: i32,
        /// How far out to look.
        max_radius: i32,
        /// Whether the square goes to the listener's variables rather than the speaker's.
        listener: bool,
    },
    /// Forget a dialogue variable.
    ///
    /// Reference: `f_remove_var` (`npctalk.cpp:3656`) for `u_lose_var`, `npc_lose_var` and `global_lose_var`: the member names the variable to drop.
    LoseVar {
        /// The variable's name.
        name: String,
        /// Which actor's variables it belongs to, in the namespaces the resolver knows: `u_val`, `npc_val` or `global_val`.
        scope: String,
    },
    /// Take several effects away from a creature, which the data writes as an array.
    RemoveEffects {
        /// The effects' ids.
        effects: Vec<String>,
        /// Whether they come from the listener rather than the speaker.
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
    /// Make a sound, which may be a snippet the port picks from a category.
    ///
    /// Reference: `f_make_sound` (`npctalk.cpp:6131-6200`): the volume is a `dbl_or_var` and required, the type names what sort of sound it is
    /// (background, speech, alarm, order), and the message may be a snippet or a `same_snippet` repeat.
    MakeSound {
        /// The words, when the sound is speech.
        words: Option<StringSource>,
        /// The snippet category, when the sound is a snippet.
        snippet: Option<StringSource>,
        /// Whether a snippet pick is remembered.
        same_snippet: bool,
        /// How loud, in the reference's units - a radius as much as a loudness.
        volume: Expression,
        /// What sort of sound it is.
        kind: SoundKind,
        /// Whether it is the listener that makes it.
        listener: bool,
    },
    /// Apply a mapgen update where the speaker stands.
    ///
    /// Reference: `f_mapgen_update` (`npctalk.cpp`): the member names an **update mapgen**, and it is applied at a target overmap tile - the
    /// speaker's own when the data names none. The port reads the no-target form and refuses `om_terrain`, `target_var`, `target_params` and
    /// `mapgen_update_variables`, which aim it somewhere else.
    MapgenUpdate {
        /// The update mapgen's id.
        update: StringSource,
        /// The square to apply it at, when the data names one; the speaker's own square otherwise.
        target: Option<AssignmentTarget>,
        /// The overmap terrain to apply it at, found by searching; the speaker's own square when neither this nor `target` is named.
        om_terrain: Option<StringSource>,
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
    /// Give the player a mission.
    ///
    /// Reference: `f_assign_mission` (`npctalk.cpp:5908-5926`): the member names a mission type, which is reserved and assigned to the player, and an optional
    /// `deadline` sets the turn it is due **after** assignment. The port takes the type; the deadline is refused because its store keeps no due turn yet.
    AssignMission {
        /// The mission type the data names.
        id: StringSource,
    },
    /// Move a creature to the square a dialogue variable holds.
    ///
    /// Reference: `f_teleport` (`npctalk.cpp:7816-7845`): the member names the variable, `force` allows a destination the game would refuse, and
    /// `fail_message`/`success_message` are lines for either outcome. The port reads the move; the two messages and `force_safe` are refused, and `force` is
    /// accepted because with nothing yet deciding a destination is unsafe there is nothing for it to override.
    Teleport {
        /// The variable's scope, as the data spells it: `u_val`, `npc_val`, `global_val`.
        scope: String,
        /// The variable's name.
        name: String,
        /// Whether the creature moving is the listener rather than the speaker.
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
            Self::RunNpcEocs { .. } => "u_run_npc_eocs",
            Self::RunMonsterEocs { .. } => "u_run_monster_eocs",
            Self::Nothing => "//",
            Self::PlaySoundEffect { .. } => "sound_effect",
            Self::SetStringVar { .. } => "set_string_var",
            Self::CopyVar { .. } => "copy_var",
            Self::SpawnItem { .. } => "u_spawn_item",
            Self::ConsumeItem { listener, .. } => {
                if *listener {
                    "npc_consume_item"
                } else {
                    "u_consume_item"
                }
            }
            Self::SpawnNpc { listener, .. } => {
                if *listener {
                    "npc_spawn_npc"
                } else {
                    "u_spawn_npc"
                }
            }
            Self::CastSpell { listener, .. } => {
                if *listener {
                    "npc_cast_spell"
                } else {
                    "u_cast_spell"
                }
            }
            Self::LearnRecipe { forget, .. } => {
                if *forget {
                    "u_forget_recipe"
                } else {
                    "u_learn_recipe"
                }
            }
            Self::If { .. } => "if",
            Self::AddEffect { .. } => "u_add_effect",
            // Both are the same member: the data writes one effect or an array of them under one name.
            Self::RemoveEffect { .. } | Self::RemoveEffects { .. } => "u_lose_effect",
            Self::LearnTrait { .. } => "u_add_trait",
            Self::LoseTrait { .. } => "u_lose_trait",
            Self::RunEoc(_) => "run_eocs",
            Self::Switch { .. } => "switch",
            Self::AssignMission { .. } => "assign_mission",
            Self::Teleport { .. } => "u_teleport",
            Self::LocationVariable { .. }
            | Self::LocationVariableSearch { .. }
            | Self::LocationVariableRadius { .. } => "u_location_variable",
            Self::WeightedList(_) => "weighted_list_eocs",
            Self::OpenDialogue { .. } => "open_dialogue",
            Self::AddMorale { .. } => "u_add_morale",
            Self::MakeSound { .. } => "u_make_sound",
            Self::MapgenUpdate { .. } => "mapgen_update",
            Self::RunEocSelector { .. } => "run_eoc_selector",
            Self::LoseVar { .. } => "u_lose_var",
            Self::AddVar { .. } => "u_add_var",
            Self::SpawnMonster { .. } => "u_spawn_monster",
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
/// Reads the three spellings of the message effect.
///
/// Reference: `f_message` is registered **three** times (`npctalk.cpp:8039` and `:8135`) - `u_message`, `npc_message`, and the bare `message`, which is the global
/// form and says the line to the player whoever the talker is. Reading only the two creature forms leaves the global one refused, and the data uses it for the
/// lines a program says rather than a talker.
fn read_message_effect(object: &cdda_json::Object) -> Option<Effect> {
    for member in ["u_message", "npc_message", "message"] {
        if let Some(value) = object.get(member) {
            return read_message(value).map(Effect::Message);
        }
    }
    None
}

/// Whether every member of an object is a comment.
///
/// Reference: `json.h:1500` - a member named `//` is a comment, and so is any name starting with it (`json.cpp:153`). An effect entry holding nothing else is a
/// no-op in the reference, because its loader finds no function to run and no rule is broken.
fn is_comment_only(value: &Value) -> bool {
    let Value::Object(object) = value else {
        return false;
    };
    !object.members().is_empty()
        && object
            .members()
            .iter()
            .all(|(name, _)| name.starts_with("//"))
}

/// Reads one effect entry, or nothing when it states a rule the port does not have.
///
/// Reference: `talk_effect_t::load_effect` (`npctalk.cpp:8348`) reads an entry as a name, an object with one effect member, or a list; the port accepts the first
/// two and refuses whatever it cannot carry out, so a refused entry is visible as a refusal rather than as a silent no-op.
pub fn read_effect(value: &Value) -> Option<Effect> {
    if is_comment_only(value) {
        return Some(Effect::Nothing);
    }
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
    if let Some(update) = read_mapgen_update(object) {
        return Some(update);
    }
    if let Some(sound) = read_make_sound(object) {
        return Some(sound);
    }
    if let Some(morale) = read_add_morale(object) {
        return Some(morale);
    }
    if let Some(dialogue) = read_open_dialogue(object) {
        return Some(dialogue);
    }
    if let Some(selector) = read_eoc_selector(object) {         return Some(selector); }
    if let Some(weighted) = read_weighted_list(object) {
        return Some(weighted);
    }
    if let Some(location) = read_location_variable(object) {
        return Some(location);
    }
    if let Some(found) = read_unnamed_effect(object) { return Some(found); }
    if let Some(switched) = read_switch(object) {
        return Some(switched);
    }
    if let Some(Value::Array(statements)) = object.get("math") {
        // Reference: `eoc_math::from_json` (`condition.cpp:2494-2502`) **concatenates** the array's strings and parses the result as
        // one statement - `combined.append( objects.get_string( i ) )` with nothing between them, then one `defer_math`. So
        // `["x", " *= ", "3"]` and `["x *= 3"]` are the same effect, and a port that read one statement per element would refuse
        // every split one. The array is a token list, not a program.
        // Joined with a space between the pieces: the reference appends them with nothing between (`condition.cpp:2498`), but the data splits at operator
        // boundaries - `["u_pain()", "-=", "5"]` - and a port that glued them into `u_pain()-=5` could not tell the operator from the name. The space is the
        // only difference, and it is the difference between reading the statement and refusing it.
        let mut joined = String::new();
        for statement in statements {
            let Value::Str(text) = statement else {
                return None;
            };
            if !joined.is_empty() {
                joined.push(' ');
            }
            joined.push_str(text);
        }
        return read_assignment(&joined).map(|one| Effect::Math(vec![one]));
    }
    if matches!(object.get("snippet"), Some(Value::Bool(true))) {
        return read_snippet(object);
    }
    if let Some(changed) = read_effect_change(object) { return Some(changed); }
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
    if let Some(carried) = read_carried_effect(object) { return Some(carried); }
    read_message_effect(object)
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







/// Reads `assign_mission`.
///
/// Reference: `f_assign_mission` (`npctalk.cpp:5908-5926`). `deadline` is a member the port has no rule for - its mission store keeps no due turn - so an entry
/// that states one is refused rather than assigned without it.
fn read_assign_mission(object: &cdda_json::Object) -> Option<Effect> {
    let value = object.get("assign_mission")?;
    if object.get("deadline").is_some() {
        return None;
    }
    Some(Effect::AssignMission {
        id: read_string_source(value)?,
    })
}

/// Reads `u_learn_recipe` and `u_forget_recipe`, which differ only in the member name.
///
/// Reference: `f_learn_recipe` / `f_forget_recipe` (`npctalk.cpp:5250-5266`) both take one recipe id and hand it to the actor.
fn read_learn_recipe(object: &cdda_json::Object) -> Option<Effect> {
    for (member, forget) in [("u_learn_recipe", false), ("npc_learn_recipe", false), ("u_forget_recipe", true), ("npc_forget_recipe", true)] {
        if let Some(value) = object.get(member) {
            return read_string_source(value).map(|recipe| Effect::LearnRecipe { recipe, forget });
        }
    }
    None
}

/// Reads `u_run_monster_eocs` and its listener spelling.
///
/// Reference: `f_run_monster_eocs` (`npctalk.cpp:6590-6700`). `mtype_ids` narrows by monster type and `monster_range` by distance; `z_min`, `z_max` and
/// `monster_must_see` are members the port has no rule for yet, so an entry stating one is refused rather than half-run.
fn read_run_monster_eocs(object: &cdda_json::Object) -> Option<Effect> {
    for member in ["u_run_monster_eocs", "npc_run_monster_eocs"] {
        let Some(value) = object.get(member) else {
            continue;
        };
        for refused in ["z_min", "z_max", "monster_must_see"] {
            if object.get(refused).is_some() {
                return None;
            }
        }
        let entries = match value {
            Value::Array(entries) => entries.as_slice(),
            other => std::slice::from_ref(other),
        };
        let programs = entries.iter().map(read_program_ref).collect::<Option<Vec<_>>>()?;
        let mtype_ids = match object.get("mtype_ids") {
            None => Vec::new(),
            Some(Value::Array(ids)) => ids
                .iter()
                .map(read_string_source)
                .collect::<Option<Vec<_>>>()?,
            Some(_) => return None,
        };
        let monster_range = match object.get("monster_range") {
            None => None,
            Some(Value::Int(range)) => Some(i32::try_from(*range).ok()?),
            Some(_) => return None,
        };
        return Some(Effect::RunMonsterEocs {
            programs,
            mtype_ids,
            monster_range,
        });
    }
    None
}

/// Reads `u_teleport` and its listener spelling.
///
/// Reference: `f_teleport` (`npctalk.cpp:7816-7845`), whose member is an object naming the variable to move to - `{ "u_val": "place" }`. The two outcome
/// messages and `force_safe` are members the port has no rule for yet, so an entry stating one is refused rather than half-run.
fn read_teleport(object: &cdda_json::Object) -> Option<Effect> {
    for (member, listener) in [("u_teleport", false), ("npc_teleport", true)] {
        let Some(Value::Object(target)) = object.get(member) else {
            continue;
        };
        for refused in ["fail_message", "success_message", "force_safe"] {
            if object.get(refused).is_some() {
                return None;
            }
        }
        let (scope, name) = target.members().iter().find_map(|(scope, value)| match value {
            Value::Str(name) => Some((scope.clone(), name.clone())),
            _ => None,
        })?;
        return Some(Effect::Teleport {
            scope,
            name,
            listener,
        });
    }
    None
}

/// Reads a message whose member names a **snippet category** rather than the text itself.
///
/// Reference: `f_message` (`npctalk.cpp:5312-5405`): with `snippet: true` the member is a category of text, the line is picked from it by weight,
/// and `same_snippet` remembers the pick so the same line repeats.
fn read_snippet(object: &cdda_json::Object) -> Option<Effect> {
    if let Some(carried) = read_carried_effect(object) { return Some(carried); }
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



/// Reads a mapgen update that applies where the speaker stands.
///
/// Reference: `f_mapgen_update` (`npctalk.cpp`): a `target_var`, an `om_terrain`, `target_params` or `mapgen_update_variables` aim the update
/// somewhere else, and the port refuses those rather than applying the update in the wrong place.
fn read_mapgen_update(object: &cdda_json::Object) -> Option<Effect> {
    let update = object.get("mapgen_update")?;
    // A `target_var` names a square the dialogue already holds and `om_terrain` names a tile to search for; `target_params` is a whole mission
    // target - search range, filters, reveal - which the port has not built.
    for aiming in ["target_params", "mapgen_update_variables"] {
        if object.get(aiming).is_some() {
            return None;
        }
    }
    let target = match object.get("target_var") {
        Some(value) => Some(read_variable_target(value)?),
        None => None,
    };
    let om_terrain = match object.get("om_terrain") {
        Some(value) => Some(read_string_source(value)?),
        None => None,
    };
    Some(Effect::MapgenUpdate {
        update: read_string_source(update)?,
        target,
        om_terrain,
    })
}


/// Reads a `target_var`, which names a variable holding a square.
///
/// Reference: `read_var_value` (`npctalk.cpp`): the shape is `{"global_val": "here"}`, `{"u_val": …}` or `{"npc_val": …}`, which the port reads
/// the same way `math` names its destination.
fn read_variable_target(value: &Value) -> Option<AssignmentTarget> {
    let Value::Object(object) = value else {
        return None;
    };
    let (scope, target) = object.members().first()?;
    let Value::Str(name) = target else {
        return None;
    };
    match scope.as_str() {
        "u_val" | "npc_val" | "global_val" | "n_val" => Some(AssignmentTarget::Variable {
            scope: scope.clone(),
            name: name.clone(),
        }),
        _ => None,
    }
}

/// Reads a sound: a message or a snippet, a volume, and a kind.
///
/// Reference: `f_make_sound` (`npctalk.cpp:6131-6200`): `volume` is required, `type` defaults to `background`, and the member itself is either
/// the words or - with `snippet: true` - the category they are picked from.
fn read_make_sound(object: &cdda_json::Object) -> Option<Effect> {
    for (member, listener) in [("u_make_sound", false), ("npc_make_sound", true)] {
        let Some(words) = object.get(member) else {
            continue;
        };
        let snippet = matches!(object.get("snippet"), Some(Value::Bool(true)));
        // Reference: `f_make_sound` (`npctalk.cpp:6145-6171`) reads the sound's **kind** from `type`, and the list is longer than the four the port knew:
        // `background`, `weather`, `music`, `movement`, `speech`, `electronic_speech`, `activity`, `destructive_activity`, `alarm`, `combat`, `alert`, `order`.
        // A name outside that list is an error in the reference, and refusing it here is the same answer. The port's noise has no per-kind rule yet, so the
        // kind is **carried** and not yet used - which is a gap in the noise model, not in the reader.
        let kind = match object.get("type") {
            Some(Value::Str(name)) => match name.as_str() {
                "background" => SoundKind::Background,
                "weather" => SoundKind::Weather,
                "music" => SoundKind::Music,
                "movement" => SoundKind::Movement,
                "speech" => SoundKind::Speech,
                "electronic_speech" => SoundKind::ElectronicSpeech,
                "activity" => SoundKind::Activity,
                "destructive_activity" => SoundKind::DestructiveActivity,
                "alarm" => SoundKind::Alarm,
                "combat" => SoundKind::Combat,
                "alert" => SoundKind::Alert,
                "order" => SoundKind::Order,
                _ => return None,
            },
            Some(_) => return None,
            None => SoundKind::Background,
        };
        return Some(Effect::MakeSound {
            words: if snippet { None } else { Some(read_string_source(words)?) },
            snippet: if snippet { Some(read_string_source(words)?) } else { None },
            same_snippet: matches!(object.get("same_snippet"), Some(Value::Bool(true))),
            volume: read_math_expression(object.get("volume")?)?,
            kind,
            listener,
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
/// Reads a menu of programs: the reference's `run_eoc_selector`.
///
/// Reference: `f_run_eoc_selector` (`npctalk.cpp:6385-6440`): `names`, `descriptions` and `keys` must each be as long as the program list or absent, and
/// the port refuses a mismatched menu rather than showing labels for the wrong entries.
fn read_eoc_selector(object: &cdda_json::Object) -> Option<Effect> {
    let options = read_program_refs(object.get("run_eoc_selector"))?;
    if options.is_empty() {
        return None;
    }
    let names = match object.get("names") {
        None => Vec::new(),
        Some(Value::Array(entries)) => {
            if entries.len() != options.len() {
                return None;
            }
            entries
                .iter()
                .map(read_string_source)
                .collect::<Option<Vec<_>>>()?
        }
        Some(_) => return None,
    };
    // `descriptions` and `keys` are labels the headless port does not show; their presence is checked for length and otherwise ignored.
    for labelled in ["descriptions", "keys"] {
        if let Some(Value::Array(entries)) = object.get(labelled)
            && entries.len() != options.len()
        {
            return None;
        }
    }
    Some(Effect::RunEocSelector {
        options,
        names,
        allow_cancel: matches!(object.get("allow_cancel"), Some(Value::Bool(true))),
    })
}

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
/// Reads the search that names an **overmap terrain and nothing else** - the same walk `om_terrain` aiming uses - or nothing.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4194`) with a `target_params` (`mission_util.cpp:425-450`): a range, a minimum distance and a level.
fn read_terrain_target(
    object: &cdda_json::Object,
    target: &cdda_json::Object,
    listener: bool,
) -> Option<Effect> {
    let Value::Object(params) = object.get("target_params")? else {
        return None;
    };

    let Value::Str(wanted) = params.get("om_terrain")? else {
        return None;
    };
    if !params
        .members()
        .iter()
        .all(|(key, _)| matches!(key.as_str(), "om_terrain" | "z" | "min_distance" | "search_range"))
    {
        return None;
    }
    // A fractional range or distance is not a whole number of tiles, so the port refuses rather than rounding one: the reference evaluates these as
    // doubles and rounds where it uses them, and a silent round here would be a guess about which way.
    let whole = |key: &str, fallback: i32| match params.get(key) {
        None => Some(fallback),
        Some(Value::Int(number)) => i32::try_from(*number).ok(),
        Some(_) => None,
    };
    let search_range = whole("search_range", DEFAULT_SEARCH_RANGE)?;
    let min_distance = whole("min_distance", 0)?;
    let z = match params.get("z") {
        None => None,
        Some(Value::Int(number)) => Some(i32::try_from(*number).ok()?),
        Some(_) => return None,
    };
    // A radius beside a target search is a **second** step, not a different search: the reference finds the tile and then draws within the radius of what
    // it found (`npctalk.cpp`, the `max_radius > 0` block runs after the target search). Both are read here, and the runner does them in that order.
    let whole_radius = |key: &str| match object.get(key) {
        None => Some(0),
        Some(Value::Int(number)) => i32::try_from(*number).ok(),
        Some(_) => None,
    };
    let min_radius = whole_radius("min_radius")?;
    let max_radius = whole_radius("max_radius")?;
    // One search target only, and the furniture case is the one the data uses beside an overmap tile: `f_chair` inside `horse_farm_isherwood_13`.
    for asked in ["terrain", "field", "monster", "npc", "species", "trap", "zone"] {
        if object.get(asked).is_some() {
            return None;
        }
    }
    let furniture = match object.get("furniture") {
        None => None,
        Some(Value::Str(id)) => Some(id.clone()),
        Some(_) => return None,
    };
    let (scope, name) = named_variable(target)?;
    Some(Effect::LocationVariableSearch {
        furniture,
        scope,
        name,
        om_terrain: wanted.clone(),
        search_range,
        min_distance,
        min_radius,
        max_radius,
        z,
        listener,
    })
}

/// Reads the search that looks around the actor within a radius, or nothing.
///
/// Reference: `f_location_variable` (`npctalk.cpp`), the `max_radius > 0` block: up to 25 draws inside the radius, taking the first far enough out.
fn read_radius_search(
    object: &cdda_json::Object,
    target: &cdda_json::Object,
    listener: bool,
) -> Option<Effect> {
    let radii = object.get("min_radius").or_else(|| object.get("max_radius"))?;
    let _ = radii;
    // Nothing else may be asked: a terrain, a monster or a mission target is a different search with its own rules.
    for asked in [
        "target_params",
        "terrain",
        "furniture",
        "field",
        "monster",
        "npc",
        "species",
        "trap",
        "zone",
    ] {
        if object.get(asked).is_some() {
            return None;
        }
    }
    let whole = |key: &str| match object.get(key) {
        None => Some(0),
        Some(Value::Int(number)) => i32::try_from(*number).ok(),
        Some(_) => None,
    };
    let min_radius = whole("min_radius")?;
    let max_radius = whole("max_radius")?;
    if max_radius <= 0 {
        return None;
    }
    let (scope, name) = named_variable(target)?;
    Some(Effect::LocationVariableRadius {
        scope,
        name,
        min_radius,
        max_radius,
        listener,
    })
}

/// The one variable a location member names, as its namespace and its name.
fn named_variable(target: &cdda_json::Object) -> Option<(String, String)> {
    target.members().iter().find_map(|(scope, value)| match value {
        Value::Str(name) => Some((scope.clone(), name.clone())),
        _ => None,
    })
}

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
        if let Some(found) = read_terrain_target(object, target, listener) {
            return Some(found);
        }
        if let Some(found) = read_radius_search(object, target, listener) {
            return Some(found);
        }
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

/// How far a mission target searches when the data does not say, in overmap tiles.
///
/// Reference: `mission_util.cpp:429` reads `search_range` with a default of `OMAPX * 3` and `map_scale_constants.h:37` says `OMAPX = 180`, so 540.
pub const DEFAULT_SEARCH_RANGE: i32 = 540;

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
    // Reference: `npctalk.cpp:8015` names these `u_lose_effect` / `npc_lose_effect`. The port read `u_remove_effect`, which the data never writes -
    // so every one of the 29 `u_lose_effect` entries refused and the refusal was blamed on the effect rather than on the reader.
    ("u_lose_effect", false, false),
    ("npc_lose_effect", true, false),
    ("u_remove_effect", false, false),
    ("npc_remove_effect", true, false),
] {
    if let Some(Value::Array(effects)) = object.get(member)
        && !learn
        && effects.iter().all(|effect| matches!(effect, Value::Str(_)))
    {
        // Reference: `f_remove_effect` takes the member as a variable string or an array of them, and removes each.
        let names: Vec<String> = effects
            .iter()
            .filter_map(|effect| match effect {
                Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect();
        return Some(Effect::RemoveEffects {
            effects: names,
            listener,
        });
    }
    if let Some(Value::Str(effect)) = object.get(member) {
        // A `target_part` names the body part an effect lands on. The port has no limb model - a creature is one thing, not a set of parts - so the part
        // is **ignored rather than refused**: the effect lands on the creature, which is where the reference puts it once a part is chosen.
        let _ = object.get("target_part");
        let _ = object.get("bodypart");
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
                // Reference: `get_duration_or_var` also takes a **variable**, which the data writes as `{"math": [...]}`.
                Some(value @ Value::Object(_)) => {
                    EffectDuration::Computed(read_math_expression(value)?)
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
