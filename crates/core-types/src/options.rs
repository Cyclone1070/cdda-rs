//! The option table and the reference's get/set rules.
//!
//! Every option in the reference is registered with an `add( ... )` call in
//! `references/Cataclysm-DDA/src/options.cpp`, or comes from an `EXTERNAL_OPTION`
//! entry in `data/core/external_options.json`, which `load_external_option()`
//! turns into an `add_external()` call (`src/worldfactory.cpp:2060-2095`). The
//! table below is not typed by hand: `tools/gen_options.py` parses both sources
//! into `corpus/options/table.json`, and [`Table::parse`] reads that file.
//!
//! Only the parts of `options_manager` that affect values are ported: the type
//! of each option, the value it starts with, the value `setValue()` falls back to
//! when it rejects an input, and the per-type validation rules.

use std::collections::BTreeMap;
use std::fmt;

/// The option types the reference registers (`options.h:139-146`).
///
/// The names are the reference's own `sType` strings, so the table can be
/// compared with the corpus without a translation table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OptionType {
    /// `"bool"` — read with `get_option<bool>()`.
    Bool,
    /// `"int"` — one integer from `[min, max]`.
    Int,
    /// `"float"` — one float from `[min, max]`, stepped by `step`.
    Float,
    /// `"string_input"` — free text, truncated to `max_length` when that is > 0.
    StringInput,
    /// `"string_select"` — one of a fixed list of values.
    StringSelect,
    /// `"int_map"` — one of a fixed list of integers.
    IntMap,
}

impl OptionType {
    /// The reference's `sType` string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int => "int",
            Self::Float => "float",
            Self::StringInput => "string_input",
            Self::StringSelect => "string_select",
            Self::IntMap => "int_map",
        }
    }

    /// The type with this `sType` string, if the reference has one.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "bool" => Some(Self::Bool),
            "int" => Some(Self::Int),
            "float" => Some(Self::Float),
            "string_input" => Some(Self::StringInput),
            "string_select" => Some(Self::StringSelect),
            "int_map" => Some(Self::IntMap),
            _ => None,
        }
    }
}

impl fmt::Display for OptionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a registration comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source {
    /// An `add( ... )` call in `src/options.cpp`.
    OptionsCpp,
    /// An `EXTERNAL_OPTION` entry in `data/core/external_options.json`.
    ExternalOptionsJson,
}

impl Source {
    /// The corpus's name for this source.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OptionsCpp => "options.cpp",
            Self::ExternalOptionsJson => "external_options.json",
        }
    }

    /// The source with this corpus name, if there is one.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "options.cpp" => Some(Self::OptionsCpp),
            "external_options.json" => Some(Self::ExternalOptionsJson),
            _ => None,
        }
    }
}

/// The reference's `copt_hide_t` (`options.h:78-93`), in its order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HideFlag {
    /// `COPT_NO_HIDE` — shown.
    NoHide,
    /// `COPT_SDL_HIDE` — hidden in the SDL build.
    SdlHide,
    /// `COPT_CURSES_HIDE` — shown in SDL builds only.
    CursesHide,
    /// `COPT_POSIX_CURSES_HIDE` — hidden in non-Windows curses builds.
    PosixCursesHide,
    /// `COPT_NO_SOUND_HIDE` — hidden without sound support.
    NoSoundHide,
    /// `COPT_ALWAYS_HIDE` — never shown, only settable from data.
    AlwaysHide,
}

impl HideFlag {
    /// The corpus's name for this flag.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoHide => "no_hide",
            Self::SdlHide => "sdl_hide",
            Self::CursesHide => "curses_hide",
            Self::PosixCursesHide => "posix_curses_hide",
            Self::NoSoundHide => "no_sound_hide",
            Self::AlwaysHide => "always_hide",
        }
    }

    /// The flag with this corpus name, if there is one.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "no_hide" => Some(Self::NoHide),
            "sdl_hide" => Some(Self::SdlHide),
            "curses_hide" => Some(Self::CursesHide),
            "posix_curses_hide" => Some(Self::PosixCursesHide),
            "no_sound_hide" => Some(Self::NoSoundHide),
            "always_hide" => Some(Self::AlwaysHide),
            _ => None,
        }
    }
}

/// A value an option can hold, and the value `setValue()` accepts.
///
/// The reference stores one member per type (`sSet`, `bSet`, `iSet`, `fSet`,
/// `options.h:141-178`) and casts between them in `value_as<T>()`
/// (`options.cpp:766-783`), so the port keeps the type in the value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A `"bool"` option.
    Bool(bool),
    /// An `"int"` or `"int_map"` option.
    Int(i32),
    /// A `"float"` option.
    Float(f32),
    /// A `"string_select"` or `"string_input"` option.
    Text(String),
}

/// A value the table can name, or a slot the reference fills at runtime.
///
/// The second case is not a gap in the port: `build_tilesets_list()`,
/// `options_manager::get_lang_options()` and friends scan the filesystem or the
/// OS locale when the game starts (`options.cpp:1290-1350,1740,2650,2677`), so no
/// table can say what the value will be. The corpus records the reference's own
/// expression and the reason, and the port reports such an option as unset until
/// its owner supplies the value.
#[derive(Clone, Debug, PartialEq)]
pub enum Assigned {
    /// A value the source spells out.
    Literal(Value),
    /// A value the reference computes at startup.
    Runtime {
        /// The reference's expression, verbatim.
        expr: String,
        /// Why it cannot be known statically.
        note: String,
    },
}

impl Assigned {
    /// The literal value, if this slot has one.
    #[must_use]
    pub const fn literal(&self) -> Option<&Value> {
        match self {
            Self::Literal(value) => Some(value),
            Self::Runtime { .. } => None,
        }
    }
}

/// The selectable values of a `"string_select"` or `"int_map"` option.
#[derive(Clone, Debug, PartialEq)]
pub enum Items {
    /// A list the source spells out.
    Literal(Vec<Value>),
    /// A list the reference builds at startup, e.g. the installed soundpacks.
    Runtime {
        /// The reference's expression, verbatim.
        expr: String,
        /// Why it cannot be known statically.
        note: String,
    },
}

/// One option as the reference registers it.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionSpec {
    /// The option's id, e.g. `"SEASON_LENGTH"`.
    pub name: String,
    /// The option's type.
    pub opt_type: OptionType,
    /// The underscore-separated page id the reference files it under.
    pub page: String,
    /// The visibility flag.
    pub hide: HideFlag,
    /// The option's line in its source, for citation.
    pub line: u32,
    /// Which file the registration comes from.
    pub source: Source,
    /// Whether the option is on a page, i.e. whether `save()` writes it
    /// (`options.cpp:3941-3960` walks pages, and `add_external` adds no page item).
    pub in_page: bool,
    /// The value `setValue()` falls back to when it rejects an input.
    pub default: Assigned,
    /// The value a fresh install starts with.
    pub initial: Assigned,
    /// Lower bound of an `"int"` or `"float"` option.
    pub min: Option<Assigned>,
    /// Upper bound of an `"int"` or `"float"` option.
    pub max: Option<Assigned>,
    /// Step of a `"float"` option's slider.
    pub step: Option<Assigned>,
    /// Maximum length of a `"string_input"` option, 0 meaning unlimited.
    pub max_length: Option<Assigned>,
    /// The printf format an `"int"` or `"float"` option saves with
    /// (`options.cpp:725-745`).
    pub format: Option<String>,
    /// The selectable values of a `"string_select"` or `"int_map"` option.
    pub items: Option<Items>,
}

/// Everything wrong with a table that cannot be read.
#[derive(Debug)]
pub enum TableError {
    /// The file is not JSON the reference's loader would accept.
    Json(cdda_json::ParseError),
    /// The file is JSON, but not shaped like an option table.
    Shape(String),
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "{error}"),
            Self::Shape(detail) => f.write_str(detail),
        }
    }
}

impl std::error::Error for TableError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Shape(_) => None,
        }
    }
}

impl From<cdda_json::ParseError> for TableError {
    fn from(error: cdda_json::ParseError) -> Self {
        Self::Json(error)
    }
}

/// The option table, in the corpus's order.
/// A rename the reference applies to a saved option name, plus any value map
/// that goes with it (`options.cpp:196-207`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Migration {
    /// The name a saved file may hold.
    pub old: String,
    /// The name the reference uses now.
    pub new: String,
    /// Old value to new value, for options whose values were renamed too.
    pub values: BTreeMap<String, String>,
}

/// The option table: every registration, the rename map and the removed names,
/// as `tools/gen_options.py` extracted them from the reference.
#[derive(Clone, Debug)]
pub struct Table {
    specs: Vec<OptionSpec>,
    migrations: Vec<Migration>,
    removed: Vec<String>,
}

impl Table {
    /// Reads the table `tools/gen_options.py` writes.
    ///
    /// # Errors
    ///
    /// Returns [`TableError`] if the JSON does not parse, or if an entry lacks a
    /// field this type needs or names an option type the reference does not have.
    pub fn parse(text: &str) -> Result<Self, TableError> {
        let json = cdda_json::parse(text.as_bytes())?;
        let options = match &json {
            cdda_json::Value::Object(object) => match object.get("options") {
                Some(cdda_json::Value::Array(entries)) => entries,
                _ => return Err(TableError::Shape("`options` is not an array".to_owned())),
            },
            _ => return Err(TableError::Shape("the table is not an object".to_owned())),
        };
        let mut specs = Vec::with_capacity(options.len());
        for entry in options {
            let name = field(entry, "name")?;
            let type_name = field(entry, "type")?;
            let opt_type = OptionType::from_name(&type_name).ok_or_else(|| {
                TableError::Shape(format!("{name}: unknown option type {type_name:?}"))
            })?;
            let hide_name = field(entry, "hide")?;
            let hide = HideFlag::from_name(&hide_name).ok_or_else(|| {
                TableError::Shape(format!("{name}: unknown hide flag {hide_name:?}"))
            })?;
            let source_name = field(entry, "source")?;
            let source = Source::from_name(&source_name).ok_or_else(|| {
                TableError::Shape(format!("{name}: unknown source {source_name:?}"))
            })?;
            let line = number(entry, "line")?;
            let in_page = matches!(member(entry, "in_page"), Some(cdda_json::Value::Bool(true)));
            specs.push(OptionSpec {
                name,
                opt_type,
                page: field(entry, "page")?,
                hide,
                line,
                source,
                in_page,
                default: assigned(entry, "default")?,
                initial: assigned(entry, "initial")?,
                min: optional_assigned(entry, "min")?,
                max: optional_assigned(entry, "max")?,
                step: optional_assigned(entry, "step")?,
                max_length: optional_assigned(entry, "max_length")?,
                format: optional_text(entry, "format")?,
                items: items(entry)?,
            });
        }
        Ok(Self {
            specs,
            migrations: migrations(&json)?,
            removed: removed(&json)?,
        })
    }

    /// The rename a saved option name may need, if the reference has one.
    #[must_use]
    pub fn migration(&self, name: &str) -> Option<&Migration> {
        self.migrations
            .iter()
            .find(|migration| migration.old == name)
    }

    /// The option names `deserialize()` skips outright (`options.cpp:3976-3983`).
    #[must_use]
    pub fn removed(&self) -> &[String] {
        &self.removed
    }

    /// Whether `deserialize()` skips this saved name.
    #[must_use]
    pub fn is_removed(&self, name: &str) -> bool {
        self.removed.iter().any(|removed| removed == name)
    }

    /// The options, in the corpus's order (sorted by name).
    #[must_use]
    pub fn specs(&self) -> &[OptionSpec] {
        &self.specs
    }

    /// How many options the table has.
    #[must_use]
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    /// Whether the table has no options at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }
}

/// A required value slot of a table entry.
fn assigned(entry: &cdda_json::Value, key: &str) -> Result<Assigned, TableError> {
    optional_assigned(entry, key)?
        .ok_or_else(|| TableError::Shape(format!("`{key}` is null, but must have a value")))
}

/// An optional value slot of a table entry.
fn optional_assigned(entry: &cdda_json::Value, key: &str) -> Result<Option<Assigned>, TableError> {
    match member(entry, key) {
        Some(cdda_json::Value::Null) | None => Ok(None),
        Some(node @ cdda_json::Value::Object(_)) => {
            let kind = member(node, "kind")
                .and_then(|kind| match kind {
                    cdda_json::Value::Str(text) => Some(text.as_str()),
                    _ => None,
                })
                .ok_or_else(|| TableError::Shape(format!("`{key}.kind` is not a string")))?;
            match kind {
                "literal" => Ok(Some(Assigned::Literal(literal(entry, key)?))),
                "runtime" => Ok(Some(Assigned::Runtime {
                    expr: field(node, "expr")?,
                    note: field(node, "note")?,
                })),
                other => Err(TableError::Shape(format!(
                    "`{key}.kind` is {other:?}, expected \"literal\" or \"runtime\""
                ))),
            }
        }
        Some(other) => Err(TableError::Shape(format!(
            "`{key}` is not an object: {other}"
        ))),
    }
}

/// The literal in a value slot.
fn literal(entry: &cdda_json::Value, key: &str) -> Result<Value, TableError> {
    let Some(node) = member(entry, key).and_then(|node| member(node, "value")) else {
        return Err(TableError::Shape(format!("`{key}.value` is missing")));
    };
    match node {
        cdda_json::Value::Bool(flag) => Ok(Value::Bool(*flag)),
        cdda_json::Value::Int(number) => i32::try_from(*number)
            .map(Value::Int)
            .map_err(|_| TableError::Shape(format!("`{key}` does not fit an int: {number}"))),
        cdda_json::Value::Float(number) => narrow(*number).map(Value::Float),
        cdda_json::Value::Str(text) => Ok(Value::Text(text.clone())),
        other => Err(TableError::Shape(format!("`{key}.value` is {other}"))),
    }
}

/// The 32-bit float the reference stores.
///
/// The corpus writes the reference's floats widened exactly, so a value that
/// does not survive the narrowing means the corpus holds a double the reference
/// never had: report it rather than round it.
#[allow(clippy::float_cmp)] // exact comparison is the point: a rounded value is an error
fn narrow(value: f64) -> Result<f32, TableError> {
    #[allow(clippy::cast_possible_truncation)]
    let narrowed = value as f32;
    if f64::from(narrowed) == value {
        Ok(narrowed)
    } else {
        Err(TableError::Shape(format!(
            "{value} is not a 32-bit float the reference could hold"
        )))
    }
}

/// An optional string slot of a table entry.
fn optional_text(entry: &cdda_json::Value, key: &str) -> Result<Option<String>, TableError> {
    match member(entry, key) {
        Some(cdda_json::Value::Null) => Ok(None),
        Some(cdda_json::Value::Object(_)) => {
            Ok(Some(field(member(entry, key).unwrap_or(entry), "value")?))
        }
        Some(other) => Err(TableError::Shape(format!(
            "`{key}` is not an object: {other}"
        ))),
        None => Err(TableError::Shape(format!("`{key}` is missing"))),
    }
}

/// The selectable values of a table entry.
fn items(entry: &cdda_json::Value) -> Result<Option<Items>, TableError> {
    let Some(node) = member(entry, "items") else {
        return Ok(None);
    };
    match node {
        cdda_json::Value::Null => Ok(None),
        cdda_json::Value::Object(_) => {
            let kind = field(node, "kind")?;
            match kind.as_str() {
                "literal" => {
                    let Some(cdda_json::Value::Array(list)) = member(node, "items") else {
                        return Err(TableError::Shape(
                            "`items.items` is not an array".to_owned(),
                        ));
                    };
                    let mut values = Vec::with_capacity(list.len());
                    for item in list {
                        let Some(value) = member(item, "value") else {
                            return Err(TableError::Shape("an item has no `value`".to_owned()));
                        };
                        values.push(match value {
                            cdda_json::Value::Int(number) => {
                                Value::Int(i32::try_from(*number).map_err(|_| {
                                    TableError::Shape(format!("item {number} does not fit an int"))
                                })?)
                            }
                            cdda_json::Value::Str(text) => Value::Text(text.clone()),
                            other => {
                                return Err(TableError::Shape(format!("item value {other}")));
                            }
                        });
                    }
                    Ok(Some(Items::Literal(values)))
                }
                "runtime" => Ok(Some(Items::Runtime {
                    expr: field(node, "expr")?,
                    note: field(node, "note")?,
                })),
                other => Err(TableError::Shape(format!(
                    "`items.kind` is {other:?}, expected \"literal\" or \"runtime\""
                ))),
            }
        }
        other => Err(TableError::Shape(format!(
            "`items` is not an object: {other}"
        ))),
    }
}

/// A member of a table entry.
fn member<'a>(entry: &'a cdda_json::Value, key: &str) -> Option<&'a cdda_json::Value> {
    match entry {
        cdda_json::Value::Object(object) => object.get(key),
        _ => None,
    }
}

/// A string member of a table entry.
fn field(entry: &cdda_json::Value, key: &str) -> Result<String, TableError> {
    match member(entry, key) {
        Some(cdda_json::Value::Str(text)) => Ok(text.clone()),
        Some(_) => Err(TableError::Shape(format!("`{key}` is not a string"))),
        None => Err(TableError::Shape(format!("`{key}` is missing"))),
    }
}

/// A non-negative integer member of a table entry.
fn number(entry: &cdda_json::Value, key: &str) -> Result<u32, TableError> {
    match member(entry, key) {
        Some(cdda_json::Value::Int(value)) => u32::try_from(*value)
            .map_err(|_| TableError::Shape(format!("`{key}` is out of range: {value}"))),
        Some(_) => Err(TableError::Shape(format!("`{key}` is not an integer"))),
        None => Err(TableError::Shape(format!("`{key}` is missing"))),
    }
}

impl Table {
    /// The spec of one option, or `None` if the reference has no such option.
    ///
    /// The table is sorted by name, so this is a binary search; the reference
    /// keeps an `unordered_map` keyed the same way (`options.h:200`).
    #[must_use]
    pub fn spec(&self, name: &str) -> Option<&OptionSpec> {
        self.specs
            .binary_search_by(|spec| spec.name.as_str().cmp(name))
            .ok()
            .and_then(|index| self.specs.get(index))
    }
}

/// The live option values.
///
/// This is the part of `options_manager` that decides what a value *is*: the
/// current value of every option, and the rules `setValue()` applies when it
/// accepts or rejects an input.
#[derive(Clone, Debug)]
pub struct Options {
    table: Table,
    values: BTreeMap<String, Value>,
}

impl Options {
    /// A fresh set of values: every option starts at its table `initial`, and
    /// options whose start value the reference computes at runtime start unset.
    #[must_use]
    pub fn new(table: Table) -> Self {
        let mut values = BTreeMap::new();
        for spec in table.specs() {
            if let Some(value) = spec.initial.literal() {
                values.insert(spec.name.clone(), value.clone());
            }
        }
        Self { table, values }
    }

    /// The table these values belong to.
    #[must_use]
    pub const fn table(&self) -> &Table {
        &self.table
    }

    /// The current value of an option.
    ///
    /// `None` means either that the reference has no such option, or that the
    /// option's start value is one of the slots the reference computes at
    /// runtime (`Assigned::Runtime`) and nobody has supplied one yet; use
    /// [`Options::has_option`] to tell them apart.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    /// Whether the reference registers this option at all.
    #[must_use]
    pub fn has_option(&self, name: &str) -> bool {
        self.table.spec(name).is_some()
    }

    /// Sets a value with the same rules as `cOpt::setValue` (`options.cpp:1042-1119`).
    ///
    /// Returns `false` when the option does not exist or its type is not the
    /// value's type: the reference's overloads `debugmsg` and leave the value
    /// alone in that case (`options.cpp:1044-1047`).
    pub fn set(&mut self, name: &str, value: Value) -> bool {
        let Some(spec) = self.table.spec(name) else {
            return false;
        };
        let name = spec.name.clone();
        match (spec.opt_type, value) {
            (OptionType::Int | OptionType::IntMap, Value::Int(number)) => {
                let accepted = clamp_int(spec, number);
                self.values.insert(name, Value::Int(accepted));
                true
            }
            (OptionType::Float, Value::Float(number)) => {
                let accepted = clamp_float(spec, number);
                self.values.insert(name, Value::Float(accepted));
                true
            }
            (_, Value::Text(text)) => self.set_text(&name, &text),
            _ => false,
        }
    }

    /// Sets an option from the text a saved `options.json` holds, with the rules
    /// of `setValue( const std::string & )` (`options.cpp:1068-1119`).
    ///
    /// Returns `false` when the option does not exist. A string the reference
    /// rejects is not an error: the value quietly becomes the type's fallback, or
    /// is left alone where the reference leaves it alone.
    pub fn set_text(&mut self, name: &str, text: &str) -> bool {
        let Some(spec) = self.table.spec(name) else {
            return false;
        };
        let name = spec.name.clone();
        let value = match spec.opt_type {
            OptionType::Bool => Some(Value::Bool(matches!(text, "True" | "true" | "T" | "t"))),
            OptionType::Int => Some(Value::Int(clamp_int(spec, parse_saved_int(spec, text)))),
            OptionType::IntMap => Some(Value::Int(int_map_value(spec, text))),
            OptionType::Float => {
                parse_float_text(text).map(|number| Value::Float(clamp_float(spec, number)))
            }
            OptionType::StringSelect => string_select_value(spec, text),
            OptionType::StringInput => Some(Value::Text(truncate_bytes(text, spec))),
        };
        match value {
            Some(value) => {
                self.values.insert(name, value);
                true
            }
            // The reference keeps the current value and only logs a message when
            // a string_select receives a value that is not one of its items
            // (options.cpp:1069-1073), and a float option keeps its value when the
            // text does not parse (options.cpp:1110-1118).
            None => true,
        }
    }
}

/// The integer a saved string stands for, with a trailing '%' removed first
/// (options.cpp:1081-1086) and `iDefault` when it does not parse
/// (options.cpp:1083-1098).
fn parse_saved_int(spec: &OptionSpec, text: &str) -> i32 {
    let without_percent = text.strip_suffix('%').unwrap_or(text);
    parse_int_text(without_percent)
        .unwrap_or_else(|| spec.default.literal().and_then(as_int).unwrap_or(0))
}

/// An `int_map` value must be one of the map's keys, else `iDefault`
/// (options.cpp:1100-1109).
fn int_map_value(spec: &OptionSpec, text: &str) -> i32 {
    let default = spec.default.literal().and_then(as_int).unwrap_or(0);
    let Some(number) = parse_int_text(text) else {
        return default;
    };
    match &spec.items {
        Some(Items::Literal(items)) => {
            if items.iter().any(|item| as_int(item) == Some(number)) {
                number
            } else {
                default
            }
        }
        // The reference checks membership against a list it built at startup; a
        // port that cannot see that list cannot check membership either.
        Some(Items::Runtime { .. }) | None => number,
    }
}

/// A `string_select` only accepts a value that is one of its items
/// (options.cpp:1069-1073).
fn string_select_value(spec: &OptionSpec, text: &str) -> Option<Value> {
    match &spec.items {
        Some(Items::Literal(items)) => {
            if items
                .iter()
                .any(|item| matches!(item, Value::Text(item) if item == text))
            {
                Some(Value::Text(text.to_owned()))
            } else {
                None
            }
        }
        Some(Items::Runtime { .. }) | None => Some(Value::Text(text.to_owned())),
    }
}

/// A `string_input` is cut to `iMaxLength` bytes when that is not 0
/// (options.cpp:1074-1076).
fn truncate_bytes(text: &str, spec: &OptionSpec) -> String {
    let Some(limit) = spec
        .max_length
        .as_ref()
        .and_then(Assigned::literal)
        .and_then(as_int)
    else {
        return text.to_owned();
    };
    if limit <= 0 {
        return text.to_owned();
    }
    let limit = usize::try_from(limit).unwrap_or(0);
    // The reference cuts a std::string at a byte offset; do the same but stop at
    // a character boundary so the result is still text.
    let mut end = text.len().min(limit);
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.get(..end).unwrap_or(text).to_owned()
}

/// Parses an integer the way `try_parse_integer<int>( s, false )` does
/// (`try_parse_integer.cpp:18-52`): leading whitespace allowed, an optional
/// sign, digits, then nothing but whitespace.
fn parse_int_text(text: &str) -> Option<i32> {
    let trimmed = text.trim_start();
    let (negative, digits) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    let end = digits
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(digits.len());
    let (number, rest) = digits.split_at(end);
    if number.is_empty() || !rest.trim().is_empty() {
        return None;
    }
    let magnitude: i64 = number.parse().ok()?;
    let signed = if negative { -magnitude } else { magnitude };
    i32::try_from(signed).ok()
}

/// Parses the leading float of a string, the way `operator>>` on a float does:
/// trailing characters are not an error, a missing number is
/// (options.cpp:1110-1118).
fn parse_float_text(text: &str) -> Option<f32> {
    let trimmed = text.trim_start();
    let (sign, rest) = match trimmed.strip_prefix('-') {
        Some(rest) => (-1.0_f32, rest),
        None => (1.0, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    let lower = rest.to_ascii_lowercase();
    for (spelling, special) in [
        ("infinity", f32::INFINITY),
        ("inf", f32::INFINITY),
        ("nan", f32::NAN),
    ] {
        if lower.starts_with(spelling) {
            return Some(sign * special);
        }
    }
    let mut end = 0;
    let mut digits = 0;
    let bytes = rest.as_bytes();
    while end < bytes.len() && bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
        digits += 1;
    }
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        while end < bytes.len() && bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if bytes
        .get(end)
        .is_some_and(|byte| *byte == b'e' || *byte == b'E')
    {
        let mut after = end + 1;
        if bytes
            .get(after)
            .is_some_and(|byte| *byte == b'+' || *byte == b'-')
        {
            after += 1;
        }
        let mut exponent_digits = 0;
        while after < bytes.len() && bytes.get(after).is_some_and(u8::is_ascii_digit) {
            after += 1;
            exponent_digits += 1;
        }
        if exponent_digits > 0 {
            end = after;
        }
    }
    let number: f32 = rest.get(..end)?.parse().ok()?;
    Some(sign * number)
}

/// The value an int option keeps: the input, or the default when out of range.
///
/// `setValue( int )` stores the value and then replaces it with `iDefault` if it
/// is outside `[iMin, iMax]` (options.cpp:1055-1066), which is *not* the same as
/// clamping to the nearest bound.
fn clamp_int(spec: &OptionSpec, value: i32) -> i32 {
    let low = spec
        .min
        .as_ref()
        .and_then(Assigned::literal)
        .and_then(as_int);
    let high = spec
        .max
        .as_ref()
        .and_then(Assigned::literal)
        .and_then(as_int);
    let out_of_range = low.is_some_and(|low| value < low) || high.is_some_and(|high| value > high);
    if out_of_range {
        spec.default.literal().and_then(as_int).unwrap_or(value)
    } else {
        value
    }
}

/// Same rule as [`clamp_int`], for the float overload (options.cpp:1042-1053).
fn clamp_float(spec: &OptionSpec, value: f32) -> f32 {
    let low = spec
        .min
        .as_ref()
        .and_then(Assigned::literal)
        .and_then(as_float);
    let high = spec
        .max
        .as_ref()
        .and_then(Assigned::literal)
        .and_then(as_float);
    let out_of_range = low.is_some_and(|low| value < low) || high.is_some_and(|high| value > high);
    if out_of_range {
        spec.default.literal().and_then(as_float).unwrap_or(value)
    } else {
        value
    }
}

/// The integer a literal holds, if it is one.
fn as_int(value: &Value) -> Option<i32> {
    match value {
        Value::Int(number) => Some(*number),
        _ => None,
    }
}

/// The float a literal holds, if it is one.
fn as_float(value: &Value) -> Option<f32> {
    match value {
        Value::Float(number) => Some(*number),
        _ => None,
    }
}

/// What a saved `options.json` did when it was loaded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadReport {
    /// Entries that were read and applied.
    pub applied: usize,
    /// Entries naming an option the reference no longer reads from a save file,
    /// which `deserialize()` drops before anything else (`options.cpp:3976-3983`).
    pub skipped_removed: usize,
    /// Entries whose option name was renamed before being applied
    /// (`options.cpp:3999-4003`).
    pub migrated: usize,
    /// Entries naming an option the reference does not have.  The reference
    /// default-constructs an option of type `"VOID"` for these and drops the
    /// value, so they are counted and otherwise ignored (`options.cpp:3995`).
    pub skipped_unknown: usize,
}

/// Everything wrong with a saved `options.json`.
#[derive(Debug)]
pub enum LoadError {
    /// The file is not JSON the reference's loader would accept.
    Json(cdda_json::ParseError),
    /// The file is JSON, but not shaped like `options.json`.
    Shape(String),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "{error}"),
            Self::Shape(detail) => f.write_str(detail),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Shape(_) => None,
        }
    }
}

impl From<cdda_json::ParseError> for LoadError {
    fn from(error: cdda_json::ParseError) -> Self {
        Self::Json(error)
    }
}

impl Options {
    /// Applies the contents of a saved `options.json`, the way
    /// `options_manager::deserialize()` does (`options.cpp:3972-3997`).
    ///
    /// The file is the array `serialize()` writes: one object per option with a
    /// `"name"` and a `"value"`, both strings.  A name the reference does not
    /// register is ignored, because the reference's `options[name]` creates a
    /// `"VOID"` option whose `setValue()` accepts nothing; an entry without a
    /// `"name"` or `"value"` fails the whole load, because the reference reads
    /// them with `get_string()`, which throws.
    ///
    /// # Errors
    ///
    /// Returns [`LoadError`] if the text is not JSON, or an entry is not an
    /// object with string `"name"` and `"value"` members.
    pub fn load(&mut self, text: &str) -> Result<LoadReport, LoadError> {
        let json = cdda_json::parse(text.as_bytes())?;
        let cdda_json::Value::Array(entries) = &json else {
            return Err(LoadError::Shape("options.json is not an array".to_owned()));
        };
        let mut report = LoadReport::default();
        for entry in entries {
            let saved_name = saved_member(entry, "name")?;
            let saved_value = saved_member(entry, "value")?;
            let migration = self.table.migration(&saved_name);
            let name = match migration {
                Some(migration) => {
                    report.migrated += 1;
                    migration.new.clone()
                }
                None => saved_name.clone(),
            };
            if self.table.is_removed(&name) {
                report.skipped_removed += 1;
                continue;
            }
            if !self.has_option(&name) {
                report.skipped_unknown += 1;
                continue;
            }
            let value = migrate_value(&saved_name, &saved_value, migration);
            self.set_text(&name, &value);
            report.applied += 1;
        }
        Ok(report)
    }
}

/// A string member of one saved entry.
fn saved_member(entry: &cdda_json::Value, key: &str) -> Result<String, LoadError> {
    match entry {
        cdda_json::Value::Object(object) => match object.get(key) {
            Some(cdda_json::Value::Str(text)) => Ok(text.clone()),
            Some(other) => Err(LoadError::Shape(format!(
                "an entry's {key:?} is {other}, expected a string"
            ))),
            None => Err(LoadError::Shape(format!("an entry has no {key:?}"))),
        },
        other => Err(LoadError::Shape(format!(
            "an entry is {other}, expected an object"
        ))),
    }
}

/// The value a saved entry holds after the reference's rename map and its
/// hand-written conversions (`options.cpp:4005-4030`).
fn migrate_value(saved_name: &str, value: &str, migration: Option<&Migration>) -> String {
    // MONSTER_UPGRADE_FACTOR used to be the reciprocal of the new option scaled by
    // four; the reference converts the number and prints it with two decimals so
    // the float parser sees exactly what the options UI would have written
    // (options.cpp:4005-4018).
    if saved_name == "MONSTER_UPGRADE_FACTOR"
        && let Ok(number) = value.parse::<f32>()
    {
        return format!("{:.2}", number / 4.0);
    }
    // The table's own value map, for renames that also renamed the values, and
    // the original value for everything it does not mention (options.cpp:4022-4030).
    match migration.and_then(|migration| migration.values.get(value)) {
        Some(migrated) => migrated.clone(),
        None => value.to_owned(),
    }
}

/// The `migrations` array of a table.
fn migrations(json: &cdda_json::Value) -> Result<Vec<Migration>, TableError> {
    let cdda_json::Value::Array(list) = meta_member(json, "migrations")? else {
        return Err(TableError::Shape(
            "meta.migrations is not an array".to_owned(),
        ));
    };
    let mut result = Vec::with_capacity(list.len());
    for entry in list {
        let old = field(entry, "old")?;
        let new = field(entry, "new")?;
        let mut values = BTreeMap::new();
        let Some(node) = member(entry, "values") else {
            return Err(TableError::Shape(format!("{old}: no `values` map")));
        };
        let cdda_json::Value::Object(object) = node else {
            return Err(TableError::Shape(format!(
                "{old}: `values` is not an object"
            )));
        };
        for (from, to) in object.members() {
            let cdda_json::Value::Str(to) = to else {
                return Err(TableError::Shape(format!("{old}.{from} is not a string")));
            };
            values.insert(from.clone(), to.clone());
        }
        result.push(Migration { old, new, values });
    }
    Ok(result)
}

/// The `removed_options` array of a table.
fn removed(json: &cdda_json::Value) -> Result<Vec<String>, TableError> {
    let cdda_json::Value::Array(list) = meta_member(json, "removed_options")? else {
        return Err(TableError::Shape(
            "meta.removed_options is not an array".to_owned(),
        ));
    };
    let mut result = Vec::with_capacity(list.len());
    for entry in list {
        let cdda_json::Value::Str(name) = entry else {
            return Err(TableError::Shape(
                "a removed option is not a string".to_owned(),
            ));
        };
        result.push(name.clone());
    }
    Ok(result)
}

/// A member of the table's `meta` object.
fn meta_member<'a>(
    json: &'a cdda_json::Value,
    key: &str,
) -> Result<&'a cdda_json::Value, TableError> {
    let Some(meta) = member(json, "meta") else {
        return Err(TableError::Shape("the table has no `meta`".to_owned()));
    };
    member(meta, key).ok_or_else(|| TableError::Shape(format!("meta.{key} is missing")))
}

impl Options {
    /// Writes the `options.json` text the reference's `serialize()` writes
    /// (`options.cpp:3941-3960`).
    ///
    /// Only options on a page are written, in table order; an option whose value
    /// the reference computes at startup and nobody has supplied is skipped,
    /// because writing a value the port does not have would invent one.
    ///
    /// The reference also writes `"info"` and `"default"` members, which hold a
    /// translated tooltip and a default rendered through the UI's item names. The
    /// table carries neither, and `deserialize()` ignores both
    /// (`options.cpp:3973`, `allow_omitted_members()`), so this writes the two
    /// members a load needs.
    #[must_use]
    pub fn save(&self) -> String {
        let mut writer = cdda_json::Writer::new(true);
        writer.start_array(true);
        for spec in self.table.specs() {
            if !spec.in_page {
                continue;
            }
            let Some(value) = self.values.get(&spec.name) else {
                continue;
            };
            writer.start_object(true);
            writer.member("name");
            writer.write_str(&spec.name);
            writer.member("value");
            writer.write_str(&saved_text(spec, value));
            writer.end_object();
        }
        writer.end_array();
        writer.into_string()
    }
}

/// The text `getValue( true )` would produce for a value (`options.cpp:725-745`).
fn saved_text(spec: &OptionSpec, value: &Value) -> String {
    match value {
        Value::Bool(flag) => (if *flag { "true" } else { "false" }).to_owned(),
        Value::Int(number) => format_int(spec.format.as_deref(), *number),
        Value::Float(number) => format_float(spec.format.as_deref(), *number),
        Value::Text(text) => text.clone(),
    }
}

/// Applies an int option's printf format, e.g. `"%i"` or `"%i%%"`.
fn format_int(format: Option<&str>, value: i32) -> String {
    let Some(format) = format else {
        return value.to_string();
    };
    let mut out = String::new();
    let mut rest = format;
    let mut written = false;
    while let Some(index) = rest.find('%') {
        let Some(prefix) = rest.get(..index) else {
            break;
        };
        out.push_str(prefix);
        let Some(tail) = rest.get(index + 1..) else {
            break;
        };
        if let Some(after) = tail.strip_prefix('%') {
            out.push('%');
            rest = after;
            continue;
        }
        if let Some(after) = tail.strip_prefix(['i', 'd', 'u']) {
            if !written {
                out.push_str(&value.to_string());
                written = true;
            }
            rest = after;
            continue;
        }
        out.push('%');
        rest = tail;
    }
    out.push_str(rest);
    if written {
        out
    } else {
        // A format with no conversion specifier: the reference's string_format
        // would print the number, so print it.
        format!("{value}{out}")
    }
}

/// Applies a float option's printf format, e.g. `"%.2f"`.
fn format_float(format: Option<&str>, value: f32) -> String {
    let format = format.unwrap_or("%.2f");
    let precision = format
        .split_once("%.")
        .and_then(|(_, rest)| rest.split_once('f'))
        .and_then(|(digits, _)| digits.parse::<usize>().ok())
        .unwrap_or(2);
    format!("{value:.precision$}")
}
