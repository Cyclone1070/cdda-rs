//! Drawing from a group: one of these, or all of these.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::item_group_draw::{Definition, Entry, draw, load_definitions};
use cdda_core_types::rng::Rolls;
use std::collections::BTreeMap;

fn from_json(text: &str) -> BTreeMap<String, Definition> {
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    load_definitions(&value)
}

#[test]
fn a_distribution_gives_exactly_one_thing() {
    let groups = from_json(
        r#"[{"type":"item_group","id":"one_of","subtype":"distribution","entries":[
            {"item":"meat","prob":100},{"item":"bone","prob":100},{"item":"skin","prob":100}]}]"#,
    );
    for seed in 0..20 {
        let mut rolls = Rolls::new(seed);
        let drawn = draw(&groups, "one_of", &mut rolls);
        assert_eq!(
            drawn.len(),
            1,
            "a distribution is one pick, seed {seed}: {drawn:?}"
        );
    }
}

#[test]
fn a_collection_can_give_everything_it_lists() {
    let groups = from_json(
        r#"[{"type":"item_group","id":"a_pile","entries":[
            {"item":"meat"},{"item":"bone"},{"item":"skin"}]}]"#,
    );
    let mut rolls = Rolls::new(3);
    let drawn = draw(&groups, "a_pile", &mut rolls);
    assert_eq!(
        drawn.len(),
        3,
        "all three, since none states a chance: {drawn:?}"
    );
}

#[test]
fn a_chance_of_zero_never_gives_anything() {
    let groups = from_json(
        r#"[{"type":"item_group","id":"never","entries":[
            {"item":"meat","prob":0},{"item":"bone","prob":0}]}]"#,
    );
    for seed in 0..20 {
        let mut rolls = Rolls::new(seed);
        assert!(draw(&groups, "never", &mut rolls).is_empty());
    }
}

#[test]
fn a_count_gives_the_number_it_states() {
    let groups =
        from_json(r#"[{"type":"item_group","id":"six","entries":[{"item":"meat","count":6}]}]"#);
    let mut rolls = Rolls::new(9);
    let drawn = draw(&groups, "six", &mut rolls);
    assert_eq!(drawn, vec![("meat".to_owned(), 6)]);
}

#[test]
fn a_count_range_stays_inside_its_ends() {
    let groups = from_json(
        r#"[{"type":"item_group","id":"some","entries":[{"item":"meat","count":[2,4]}]}]"#,
    );
    for seed in 0..20 {
        let mut rolls = Rolls::new(seed);
        let drawn = draw(&groups, "some", &mut rolls);
        let count = drawn.first().expect("one entry").1;
        assert!((2..=4).contains(&count), "seed {seed}: {count}");
    }
}

#[test]
fn a_nested_group_is_expanded_where_it_stands() {
    let groups = from_json(
        r#"[{"type":"item_group","id":"outer","entries":[{"item":"meat"},{"group":"inner"}]},
            {"type":"item_group","id":"inner","entries":[{"item":"bone"}]}]"#,
    );
    let mut rolls = Rolls::new(11);
    let drawn = draw(&groups, "outer", &mut rolls);
    assert_eq!(
        drawn,
        vec![("meat".to_owned(), 1), ("bone".to_owned(), 1)],
        "the nested group's items appear in its place"
    );
}

#[test]
fn a_group_that_names_itself_stops_rather_than_drawing_forever() {
    let groups = from_json(r#"[{"type":"item_group","id":"loop","entries":[{"group":"loop"}]}]"#);
    let mut rolls = Rolls::new(12);
    assert!(
        draw(&groups, "loop", &mut rolls).is_empty(),
        "a cycle yields nothing"
    );
}

#[test]
fn a_group_that_does_not_exist_yields_nothing_rather_than_inventing_items() {
    let groups: BTreeMap<String, Definition> = BTreeMap::new();
    let mut rolls = Rolls::new(13);
    assert!(draw(&groups, "not_a_group", &mut rolls).is_empty());
}

#[test]
fn the_same_seed_draws_the_same_things() {
    let groups = from_json(
        r#"[{"type":"item_group","id":"one_of","subtype":"distribution","entries":[
            {"item":"meat","prob":50},{"item":"bone","prob":50}]}]"#,
    );
    let draw_with = |seed: u32| {
        let mut rolls = Rolls::new(seed);
        draw(&groups, "one_of", &mut rolls)
    };
    assert_eq!(draw_with(777), draw_with(777));
}

#[test]
fn every_entry_kind_the_loader_knows_is_kept() {
    let value = cdda_json::parse(
        br#"[{"type":"item_group","id":"kinds","entries":[
            {"item":"meat","prob":3,"count":[1,2]},{"group":"other","prob":4},{"item":"null","prob":5}]}]"#,
    )
    .expect("valid json");
    let groups = load_definitions(&value);
    let definition = groups.get("kinds").expect("the group");
    assert_eq!(definition.entries.len(), 3);
    assert!(matches!(
        definition.entries.first(),
        Some(Entry::Item {
            probability: 3,
            count: (1, 2),
            ..
        })
    ));
    assert!(matches!(
        definition.entries.get(1),
        Some(Entry::Group { probability: 4, .. })
    ));
    assert!(matches!(
        definition.entries.get(2),
        Some(Entry::Nothing { probability: 5 })
    ));
}
