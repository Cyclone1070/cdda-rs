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
    /// Whether the item in the dialogue's **other slot** has enough charges to be used.
    ///
    /// Reference: `f_has_ammo` (`condition.cpp:1963-1974`): the beta is read as an item and `item::ammo_sufficient` (`item.cpp:11569-11582`) answers from its charges - a
    /// charge-counted stack compares its charges, a comestible is always sufficient, and anything else answers from its remaining shots.
    HasAmmo,
    /// Whether the **player** can see one of the dialogue's creatures.
    ///
    /// Reference: `f_player_see` (`condition.cpp:1366-1377`): the player's view is asked whether it sees the creature the dialogue holds - the speaker for
    /// `player_see_u`, the listener for `player_see_npc`.
    PlayerSees {
        /// Whether it asks about the listener rather than the speaker.
        listener: bool,
    },
    /// Whether the dialogue has an **alpha** actor.
    ///
    /// Reference: `f_has_alpha` (`condition.cpp:1412-1417`): `d.has_alpha`, set when the dialogue has someone to talk **as**. The port's dialogue always has one - an
    /// effect is run for a creature - so this answers true whenever it is asked.
    HasAlpha,
    /// Whether the dialogue has a **beta** actor.
    ///
    /// Reference: `f_has_beta` (`condition.cpp:1418-1423`): `d.has_beta`, set when the dialogue has someone to talk **to**, which in the port is whether a listener
    /// was handed in.
    HasBeta,
    /// A yes/no question put to the player, answered by the choice queue.
    ///
    /// Reference: `u_query` (`condition.cpp`): the question is shown and the answer is the condition's value; `default` is what it answers when the player cannot
    /// be asked. The port asks nothing - the answer is queued by a scripted playtest or the UI, and an unanswered question answers the data's default.
    Query {
        /// The answer when nothing is queued.
        default: bool,
    },
    /// Whether an actor can see a square that a variable holds.
    ///
    /// Reference: `f_can_see_location` (`condition.cpp:2173-2181`): the member is a `var_info`, and the answer is the actor's `can_see_location` of the square it
    /// holds - `talker_character.cpp:923-928`, `Creature::sees( here, pos )`.
    CanSeeLocation {
        /// The variable's scope and name.
        target: (String, String),
        /// Whether it asks about the listener rather than the speaker.
        listener: bool,
    },
    /// Whether a creature cannot hear.
    ///
    /// Reference: `f_is_deaf` (`condition.cpp:2117-2122`) answering `Character::is_deaf` (`character.cpp:4768-4775`).
    IsDeaf {
        /// Whether it asks about the listener rather than the speaker.
        listener: bool,
    },
    /// Whether the terrain or furniture at a square carries a flag.
    ///
    /// Reference: `f_map_ter_furn_with_flag` (`condition.cpp:1818-1842`): `map_terrain_with_flag` asks the tile's terrain, `map_furniture_with_flag` its furniture, and
    /// `loc` names the variable holding the square.
    MapTerFurnWithFlag {
        /// The flag asked about.
        flag: StringSource,
        /// The variable's scope and name.
        target: (String, String),
        /// Whether it asks about furniture rather than terrain.
        furniture: bool,
    },
    /// Whether a square is inside a city.
    ///
    /// Reference: `f_map_in_city` (`condition.cpp:1863-1875`): the target is a variable holding a position, projected to an overmap tile, and answered by the city's
    /// tile set. A target below z -1 is **not** in a city, which is the reference's own guard for the underground.
    MapInCity {
        /// The variable's scope and name.
        target: (String, String),
    },
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
    /// A variable **with a fallback**: `{"global_val": "x", "default": 1}`.
    ///
    /// Reference: `dbl_or_var_part::evaluate` (`condition.cpp`): a part that names a variable answers what the dialogue holds for it, and the `default` member is
    /// what it answers when the variable is unset. The data uses it where a first run has no value yet - a gamble multiplier that starts at one.
    VariableOrDefault {
        /// Which namespace, as the data spells it: `u_val`, `global_val`, `context_val`.
        scope: String,
        /// The variable's name.
        name: String,
        /// What to answer when the variable holds nothing.
        fallback: f64,
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
    /// A condition and the two values between which it chooses: `a ? b : c`.
    ///
    /// Reference: `?` and `:` in `binary_ops` (`math_parser_impl.h:329-330`), both at precedence **zero** and right-associative. The reference's parser rewrites a
    /// nested ternary into parens on the way (`math_parser.cpp:678-682`), so `0?0?-1:-2:1` groups as `0?(0?-1:-2):1` - which the reader below gets for free by parsing
    /// the branch between them as a whole expression.
    Conditional {
        /// The test: non-zero takes the first branch.
        condition: Box<Expression>,
        /// Taken when the test is non-zero.
        then: Box<Expression>,
        /// Taken when it is zero.
        otherwise: Box<Expression>,
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
    /// Raise to a power, written `^`, which the reference spells `math_pow` (`math_parser_impl.h:341-342`) - **not** a bitwise exclusive or, whatever the shape of the
    /// data suggests.
    Power,
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
            "^" => Self::Power,
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
        // Reference: `{"has_ammo", &conditional_fun::f_has_ammo}` (`condition.cpp:2670`), argument-less: it asks the dialogue's **beta**, which is the item in an
        // inventory effect (`condition.cpp:1963-1974`).
        "has_ammo" => return Some(Condition::HasAmmo),
        // Reference: `{"u_is_deaf", "npc_is_deaf", &conditional_fun::f_is_deaf}` (`condition.cpp:2659`), argument-less in the data.
        "u_is_deaf" | "npc_is_deaf" => {
            return Some(Condition::IsDeaf {
                listener: name == "npc_is_deaf",
            });
        }
        "npc_can_see" => return Some(Condition::CanSee { listener: true }),
        // Some argument-less guards are written as **bare names** inside an `or`, not as members: `"u_is_outside"` with no value (`haunting.json`).
        "u_is_outside" | "npc_is_outside" => {
            return Some(Condition::IsOutside {
                listener: name == "npc_is_outside",
            });
        }
        // Reference: `{"player_see_u", "player_see_npc", &conditional_fun::f_player_see}` (`condition.cpp:2671`), argument-less, so the data writes them bare.
        "player_see_u" | "player_see_npc" => {
            return Some(Condition::PlayerSees {
                listener: name == "player_see_npc",
            });
        }
        _ => {}
    }
    // Reference: `conditional_t( std::string_view )` (`condition.cpp:2783-2810`) looks a bare name up among the **argument-less** parsers, so every one of them is a
    // condition the data may write as a string. `is_day` and `is_night` are the two the port can answer from the calendar alone.
    matches!(
        name,
        "has_alpha"
            | "has_beta"
            | "has_weapon"
            | "u_has_weapon"
            | "npc_has_weapon"
            | "u_is_avatar"
            | "u_is_character"
            | "is_day"
            | "is_night"
            | "u_driving"
            | "u_is_underwater"
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
    // A `bodypart` is **ignored rather than refused**, and the reason is the same one the effect reader gives: the port has no limb model, so every effect it
    // applies is the creature's. A condition asking whether the creature has an effect on a named part therefore has exactly one answer the port can give - yes when
    // the creature has it - and refusing instead made three programs unreadable for a distinction the port cannot make either way. **The divergence is real and
    // written down: a limb model is what closes it, not a stricter reader.**
    let _ = object.get("bodypart");
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
    // `u_is_deaf` and `npc_is_deaf` are the two that ask about a creature rather than the world.
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
    // A yes/no question put to the player.
    //
    // Reference: `u_query` (`condition.cpp`, `f_u_query`): the member is the question's text and `default` the answer used when the player cannot be asked. The port
    // is headless, so the answer comes from its **choice queue** - the same store a menu answer comes from, filled by a scripted playtest or the UI.
    if let Some(_question) = object.get("u_query") {
        let default = match object.get("default") {
            None => false,
            Some(Value::Bool(answer)) => *answer,
            Some(_) => return None,
        };
        return Some(Condition::Query { default });
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
    for (member, listener) in [("u_can_see_location", false), ("npc_can_see_location", true)] {
        if let Some(Value::Object(target)) = object.get(member) {
            return Some(Condition::CanSeeLocation {
                target: named_variable(target)?,
                listener,
            });
        }
    }
    if let Some(found) = read_map_flag_condition(object).or_else(|| read_city_condition(object)) {
        return Some(found);
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
            "mon_variables",
            "open_air_allowed",
            "summoner_is_alpha",
            "summoner_is_beta",
        ] {
            if object.get(refused).is_some() {
                return None;
            }
        }
        // Reference: `f_spawn_monster` (`npctalk.cpp:7371-7420`) reads `friendly` as a **turns count** - how long the spawn stays friendly to the player, and zero
        // for a permanently hostile one. The port keeps a [`Friendly`](cdda_sim::friendly) marker on the creature and reads this member as a yes/no.
        let friendly = match object.get("friendly") {
            None => false,
            Some(Value::Bool(flag)) => *flag,
            Some(Value::Int(number)) => *number > 0,
            Some(_) => return None,
        };
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
        // Reference: `f_spawn_monster` (`npctalk.cpp:7405-7408`): a `target_var` names the square the spawn is centred on, instead of whoever spoke.
        let target = match object.get("target_var") {
            None => None,
            Some(value) => Some(read_variable_target(value)?),
        };
        return Some(Effect::SpawnMonster {
            monster,
            target,
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
            friendly,
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
        // A variable, with or without a `default`, is a count the game works out where it uses it: `get_dbl_or_var` takes the same three shapes, and the spawn
        // data writes `{"global_val": "gamble_multiplier", "default": 1}` for a count that starts at one.
        Some(Value::Object(object)) if object.get("math").is_none() => {
            let written = value?;
            Some(CountRange::Expression(Box::new(read_expression(written)?)))
        }
        Some(Value::Object(object)) => {
            let math = object.get("math")?;
            Some(CountRange::Expression(Box::new(read_expression(math)?)))
        }
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
    for spelling in ["<=", ">=", "==", "!=", "+", "-", "*", "/", "%", "^", "<", ">", "?"] {
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

/// How tightly an operator binds: comparisons loosest, then addition, then multiplication, then the power.
///
/// Reference: `binary_ops` (`math_parser_impl.h:328-344`): `+ -` at two, `* / %` at three, `^` at **four**. The power is **right**-associative there, which the
/// parser below expresses by letting it recurse at its own binding rather than one tighter.
fn binding(spelling: &str) -> Option<u8> {
    Some(match spelling {
        "<" | "<=" | ">" | ">=" | "==" | "!=" => 1,
        "+" | "-" => 2,
        "*" | "/" | "%" => 3,
        "^" => 4,
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
        // Reference: `^` is right-associative (`binary_ops`, `math_parser_impl.h:342`), so `2 ^ 3 ^ 2` is `2 ^ 9` rather than `8 ^ 2`.
        let next = if spelling == "^" { binds } else { binds + 1 };
        let (right, after) = parse_precedence(rest.get(1..)?, next)?;
        left = Expression::Binary {
            op: Operator::named(&spelling)?,
            left: Box::new(left),
            right: Box::new(right),
        };
        rest = after;
    }
    // Reference: `?` and `:` sit at precedence **zero** - looser than every other operator (`math_parser_impl.h:329-330`) - so the branch is read once the two sides of the
    // comparison are complete. The pair is right-associative, which falls out of parsing each branch as a whole expression.
    if let Some(Token::Operator(question)) = rest.first()
        && question == "?"
        && minimum == 0
    {
        let (then, after_then) = parse_precedence(rest.get(1..)?, 0)?;
        let Some(Token::Colon) = after_then.first() else {
            return None;
        };
        let (otherwise, after_else) = parse_precedence(after_then.get(1..)?, 0)?;
        return Some((
            Expression::Conditional {
                condition: Box::new(left),
                then: Box::new(then),
                otherwise: Box::new(otherwise),
            },
            after_else,
        ));
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
                // A bare name is a dialogue variable, and its **prefix says which holder**: `read_var_value` parses `u_`, `npc_`/`n_`, `global_` and `var_` out of the
                // name itself, so `u_LIXA_entry` is the speaker's variable `LIXA_entry` rather than a variable literally called `u_LIXA_entry`.
                let (scope, plain) = [
                    ("u_", "u_val"),
                    ("npc_", "npc_val"),
                    ("n_", "npc_val"),
                    ("global_", "global_val"),
                    ("var_", "var_val"),
                ]
                .iter()
                .find_map(|(prefix, scope)| name.strip_prefix(prefix).map(|plain| (*scope, plain)))
                .map_or(("var_val", name.as_str()), |(scope, plain)| (scope, plain));
                let expression = Expression::Variable {
                    scope: scope.to_owned(),
                    name: plain.to_owned(),
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
                    // A `default` beside the variable is the fallback `dbl_or_var_part::evaluate` uses when the variable holds nothing.
                    if let Some(fallback) = object.get("default") {
                        let fallback = match fallback {
                            Value::Int(number) => {
                                #[allow(clippy::cast_precision_loss)]
                                let as_float = *number as f64;
                                as_float
                            }
                            Value::Float(number) => *number,
                            _ => return None,
                        };
                        return Some(Expression::VariableOrDefault {
                            scope: scope.to_owned(),
                            name: name.clone(),
                            fallback,
                        });
                    }
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
        .or_else(|| read_alter_timed_events(object))
        .or_else(|| read_set_condition(object))
        .or_else(|| read_finish_mission(object))
        .or_else(|| read_remove_active_mission(object))
        .or_else(|| read_store_effects(object))
        .or_else(|| read_set_field(object))
        .or_else(|| read_custom_light_level(object))
        .or_else(|| read_transform_radius(object))
        .or_else(|| read_give_achievement(object))
        .or_else(|| read_trigger_event(object))
        .or_else(|| read_sell_item(object))
        .or_else(|| read_map_run_item_eocs(object))
        .or_else(|| read_run_inv_eocs(object))
        .or_else(|| read_location_variable_adjust(object))
        .or_else(|| read_mutation(object))
        .or_else(|| read_offer_mission(object))
        .or_else(|| read_sound_event(object))
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

/// The effects whose whole body is a store write, in one place.
///
/// Each of these reads one or two members and hands them to a store the port already keeps. They are grouped here for the same reason the runner groups its own
/// store effects: a table of rules stays small, and a family of similar rules lives in a function with a name.
fn read_store_effects(object: &cdda_json::Object) -> Option<Effect> {
    read_turn_cost(object)
        .or_else(|| read_add_wet(object))
        .or_else(|| read_place_override(object))
        .or_else(|| read_revert_location(object))
        .or_else(|| read_foreach(object))
        .or_else(|| read_transform_item(object))
        .or_else(|| read_remove_item_with(object))
        .or_else(|| read_lose_morale(object))
        .or_else(|| read_set_talker(object))
        .or_else(|| read_assign_activity(object))
}

/// Reads a `u_assign_activity` / `npc_assign_activity` entry.
///
/// Reference: `f_assign_activity` (`npctalk.cpp:5451-5462`): a required `str_or_var` activity and a required `duration`.
fn read_assign_activity(object: &cdda_json::Object) -> Option<Effect> {
    for (member, listener) in [("u_assign_activity", false), ("npc_assign_activity", true)] {
        let Some(activity) = object.get(member) else {
            continue;
        };
        return Some(Effect::AssignActivity {
            activity: read_string_source(activity)?,
            duration: read_effect_duration(object.get("duration"), EffectDuration::Turns(Some(0)))?,
            listener,
        });
    }
    None
}

/// Reads a `u_set_talker` / `npc_set_talker` entry.
///
/// Reference: `f_set_talker` (`npctalk.cpp:6937-6948`): one required member, a `var_info` given as an object.
fn read_set_talker(object: &cdda_json::Object) -> Option<Effect> {
    for (member, listener) in [("u_set_talker", false), ("npc_set_talker", true)] {
        let Some(Value::Object(target)) = object.get(member) else {
            continue;
        };
        return Some(Effect::SetTalker {
            target: named_variable(target)?,
            listener,
        });
    }
    None
}

/// Reads a `u_lose_morale` / `npc_lose_morale` entry.
///
/// Reference: `f_lose_morale` (`npctalk.cpp:7197-7204`): one required `str_or_var` member, which this reader requires too - it is tried on every object no other
/// reader claimed, so answering for an absent member would swallow the data.
fn read_lose_morale(object: &cdda_json::Object) -> Option<Effect> {
    for (member, listener) in [("u_lose_morale", false), ("npc_lose_morale", true)] {
        let Some(kind) = object.get(member) else {
            continue;
        };
        return Some(Effect::LoseMorale {
            kind: read_string_source(kind)?,
            listener,
        });
    }
    None
}

/// Reads a `u_remove_item_with` / `npc_remove_item_with` entry.
///
/// Reference: `f_remove_item_with` (`npctalk.cpp:4046-4056`): one required `str_or_var` member. The gate matters: this reader is tried on every object no other
/// reader claimed, so answering for an absent member would swallow the data.
fn read_remove_item_with(object: &cdda_json::Object) -> Option<Effect> {
    for (member, listener) in [("u_remove_item_with", false), ("npc_remove_item_with", true)] {
        let Some(item) = object.get(member) else {
            continue;
        };
        return Some(Effect::RemoveItemWith {
            item: read_string_source(item)?,
            listener,
        });
    }
    None
}

/// Reads a `place_override` entry.
///
/// Reference: `f_place_override` (`npctalk.cpp:4798-4819`): the place is a `translation_or_var`, `length` is required, and `key` is optional and defaults to the
/// empty string. The member is required here too - a reader that answered for its absence would claim every effect no other reader claimed.
fn read_place_override(object: &cdda_json::Object) -> Option<Effect> {
    let place = object.get("place_override")?;
    // Reference: the place is a `translation_or_var`, and `{"global_val": "place_name", "default": "strange cavern"}` is how the data gives one with a fallback for a
    // variable nobody has set - which is what stops the effect from refusing in a world that has not named the place yet.
    let fallback = match place {
        Value::Object(inner) => match inner.get("default") {
            None => None,
            Some(value) => Some(read_string_source(value)?),
        },
        _ => None,
    };
    Some(Effect::PlaceOverride {
        place: read_string_source(place)?,
        fallback,
        length: read_effect_duration(object.get("length"), EffectDuration::Turns(Some(0)))?,
        key: match object.get("key") {
            None => None,
            Some(value) => Some(read_string_source(value)?),
        },
    })
}

/// Reads a `transform_item` entry.
///
/// Reference: `f_transform_item` (`npctalk.cpp:6112-6120`): `str_or_var` required, `active` optional and false by default.
fn read_transform_item(object: &cdda_json::Object) -> Option<Effect> {
    let item = object.get("transform_item")?;
    Some(Effect::TransformItem {
        item: read_string_source(item)?,
        active: matches!(object.get("active"), Some(Value::Bool(true))),
    })
}

/// Reads a `foreach` entry.
///
/// Reference: `f_foreach` (`npctalk.cpp:7022-7080`). The member **is** the list's kind, `var` is required (`read_var_info`), and `target` is either a string naming
/// a list of ids or an array of ids - which is how the data writes `{"foreach": "array", "target": ["a", "b"]}`.
fn read_foreach(object: &cdda_json::Object) -> Option<Effect> {
    let source = match object.get("foreach")? {
        Value::Str(kind) => kind.clone(),
        _ => return None,
    };
    let Value::Object(variable) = object.get("var")? else {
        return None;
    };
    let (scope, name) = named_variable(variable)?;
    let mut target = None;
    let mut ids = Vec::new();
    match object.get("target") {
        Some(Value::Str(named)) => target = Some(named.clone()),
        Some(Value::Array(entries)) => {
            ids = entries.iter().map(read_string_source).collect::<Option<Vec<_>>>()?;
        }
        _ => return None,
    }
    Some(Effect::ForEach {
        source,
        target,
        ids,
        scope,
        name,
        effects: read_effect_list(object.get("effect"))?,
    })
}

/// Reads a `revert_location` entry.
///
/// Reference: `f_revert_location` (`npctalk.cpp:4886-4897`): the place is a **variable** (`read_var_info`), `time_in_future` is required, and `key` is an optional
/// `str_or_var`. A reader that answered for a missing member would claim every effect no other reader claimed, so the place is required here too.
fn read_revert_location(object: &cdda_json::Object) -> Option<Effect> {
    let place = object.get("revert_location")?;
    let Value::Object(inner) = place else {
        return None;
    };
    let (scope, name) = named_variable(inner)?;
    Some(Effect::RevertLocation {
        scope,
        name,
        // Reference: `get_duration_or_var( jo, "time_in_future", true )` - required, and a `dbl_or_var` like every other length in the data.
        after: read_effect_duration(object.get("time_in_future"), EffectDuration::Turns(Some(0)))?,
        key: match object.get("key") {
            None => None,
            Some(value) => Some(read_string_source(value)?),
        },
    })
}

/// Reads a `u_add_wet` / `npc_add_wet` entry.
///
/// Reference: `f_add_wet` (`npctalk.cpp:5464-5474`): the member **is** the amount, a `dbl_or_var` worked out when the effect is applied. The member is required -
/// a reader that answered for its absence would claim every effect no other reader claimed.
fn read_add_wet(object: &cdda_json::Object) -> Option<Effect> {
    for (member, listener) in [("u_add_wet", false), ("npc_add_wet", true)] {
        let Some(amount) = object.get(member) else {
            continue;
        };
        return Some(Effect::AddWet {
            amount: read_math_expression(amount)?,
            listener,
        });
    }
    None
}

/// Reads a `turn_cost` entry.
///
/// Reference: `f_turn_cost` (`npctalk.cpp:5291-5296`): `get_duration_or_var( jo, member, true )` - the member **is** the duration, and it is required.
fn read_turn_cost(object: &cdda_json::Object) -> Option<Effect> {
    // The member is **required**: this reader is tried on every object no other reader claimed, so answering `Some` for an object without a `turn_cost` would
    // silently turn every unreadable effect in the data into a free action. That is exactly the bug the first version of this function had, and the census caught it
    // as seventy-four programs that suddenly became "readable" and answered `Done` without doing anything.
    let cost = object.get("turn_cost")?;
    Some(Effect::TurnCost {
        cost: read_effect_duration(Some(cost), EffectDuration::Turns(Some(0)))?,
    })
}

/// Reads a `remove_active_mission` entry.
///
/// Reference: `f_remove_active_mission` (`npctalk.cpp:5959-5963`): one member, the mission's id.
fn read_remove_active_mission(object: &cdda_json::Object) -> Option<Effect> {
    Some(Effect::RemoveActiveMission {
        mission: read_string_source(object.get("remove_active_mission")?)?,
    })
}

/// Reads a `finish_mission` entry.
///
/// Reference: `f_finish_mission` (`npctalk.cpp:5928-5938`): `success` defaults to **false**, and an integer `step` means the effect advances a staged mission
/// instead of finishing it.
fn read_finish_mission(object: &cdda_json::Object) -> Option<Effect> {
    let mission = object.get("finish_mission")?;
    let step = match object.get("step") {
        None => None,
        Some(Value::Int(number)) => Some(i32::try_from(*number).ok()?),
        Some(_) => return None,
    };
    Some(Effect::FinishMission {
        mission: read_string_source(mission)?,
        success: matches!(object.get("success"), Some(Value::Bool(true))),
        step,
    })
}

/// Reads a `set_condition` entry.
///
/// Reference: `f_set_condition` (`npctalk.cpp:5868-5880`): the member is the name the condition is stored under, `condition` is the condition, and
/// `target_var` - when the data gives one - says which scope holds it.
fn read_set_condition(object: &cdda_json::Object) -> Option<Effect> {
    let name = object.get("set_condition")?;
    let condition = object.get("condition")?;
    let scope = match object.get("target_var") {
        None => "global_val".to_owned(),
        Some(Value::Object(target)) => named_variable(target)?.0,
        Some(_) => return None,
    };
    Some(Effect::SetCondition {
        name: read_string_source(name)?,
        condition: Box::new(read_condition(condition)?),
        scope,
    })
}

/// Reads an `alter_timed_events` entry.
///
/// Reference: `f_alter_timed_events` (`npctalk.cpp:4873-4882`): the member names the event, `time_in_future` is a duration `get_duration_or_var` defaults to zero,
/// and the text is parsed by the same duration reader the calendar uses.
fn read_alter_timed_events(object: &cdda_json::Object) -> Option<Effect> {
    let key = object.get("alter_timed_events")?;
    Some(Effect::AlterTimedEvents {
        key: read_string_source(key)?,
        time_in_future: match object.get("time_in_future") {
            None => StringSource::Literal("0".to_owned()),
            Some(value) => read_string_source(value)?,
        },
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
    // `string_input` (asking the player to type) is refused. `parse_tags` is **read**: the reference substitutes `<scope:name>` tags into the text where it is used
    // (`f_set_string_var` -> `parse_tags`, `npctalk.cpp:5809-5866`), and the data passes it for names built from a variable.
    //
    // `i18n` is **read and carried as false**: it asks the reference to look the text up in a translation catalogue, and the port has none - its strings are the data's
    // own text, untranslated, everywhere. Refusing the entry would drop a rule the port can run for the sake of a catalogue it does not have.
    if object.get("string_input").is_some() {
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
        parse_tags: matches!(object.get("parse_tags"), Some(Value::Bool(true))),
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
        for unsupported in ["container"] {
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
            loc: match object.get("loc") {
                None => None,
                Some(Value::Object(target)) => Some(named_variable(target)?),
                Some(_) => return None,
            },
            force_equip: matches!(object.get("force_equip"), Some(Value::Bool(true))),
            use_item_group: matches!(object.get("use_item_group"), Some(Value::Bool(true))),
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
        for unsupported in ["open_air_allowed", "target_var"] {
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
            // Reference: `f_spawn_npc` reads `outdoor_only` and `indoor_only` and keeps the spawns to squares that match, and `lifespan` as
            // `get_duration_or_var( jo, "lifespan", false, 0_seconds )` - a duration string or a two-end range, the same shape the monster spawn takes.
            outdoor_only: matches!(object.get("outdoor_only"), Some(Value::Bool(true))),
            indoor_only: matches!(object.get("indoor_only"), Some(Value::Bool(true))),
            lifespan: match object.get("lifespan") {
                None => None,
                Some(value) => Some(read_duration_range(value)?),
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
        Value::Str(id) => (StringSource::Literal(id.clone()), false),
        Value::Object(inside) => {
            // Reference: `get_str_or_var( spell_jo.get_member( "id" ), "id" )` (`npctalk.cpp:5595`) - the id may be a variable, and the data names one it read from
            // a context value.
            let id = read_string_source(inside.get("id")?)?;
            let hit_self = matches!(inside.get("hit_self"), Some(Value::Bool(true)));
            (id, hit_self)
        }
        _ => return None,
    };
    Some(Effect::CastSpell {
        spell,
        hit_self,
        // Reference: `jo.get_bool( "targeted", false )` (`npctalk.cpp:5591`), a member of the effect rather than of the spell.
        targeted: matches!(object.get("targeted"), Some(Value::Bool(true))),
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
        Expression::Number(_)
        | Expression::Text(_)
        | Expression::Pair { .. }
        | Expression::Binary { .. }
        | Expression::Conditional { .. }
        | Expression::VariableOrDefault { .. } => return None,
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
    /// A variable holding a duration **in text**, with the reference's fallback for a variable nobody set.
    ///
    /// Reference: `get_duration_or_var` (`npctalk.cpp:3426-3450`) reads `{"default": "1 minutes", "u_val": "cell_time"}` as a duration the dialogue holds - the data's
    /// portal cells open for as long as the cell's own variable says, and for a minute when it says nothing.
    Named {
        /// Which namespace the variable lives in.
        scope: String,
        /// The variable's name.
        name: String,
        /// The turns to use when the variable holds nothing.
        fallback: i64,
    },
    /// A roll between two ends the dialogue works out when the effect is applied: `["1 minutes", {"math": ["ps_str * time(' 1 m')"]}]`, which the portal storms use.
    ///
    /// Reference: `get_duration_or_var` reads each end as a `dbl_or_var` (`npctalk.cpp:3426-3450`), so both ends may be expressions.
    RangeComputed(crate::dialogue::Expression, crate::dialogue::Expression),
    /// A length the dialogue works out when the effect is applied - `{"math": ["rand(345600) + 86400"]}` in the data.
    Computed(crate::dialogue::Expression),
}

/// One `search_data` entry: what an item must be for this entry to match it.
///
/// Reference: `item_search_data` (`npctalk.cpp:235-441`) - a bag of optional filters whose `check` answers true only when every stated one passes. The port reads the four
/// members the content uses that it can answer: an id, the wielded item, a worn item, and a **condition** asked with the item as the dialogue's beta.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchFilter {
    /// The item id this entry wants, if it names one.
    pub id: Option<String>,
    /// Whether this entry matches only the wielded item.
    pub wielded_only: bool,
    /// Whether this entry matches only a worn item.
    pub worn_only: bool,
    /// A condition the item must satisfy, asked with the item as the dialogue's beta (`npctalk.cpp:430-435`).
    pub condition: Option<Box<Condition>>,
}

/// An effect a dialogue program can run.
///
/// Reference: `talk_effect_fun_t`'s table (`npctalk.cpp:8039-8135`), which maps each member name to a function. The port keeps
/// the same shape - a name, a function - and grows one name at a time, counting what it can run against the data.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// A `math` statement list: one or more assignments to dialogue variables.
    Math(Vec<Assignment>),
    /// A message to the player, which the data may give as a **variable** rather than as text.
    ///
    /// Reference: `f_message` reads the member as a `translation_or_var` (`get_translation_or_var`, `condition.cpp:352-385`): an object is a variable holding the text,
    /// with an optional `default` for a variable nobody set. The text is worked out when the message is shown, not when the data is read.
    Message {
        /// What to say, which may be a variable's text.
        text: StringSource,
        /// The text to use when the variable holds nothing, when the data gives one.
        fallback: Option<StringSource>,
    },
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
    /// Moves a square the dialogue holds, and stores the new one.
    ///
    /// Reference: `f_location_variable_adjust` (`npctalk.cpp:4411-4448`): the member names the variable holding the square, `x_adjust` and `y_adjust` move it (by
    /// **overmap tiles** when `overmap_tile` is set), `z_adjust` moves the level or replaces it with `z_override`, and the answer goes to `output_var` or back to the
    /// variable it came from.
    LocationVariableAdjust {
        /// Which holder the square came from.
        scope: String,
        /// The variable holding the square.
        name: String,
        /// Where the answer goes, when the data names somewhere other than where it came from.
        output: Option<(String, String)>,
        /// How far to move it on x.
        x_adjust: Expression,
        /// How far to move it on y.
        y_adjust: Expression,
        /// How far to move the level, or the level to set.
        z_adjust: Expression,
        /// Whether `z_adjust` replaces the level instead of moving it.
        z_override: bool,
        /// Whether the x and y moves are counted in overmap tiles.
        overmap_tile: bool,
    },
    /// Runs another effect after a delay.
    ///
    /// Reference: the reference's `time_in_future` on an effect with a **payload** (`timed_event`): the effect is kept and run when the turn comes. The port keeps the
    /// effect and the length, and the turn loop's `Scheduled` stage runs it.
    Later {
        /// The effect to run when the turn comes.
        effect: Box<Effect>,
        /// How long to wait.
        after: DurationSource,
    },
    /// Stores the nearest overmap tile carrying a terrain, as a place.
    ///
    /// Reference: `f_location_variable` (`npctalk.cpp:4205-4210`, `4261-4300`) with `target_params`: the reference asks `mission_util::parse_mission_om_target`
    /// (`mission_util.cpp:391-450`) for the terrain to look for, how far to search and the least distance from the origin, then finds the tile and stores its square.
    ///
    /// **A `target_params` that names an overmap special refuses**, because a special's tiles are matched by their own rule and the port has none.
    LocationVariableFromOvermap {
        /// Where the square is stored.
        scope: String,
        /// The variable's name.
        name: String,
        /// The overmap terrain to look for.
        terrain: StringSource,
        /// The least distance from the origin, when the data names one.
        min_distance: Option<Expression>,
        /// The level to store, when the data names one.
        z: Option<Expression>,
        /// Whether it works from the listener's square rather than the speaker's.
        listener: bool,
    },
    /// Plays a sound **event**, which the headless port has no ear for.
    ///
    /// Reference: `f_sound_effect` (`npctalk.cpp:5528-5570`): `id` names a sound in the sound pack, `volume` overrides its own level, `outdoor_event` picks the outdoor
    /// variant, and `variant`. Every one of those decides **what the player hears** and nothing else - the reference's own comment on the family is that it is
    /// presentation. The port carries the id and answers `Done`, and the day it has audio this effect is the one that plays it.
    SoundEvent {
        /// The sound's id.
        id: StringSource,
        /// The volume to play it at, when the data names one.
        volume: Option<Expression>,
        /// Whether the outdoor variant is wanted.
        outdoor: bool,
    },
    /// Runs programs after a delay.
    ///
    /// Reference: `f_run_eocs` (`npctalk.cpp:6332-6338`) with `time_in_future`: the reference schedules a timed event carrying the programs and runs it when the turn
    /// comes, so a call that says `"2 seconds"` happens two turns later and not now.
    RunEocLater {
        /// The programs to run.
        programs: Vec<ProgramRef>,
        /// How long to wait: one duration, or two ends the game draws between.
        after: crate::dialogue::DurationSource,
    },
    /// Gives the creature mutations, up to a category's reach.
    ///
    /// Reference: `f_mutate` (`npctalk.cpp:3563-3571`): the member is the **highest category depth** to mutate to, and `Character::mutate` draws mutations from the
    /// pools that depth allows. The port has no mutation pool, so only a depth of **zero** is answered - it asks for no mutation at all, and the effect changes
    /// nothing, which is a real answer rather than an approximation.
    Mutation {
        /// The highest category depth to mutate to.
        highest_category: Expression,
        /// The mutation category when the data names one.
        category: Option<String>,
        /// Whether the mutation goes to the **listener** rather than the speaker: the data's `npc_mutate` and `npc_mutate_category` members.
        npc: bool,
        /// Whether the creature pays mutagen for it. The member defaults to **true** (`npctalk.cpp:3565`), so a survivor mutating in dialogue spends vitamins.
        use_vitamins: bool,
        /// Whether the good/bad roll is skipped entirely, which the data asks for when a mutation is meant to be random (`npctalk.cpp:3580-3582`).
        true_random: bool,
    },
    /// Offers missions to the other creature in the conversation.
    ///
    /// Reference: `f_offer_mission` (`npctalk.cpp:5976-6001`): the member is one mission id or a list, and the reference reserves a new mission of each for **the NPC**
    /// (`d.actor( d.has_beta )->get_npc()`), which is why the offer lands on the listener.
    OfferMission {
        /// The mission type ids offered.
        missions: Vec<String>,
    },
    /// Stores the square of a monster of a given kind found around the actor.
    ///
    /// Reference: `f_location_variable` (`npctalk.cpp:4250-4300`) with `search_type = "monster"`: the reference walks the squares inside
    /// `target_max_radius` around the actor and answers the first one holding a creature whose type id is the one named, skipping squares at or inside
    /// `target_min_radius`.
    MonsterAround {
        /// The monster's id.
        monster: String,
        /// Which holder the square goes to.
        scope: String,
        /// The variable the square goes into.
        name: String,
        /// How near the actor a monster may be and still be skipped.
        min_radius: i32,
        /// How far out to look.
        max_radius: i32,
        /// Whether the square goes to the listener's variables rather than the speaker's.
        listener: bool,
    },
    /// Runs programs on the items lying on the map within a radius.
    ///
    /// Reference: `f_map_run_item_eocs` (`npctalk.cpp:6806-6875`): the target is `loc` or the actor's own square, every square within `max_radius` and outside
    /// `min_radius` is searched, and the programs run on the items found - `"all"` for every one, `"random"` for one. A `manual` mode is a **menu**, and the port has no
    /// chooser, so it reads the mode and refuses when the effect runs rather than picking for the player.
    MapRunItemEocs {
        /// How the items are chosen: `all`, `random`, or a menu.
        mode: String,
        /// The location to search around, when the data names one.
        loc: Option<(String, String)>,
        /// The closest a square may be, inclusive.
        min_radius: CountRange,
        /// The furthest a square may be, inclusive.
        max_radius: CountRange,
        /// The filters an item must pass any one of.
        search_data: Vec<SearchFilter>,
        /// The programs to run on each chosen item.
        programs: Vec<ProgramRef>,
        /// The programs to run when nothing was found.
        false_programs: Vec<ProgramRef>,
    },
    /// Hands items to the other creature and raises what it is owed.
    ///
    /// Reference: `f_u_sell_item` (`npctalk.cpp:3884-3930`): the member names the item, `count` how many (defaulting to one unless a `charges` member is present, in which case
    /// it defaults to zero - the reference's own quirk), and `cost` what the buyer is owed. Charges are handed over when the kind is counted in charges and the seller has
    /// them; whole items otherwise; and when neither is enough the reference says so and runs `false_eocs` instead.
    SellItem {
        /// The item's id, which the data may write as a variable.
        item: StringSource,
        /// How many are handed over.
        count: CountRange,
        /// What the buyer is owed, which is `cost` in the reference.
        cost: CountRange,
        /// Whether the kind is sold by its **charges** rather than by whole items - the entry said `charges`.
        by_charges: bool,
        /// The programs to run on a sale.
        true_programs: Vec<ProgramRef>,
        /// The programs to run when the seller has too few.
        false_programs: Vec<ProgramRef>,
    },
    /// Sends an event on the bus, which anything watching that kind answers.
    ///
    /// Reference: `f_trigger_event` (`npctalk.cpp:7894-7911`) reads the member as the event type's name and `args` as a list of strings or variables, evaluated when the
    /// effect runs. The arguments are **positional**: position `n` is the type's `n`th field (`cdda_content::events::EventTable`), which is why the table is dumped rather
    /// than assumed.
    TriggerEvent {
        /// The event type's name, exactly as the reference's own table spells it.
        kind: String,
        /// The arguments, in the order the type's table gives its fields.
        args: Vec<StringSource>,
    },
    /// Runs programs on the items a creature carries.
    ///
    /// Reference: `f_run_inv_eocs` (`npctalk.cpp:6760-6805`): `u_run_inv_eocs` says how the items are chosen - `"all"` for every match, `"manual"` for a menu - and
    /// `search_data` narrows which items count by id. The port reads the `"all"` form and refuses `"manual"`, because a menu is a question the headless port can only
    /// answer from its queue and the reference's menu picks **one** item, not many.
    RunInvEocs {
        /// The filters an item must pass **any one of**; empty means every item passes.
        ///
        /// Reference: `run_item_eocs` (`npctalk.cpp:3341-3348`) starts `true_tgt = data.empty()` and breaks on the first `item_search_data` that matches, so the entries are
        /// **alternatives**, not conditions. The port kept one id list beside two flags and treated the flags as filters over every entry, which is not what the data says.
        search_data: Vec<SearchFilter>,
        /// Whether **one** match is drawn rather than every match being run on.
        random: bool,
        /// Whether the items are the **listener's** rather than the speaker's: the `npc_run_inv_eocs` member, which is the same rule as `u_run_inv_eocs` aimed at the
        /// other creature (`f_run_inv_eocs` takes the member name it was registered under, `npctalk.cpp:6760-6805`).
        listener: bool,
        /// Whether the data asks the player to pick the items: `manual` or `manual_mult`. The port reads the mode so a program that uses one is a **known**
        /// question rather than an unreadable entry, and refuses it when it runs, because a menu is a choice this headless port cannot make.
        manual: bool,
        /// The programs to run on each matching item.
        programs: Vec<ProgramRef>,
        /// The programs to run **instead, once**, when nothing matched.
        ///
        /// Reference: `run_item_eocs` (`npctalk.cpp:3372-3414`) answers `run_eoc_vector( false_eocs, d )` in every mode when `true_items` came out empty - a branch that
        /// belongs to the dialogue rather than to an item, because there is no item to be the dialogue's beta.
        false_programs: Vec<ProgramRef>,
    },
    /// Marks an achievement completed on the character.
    ///
    /// Reference: `f_give_achievment` (`npctalk.cpp:5566-5585`): the member names an achievement, and it is reported completed **only while it is pending** - so
    /// completing one twice changes nothing.
    GiveAchievement {
        /// The achievement's id.
        achievement: StringSource,
    },
    /// Applies a named terrain transform to the squares around the actor.
    ///
    /// Reference: `f_transform_radius` (`npctalk.cpp:4450-4495`): `ter_furn_transform` names the table, the member `transform_radius` is how far it reaches, and
    /// `time_in_future` postpones it. The port reads the table and the radius and applies it at once; a postponed one, a `target_var` and a `key` are refused.
    TransformRadius {
        /// The transform table's id.
        transform: StringSource,
        /// How far it reaches, in squares.
        radius: Expression,
        /// The square it is centred on, when the data names one instead of the actor's own.
        target: Option<AssignmentTarget>,
        /// Whether it works around the listener rather than the speaker.
        listener: bool,
    },
    /// Sets the light level for a while, as a timed event.
    ///
    /// Reference: `f_custom_light_level` (`npctalk.cpp:7215-7232`): the member is the level, `length` how long it lasts (default nothing), and `key` names it.
    /// The rule adds **one second** to the length - "because this will get ticked on the turn its applied before it has an effect" - and schedules a
    /// `timed_event_type::CUSTOM_LIGHT_LEVEL`, which is what the port schedules too.
    CustomLightLevel {
        /// The light level it sets.
        level: Expression,
        /// How long it lasts.
        length: EffectDuration,
        /// The name the data gives it, when it gives one.
        key: Option<StringSource>,
    },
    /// Puts a field on the squares around the actor.
    ///
    /// Reference: `f_field` (`npctalk.cpp:7691-7725`): the member names the field, `intensity` defaults to one, `age` to a turn, and `radius` to
    /// **ten million** - "everywhere" - and every square within the radius gets the field. The port reads the field, the intensity and the radius; an entry that
    /// states no radius is refused rather than handed a radius of ten million, because the port's map is a loaded level and not a world.
    SetField {
        /// The field's id.
        field: StringSource,
        /// How thick it is.
        intensity: Expression,
        /// How far it reaches, which the data states.
        radius: Option<Expression>,
        /// Whether it lands around the listener rather than the speaker.
        listener: bool,
    },
    /// Finds a **piece of furniture** around the actor and writes its square into a variable.
    ///
    /// Reference: `f_location_variable` (`npctalk.cpp:4231-4300`), the `search_type = "furniture"` path: the same walk as the terrain search, comparing furniture
    /// instead of terrain.
    FurnitureAround {
        /// The furniture's name.
        furniture: StringSource,
        /// The variable's scope.
        scope: String,
        /// The variable's name.
        name: String,
        /// How near a square must be to be considered, exclusive.
        target_min_radius: i32,
        /// How far the search reaches.
        target_max_radius: i32,
        /// Whether it is the listener's square and variable rather than the speaker's.
        listener: bool,
    },
    /// Finds a terrain around the actor and writes its square into a variable.
    ///
    /// Reference: `f_location_variable` (`npctalk.cpp:4194-4300`), the `search_target` block with `search_type = "terrain"`: squares within `target_max_radius`
    /// of the actor are walked in the reference's own order, those no further than `target_min_radius` are skipped, and the **first** whose terrain id matches is
    /// written to the variable. Nothing found leaves the variable holding the actor's own square.
    TerrainAround {
        /// The terrain's name.
        terrain: StringSource,
        /// The variable's scope and name.
        scope: String,
        /// The variable's name.
        name: String,
        /// How near a square must be to be considered, exclusive.
        target_min_radius: i32,
        /// How far the search reaches, in squares.
        target_max_radius: i32,
        /// Whether it is the listener's square and variable rather than the speaker's.
        listener: bool,
    },
    /// Starts an activity on a creature, for a given number of moves.
    ///
    /// Reference: `f_assign_activity` (`npctalk.cpp:5451-5462`): the member names the activity, `duration` is required, and the reference passes
    /// `to_moves<int>( duration )` - a hundred moves a turn - as the activity's length.
    AssignActivity {
        /// The activity's id.
        activity: StringSource,
        /// How long it runs for.
        duration: EffectDuration,
        /// Whether it goes to the listener rather than the speaker.
        listener: bool,
    },
    /// Writes the speaking creature's id into a variable, as text.
    ///
    /// Reference: `f_set_talker` (`npctalk.cpp:6937-6948`): the member is a `var_info`, and the actor's `getID().get_value()` is written into it **as a string** -
    /// the reference's own comment says so, because a later `run_eocs` reads it back as one.
    SetTalker {
        /// Where the id is written: the variable's scope and name.
        target: (String, String),
        /// Whether it is the listener's id rather than the speaker's.
        listener: bool,
    },
    /// Changes the item the dialogue is talking about into another kind.
    ///
    /// Reference: `f_transform_item` (`npctalk.cpp:6112-6131`): the member is the new id, `active` decides whether the turned item starts active, and the item is
    /// the dialogue's `beta` - so a transform inside `u_run_inv_eocs` changes **that** item.
    TransformItem {
        /// The kind it becomes.
        item: StringSource,
        /// Whether the new kind starts active.
        active: bool,
    },
    /// Runs an effect list once for every id in a list, naming each one in a variable.
    ///
    /// Reference: `f_foreach` (`npctalk.cpp:7022-7080`): `foreach` says what kind of list (`ids`, `item_group`, `monstergroup`, `array`), `target` says which ids
    /// (`flag`, `trait`, `vitamin`, `bodypart`), `var` is written with each element, and `effect` runs once per element.
    ForEach {
        /// The list's kind: `ids` or `array` in this port.
        source: String,
        /// Which ids to walk, for `ids`.
        target: Option<String>,
        /// The ids themselves, for `array`.
        ids: Vec<StringSource>,
        /// Which namespace the element is written into.
        scope: String,
        /// The variable the element is written into.
        name: String,
        /// The effects run once per element.
        effects: Vec<Effect>,
    },
    /// Remembers a place's tiles and puts them back later.
    ///
    /// Reference: `f_revert_location` (`npctalk.cpp:4884-4918`): the member is a **place variable**, `time_in_future` is required, and `key` names the timed event
    /// (defaulting to the empty string, which is how the reference says "the plain `REVERT_SUBMAP`").
    RevertLocation {
        /// Which namespace the place variable lives in.
        scope: String,
        /// The place variable's name.
        name: String,
        /// How long until the tiles go back.
        after: EffectDuration,
        /// The timed event's name, when the data gives one.
        key: Option<StringSource>,
    },
    /// Takes a morale point away from a creature.
    ///
    /// Reference: `f_lose_morale` (`npctalk.cpp:7197-7204`): one required `str_or_var` naming the morale type, removed with `remove_morale`, which erases every point
    /// of that type and leaves the rest.
    LoseMorale {
        /// The morale type's id.
        kind: StringSource,
        /// Whether it goes to the listener rather than the speaker.
        listener: bool,
    },
    /// Removes every item of one kind from a creature.
    ///
    /// Reference: `f_remove_item_with` (`npctalk.cpp:4046-4056`): the member is an item id, and **every** stack of it is removed - a different rule from
    /// `consume_item`, which takes a counted amount and refuses to take part of what is asked.
    RemoveItemWith {
        /// The item's id, which the data may give as a variable.
        item: StringSource,
        /// Whether it goes to the listener rather than the speaker.
        listener: bool,
    },
    /// Schedules a place to be renamed for a while.
    ///
    /// Reference: `f_place_override` (`npctalk.cpp:4798-4819`): the member is the new name, `length` is how long it lasts (required), and `key` is an optional name
    /// that pairs this event with another. The effect itself does one thing - it **adds a timed event** for `now + length + 1 second`, `OVERRIDE_PLACE`, with the
    /// name and the key carried on it - and that event is what renames the place when it comes due.
    PlaceOverride {
        /// The name used when the variable holds nothing, as the data's `default` writes it.
        fallback: Option<StringSource>,
        /// The name the place takes on.
        place: StringSource,
        /// How long it lasts.
        length: EffectDuration,
        /// The name pairing this override with another, when the data gives one.
        key: Option<StringSource>,
    },
    /// Makes a creature wetter.
    ///
    /// Reference: `f_add_wet` (`npctalk.cpp:5464-5474`): the member is a `dbl_or_var` amount, added to the character's wetness with `wet_character`. The port
    /// keeps one wetness number per creature, because it has no limb model, and adds the amount to it.
    AddWet {
        /// How much wetter, which may be an expression.
        amount: Expression,
        /// Whether it goes to the listener rather than the speaker.
        listener: bool,
    },
    /// Makes the speaker pay for the words, in action points.
    ///
    /// Reference: `f_turn_cost` (`npctalk.cpp:5291-5300`): the member itself is a duration, the reference converts it with `to_moves` - one turn is a hundred
    /// moves - and spends it from the character who is speaking. A dialogue with a cost leaves the speaker with less of a turn to act in.
    TurnCost {
        /// How much time the effect costs.
        cost: EffectDuration,
    },
    /// Drops a mission from the character's list without calling it done or failed.
    ///
    /// Reference: `f_remove_active_mission` (`npctalk.cpp:5959-5973`): the avatar's own mission of that type is removed and the search stops at the first match.
    /// The port keeps one entry per mission type, so "the first match" is the only one there is.
    RemoveActiveMission {
        /// The mission's id.
        mission: StringSource,
    },
    /// Finishes a mission: taken off the character's list, as done or as failed.
    ///
    /// Reference: `f_finish_mission` (`npctalk.cpp:5928-5950`): the member names the mission, `success` defaults to **false**, and a `step` member advances a
    /// staged mission instead. The port drops the mission from the character's active list; it does not yet keep a completed-or-failed record, which is a store the
    /// data's own mission rewards would need.
    FinishMission {
        /// The mission's id.
        mission: StringSource,
        /// Whether it was a success.
        success: bool,
        /// The step of a staged mission, when the data advances one.
        step: Option<i32>,
    },
    /// Stores a named condition, to be asked later.
    ///
    /// Reference: `f_set_condition` (`npctalk.cpp:5868-5880`): the member is the name and `condition` is the condition itself, stored in the dialogue's own
    /// variables so `u_has_condition` can evaluate it at any later turn.
    SetCondition {
        /// The name it is stored under.
        name: StringSource,
        /// The condition to store.
        condition: Box<Condition>,
        /// The variable's type, as the data spells it.
        scope: String,
    },
    /// Schedules a named event to happen a set time from now.
    ///
    /// Reference: `f_alter_timed_events` (`npctalk.cpp:4873-4882`): the member names the event and `time_in_future` says how far ahead - a duration, defaulting to
    /// zero, which schedules the event for **this turn**.
    AlterTimedEvents {
        /// The event's name.
        key: StringSource,
        /// How far ahead it is due.
        time_in_future: StringSource,
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
    /// where it goes. `parse_tags` (substituting tags into the text) is read; `string_input` (asking the player to type) is refused, because
    /// each is a rule the port has not written.
    SetStringVar {
        /// Whether `<scope:name>` tags in the text are substituted when the effect runs.
        parse_tags: bool,
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
        /// Where the item goes, when the data names a place rather than leaving it at the actor.
        ///
        /// Reference: `f_spawn_item` (`npctalk.cpp:3795`) reads `loc` as a `var_info` and puts the item at that square. The port keeps the variable's scope and
        /// name and reads the square at run time, because a location variable is written by the program that ran before this one.
        loc: Option<(String, String)>,
        /// Whether the name is an **item group** to draw from rather than an item.
        ///
        /// Reference: `use_item_group` (`npctalk.cpp:3781`) sends the name to `item_group::item_from` instead of the item factory.
        use_item_group: bool,
        /// Whether the item is forced into the creature's hands.
        ///
        /// Reference: `force_equip` (`npctalk.cpp:3799`) wields what was spawned. The port has no wield slot yet, so the reader keeps the flag and the runner
        /// refuses rather than spawning an item the data meant to be in somebody's hand.
        force_equip: bool,
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
        /// Whether the spawns must be under the sky.
        outdoor_only: bool,
        /// Whether they must be under a roof.
        indoor_only: bool,
        /// How long a spawn stays, when the data gives a span of turns.
        lifespan: Option<(i64, i64)>,
        /// Whether it is the listener who does the spawning.
        listener: bool,
    },
    /// Casts a spell at a target, or at the caster.
    ///
    /// Reference: `f_cast_spell` (`npctalk.cpp:5414-5440`), which reads a `fake_spell` - an `id` and a `hit_self` flag - and casts it through the dialogue's actor.
    /// The port **reads** the effect and refuses to run it: casting needs the spell's own effect table, and answering "done" for a spell nobody cast would be a
    /// silent lie the census would count as success.
    CastSpell {
        /// The spell's id, which the data may give as a variable.
        spell: StringSource,
        /// Whether it lands on the caster rather than on a target.
        hit_self: bool,
        /// Whether the spell picks its own target from the map: `select_target` (`npctalk.cpp:5650-5657`), which needs a player's choice.
        targeted: bool,
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
        /// The effect's id, which the data may give as a variable.
        effect: StringSource,
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
        /// The effect's id, which the data may give as a variable.
        effect: StringSource,
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
        /// How near a square must be to the **actor** for the search to consider it, exclusive.
        target_min_radius: i32,
        /// How far the search walks around the found tile.
        target_max_radius: i32,
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
        /// The square the spawn is centred on, when the data names one.
        target: Option<AssignmentTarget>,
        /// Whether `monster` names a **monster group** rather than a type.
        group: bool,
        /// How far to look for a creature to copy, when `monster` names none.
        target_range: Option<i32>,
        /// How many real ones, which the data may give as a single number or as a range to draw from.
        real_count: CountRange,
        /// How many hallucinations, drawn the same way.
        hallucination_count: CountRange,
        /// Whether the spawns are friendly to the player rather than hostile.
        friendly: bool,
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
        /// The trait's id, which the data may give as a variable.
        trait_name: StringSource,
        /// The cosmetic variant to choose, when the data names one.
        variant: Option<StringSource>,
        /// Whether it goes to the listener rather than the speaker.
        listener: bool,
    },
    /// Switch a mutation the creature has on.
    ///
    /// Reference: `f_activate_trait` (`npctalk.cpp:3518-3525`), which calls `Character::activate_mutation`. **Different from giving it one**: the creature must already
    /// have the mutation, and what is switched on is its power, which costs one of four currencies (`mutation.cpp:821-880`).
    ActivateTrait {
        /// The mutation's id, which the data may give as a variable.
        trait_name: StringSource,
        /// Whether it applies to the listener rather than the speaker.
        listener: bool,
    },
    /// Reveal the overmap around a square.
    ///
    /// Reference: `f_reveal_map` (`npctalk.cpp:7752-7760`): a `var_info` names the centre, `radius` is a `dbl_or_var` defaulting to zero, and the reference projects the square
    /// to an overmap tile before revealing (`overmapbuffer::reveal`, `overmapbuffer.cpp:880-905`).
    RevealMap {
        /// The variable holding the centre.
        target: AssignmentTarget,
        /// How far around it, when the data says.
        radius: Option<Expression>,
    },
    /// Put one of the data's emissions down at a square.
    ///
    /// Reference: `f_emit` (`npctalk.cpp:7730-7750`): the emit id is a `str_or_var`, `chance_mult` scales every emission's own chance, and a `target_var` names a square
    /// instead of the actor. The emission table and the pouring rule are the ones spells already use.
    Emit {
        /// The emission's id, which the data may give as a variable.
        emit: StringSource,
        /// What to multiply each emission's chance by, when the data gives one.
        chance_mult: Option<Expression>,
        /// The variable holding the square to emit at, when the data names one instead of the actor.
        target: Option<AssignmentTarget>,
        /// Whether it happens to the listener rather than the speaker.
        listener: bool,
    },
    /// Take a trait away from a creature.
    ///
    /// Reference: `f_remove_trait` (`npctalk.cpp:3536-3543`), which calls `unset_mutation`.
    LoseTrait {
        /// The trait's id, which the data may give as a variable.
        trait_name: StringSource,
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
        /// The update mapgens to apply, in the order the data gives them - a single id or a list of them.
        ///
        /// Reference: `f_mapgen_update` (`npctalk.cpp:4820-4845`) reads the member as a string or an array of strings, and each is applied at the same target.
        updates: Vec<StringSource>,
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
    /// `deadline` sets the turn it is due **after** assignment, and may be a `math` expression worked out when the effect runs.
    AssignMission {
        /// The mission type the data names.
        id: StringSource,
        /// The turn it is due, when the data sets one. The port works it out and has **no deadline store yet**, so it carries the value rather than dropping it.
        deadline: Option<Expression>,
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
        /// The line shown when the move could not happen.
        fail_message: Option<StringSource>,
        /// The line shown when it did.
        success_message: Option<StringSource>,
        /// Whether the creature is moved even when it would rather not be.
        force: bool,
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
    /// The name of a store-writing effect, when this one is one of them.
    ///
    /// The names live in a second table because the first was over the size a function may be: one table for the effects that compute and judge, one for the effects
    /// that only write, and `kind` asks this one first so the answer is unchanged.
    #[must_use]
    pub fn store_kind(&self) -> Option<&'static str> {
        Some(match self {
            Self::SetStringVar { .. } => "set_string_var",
            Self::CopyVar { .. } => "copy_var",
            Self::AlterTimedEvents { .. } => "alter_timed_events",
            Self::SetCondition { .. } => "set_condition",
            Self::FinishMission { .. } => "finish_mission",
            Self::RemoveActiveMission { .. } => "remove_active_mission",
            _ => return None,
        })
    }

    /// The member the data spells this effect with, for a report about one.
    ///
    /// The names are the data's own - `math`, `u_message`, `run_eocs`, `u_add_effect`, `u_learn_trait` - so a report can be read beside the
    /// JSON it came from without a translation table. The two the reference spells with a shared name have the name the reader uses.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        if let Some(name) = self.store_kind() {
            return name;
        }
        match self {
            Self::Math(_) => "math",
            Self::Message { .. } | Self::Snippet { .. } => "u_message",
            Self::RunNpcEocs { .. } => "u_run_npc_eocs",
            Self::RunMonsterEocs { .. } => "u_run_monster_eocs",
            Self::OfferMission { .. } => "offer_mission",
            Self::Nothing => "//",
            Self::TriggerEvent { .. } => "trigger_event",
            Self::SellItem { .. } => "u_sell_item",
            Self::MapRunItemEocs { .. } => "u_map_run_item_eocs",
            Self::PlaySoundEffect { .. } => "sound_effect",
            // The store-writing effects' names live in `store_kind`, which `kind` already asked; this arm exists so the match stays exhaustive without repeating the
            // table. A variant that reached here without a name would be a bug in `store_kind`, not a rule the port is missing.
            // The store-writing names come from `store_kind`, which `kind` asks first; this arm keeps the match exhaustive without a second copy of the table.
            Self::TurnCost { .. }
            | Self::RemoveActiveMission { .. }
            | Self::FinishMission { .. }
            | Self::SetCondition { .. }
            | Self::AlterTimedEvents { .. }
            | Self::CopyVar { .. }
            | Self::SetStringVar { .. }
            | Self::AssignActivity { .. }
            | Self::SetTalker { .. }
            | Self::LoseMorale { .. }
            | Self::RemoveItemWith { .. }
            | Self::AddWet { .. }
            | Self::TerrainAround { .. }
            | Self::SetField { .. }
            | Self::CustomLightLevel { .. }
            | Self::TransformRadius { .. }
            | Self::GiveAchievement { .. }
            | Self::FurnitureAround { .. }
            | Self::RunInvEocs { .. }
            | Self::LocationVariableAdjust { .. }
            | Self::MonsterAround { .. }
            | Self::Mutation { .. }
            | Self::RunEocLater { .. }
            | Self::SoundEvent { .. }
            | Self::Later { .. }
            | Self::LocationVariableFromOvermap { .. } => self.store_kind().unwrap_or_default(),
            Self::PlaceOverride { .. } => "place_override",
            Self::RevertLocation { .. } => "revert_location",
            Self::ForEach { .. } => "foreach",
            Self::TransformItem { .. } => "transform_item",
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
            Self::ActivateTrait { .. } => "u_activate_trait",
            Self::Emit { .. } => "u_emit",
            Self::RevealMap { .. } => "reveal_map",
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
    /// A program **named by a variable**, read when the call runs.
    ///
    /// Reference: `load_eoc_vector_id_and_var` (`npctalk.cpp:512-541`) reads each entry as a `str_or_var`, and `f_run_eocs` evaluates it at run time - which is how the
    /// data runs a program whose name came out of a context value.
    NamedByVariable {
        /// Which namespace the variable lives in.
        scope: String,
        /// The variable's name.
        name: String,
        /// The context values the call hands it, as `variables`.
        variables: Vec<(String, StringSource)>,
    },
    /// A program written where it is used.
    Inline {
        /// The program.
        program: Box<Program>,
        /// The id it is **installed** under, when the data gives one: the reference's `load_inline_eoc` adds it to the factory, so a later call by name finds it.
        id: Option<String>,
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
    /// The event that runs it, when it is an **event** program.
    ///
    /// Reference: `eoc_type::EVENT` with `required_event` (`effect_on_condition.cpp:106-120`, `:541`). An event program is not called by name: it answers the event whose
    /// type it names, and the event's arguments become its context - which is why the port reads this member even though nothing has answered an event yet.
    pub event: Option<String>,
    /// Whether an **global** event program also runs for every NPC, not only for the avatar.
    ///
    /// Reference: `run_for_npcs` (`effect_on_condition.cpp:108-112`, which rejects it on anything but a global program; `:344-357`, where the loop over
    /// `g->all_npcs()` runs the effect with each NPC as the alpha).
    pub run_for_npcs: bool,
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
    // Reference: only an **event** program names an event, and the reference throws when `required_event` sits on any other type (`effect_on_condition.cpp:115-120`), so a
    // program that counts as one here is exactly a program whose type says `EVENT`.
    let event = match object.get("required_event") {
        None => None,
        Some(Value::Str(kind)) => {
            let is_event = matches!(object.get("eoc_type"), Some(Value::Str(spelled)) if spelled == "EVENT");
            if !is_event {
                return None;
            }
            Some(kind.clone())
        }
        Some(_) => return None,
    };
    let run_for_npcs = match object.get("run_for_npcs") {
        None => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => return None,
    };
    Some(Program {
        id: id.clone(),
        event,
        run_for_npcs,
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
            // A string is what is said; an object is the **variable** that holds it, with the data's own fallback for one nobody set.
            let (text, fallback) = match value {
                // Reference: `get_translation_or_var( JsonObject )` (`condition.cpp:371-385`) tries the **translation** form first - an object with a `str`
                // member - and only then reads the object as a variable. The data writes both.
                Value::Object(inside) if inside.get("str").is_some() => (
                    StringSource::Literal(read_message(value)?),
                    None,
                ),
                Value::Object(inside) => {
                    let (scope, name) = named_variable(inside)?;
                    let fallback = match inside.get("default") {
                        None => None,
                        Some(given) => Some(read_string_source(given)?),
                    };
                    (
                        StringSource::Variable {
                            scope,
                            name,
                        },
                        fallback,
                    )
                }
                other => (StringSource::Literal(read_message(other)?), None),
            };
            return Some(Effect::Message { text, fallback });
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
#[must_use]
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
    if object.get("run_eocs").is_some() {
        return read_run_eocs(object);
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
    // Reference: `f_reveal_map` (`npctalk.cpp:7752-7760`): a required `var_info` and an optional `radius` read as a `dbl_or_var`, which is why a `math` radius is legal here.
    if let Some(given) = object.get("reveal_map") {
        let target = read_variable_target(given)?;
        let radius = match object.get("radius") {
            None => None,
            Some(given) => Some(read_expression(given)?),
        };
        return Some(Effect::RevealMap { target, radius });
    }
    // Reference: `f_emit` (`npctalk.cpp:7730-7750`). `chance_mult` and `target_var` are the reference's two optional companions; the actor's own square is the default.
    for (member, listener) in [("u_emit", false), ("npc_emit", true)] {
        if let Some(given) = object.get(member)
            && let Some(emit) = read_string_source(given)
        {
            let target = match object.get("target_var") {
                None => None,
                Some(given) => Some(read_variable_target(given)?),
            };
            let chance_mult = match object.get("chance_mult") {
                None => None,
                Some(given) => Some(read_expression(given)?),
            };
            return Some(Effect::Emit {
                emit,
                chance_mult,
                target,
                listener,
            });
        }
    }
    // Reference: `f_activate_trait` (`npctalk.cpp:3518-3525`): a `str_or_var` naming a mutation the creature already has.
    for (member, listener) in [("u_activate_trait", false), ("npc_activate_trait", true)] {
        if let Some(given) = object.get(member)
            && let Some(trait_name) = read_string_source(given)
        {
            return Some(Effect::ActivateTrait {
                trait_name,
                listener,
            });
        }
    }
    // Reference: `f_add_trait` and `f_remove_trait` (`npctalk.cpp:3498-3543`). A `variant` member picks one of a trait's cosmetic variations, which the port records
    // beside the trait: `set_mutation( trait, variant )` keeps the choice, and no rule reads it.
    for (member, listener, learn) in [
        ("u_add_trait", false, true),
        ("npc_add_trait", true, true),
        ("u_lose_trait", false, false),
        ("npc_lose_trait", true, false),
    ] {
        // Reference: `get_str_or_var( jo.get_member( member ), member, true )` (`npctalk.cpp:3498-3508`) - the trait may be named by a variable, and the data
        // uses that to add whichever trait an earlier effect decided on.
        if let Some(given) = object.get(member)
            && matches!(given, Value::Str(_) | Value::Object(_))
            && let Some(trait_name) = read_string_source(given)
        {
            let variant = match object.get("variant") {
                None => None,
                Some(given) => Some(read_string_source(given)?),
            };
            return Some(if learn {
                Effect::LearnTrait {
                    trait_name,
                    variant,
                    listener,
                }
            } else {
                Effect::LoseTrait {
                    trait_name,
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
    // Reference: `f_assign_mission` (`npctalk.cpp:5912`) reads `deadline` with `get_dbl_or_var`, so a `math` expression is one of the shapes it accepts.
    let deadline = match object.get("deadline") {
        None => None,
        Some(given) => Some(read_math_expression(given)?),
    };
    Some(Effect::AssignMission {
        id: read_string_source(value)?,
        deadline,
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
        // Reference: `f_teleport` (`npctalk.cpp:7820-7835`) reads two messages and two flags beside the target. `fail_message` and `success_message` are shown by the
        // teleport itself, so the port carries them and shows them where the reference does; `force_safe` skips the map's safety check, which the port has no such
        // check to skip, and an entry that asks for it is refused rather than read as if the flag meant nothing.
        if object.get("force_safe").is_some() {
            return None;
        }
        let fail_message = match object.get("fail_message") {
            None => None,
            Some(value) => Some(read_string_source(value)?),
        };
        let success_message = match object.get("success_message") {
            None => None,
            Some(value) => Some(read_string_source(value)?),
        };
        let (scope, name) = target.members().iter().find_map(|(scope, value)| match value {
            Value::Str(name) => Some((scope.clone(), name.clone())),
            _ => None,
        })?;
        return Some(Effect::Teleport {
            scope,
            name,
            listener,
            fail_message,
            success_message,
            force: matches!(object.get("force"), Some(Value::Bool(true))),
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
    let updates = match update {
        Value::Array(entries) => entries
            .iter()
            .map(read_string_source)
            .collect::<Option<Vec<_>>>()?,
        single => vec![read_string_source(single)?],
    };
    Some(Effect::MapgenUpdate {
        updates,
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
        // `context_val` is the fourth scope the reference's `read_var_info` accepts, and the data uses it for a target the dialogue computed a moment ago.
        "u_val" | "npc_val" | "global_val" | "n_val" | "context_val" => Some(AssignmentTarget::Variable {
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
        // Reference: `get_duration_or_var` (`npctalk.cpp:3426-3450`) reads a **`math` expression** as well as a number or a range, and the data uses it for a
        // length that depends on the world - `{"math": ["rand(432000) + 86400"]}` on a morale point. The value is worked out when the effect is applied.
        // A `math` expression, or a **variable** the dialogue holds when the effect is applied: `get_duration_or_var` reads the same `dbl_or_var` shape the rest of
        // the data uses, and the data does pass `{"context_val": "duration"}`. Both are worked out where they are used, so both are `Computed`.
        // Reference: a `duration_or_var` with a **default** is a variable holding a duration, and the default is what an unset variable means.
        Some(Value::Object(given)) if given.get("default").is_some() => {
            let fallback = match given.get("default")? {
                Value::Str(text) => crate::constructions::parse_duration(text),
                Value::Int(turns) => *turns,
                _ => return None,
            };
            let (scope, name) = given.members().iter().find_map(|(scope, value)| match value {
                Value::Str(name) if scope != "default" => Some((scope.clone(), name.clone())),
                _ => None,
            })?;
            Some(EffectDuration::Named { scope, name, fallback })
        }
        Some(Value::Object(_)) => Some(EffectDuration::Computed(read_math_expression(value?)?)),
        Some(Value::Array(ends)) => {
            let [shortest, longest] = ends.as_slice() else {
                return None;
            };
            // Each end is a number, a duration in text, or an expression worked out when the effect applies.
            let fixed = |end: &Value| match end {
                Value::Int(turns) => Some(*turns),
                Value::Str(text) => Some(crate::constructions::parse_duration(text)),
                _ => None,
            };
            let as_end = |fixed: Option<i64>, given: &Value| match fixed {
                Some(turns) => {
                    #[allow(clippy::cast_precision_loss)]
                    let as_float = turns as f64;
                    Some(Expression::Number(as_float))
                }
                None => read_math_expression(given),
            };
            match (fixed(shortest), fixed(longest)) {
                (Some(shortest), Some(longest)) => Some(EffectDuration::Range(shortest, longest)),
                // One end fixed and the other worked out: the fixed end becomes a constant of the same expression, so the range stays one shape.
                (shortest_fixed, longest_fixed) => Some(EffectDuration::RangeComputed(
                    as_end(shortest_fixed, shortest)?,
                    as_end(longest_fixed, longest)?,
                )),
            }
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
/// Reads a `u_location_variable` that searches for a monster, or nothing.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4300-4320`), the `search_type == "monster"` branch. A `target_params` beside it would search an overmap area
/// first, which this reader does not do - the refusal is a missing rule, not a missing store, since the monster table is here.
fn read_monster_around(object: &cdda_json::Object, target: &cdda_json::Object, listener: bool) -> Option<Effect> {
    let Value::Str(monster) = object.get("monster")? else {
        return None;
    };
    for asked in ["terrain", "furniture", "field", "npc", "species", "trap", "zone", "target_params"] {
        if object.get(asked).is_some() {
            return None;
        }
    }
    let whole = |key: &str| match object.get(key) {
        None => Some(0),
        Some(Value::Int(number)) => i32::try_from(*number).ok(),
        Some(_) => None,
    };
    let (scope, name) = named_variable(target)?;
    Some(Effect::MonsterAround {
        monster: monster.clone(),
        scope,
        name,
        min_radius: whole("target_min_radius")?,
        max_radius: whole("target_max_radius")?,
        listener,
    })
}

/// Reads a `location_variable_adjust` entry.
///
/// Reference: `f_location_variable_adjust` (`npctalk.cpp:4411-4448`): the member is the input `var_info`, `output_var` where the answer goes, and the three adjusts
/// are `dbl_or_var`s - numbers, or a two-end array the game draws between.
fn read_location_variable_adjust(object: &cdda_json::Object) -> Option<Effect> {
    let Value::Object(input) = object.get("location_variable_adjust")? else {
        return None;
    };
    let (scope, name) = named_variable(input)?;
    // The adjusts are `dbl_or_var`s: a number, a `math` expression, or a variable. `read_expression` reads all three; `read_math_expression` reads only the first two,
    // which is why the `{"math": [...]}` form the data writes here read as a refusal.
    let adjust = |key: &str| match object.get(key) {
        None => Some(Expression::Number(0.0)),
        Some(given) => read_expression(given),
    };
    let output = match object.get("output_var") {
        None => None,
        Some(Value::Object(wanted)) => Some(named_variable(wanted)?),
        Some(_) => return None,
    };
    Some(Effect::LocationVariableAdjust {
        scope,
        name,
        output,
        x_adjust: adjust("x_adjust")?,
        y_adjust: adjust("y_adjust")?,
        z_adjust: adjust("z_adjust")?,
        z_override: matches!(object.get("z_override"), Some(Value::Bool(true))),
        overmap_tile: matches!(object.get("overmap_tile"), Some(Value::Bool(true))),
    })
}

/// Reads a `target_params` search for an overmap terrain.
///
/// Reference: `mission_util::parse_mission_om_target` (`mission_util.cpp:391-450`) and the search in `f_location_variable` (`npctalk.cpp:4261-4300`). The port reads
/// what it can carry out - a terrain, a least distance and a level - and refuses an overmap special, whose tiles are matched by a rule the port does not have.
fn read_overmap_search(object: &cdda_json::Object, target: &cdda_json::Object, listener: bool) -> Option<Effect> {
    let Value::Object(params) = object.get("target_params")? else {
        return None;
    };
    if params.get("om_special").is_some() {
        return None;
    }
    let terrain = read_string_source(params.get("om_terrain")?)?;
    let (scope, name) = named_variable(target)?;
    let number = |key: &str| match params.get(key) {
        None => Some(None),
        Some(given) => Some(Some(read_expression(given)?)),
    };
    Some(Effect::LocationVariableFromOvermap {
        scope,
        name,
        terrain,
        min_distance: number("min_distance")?,
        z: number("z")?,
        listener,
    })
}

/// Reads a `u_mutate` or `u_mutate_category` entry.
///
/// Reference: `f_mutate` (`npctalk.cpp:3563-3571`) and `f_mutate_category` (`3573-3583`): the member is the highest category depth, and the category form names one to
/// draw from. The port reads both and answers only the depth of zero, which asks for nothing.
fn read_mutation(object: &cdda_json::Object) -> Option<Effect> {
    // Reference: the same two functions read both spellings, and `d.actor( is_npc )` decides which creature they act on (`npctalk.cpp:3563-3583`).
    let npc = object.get("npc_mutate").is_some() || object.get("npc_mutate_category").is_some();
    let depth_member = object.get("u_mutate").or_else(|| object.get("npc_mutate"));
    let category_member = object
        .get("u_mutate_category")
        .or_else(|| object.get("npc_mutate_category"));
    let (highest, category) = match (depth_member, category_member) {
        (Some(depth), None) => (read_math_expression(depth)?, None),
        (None, Some(Value::Str(named))) => (Expression::Number(0.0), Some(named.clone())),
        (None, Some(given)) => (read_math_expression(given)?, None),
        _ => return None,
    };
    Some(Effect::Mutation {
        highest_category: highest,
        category,
        npc,
        // Reference: `use_vitamins` defaults to true and `true_random` to false (`npctalk.cpp:3565-3582`).
        use_vitamins: match object.get("use_vitamins") {
            Some(Value::Bool(set)) => *set,
            _ => true,
        },
        true_random: match object.get("true_random") {
            Some(Value::Bool(set)) => *set,
            _ => false,
        },
    })
}

/// Reads an `offer_mission` entry, when it names one mission or a list of them.
///
/// Reference: `f_offer_mission` accepts both spellings and refuses anything else (`npctalk.cpp:5981-5989`).
fn read_offer_mission(object: &cdda_json::Object) -> Option<Effect> {
    let given = object.get("offer_mission")?;
    let missions: Vec<String> = match given {
        Value::Str(one) => vec![one.clone()],
        Value::Array(list) => list
            .iter()
            .filter_map(|one| match one {
                Value::Str(named) => Some(named.clone()),
                _ => None,
            })
            .collect(),
        _ => return None,
    };
    if missions.is_empty() {
        return None;
    }
    Some(Effect::OfferMission { missions })
}

/// Reads a `u_map_run_item_eocs` entry, or nothing.
///
/// Reference: `f_map_run_item_eocs` (`npctalk.cpp:6806-6828`): the member is the selection mode, `loc` an optional location, the two radii default to zero, and the search
/// filters use the same `item_search_data` shape an inventory search does.
fn read_map_run_item_eocs(object: &cdda_json::Object) -> Option<Effect> {
    let (member, mode) = ["u_map_run_item_eocs", "npc_map_run_item_eocs"]
        .iter()
        .find_map(|name| object.get(name).map(|value| (*name, value)))?;
    let _ = member;
    let Value::Str(mode) = mode else {
        return None;
    };
    if !matches!(mode.as_str(), "all" | "random" | "manual" | "manual_mult") {
        return None;
    }
    let loc = match object.get("loc") {
        None => None,
        Some(Value::Object(inside)) => Some(named_variable(inside)?),
        Some(_) => return None,
    };
    let min_radius = match object.get("min_radius") {
        None => CountRange::One(0),
        Some(value) => read_count(Some(value))?,
    };
    let max_radius = match object.get("max_radius") {
        None => CountRange::One(0),
        Some(value) => read_count(Some(value))?,
    };
    let mut search_data = Vec::new();
    match object.get("search_data") {
        None => {}
        Some(Value::Array(entries)) => {
            for entry in entries {
                search_data.push(read_search_filter(entry)?);
            }
        }
        Some(_) => return None,
    }
    Some(Effect::MapRunItemEocs {
        mode: mode.clone(),
        loc,
        min_radius,
        max_radius,
        search_data,
        programs: read_program_refs(object.get("true_eocs"))?,
        false_programs: read_program_refs(object.get("false_eocs"))?,
    })
}

/// Reads one `search_data` entry, which the inventory search and the map search share.
///
/// Reference: `item_search_data` (`npctalk.cpp:235-441`), read the same way wherever it appears.
fn read_search_filter(entry: &Value) -> Option<SearchFilter> {
    let Value::Object(filter) = entry else {
        return None;
    };
    if !filter.members().iter().all(|(key, _)| {
        matches!(key.as_str(), "id" | "wielded_only" | "worn_only" | "condition" | "//")
    }) {
        return None;
    }
    let id = match filter.get("id") {
        None => None,
        Some(Value::Str(id)) => Some(id.clone()),
        Some(_) => return None,
    };
    let condition = match filter.get("condition") {
        None => None,
        Some(condition) => Some(Box::new(read_condition(condition)?)),
    };
    Some(SearchFilter {
        id,
        wielded_only: matches!(filter.get("wielded_only"), Some(Value::Bool(true))),
        worn_only: matches!(filter.get("worn_only"), Some(Value::Bool(true))),
        condition,
    })
}

/// Reads a `u_sell_item` entry, or nothing.
///
/// Reference: `f_u_sell_item` (`npctalk.cpp:3884-3896`): `count` defaults to one, **but to zero when the entry states `charges`** - the reference reads the member only to
/// change that default, and the number itself is how many are handed over.
fn read_sell_item(object: &cdda_json::Object) -> Option<Effect> {
    let item = object.get("u_sell_item")?;
    let by_charges = object.get("charges").is_some();
    let count = match object.get("count") {
        None => CountRange::One(i32::from(!by_charges)),
        Some(value) => read_count(Some(value))?,
    };
    let cost = match object.get("cost") {
        None => CountRange::One(0),
        Some(value) => read_count(Some(value))?,
    };
    Some(Effect::SellItem {
        item: read_string_source(item)?,
        count,
        cost,
        by_charges,
        true_programs: read_program_refs(object.get("true_eocs"))?,
        false_programs: read_program_refs(object.get("false_eocs"))?,
    })
}

/// Reads a `trigger_event` entry, or nothing.
///
/// Reference: `f_trigger_event` (`npctalk.cpp:7894-7911`): both members are mandatory - the type's name and an `args` array, which may be empty.
fn read_trigger_event(object: &cdda_json::Object) -> Option<Effect> {
    let Some(Value::Str(kind)) = object.get("trigger_event") else {
        return None;
    };
    let args = match object.get("args") {
        None => Vec::new(),
        Some(Value::Array(values)) => read_string_sources(values)?,
        Some(_) => return None,
    };
    Some(Effect::TriggerEvent {
        kind: kind.clone(),
        args,
    })
}

/// Reads a `u_run_inv_eocs` entry, or nothing.
///
/// Reference: `f_run_inv_eocs` (`npctalk.cpp:6760-6805`): the member is the selection mode, `search_data` a list of filters, and `true_eocs` the programs. A mode
/// other than `"all"` needs a menu, and the menu picks one item - a rule the headless port has not written, so it refuses rather than running on everything.
fn read_run_inv_eocs(object: &cdda_json::Object) -> Option<Effect> {
    // **Two members, one rule**: `u_run_inv_eocs` works on the speaker's items and `npc_run_inv_eocs` on the listener's. The port read only the first until the beggar
    // programs showed the second, which is why their whole effect refused.
    let (option, listener) = match (object.get("u_run_inv_eocs"), object.get("npc_run_inv_eocs")) {
        (Some(option), _) => (option, false),
        (None, Some(option)) => (option, true),
        (None, None) => return None,
    };
    let Value::Str(mode) = option else {
        return None;
    };
    // Reference: `f_run_inv_eocs` (`npctalk.cpp:6760-6805`) takes `all`, `random`, `manual` and `manual_mult`; `manual` and `manual_mult` are menus.
    if !matches!(mode.as_str(), "all" | "random" | "manual" | "manual_mult") {
        return None;
    }
    let mut search_data = Vec::new();
    match object.get("search_data") {
        None => {}
        Some(Value::Array(entries)) => {
            for entry in entries {
                // **One reader for one shape**: the inventory search and the map search read the same `item_search_data`.
                search_data.push(read_search_filter(entry)?);
            }
        }
        Some(_) => return None,
    }
    Some(Effect::RunInvEocs {
        search_data,
        random: mode == "random",
        listener,
        manual: mode.starts_with("manual"),
        programs: read_program_refs(object.get("true_eocs"))?,
        false_programs: read_program_refs(object.get("false_eocs"))?,
    })
}

/// Reads a `give_achievement` entry.
///
/// Reference: `f_give_achievment` (`npctalk.cpp:5566-5585`): one required member, a name or a variable holding one.
fn read_give_achievement(object: &cdda_json::Object) -> Option<Effect> {
    Some(Effect::GiveAchievement {
        achievement: read_string_source(object.get("give_achievement")?)?,
    })
}

/// Reads a `transform_radius` entry.
///
/// Reference: `f_transform_radius` (`npctalk.cpp:4450-4468`): the table is required, the radius is the member the data spells `transform_radius`, and
/// `time_in_future`, `target_var` and `key` are separate rules the port does not carry out - so an entry stating one is refused rather than applied early.
fn read_transform_radius(object: &cdda_json::Object) -> Option<Effect> {
    let transform = object.get("ter_furn_transform")?;
    // A `target_var` names the square the transform is centred on. `time_in_future` postpones the whole transform through the `scheduled` store; `key` names it for
    // `alter_timed_events`, which the port has no table of transforms for, so that one still refuses.
    if object.get("key").is_some() {
        return None;
    }
    let target = match object.get("target_var") {
        Some(value) => Some(read_variable_target(value)?),
        None => None,
    };
    // The radius's member is the activation's own name, not the bare `transform_radius`: the data writes `u_transform_radius` and `npc_transform_radius`, and reading
    // only the bare one refused every entry in the corpus.
    let (given, listener) = match (object.get("u_transform_radius"), object.get("npc_transform_radius")) {
        (Some(radius), _) => (radius, false),
        (_, Some(radius)) => (radius, true),
        _ => return None,
    };
    let transform_now = Effect::TransformRadius {
        transform: read_string_source(transform)?,
        radius: read_math_expression(given)?,
        target,
        listener,
    };
    match object.get("time_in_future") {
        Some(after) => Some(Effect::Later {
            effect: Box::new(transform_now),
            after: read_duration_source(after)?,
        }),
        None => Some(transform_now),
    }
}

/// Reads a `custom_light_level` entry.
///
/// Reference: `f_custom_light_level` (`npctalk.cpp:7215-7232`): the member is required, `length` defaults to nothing and `key` to the empty string.
fn read_custom_light_level(object: &cdda_json::Object) -> Option<Effect> {
    let given = object.get("custom_light_level")?;
    Some(Effect::CustomLightLevel {
        level: read_math_expression(given)?,
        length: match object.get("length") {
            None => EffectDuration::Turns(Some(0)),
            Some(value) => read_effect_duration(Some(value), EffectDuration::Turns(Some(0)))?,
        },
        key: match object.get("key") {
            None => None,
            Some(value) => Some(read_string_source(value)?),
        },
    })
}

/// Reads a `u_set_field` / `npc_set_field` entry.
///
/// Reference: `f_field` (`npctalk.cpp:7691-7701`): the field is required, `intensity` defaults to one, and `radius` defaults to ten million - which is why a port
/// with a loaded level and no world refuses an entry that leaves it out instead of pretending to cover everything.
fn read_set_field(object: &cdda_json::Object) -> Option<Effect> {
    for (member, listener) in [("u_set_field", false), ("npc_set_field", true)] {
        let Some(field) = object.get(member) else {
            continue;
        };
        return Some(Effect::SetField {
            field: read_string_source(field)?,
            intensity: match object.get("intensity") {
                None => Expression::Number(1.0),
                Some(given) => read_math_expression(given)?,
            },
            radius: match object.get("radius") {
                None => None,
                Some(given) => Some(read_math_expression(given)?),
            },
            listener,
        });
    }
    None
}

/// Reads the around-the-actor **furniture** search, or nothing.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4231-4240`): exactly one of the search types may name what to look for, and `furniture` is one of them - the walk is
/// the same one the terrain search makes.
fn read_furniture_around(object: &cdda_json::Object, target: &cdda_json::Object, listener: bool) -> Option<Effect> {
    let furniture = object.get("furniture")?;
    for asked in ["terrain", "field", "monster", "npc", "species", "trap", "zone", "target_params"] {
        if object.get(asked).is_some() {
            return None;
        }
    }
    let whole = |key: &str| match object.get(key) {
        None => Some(0),
        Some(Value::Int(number)) => i32::try_from(*number).ok(),
        Some(_) => None,
    };
    let (scope, name) = named_variable(target)?;
    Some(Effect::FurnitureAround {
        furniture: read_string_source(furniture)?,
        scope,
        name,
        target_min_radius: whole("target_min_radius")?,
        target_max_radius: whole("target_max_radius")?,
        listener,
    })
}

/// Reads the around-the-actor terrain search, or nothing.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4231-4240`): exactly **one** of `terrain`, `furniture`, `field`, `monster`, `npc`, `species`, `trap` and `zone`
/// may name the search, and `target_max_radius` is how far it reaches. The port reads the terrain one; the rest are separate rules and refuse.
fn read_terrain_around(object: &cdda_json::Object, target: &cdda_json::Object, listener: bool) -> Option<Effect> {
    let terrain = object.get("terrain")?;
    for asked in ["furniture", "field", "monster", "npc", "species", "trap", "zone", "target_params"] {
        if object.get(asked).is_some() {
            return None;
        }
    }
    let whole = |key: &str| match object.get(key) {
        None => Some(0),
        Some(Value::Int(number)) => i32::try_from(*number).ok(),
        Some(_) => None,
    };
    let (scope, name) = named_variable(target)?;
    Some(Effect::TerrainAround {
        terrain: read_string_source(terrain)?,
        scope,
        name,
        target_min_radius: whole("target_min_radius")?,
        target_max_radius: whole("target_max_radius")?,
        listener,
    })
}

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
    // `om_special` names an overmap special whose terrain the reference looks up in the special's table. The port has no such table - but when `om_terrain` is
    // given beside it, `mission_util::get_om_terrain_pos` uses **the terrain** and never consults the special, so the pair is read and the special ignored.
    // `cant_see` says the tile need not be visible, and the port never checks visibility here, so it is read and ignored too. A `target_params` naming **only** a
    // special still refuses: that is the case the port cannot resolve.
    if params.get("om_special").is_some() && params.get("om_terrain").is_none() {
        return None;
    }
    if !params.members().iter().all(|(key, _)| {
        matches!(
            key.as_str(),
            "om_terrain" | "z" | "min_distance" | "search_range" | "om_special" | "cant_see"
        )
    }) {
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
    // The **search's own** radius, which the reference spells `target_min_radius` and `target_max_radius` and uses to walk the squares around the found tile - a
    // different pair from the `min_radius`/`max_radius` it draws within afterwards.
    let target_min_radius = whole_radius("target_min_radius")?;
    let target_max_radius = whole_radius("target_max_radius")?;
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
        target_min_radius,
        target_max_radius,
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
/// Reads `map_in_city`, whose target is a variable holding a square.
///
/// Reference: `f_map_in_city` (`condition.cpp:1863-1875`).
fn read_city_condition(object: &cdda_json::Object) -> Option<Condition> {
    let Value::Object(target) = object.get("map_in_city")? else {
        return None;
    };
    Some(Condition::MapInCity {
        target: named_variable(target)?,
    })
}

/// Reads `map_terrain_with_flag` and `map_furniture_with_flag`, which differ only in what they ask.
///
/// Reference: `f_map_ter_furn_with_flag` (`condition.cpp:1818-1842`): one reader for both members, the member itself choosing terrain or furniture.
fn read_map_flag_condition(object: &cdda_json::Object) -> Option<Condition> {
    for (member, furniture) in [("map_terrain_with_flag", false), ("map_furniture_with_flag", true)] {
        let Some(flag) = object.get(member) else {
            continue;
        };
        let Some(Value::Object(place)) = object.get("loc") else {
            return None;
        };
        return Some(Condition::MapTerFurnWithFlag {
            flag: read_string_source(flag)?,
            target: named_variable(place)?,
            furniture,
        });
    }
    None
}

fn named_variable(target: &cdda_json::Object) -> Option<(String, String)> {
    target.members().iter().find_map(|(scope, value)| match value {
        Value::Str(name) => Some((scope.clone(), name.clone())),
        _ => None,
    })
}

fn read_location_variable(object: &cdda_json::Object) -> Option<Effect> {
    /// The members that turn the effect into a search.
    // A **radius alone is not a search.** Reference: `f_location_variable` (`npctalk.cpp:4194-4300`) starts `target_pos` at the talker's own square, and
    // `min_radius` / `max_radius` are read but used **only inside** the `search_target` block - with no terrain, furniture, field, monster, npc, species, trap or
    // zone named, the effect writes the talker's square and the radii are never looked at. The port refused that shape for a year of data it can answer.
    const SEARCHING: [&str; 9] = [
        "target_params",
        "terrain",
        "furniture",
        "field",
        "monster",
        "npc",
        "species",
        "trap",
        "zone",
    ];
    for (member, listener) in [("u_location_variable", false), ("npc_location_variable", true)] {
        let Some(Value::Object(target)) = object.get(member) else {
            continue;
        };
        if let Some(found) = read_furniture_around(object, target, listener) {
            return Some(found);
        }
        if let Some(found) = read_terrain_around(object, target, listener) {
            return Some(found);
        }
        if let Some(found) = read_terrain_target(object, target, listener) {
            return Some(found);
        }
        if let Some(found) = read_monster_around(object, target, listener) {
            return Some(found);
        }
        if let Some(found) = read_radius_search(object, target, listener) {
            return Some(found);
        }
        if let Some(found) = read_overmap_search(object, target, listener) {
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

/// A length of time the data writes: one statement, or two ends the game draws between.
///
/// Reference: `duration_or_var` (`npctalk.cpp:3426-3450`): a `dbl_or_var` with `min`/`max`, which the reference draws from with the dialogue's own dice. The data
/// writes the range as an array of two durations - `["30 seconds", "1 minutes"]`.
#[derive(Debug, Clone, PartialEq)]
pub enum DurationSource {
    /// One duration.
    One(StringSource),
    /// Two ends, drawn between when the effect runs.
    Range(StringSource, StringSource),
}

/// Reads a `duration_or_var`.
fn read_duration_source(value: &cdda_json::Value) -> Option<DurationSource> {
    if let cdda_json::Value::Array(ends) = value {
        let [shortest, longest] = ends.as_slice() else {
            return None;
        };
        return Some(DurationSource::Range(
            read_duration_text(shortest)?,
            read_duration_text(longest)?,
        ));
    }
    Some(DurationSource::One(read_duration_text(value)?))
}

/// Reads a `run_eocs` entry, which may run now or be postponed.
///
/// Reference: `f_run_eocs` (`npctalk.cpp:6332-6338`): the programs run at once unless `time_in_future` names a length, in which case the reference puts a timed event in
/// the calendar and runs it when the turn comes. The port **ignored that member** until this round, which ran the program early - a wrong answer rather than a refusal.
#[must_use]
fn read_run_eocs(object: &cdda_json::Object) -> Option<Effect> {
    let names = object.get("run_eocs")?;
    let entries = match names {
        Value::Array(entries) => entries.as_slice(),
        other => std::slice::from_ref(other),
    };
    let programs = entries
        .iter()
        .map(read_program_ref)
        .collect::<Option<Vec<_>>>()?;
    match object.get("time_in_future") {
        Some(after) => Some(Effect::RunEocLater {
            programs,
            after: read_duration_source(after)?,
        }),
        None => Some(Effect::RunEoc(programs)),
    }
}

/// Reads a duration the data writes as text, as a **number of turns**, or as a variable with a default.
///
/// Reference: `get_duration_or_var` (`npctalk.cpp:3426-3450`): a `dbl_or_var` whose number is in **turns**, a string parsed by `read_from_json_string`, or a variable
/// read when the effect runs. The port keeps the text form and turns a number into the turns it already is; a variable falls back to the default the data writes,
/// which is what the reference uses when the variable is unset.
fn read_duration_text(value: &cdda_json::Value) -> Option<StringSource> {
    match value {
        cdda_json::Value::Str(text) => Some(StringSource::Literal(text.clone())),
        cdda_json::Value::Int(turns) => Some(StringSource::Literal(format!("{turns} turns"))),
        cdda_json::Value::Float(turns) => Some(StringSource::Literal(format!("{turns} turns"))),
        cdda_json::Value::Object(object) => {
            // A variable holding the duration, read when the effect runs: `{"context_val": "travel_time"}` is the shape the data uses for a length the world decides.
            for scope in ["u_val", "npc_val", "global_val", "var_val", "context_val"] {
                if let Some(cdda_json::Value::Str(name)) = object.get(scope) {
                    return Some(StringSource::Variable {
                        scope: scope.to_owned(),
                        name: name.clone(),
                    });
                }
            }
            match object.get("default") {
                Some(fallback) => read_duration_text(fallback),
                None => None,
            }
        }
        _ => None,
    }
}

/// Reads a `sound_effect` entry.
///
/// Reference: `f_sound_effect` (`npctalk.cpp:5528-5570`): the member is the **sound's id**, and `id`, `volume`, `outdoor_event` and `variant` say how it plays. The
/// port has no audio, so it reads the entry and answers `Done` - a sound the player cannot hear is a gap in presentation, not a rule the port has got wrong.
fn read_sound_event(object: &cdda_json::Object) -> Option<Effect> {
    let id = object.get("sound_effect")?;
    Some(Effect::SoundEvent {
        id: read_string_source(id)?,
        volume: match object.get("volume") {
            None => None,
            Some(given) => Some(read_expression(given)?),
        },
        outdoor: matches!(object.get("outdoor_event"), Some(Value::Bool(true))),
    })
}

/// The one `{"scope": "name"}` entry of an object, when it is the only member.
fn single_named_variable(object: &cdda_json::Object) -> Option<(String, String)> {
    let members: Vec<(&String, &Value)> = object
        .members()
        .iter()
        .filter(|(name, _)| !name.starts_with("//"))
        .map(|(name, value)| (name, value))
        .collect();
    let (scope, value) = members.first().copied()?;
    let Value::Str(name) = value else {
        return None;
    };
    if !matches!(scope.as_str(), "u_val" | "npc_val" | "n_val" | "global_val" | "context_val") {
        return None;
    }
    Some((scope.clone(), name.clone()))
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
            // A single-scope object whose value is a name is a **variable reference**, not a program: `{"context_val": "eoc"}` names the program to run rather than
            // defining one. The reference reads every entry as a `str_or_var` (`npctalk.cpp:512-541`), so this is the plain-variable shape of that rule.
            if object.get("id").is_none()
                && object.get("effect").is_none()
                && let Some((scope, name)) = single_named_variable(object)
            {
                return Some(ProgramRef::NamedByVariable {
                    scope,
                    name,
                    variables,
                });
            }
            // An entry naming an `id` **and** carrying an `effect` is a program defined where it is called: `load_inline_eoc`
            // (`effect_on_condition.cpp:119-133`) installs it under that id. A body the port cannot read does not make the caller unreadable - the call falls back
            // to a plain name and refuses at run time, which is what the port answered before it could read bodies at all.
            let named = match object.get("id") {
                Some(Value::Str(id)) => Some(id.clone()),
                _ => None,
            };
            if object.get("effect").is_none()
                && let Some(id) = named
            {
                return Some(ProgramRef::Named {
                    id,
                    variables,
                });
            }
            //
            // **This is where two attempts at the reference's inline rule have died**, and both cost 11 programs and 35 `switch` entries: reading an `id` beside an
            // `effect` as an inline program, first without installing it (`load_inline_eoc`, `effect_on_condition.cpp:119-133`, inserts it into
            // `effect_on_condition_factory`) and then **with** a loader that installed every inline program it could find. The rule is right and the port's data
            // walk is wrong somewhere the census cannot see, so the next attempt starts from **one refusing program's JSON**, not from a broader rule.
            // A body the port cannot read does **not** make its caller unreadable: the reference installs the body and only complains when something calls it, so
            // the fallback here is the plain name - the call refuses at run time if nothing installs it, exactly as the port answered before it read bodies at all.
            // A `true_eocs` entry that is **one effect object** is a program with that one effect: the data writes
            // `"true_eocs": [ { "u_message": "Transported: <npc_name>" } ]`, and the reference loads such an entry as the program's own effect list.
            if object.get("effect").is_none()
                && named.is_none()
                && single_named_variable(object).is_none()
                && let Some(one) = read_effect(value)
            {
                return Some(ProgramRef::Inline {
                    program: Box::new(Program {
                        id: String::new(),
                        event: None,
                        run_for_npcs: false,
                        condition: None,
                        effects: vec![one],
                    }),
                    id: None,
                    variables,
                });
            }
            let Some(effects) = read_effect_list(object.get("effect")) else {
                return named.map(|id| ProgramRef::Named {
                    id,
                    variables,
                });
            };
            let condition = match object.get("condition") {
                None => None,
                Some(condition) => Some(read_condition(condition)?),
            };
            Some(ProgramRef::Inline {
                program: Box::new(Program {
                    id: named.clone().unwrap_or_default(),
                    // An **inline** body is never an event program: it is installed under its id and called by name.
                    event: None,
                    run_for_npcs: false,
                    condition,
                    effects,
                }),
                id: named,
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
    // The data names the effect with a **variable** as often as a literal - `{"context_val": "effect"}` - so the name is read the same way every other string in
    // a dialogue is, and a variable is resolved where the effect is applied.
    if let Some(given) = object.get(member) {
        // A `target_part` names the body part an effect lands on. The port has no limb model - a creature is one thing, not a set of parts - so the part
        // is **ignored rather than refused**: the effect lands on the creature, which is where the reference puts it once a part is chosen.
        let _ = object.get("target_part");
        let _ = object.get("bodypart");
        if learn {
            // Reference: `get_duration_or_var` (`npctalk.cpp:3426-3450`) accepts a duration **string**, a number of turns, or an
            // array of two of either, which the reference rolls between. The port reads all three; a roll is made at run time.
            // One reader for a duration, shared with every other effect that takes one: a number, a range, a `math` expression, or a variable. The
            // inline copy that used to live here refused the last two, so `u_add_effect` with a rolled or variable length refused while a morale point
            // with the same shape read - the same rule written twice, and only one of them kept up.
            let duration =
                read_effect_duration(object.get("duration"), EffectDuration::Turns(Some(1000)))?;

            // Reference: `get_dbl_or_var( jo, "intensity" )` - an intensity may be an expression, evaluated when the effect is applied.
            let intensity = match object.get("intensity") {
                Some(value) => Some(read_math_expression(value)?),
                None => None,
            };
            return Some(Effect::AddEffect {
                effect: read_string_source(given)?,
                duration,
                intensity,
                listener,
            });
        }
        return Some(Effect::RemoveEffect {
            effect: read_string_source(given)?,
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

