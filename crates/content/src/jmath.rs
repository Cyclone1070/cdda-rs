//! Data-defined math functions.
//!
//! Reference: `jmath_function` (`math_parser_jmath.cpp`), which lets the data write a function body as an expression string and binds its arguments to `_0`, `_1`, and so
//! on. The weather conditions depend on three of them, one inside another: `dew_point_factor` calls `dew_point`, which calls `dprhx`, which reads the weather's humidity.
//!
//! Following the chain matters more than it looks. A port that hard-coded the three formulas would agree with this version of the data and disagree with any mod that
//! changed one of them, and it would have nothing to say about the third level reading a field two layers up.

use crate::math_expr::{Context, MathError, Value, evaluate};
use cdda_json::{Object, Value as Json, parse};

/// One function the data defined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// What the expression calls it.
    pub id: String,
    /// How many arguments it takes; the body sees them as `_0` through `_n`.
    pub num_args: usize,
    /// The body, as written in the data.
    pub body: String,
}

/// The functions a data set defined, by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registry {
    functions: Vec<Function>,
}

impl Registry {
    /// Reads a JSON array of `jmath_function` entries.
    ///
    /// # Errors
    ///
    /// Returns the reason the data could not be read, or the name of an entry that is missing something it needs.
    pub fn read_json(json: &[u8]) -> Result<Registry, String> {
        let parsed = parse(json).map_err(|error| error.to_string())?;
        let Json::Array(entries) = parsed else {
            return Err("the jmath data is an array".to_string());
        };
        let mut functions = Vec::new();
        for entry in entries {
            let Json::Object(object) = entry else {
                return Err("every jmath entry is an object".to_string());
            };
            if object.get("type").and_then(as_str) != Some("jmath_function") {
                continue;
            }
            let Some(id) = object.get("id").and_then(as_str) else {
                return Err("a jmath function with no id".to_string());
            };
            let Some(body) = object.get("return").and_then(as_str) else {
                return Err(format!("{id} has no body"));
            };
            let num_args = match object.get("num_args") {
                Some(Json::Int(count)) => usize::try_from(*count).unwrap_or(0),
                _ => 0,
            };
            functions.push(Function { id: id.to_string(), num_args, body: body.to_string() });
        }
        Ok(Registry { functions })
    }

    /// How many functions were defined.
    #[must_use]
    pub fn len(&self) -> usize {
        self.functions.len()
    }

    /// Whether nothing was defined.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    /// One function by name.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Function> {
        self.functions.iter().find(|function| function.id == id)
    }
}

fn as_str(value: &cdda_json::Value) -> Option<&str> {
    match value {
        cdda_json::Value::Str(text) => Some(text),
        _ => None,
    }
}

/// A context that has a data-defined function's arguments in scope and answers everything else through an outer context.
pub struct Bound<'a> {
    outer: &'a dyn Context,
    functions: &'a Registry,
    arguments: &'a [Value],
}

impl<'a> Bound<'a> {
    /// Binds `arguments` to `_0`, `_1`, and so on, and defers to `outer` for every other name.
    #[must_use]
    pub fn new(outer: &'a dyn Context, functions: &'a Registry, arguments: &'a [Value]) -> Bound<'a> {
        Bound { outer, functions, arguments }
    }
}

impl Context for Bound<'_> {
    fn number(&self, name: &str) -> Option<f64> {
        // `_0` through `_n` are the bound arguments; anything else belongs to the outer context.
        if let Some(index) = name.strip_prefix('_').and_then(|digits| digits.parse::<usize>().ok()) {
            return self.arguments.get(index).and_then(Value::as_number);
        }
        self.outer.number(name)
    }

    fn call(&self, name: &str, arguments: &[Value]) -> Result<Value, MathError> {
        if let Some(function) = self.functions.get(name) {
            if arguments.len() != function.num_args {
                return Err(MathError::Type {
                    message: format!("{name} takes {} arguments, not {}", function.num_args, arguments.len()),
                });
            }
            let inner = Bound::new(self.outer, self.functions, arguments);
            return evaluate(&function.body, &inner);
        }
        self.outer.call(name, arguments)
    }
}

/// Evaluates a data-defined function by name.
///
/// # Errors
///
/// Returns [`MathError::UnknownName`] when no such function exists, and whatever the body's own evaluation returns.
pub fn call(registry: &Registry, outer: &dyn Context, name: &str, arguments: &[Value]) -> Result<Value, MathError> {
    let Some(function) = registry.get(name) else {
        return Err(MathError::UnknownName { name: name.to_string() });
    };
    let inner = Bound::new(outer, registry, arguments);
    evaluate(&function.body, &inner)
}

/// Reads the `jmath_function` entries out of a JSON array of objects, the shape `data/json/weather_type.json` uses.
///
/// # Errors
///
/// Returns the reason an entry could not be read.
pub fn read_entries(entries: &[Object]) -> Result<Registry, String> {
    let mut functions = Vec::new();
    for object in entries {
        if object.get("type").and_then(as_str) != Some("jmath_function") {
            continue;
        }
        let Some(id) = object.get("id").and_then(as_str) else {
            return Err("a jmath function with no id".to_string());
        };
        let Some(body) = object.get("return").and_then(as_str) else {
            return Err(format!("{id} has no body"));
        };
        let num_args = match object.get("num_args") {
            Some(cdda_json::Value::Int(count)) => usize::try_from(*count).unwrap_or(0),
            _ => 0,
        };
        functions.push(Function { id: id.to_string(), num_args, body: body.to_string() });
    }
    Ok(Registry { functions })
}
