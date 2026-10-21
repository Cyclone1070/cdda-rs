//! The option table, checked item by item against `corpus/options/table.json`.
//!
//! `corpus/options/table.json` is generated from the pinned reference checkout by
//! `tools/gen_options.py`; this test suite is the only thing that reads it, so a
//! disagreement here means either the generator or the port is wrong, and the
//! corpus wins until the generator is re-read.
//!
//! Reference: `references/Cataclysm-DDA/src/options.cpp` (line numbers in
//! comments) and `src/options.h`.
#![allow(clippy::expect_used, clippy::float_cmp, clippy::panic)]
// Comparisons of f32 are exact on purpose: the corpus stores the reference's
// 32-bit floats widened exactly, so equal means bit-equal.

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
    assert_eq!(spec_of(&table, "SEASON_LENGTH").opt_type, OptionType::Int);
    assert_eq!(
        spec_of(&table, "PLAYER_HUNGER_RATE").opt_type,
        OptionType::Float
    );
    assert_eq!(
        spec_of(&table, "PREVENT_OCCLUSION").opt_type,
        OptionType::IntMap
    );
}

fn spec_of<'a>(table: &'a Table, name: &str) -> &'a OptionSpec {
    table
        .specs()
        .iter()
        .find(|spec| spec.name == name)
        .unwrap_or_else(|| panic!("{name} is missing from the table"))
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
    let spec = |name: &str| spec_of(&table, name);

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

#[test]
fn options_the_reference_fills_at_runtime_start_unset() {
    // build_display_list() and SystemLocale::UseMetricSystem() run when the game
    // starts (options.cpp:1809,2650), so the table records the expression instead
    // of a value.  Such an option exists, but has no value until one is supplied.
    let mut options = options();
    for name in [
        "DISPLAY",
        "USE_METRIC_SPEEDS",
        "USE_METRIC_WEIGHTS",
        "DISTANCE_UNITS",
    ] {
        assert!(options.has_option(name), "{name}");
        assert_eq!(options.get(name), None, "{name}");
    }
    assert!(options.set_text("USE_METRIC_SPEEDS", "km/h"));
    assert_eq!(
        options.get("USE_METRIC_SPEEDS"),
        Some(&Value::Text("km/h".to_owned()))
    );

    // TILES writes a literal default — but a runtime item list — so it starts at
    // that literal.  The reference then replaces the default with items[0] if the
    // installed tilesets do not include it (options.cpp:357-370); a port that
    // cannot see the installed tilesets cannot reproduce that substitution.
    assert_eq!(
        options.get("TILES"),
        Some(&Value::Text("UltimateCataclysm".to_owned()))
    );
    assert!(matches!(
        options.table().spec("TILES").map(|spec| &spec.items),
        Some(Some(Items::Runtime { .. }))
    ));
}

#[test]
fn int_map_values_must_be_one_of_the_map_keys() {
    // options.cpp:1100-1109: parse, then findInt(); a value the map does not hold
    // becomes iDefault.  PREVENT_OCCLUSION is {0, 1, 2}, initial and default 2.
    let mut options = options();
    assert_eq!(options.get("PREVENT_OCCLUSION"), Some(&Value::Int(2)));
    assert!(options.set_text("PREVENT_OCCLUSION", "1"));
    assert_eq!(options.get("PREVENT_OCCLUSION"), Some(&Value::Int(1)));
    assert!(options.set_text("PREVENT_OCCLUSION", "7"));
    assert_eq!(options.get("PREVENT_OCCLUSION"), Some(&Value::Int(2)));
    assert!(options.set_text("PREVENT_OCCLUSION", "many"));
    assert_eq!(options.get("PREVENT_OCCLUSION"), Some(&Value::Int(2)));

    // The typed overloads refuse a value of the wrong type, and an unknown
    // option, without touching anything (options.cpp:1044-1047,1106-1110).
    assert!(!options.set("SEASON_LENGTH", Value::Bool(true)));
    assert_eq!(options.get("SEASON_LENGTH"), Some(&Value::Int(91)));
    assert!(!options.set("ETERNAL_SEASON", Value::Int(1)));
    assert_eq!(options.get("ETERNAL_SEASON"), Some(&Value::Bool(false)));
    assert!(!options.set("NO_SUCH_OPTION", Value::Int(1)));
    assert!(!options.has_option("NO_SUCH_OPTION"));
    assert_eq!(options.get("NO_SUCH_OPTION"), None);
}

#[test]
fn loading_a_saved_file_applies_entries_and_leaves_the_rest_alone() {
    // deserialize() walks the array and calls setValue() per entry, so a name the
    // file omits keeps its start value, an unknown name is silently ignored (the
    // map default-constructs a VOID option), and an out-of-range value becomes the
    // default (options.cpp:3972-3997; MUSIC_VOLUME is 0..128 with default 100).
    let mut options = options();
    let saved = r#"[
        { "info": "Season length", "default": "91", "name": "SEASON_LENGTH", "value": "100" },
        { "name": "MONSTER_SPEED", "value": "150%" },
        { "name": "NOT_AN_OPTION", "value": "whatever" },
        { "name": "MUSIC_VOLUME", "value": "999" }
    ]"#;
    let report = options
        .load(saved)
        .expect("a saved file the game could write");

    assert_eq!(options.get("SEASON_LENGTH"), Some(&Value::Int(100)));
    assert_eq!(options.get("MONSTER_SPEED"), Some(&Value::Int(150)));
    assert_eq!(options.get("MUSIC_VOLUME"), Some(&Value::Int(100)));
    assert!(!options.has_option("NOT_AN_OPTION"));
    // Untouched: CITY_SIZE is 0..16 with default 8 and the file says nothing.
    assert_eq!(options.get("CITY_SIZE"), Some(&Value::Int(8)));

    assert_eq!(report.applied, 3);
    assert_eq!(report.skipped_unknown, 1);
}

#[test]
fn a_saved_entry_missing_a_value_is_an_error_not_a_silent_skip() {
    // The reference reads both members with get_string(), which throws, so a
    // malformed entry fails the load instead of being ignored.
    let mut options = options();
    assert!(options.load(r#"[ { "name": "SEASON_LENGTH" } ]"#).is_err());
    assert!(options.load(r#"[ { "value": "100" } ]"#).is_err());
    assert!(
        options
            .load(r#"[ { "name": "SEASON_LENGTH", "value": 100 } ]"#)
            .is_err()
    );
    assert!(options.load("not json").is_err());
    // A failed load leaves the values it did not reach untouched.
    assert_eq!(options.get("SEASON_LENGTH"), Some(&Value::Int(91)));
}

#[test]
fn an_entry_for_a_removed_option_is_skipped_before_anything_else() {
    // deserialize() drops eight option names the reference no longer reads from a
    // save file (options.cpp:3976-3983) — and INITIAL_STAT_POINTS is one of them
    // even though the option itself still exists as a JSON-declared external
    // option (external_options.json:578), so the skip has to come first.
    let mut options = options();
    assert!(options.has_option("INITIAL_STAT_POINTS"));
    // The list itself is part of the table, extracted from the reference.
    assert_eq!(options.table().removed().len(), 8);
    assert!(options.table().is_removed("INITIAL_STAT_POINTS"));
    let report = options
        .load(r#"[ { "name": "INITIAL_STAT_POINTS", "value": "99" } ]"#)
        .expect("a saved file the game could write");
    assert_eq!(report.skipped_removed, 1);
    assert_eq!(report.applied, 0);
    assert_eq!(report.skipped_unknown, 0);
    assert_eq!(options.get("INITIAL_STAT_POINTS"), Some(&Value::Int(6)));
}

#[test]
fn old_option_names_and_values_are_migrated_before_the_value_is_applied() {
    // migrateOptionName() renames DELETE_WORLD to WORLD_END and
    // migrateOptionValue() maps "no" to "keep" and "yes" to "delete"
    // (options.cpp:196-207,3999-4030); an unmapped value passes through.
    let mut options = options();
    let report = options
        .load(r#"[ { "name": "DELETE_WORLD", "value": "no" } ]"#)
        .expect("a saved file the game could write");
    assert_eq!(report.migrated, 1);
    assert_eq!(
        options.get("WORLD_END"),
        Some(&Value::Text("keep".to_owned()))
    );

    options
        .load(r#"[ { "name": "DELETE_WORLD", "value": "yes" } ]"#)
        .expect("a saved file the game could write");
    assert_eq!(
        options.get("WORLD_END"),
        Some(&Value::Text("delete".to_owned()))
    );

    options
        .load(r#"[ { "name": "DELETE_WORLD", "value": "query" } ]"#)
        .expect("a saved file the game could write");
    assert_eq!(
        options.get("WORLD_END"),
        Some(&Value::Text("query".to_owned()))
    );

    // MONSTER_UPGRADE_FACTOR became EVOLUTION_INVERSE_MULTIPLIER, and its value is
    // divided by four and printed with two decimals before being parsed back
    // (options.cpp:4005-4015), so 8 becomes 2.00 and then 2.0.
    options
        .load(r#"[ { "name": "MONSTER_UPGRADE_FACTOR", "value": "8" } ]"#)
        .expect("a saved file the game could write");
    assert_eq!(
        options.get("EVOLUTION_INVERSE_MULTIPLIER"),
        Some(&Value::Float(2.0))
    );
}

fn saved_entries(text: &str) -> Vec<(String, String)> {
    let json = cdda_json::parse(text.as_bytes()).expect("saved options.json parses");
    let Json::Array(list) = json else {
        panic!("saved options.json is not an array");
    };
    list.iter()
        .map(|entry| {
            (
                text_of(entry, "name").to_owned(),
                text_of(entry, "value").to_owned(),
            )
        })
        .collect()
}

fn text_of<'a>(entry: &'a Json, key: &str) -> &'a str {
    match member(entry, key) {
        Json::Str(text) => text,
        other => panic!("{key} is {other}, expected a string"),
    }
}

#[test]
fn saving_writes_exactly_the_options_that_are_on_a_page() {
    // serialize() walks the pages and writes only the options placed on one, so an
    // EXTERNAL_OPTION (which add_external never puts on a page,
    // options.cpp:311-348) is never saved; deserialize() re-derives it from data.
    let mut options = options();
    assert!(options.set_text("SEASON_LENGTH", "100"));
    // The exception is the option whose start value the reference computes at
    // startup and which nobody has set: there is nothing honest to write for it.
    let on_page = options
        .table()
        .specs()
        .iter()
        .filter(|spec| spec.in_page)
        .count();
    let start_unset = match meta(&table_json(), "options_starting_unset") {
        Json::Array(names) => names.clone(),
        _ => panic!("meta.options_starting_unset is not an array"),
    };
    let unset = options
        .table()
        .specs()
        .iter()
        .filter(|spec| matches!(spec.initial, Assigned::Runtime { .. }))
        .count();
    assert_eq!(unset, start_unset.len());
    let saved = saved_entries(&options.save());
    assert_eq!(saved.len(), on_page - unset);

    let names: Vec<&str> = saved.iter().map(|(name, _)| name.as_str()).collect();
    assert!(names.contains(&"SEASON_LENGTH"));
    assert!(!names.contains(&"PLAYER_HUNGER_RATE"));

    // What is written loads back unchanged.
    let mut reloaded = Options::new(Table::parse(&table_text()).expect("table parses"));
    reloaded.load(&options.save()).expect("a round trip");
    for spec in options.table().specs() {
        assert_eq!(
            reloaded.get(&spec.name),
            options.get(&spec.name),
            "{}",
            spec.name
        );
    }
}

#[test]
fn a_saved_value_is_written_with_the_options_own_format() {
    // getValue( true ) prints an int with its format ("%i%%" gives "100%"), a float
    // with two decimals, a bool as "true"/"false" and text as itself
    // (options.cpp:725-745,1096-1109).
    let mut options = options();
    assert!(options.set_text("MONSTER_SPEED", "150"));
    assert!(options.set_text("EVOLUTION_INVERSE_MULTIPLIER", "2.5"));
    assert!(options.set_text("ETERNAL_SEASON", "t"));
    assert!(options.set_text("MORALE_STYLE", "horizontal"));
    let saved = saved_entries(&options.save());
    let value = |name: &str| -> &str {
        saved
            .iter()
            .find(|(saved_name, _)| saved_name == name)
            .map_or_else(
                || panic!("{name} is not in the saved file"),
                |(_, value)| value.as_str(),
            )
    };
    assert_eq!(value("MONSTER_SPEED"), "150%");
    assert_eq!(value("SEASON_LENGTH"), "91");
    assert_eq!(value("EVOLUTION_INVERSE_MULTIPLIER"), "2.50");
    assert_eq!(value("ETERNAL_SEASON"), "true");
    assert_eq!(value("MORALE_STYLE"), "horizontal");
}

#[test]
fn a_bool_option_accepts_its_own_type_but_not_another() {
    // The reference sets a bool only from text (options.h:120-122 has no
    // setValue( bool )), computing `bSet` from four spellings
    // (options.cpp:1075-1077); assigning the flag directly is that same rule with
    // the string round trip removed, and still refuses a value of another type.
    let mut options = options();
    assert!(options.set("ETERNAL_SEASON", Value::Bool(true)));
    assert_eq!(options.get("ETERNAL_SEASON"), Some(&Value::Bool(true)));
    assert!(options.set("ETERNAL_SEASON", Value::Bool(false)));
    assert_eq!(options.get("ETERNAL_SEASON"), Some(&Value::Bool(false)));

    assert!(!options.set("ETERNAL_SEASON", Value::Int(1)));
    assert!(!options.set("ETERNAL_SEASON", Value::Float(1.0)));
    assert!(!options.set("MORALE_STYLE", Value::Bool(true)));
    assert_eq!(options.get("ETERNAL_SEASON"), Some(&Value::Bool(false)));
}
