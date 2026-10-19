//! The option table, checked item by item against `corpus/options/table.json`.
//!
//! `corpus/options/table.json` is generated from the pinned reference checkout by
//! `tools/gen_options.py`; this test suite is the only thing that reads it, so a
//! disagreement here means either the generator or the port is wrong, and the
//! corpus wins until the generator is re-read.
//!
//! Reference: `references/Cataclysm-DDA/src/options.cpp` (line numbers in
//! comments) and `src/options.h`.
#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use cdda_core_types::options::{
    Assigned, Items, OptionSpec, OptionType, Options, Source, Table, Value,
};
use cdda_json::Value as Json;

fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("corpus")
        .join("options")
        .join("table.json")
}

fn table_text() -> String {
    let path = corpus_path();
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()))
}

fn table_json() -> Json {
    cdda_json::parse(table_text().as_bytes()).expect("corpus/options/table.json parses")
}

fn member<'a>(json: &'a Json, key: &str) -> &'a Json {
    match json {
        Json::Object(object) => object
            .get(key)
            .unwrap_or_else(|| panic!("{key} is missing")),
        _ => panic!("expected an object holding {key}"),
    }
}

fn meta<'a>(json: &'a Json, key: &str) -> &'a Json {
    member(member(json, "meta"), key)
}

#[test]
fn corpus_table_lists_every_option_once() {
    let table = Table::parse(&table_text()).expect("table parses");

    // 188 registrations in options.cpp plus 88 EXTERNAL_OPTION entries in
    // data/core/external_options.json, after the 4 `stub` entries and the 45
    // registrations this build profile excludes are removed.  The corpus states
    // the total itself, so compare with that rather than with a number copied
    // from a run.
    let json = table_json();
    let Json::Int(total) = member(meta(&json, "counts"), "total") else {
        panic!("meta.counts.total is not an integer")
    };
    assert_eq!(table.len(), usize::try_from(*total).expect("count fits"));

    let names: Vec<&str> = table
        .specs()
        .iter()
        .map(|spec| spec.name.as_str())
        .collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        names, sorted,
        "the table is sorted by name and has no duplicates"
    );

    let mut by_type: BTreeMap<&str, usize> = BTreeMap::new();
    for spec in table.specs() {
        *by_type.entry(spec.opt_type.as_str()).or_default() += 1;
    }
    let Json::Object(object) = member(meta(&json, "counts"), "by_type") else {
        panic!("meta.counts.by_type is not an object")
    };
    let mut expected: BTreeMap<&str, usize> = BTreeMap::new();
    for (name, count) in object.members() {
        match count {
            Json::Int(number) => {
                expected.insert(name.as_str(), usize::try_from(*number).expect("count fits"));
            }
            _ => panic!("meta.counts.by_type.{name} is not an integer"),
        }
    }
    assert_eq!(by_type, expected);

    // Both sources reach the table: options.cpp registers SEASON_LENGTH
    // (options.cpp:2794), external_options.json declares PLAYER_HUNGER_RATE, and
    // PREVENT_OCCLUSION is the int_map overload (options.cpp:2857).
    let find = |name: &str| -> OptionType {
        table
            .specs()
            .iter()
            .find(|spec| spec.name == name)
            .map(|spec| spec.opt_type)
            .unwrap_or_else(|| panic!("{name} is missing from the table"))
    };
    assert_eq!(find("SEASON_LENGTH"), OptionType::Int);
    assert_eq!(find("PLAYER_HUNGER_RATE"), OptionType::Float);
    assert_eq!(find("PREVENT_OCCLUSION"), OptionType::IntMap);
}

fn entries(json: &Json) -> &[Json] {
    match member(json, "options") {
        Json::Array(list) => list,
        _ => panic!("options is not an array"),
    }
}

fn text<'a>(entry: &'a Json, key: &str) -> &'a str {
    match member(entry, key) {
        Json::Str(text) => text,
        _ => panic!("{key} is not a string"),
    }
}

fn integer(entry: &Json, key: &str) -> i64 {
    match member(entry, key) {
        Json::Int(number) => *number,
        _ => panic!("{key} is not an integer"),
    }
}

#[test]
fn spec_metadata_matches_the_corpus() {
    let table = Table::parse(&table_text()).expect("table parses");
    let json = table_json();
    let entries = entries(&json);
    assert_eq!(entries.len(), table.len());

    for (spec, entry) in table.specs().iter().zip(entries) {
        assert_eq!(spec.name, text(entry, "name"));
        assert_eq!(spec.opt_type.as_str(), text(entry, "type"), "{}", spec.name);
        assert_eq!(spec.page, text(entry, "page"), "{}", spec.name);
        assert_eq!(spec.hide.as_str(), text(entry, "hide"), "{}", spec.name);
        assert_eq!(
            i64::from(spec.line),
            integer(entry, "line"),
            "{}",
            spec.name
        );
        assert_eq!(spec.source.as_str(), text(entry, "source"), "{}", spec.name);
        assert_eq!(
            spec.in_page,
            matches!(member(entry, "in_page"), Json::Bool(true))
        );
    }

    // The corpus's own classification and the port's must agree: an option from
    // options.cpp is on a page (addOptionToPage, options.cpp:270-288) and an
    // EXTERNAL_OPTION is not (add_external only fills `options`, options.cpp:311).
    for spec in table.specs() {
        let external = spec.source == Source::ExternalOptionsJson;
        assert_eq!(spec.in_page, !external, "{}", spec.name);
        assert_eq!(spec.page == "external_options", external, "{}", spec.name);
    }
}

fn check_assigned(owner: &str, slot: &str, actual: &Assigned, node: &Json) {
    let kind = text(node, "kind");
    match actual {
        Assigned::Literal(value) => {
            assert_eq!(kind, "literal", "{owner}.{slot}");
            let value_of = member(node, "value");
            match (value, value_of) {
                (Value::Bool(lhs), Json::Bool(rhs)) => assert_eq!(lhs, rhs, "{owner}.{slot}"),
                (Value::Int(lhs), Json::Int(rhs)) => {
                    assert_eq!(i64::from(*lhs), *rhs, "{owner}.{slot}");
                }
                (Value::Float(lhs), Json::Float(rhs)) => {
                    // The corpus stores the reference's 32-bit floats widened
                    // exactly, so this comparison is exact, not approximate.
                    assert_eq!(f64::from(*lhs), *rhs, "{owner}.{slot}");
                }
                (Value::Text(lhs), Json::Str(rhs)) => assert_eq!(lhs, rhs, "{owner}.{slot}"),
                (_, other) => panic!("{owner}.{slot}: corpus and port disagree: {other}"),
            }
        }
        Assigned::Runtime { expr, note } => {
            assert_eq!(kind, "runtime", "{owner}.{slot}");
            assert_eq!(expr, text(node, "expr"), "{owner}.{slot}");
            assert!(
                !note.is_empty(),
                "{owner}.{slot}: runtime slot without a reason"
            );
        }
    }
}

fn check_optional(owner: &str, slot: &str, actual: Option<&Assigned>, node: &Json) {
    match (actual, node) {
        (Some(assigned), Json::Object(_)) => check_assigned(owner, slot, assigned, node),
        (None, Json::Null) => {}
        (None, other) => panic!("{owner}.{slot}: port has no value, corpus has {other}"),
        (Some(assigned), Json::Null) => {
            panic!("{owner}.{slot}: port has {assigned:?}, corpus has null");
        }
        (Some(_), other) => panic!("{owner}.{slot}: unexpected corpus node {other}"),
    }
}

#[test]
fn spec_values_match_the_corpus() {
    let table = Table::parse(&table_text()).expect("table parses");
    let json = table_json();
    for (spec, entry) in table.specs().iter().zip(entries(&json)) {
        let name = spec.name.as_str();
        assert_eq!(spec.name, text(entry, "name"));
        check_assigned(name, "default", &spec.default, member(entry, "default"));
        check_assigned(name, "initial", &spec.initial, member(entry, "initial"));
        check_optional(name, "min", spec.min.as_ref(), member(entry, "min"));
        check_optional(name, "max", spec.max.as_ref(), member(entry, "max"));
        check_optional(name, "step", spec.step.as_ref(), member(entry, "step"));
        check_optional(
            name,
            "max_length",
            spec.max_length.as_ref(),
            member(entry, "max_length"),
        );
        match (&spec.format, member(entry, "format")) {
            (Some(format), Json::Object(_)) => {
                assert_eq!(
                    format,
                    &text(member(entry, "format"), "value"),
                    "{name}.format"
                );
            }
            (None, Json::Null) => {}
            (actual, corpus) => panic!("{name}.format: {actual:?} vs {corpus}"),
        }
        match (&spec.items, member(entry, "items")) {
            (Some(Items::Literal(items)), Json::Object(_)) => {
                assert_eq!(
                    text(member(entry, "items"), "kind"),
                    "literal",
                    "{name}.items"
                );
                let Json::Array(corpus_items) = member(member(entry, "items"), "items") else {
                    panic!("{name}.items is not an array");
                };
                assert_eq!(items.len(), corpus_items.len(), "{name}.items");
                for (item, corpus_item) in items.iter().zip(corpus_items) {
                    match (item, member(corpus_item, "value")) {
                        (Value::Text(lhs), Json::Str(rhs)) => assert_eq!(lhs, rhs, "{name}.items"),
                        (Value::Int(lhs), Json::Int(rhs)) => {
                            assert_eq!(i64::from(*lhs), *rhs, "{name}.items");
                        }
                        (actual, corpus) => panic!("{name}.items: {actual:?} vs {corpus}"),
                    }
                }
            }
            (Some(Items::Runtime { expr, .. }), Json::Object(_)) => {
                assert_eq!(
                    text(member(entry, "items"), "kind"),
                    "runtime",
                    "{name}.items"
                );
                assert_eq!(expr, text(member(entry, "items"), "expr"), "{name}.items");
            }
            (None, Json::Null) => {}
            (actual, corpus) => panic!("{name}.items: {actual:?} vs {corpus}"),
        }
    }
}

#[test]
fn the_add_overloads_keep_the_values_the_reference_writes() {
    let table = Table::parse(&table_text()).expect("table parses");
    let spec = |name: &str| -> &OptionSpec {
        table
            .specs()
            .iter()
            .find(|spec| spec.name == name)
            .unwrap_or_else(|| panic!("{name} is missing from the table"))
    };

    // SEASON_LENGTH is registered as add(..., 14, 127, 91, COPT_ALWAYS_HIDE)
    // (options.cpp:2794).
    let season = spec("SEASON_LENGTH");
    assert_eq!(season.default, Assigned::Literal(Value::Int(91)));
    assert_eq!(season.min, Some(Assigned::Literal(Value::Int(14))));
    assert_eq!(season.max, Some(Assigned::Literal(Value::Int(127))));
    assert_eq!(season.format.as_deref(), Some("%i"));

    // MONSTER_SPEED carries the printf format "%i%%", which is why its saved
    // value is "100%" and setValue() strips the '%' (options.cpp:2782,1082-1086).
    let speed = spec("MONSTER_SPEED");
    assert_eq!(speed.format.as_deref(), Some("%i%%"));

    // A float step of 0.01 is the 32-bit float the reference stores
    // (options.cpp:2790); comparing the widened value proves no double rounding.
    let evolution = spec("EVOLUTION_INVERSE_MULTIPLIER");
    assert_eq!(evolution.opt_type, OptionType::Float);
    assert_eq!(
        evolution.step,
        Some(Assigned::Literal(Value::Float(0.01_f32)))
    );
    assert_eq!(f64::from(0.01_f32), 0.009_999_999_776_482_582_f64);

    // An EXTERNAL_OPTION keeps the declared value as `initial` but resets to the
    // zero default on rejection (options.cpp:311-348, worldfactory.cpp:2080-2088).
    let hunger = spec("PLAYER_HUNGER_RATE");
    assert_eq!(hunger.initial, Assigned::Literal(Value::Float(1.0)));
    assert_eq!(hunger.default, Assigned::Literal(Value::Float(0.0)));
}

fn options() -> Options {
    Options::new(Table::parse(&table_text()).expect("table parses"))
}

#[test]
fn int_values_outside_the_range_fall_back_to_the_default() {
    // setValue( int ) assigns, then replaces an out-of-range value with
    // iDefault (options.cpp:1055-1066).  SEASON_LENGTH is 14..127, default 91
    // (options.cpp:2794).
    let mut options = options();
    assert_eq!(options.get("SEASON_LENGTH"), Some(&Value::Int(91)));

    assert!(options.set("SEASON_LENGTH", Value::Int(14)));
    assert_eq!(options.get("SEASON_LENGTH"), Some(&Value::Int(14)));
    assert!(options.set("SEASON_LENGTH", Value::Int(127)));
    assert_eq!(options.get("SEASON_LENGTH"), Some(&Value::Int(127)));

    assert!(options.set("SEASON_LENGTH", Value::Int(128)));
    assert_eq!(options.get("SEASON_LENGTH"), Some(&Value::Int(91)));
    assert!(options.set("SEASON_LENGTH", Value::Int(13)));
    assert_eq!(options.get("SEASON_LENGTH"), Some(&Value::Int(91)));
}

#[test]
fn float_values_outside_the_range_fall_back_to_the_default() {
    // The float overload does the same with fDefault (options.cpp:1042-1053).
    // EVOLUTION_INVERSE_MULTIPLIER is 0.0..100.0, default 1.0, step 0.01.
    let mut options = options();
    assert_eq!(
        options.get("EVOLUTION_INVERSE_MULTIPLIER"),
        Some(&Value::Float(1.0))
    );

    assert!(options.set("EVOLUTION_INVERSE_MULTIPLIER", Value::Float(0.0)));
    assert_eq!(
        options.get("EVOLUTION_INVERSE_MULTIPLIER"),
        Some(&Value::Float(0.0))
    );
    assert!(options.set("EVOLUTION_INVERSE_MULTIPLIER", Value::Float(100.5)));
    assert_eq!(
        options.get("EVOLUTION_INVERSE_MULTIPLIER"),
        Some(&Value::Float(1.0))
    );
    assert!(options.set("EVOLUTION_INVERSE_MULTIPLIER", Value::Float(-0.5)));
    assert_eq!(
        options.get("EVOLUTION_INVERSE_MULTIPLIER"),
        Some(&Value::Float(1.0))
    );
}

#[test]
fn int_values_set_from_text_strip_one_trailing_percent_sign() {
    // setValue( const std::string & ) strips a trailing '%' before parsing,
    // because an int option with format "%i%%" saves as e.g. "100%"
    // (options.cpp:1081-1086).
    let mut options = options();
    assert!(options.set_text("MONSTER_SPEED", "150%"));
    assert_eq!(options.get("MONSTER_SPEED"), Some(&Value::Int(150)));

    // The strip is unconditional — it is not tied to the option's format — so even
    // an option that saves as plain "%i" accepts "100%".
    assert!(options.set_text("SEASON_LENGTH", "100%"));
    assert_eq!(options.get("SEASON_LENGTH"), Some(&Value::Int(100)));
    // Only one sign is stripped, and only at the end.
    assert!(options.set_text("SEASON_LENGTH", "100%%"));
    assert_eq!(options.get("SEASON_LENGTH"), Some(&Value::Int(91)));
}

#[test]
fn unparseable_int_text_falls_back_to_the_default() {
    // A parse failure stores iDefault (options.cpp:1083-1098), and
    // try_parse_integer() rejects stray characters after the number
    // (try_parse_integer.cpp:45-50).
    let mut options = options();
    assert!(options.set_text("SEASON_LENGTH", "100"));
    assert_eq!(options.get("SEASON_LENGTH"), Some(&Value::Int(100)));

    for rejected in ["abc", "12x", "", "  ", "1.5", "999999999999999999999"] {
        assert!(options.set_text("SEASON_LENGTH", "100"));
        assert!(options.set_text("SEASON_LENGTH", rejected), "{rejected:?}");
        assert_eq!(
            options.get("SEASON_LENGTH"),
            Some(&Value::Int(91)),
            "{rejected:?}"
        );
    }

    // Leading and trailing whitespace is accepted: the stream skips it and the
    // stray-character probe then hits end of input (try_parse_integer.cpp:26-50).
    assert!(options.set_text("SEASON_LENGTH", " 120 "));
    assert_eq!(options.get("SEASON_LENGTH"), Some(&Value::Int(120)));
}

#[test]
fn bool_values_set_from_text_accept_exactly_four_spellings() {
    // setValue( string ) on a bool option is
    // `bSet = s == "True" || s == "true" || s == "T" || s == "t"`
    // (options.cpp:1075-1077), so every other spelling means false.
    let mut options = options();
    assert_eq!(options.get("ETERNAL_SEASON"), Some(&Value::Bool(false)));

    for truthy in ["True", "true", "T", "t"] {
        assert!(options.set_text("ETERNAL_SEASON", truthy));
        assert_eq!(
            options.get("ETERNAL_SEASON"),
            Some(&Value::Bool(true)),
            "{truthy:?}"
        );
    }
    for falsy in ["TRUE", "yes", "1", "False", "false", "", " t"] {
        assert!(options.set_text("ETERNAL_SEASON", "true"));
        assert!(options.set_text("ETERNAL_SEASON", falsy));
        assert_eq!(
            options.get("ETERNAL_SEASON"),
            Some(&Value::Bool(false)),
            "{falsy:?}"
        );
    }
}

#[test]
fn float_text_that_does_not_parse_leaves_the_value_alone() {
    // The float branch of setValue( string ) parses with a stream and, when the
    // stream fails, only logs: the current value survives
    // (options.cpp:1110-1118).  An int would have reset to its default there.
    let mut options = options();
    assert!(options.set_text("EVOLUTION_INVERSE_MULTIPLIER", "2.5"));
    assert_eq!(
        options.get("EVOLUTION_INVERSE_MULTIPLIER"),
        Some(&Value::Float(2.5))
    );

    assert!(options.set_text("EVOLUTION_INVERSE_MULTIPLIER", "fast"));
    assert_eq!(
        options.get("EVOLUTION_INVERSE_MULTIPLIER"),
        Some(&Value::Float(2.5))
    );

    // The stream stops at the first character it cannot use, so trailing text is
    // not a failure: "3.5x" is 3.5.
    assert!(options.set_text("EVOLUTION_INVERSE_MULTIPLIER", "3.5x"));
    assert_eq!(
        options.get("EVOLUTION_INVERSE_MULTIPLIER"),
        Some(&Value::Float(3.5))
    );

    // A parsed value still goes through setValue( float ), which replaces an
    // out-of-range value with fDefault (options.cpp:1042-1053).
    assert!(options.set_text("EVOLUTION_INVERSE_MULTIPLIER", "1000"));
    assert_eq!(
        options.get("EVOLUTION_INVERSE_MULTIPLIER"),
        Some(&Value::Float(1.0))
    );
}

#[test]
fn string_select_ignores_a_value_that_is_not_one_of_its_items() {
    // setValue( string ) on a string_select only assigns when getItemPos() finds
    // the value, and otherwise keeps the current one (options.cpp:1069-1073).
    let mut options = options();
    let vertical = Value::Text("vertical".to_owned());
    let horizontal = Value::Text("horizontal".to_owned());

    // MORALE_STYLE's items are "vertical" and "horizontal" (options.cpp:2151-2155),
    // so the "Vertical" the source writes as its default is not acceptable either.
    assert_eq!(options.get("MORALE_STYLE"), Some(&vertical));
    assert!(options.set_text("MORALE_STYLE", "Vertical"));
    assert_eq!(options.get("MORALE_STYLE"), Some(&vertical));

    assert!(options.set_text("MORALE_STYLE", "horizontal"));
    assert_eq!(options.get("MORALE_STYLE"), Some(&horizontal));
    assert!(options.set_text("MORALE_STYLE", "Horizontal"));
    assert_eq!(options.get("MORALE_STYLE"), Some(&horizontal));
}

#[test]
fn string_input_is_cut_to_its_maximum_length() {
    // setValue( string ) on a string_input assigns sSetIn.substr(0, iMaxLength)
    // when iMaxLength > 0 (options.cpp:1074-1076).
    let mut options = options();
    let long = "x".repeat(40);
    assert!(options.set_text("DEF_CHAR_NAME", &long));
    assert_eq!(
        options.get("DEF_CHAR_NAME"),
        Some(&Value::Text("x".repeat(30)))
    );

    // add_external leaves iMaxLength at 0 (options.cpp:311-348), so a JSON-declared
    // string_input has no limit at all.
    assert!(options.set_text("DEFAULT_REGION", &long));
    assert_eq!(options.get("DEFAULT_REGION"), Some(&Value::Text(long)));
}
