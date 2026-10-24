//! Drawing from a group: one of these, or all of these.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::item_group_draw::{Definition, Entry, draw, load_definitions};
use cdda_core_types::rng::Rolls;
use std::fmt::Write as _;
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

/// The dice the reference draws, with the ranges it asks for.
///
/// Reference: `Item_group::create` (`src/item_group.cpp:832-864`). A **collection** asks `rng( 0, 99 )` per entry and skips it when the answer is at least its chance:
/// `rng( 0, 99 ) >= elem->get_probability( false )`. A **distribution** asks **once** for `rng( 0, sum_prob - 1 )`, then walks the entries subtracting each one's weight and
/// takes the first that takes the running total below zero. The ranges are not decoration: a replay is a list of `(range, answer)` pairs and a port that asked
/// `rng( 1, 100 )` instead would not consume the reference's dice at all, which is why the recording panics rather than quietly answering.
#[test]
fn the_draw_asks_the_reference_for_its_dice_ranges() {
    // Three entries of equal weight: `sum_prob` is three, so the distribution's single draw is `rng( 0, 2 )`, and a zero takes the first entry.
    let groups = from_json(
        r#"[{"type":"item_group","id":"one_of","subtype":"distribution","entries":[
            {"item":"meat","prob":1},{"item":"bone","prob":1},{"item":"skin","prob":1}]},
           {"type":"item_group","id":"a_pile","entries":[
            {"item":"meat","prob":50},{"item":"bone","prob":100}]}]"#,
    );
    let distribution_rolls = "int 0 2 0\nint 1 1 1\n";
    let mut rolls = Rolls::replaying(cdda_core_types::rng::Recording::parse(distribution_rolls).expect("a recording"));
    let drawn = draw(&groups, "one_of", &mut rolls);
    assert_eq!(drawn, vec![("meat".to_owned(), 1)], "the zero takes the first entry");

    // The collection: one `rng( 0, 99 )` per entry, the first fifty and then a hundred - so the first entry is skipped at 50 and the second is taken at 0.
    let collection_rolls = "int 0 99 50\nint 0 99 0\nint 1 1 1\n";
    let mut rolls = Rolls::replaying(cdda_core_types::rng::Recording::parse(collection_rolls).expect("a recording"));
    let drawn = draw(&groups, "a_pile", &mut rolls);
    assert_eq!(drawn, vec![("bone".to_owned(), 1)], "the chance that failed, then the one that passed");
}

/// A holiday item is skipped in the middle of a distribution, and the walk keeps going.
///
/// Reference: `Item_group::create` (`src/item_group.cpp:844-856`): the loop tests `( ev_based && prob == 0 ) || p >= 0` and **continues**, where `prob` is
/// `get_probability( false )` - the holiday-gated chance - and `p` was already reduced by `get_probability( true )`, the raw weight. So an event-based entry outside its
/// holiday is passed over *even when the die chose it*, and its weight still counts towards `sum_prob`. A port that treated the two probabilities as one would give the
/// holiday item out of season.
#[test]
fn a_holiday_item_is_passed_over_and_the_next_one_is_taken() {
    let groups = from_json(
        r#"[{"type":"item_group","id":"one_of","subtype":"distribution","entries":[
            {"item":"meat","prob":1,"event":"christmas"},{"item":"bone","prob":1}]}]"#,
    );
    // The die is a zero, which chooses the first entry by weight - and the first entry is out of season.
    let recording = "int 0 1 0\nint 1 1 1\n";
    let mut rolls = Rolls::replaying(cdda_core_types::rng::Recording::parse(recording).expect("a recording"));
    let drawn = draw(&groups, "one_of", &mut rolls);
    assert_eq!(
        drawn,
        vec![("bone".to_owned(), 1)],
        "the holiday entry was passed over, and the walk carried on to the next one"
    );
}

/// A count that cannot vary draws no die at all.
///
/// Reference: `Single_item_creator::create` (`src/item_group.cpp:277-284`): `cnt = ( modifier_count.first == modifier_count.second ) ? modifier_count.first : rng(
/// modifier_count.first, modifier_count.second )`. The conditional is the rule - a range of one value is **not** a range, and the reference does not ask the dice about
/// it. A port that drew `rng( 5, 5 )` would consume a die the reference never rolled, which puts every later draw of the session one behind.
#[test]
fn a_fixed_count_draws_no_die() {
    let groups = from_json(
        r#"[{"type":"item_group","id":"a_box","subtype":"distribution","entries":[
            {"item":"meat","prob":1,"count":5}]}]"#,
    );
    // One die for the distribution's own choice, and **nothing after it**: the count is five however the dice fall.
    let recording = "int 0 0 0\n";
    let mut rolls = Rolls::replaying(
        cdda_core_types::rng::Recording::parse(recording).expect("a recording"),
    );
    let drawn = draw(&groups, "a_box", &mut rolls);
    assert_eq!(drawn, vec![("meat".to_owned(), 5)]);
    assert_eq!(
        rolls.recording_remaining(),
        Some(0),
        "the whole recording was played, which is how a missing or extra draw shows up"
    );
}

/// A count of zero produces nothing, and is not the same as a count of one.
///
/// Reference: `Single_item_creator::create` (`src/item_group.cpp:286`): `for( ; cnt > 0; cnt-- )`, so a zero count leaves the list untouched. The port used to floor every
/// count at one, which turned "none of these" into "one of these".
#[test]
fn a_count_of_zero_produces_nothing() {
    let groups = from_json(
        r#"[{"type":"item_group","id":"an_empty_box","subtype":"distribution","entries":[
            {"item":"meat","prob":1,"count":0},{"item":"bone","prob":3,"count":1}]}]"#,
    );
    // The die is a zero, which chooses the first entry by weight - and the first entry asks for none of them. The range is `rng( 0, 3 )` and not `rng( 0, 2 )`:
    // the weight is the entry's **probability**, so a zero count keeps its entry in the group's total (`add_entry`, `src/item_group.cpp:813-821`).
    let recording = "int 0 3 0\n";
    let mut rolls = Rolls::replaying(
        cdda_core_types::rng::Recording::parse(recording).expect("a recording"),
    );
    assert!(draw(&groups, "an_empty_box", &mut rolls).is_empty());
}

/// A group that names one of its own ancestors stops there, and says so by producing nothing from that branch.
///
/// Reference: `Single_item_creator::create` (`src/item_group.cpp:307-312`): the expansion keeps a `RecursionList` of the groups it is inside, and a group that is already on
/// it is an **error** - `debugmsg( "recursion in item spawn list %s" )` - and the branch ends. The test is a set of names, not a depth, which is what makes a cycle of any
/// length stop at the second visit.
#[test]
fn a_cycle_stops_at_the_second_visit() {
    let groups = from_json(
        r#"[{"type":"item_group","id":"a","entries":[{"group":"b"}]},
            {"type":"item_group","id":"b","entries":[{"group":"a"},{"item":"meat"}]}]"#,
    );
    let mut rolls = Rolls::new(7);
    let drawn = draw(&groups, "a", &mut rolls);
    assert_eq!(
        drawn,
        vec![("meat".to_owned(), 1)],
        "the cycle produced nothing, and the entry beside it still did: {drawn:?}"
    );
}

/// Nesting deeper than the old guard allowed is not an error, and loses nothing.
///
/// The reference's guard is a set of names, so a chain of ten distinct groups is drawn in full; the port used to stop at a depth of eight, which silently dropped the items
/// of a group that was nested legally deep. The fixture is ten groups, each holding the next one.
#[test]
fn deep_nesting_is_drawn_in_full() {
    let mut json = String::from("[");
    for level in 0..10 {
        if level > 0 {
            json.push(',');
        }
        let body = if level == 9 {
            r#"{"item":"meat"}"#.to_owned()
        } else {
            format!(r#"{{"group":"g{}"}}"#, level + 1)
        };
        write!(
            json,
            r#"{{"type":"item_group","id":"g{level}","entries":[{body}]}}"#
        )
        .expect("a string write cannot fail");
    }
    json.push(']');
    let groups = from_json(&json);
    let mut rolls = Rolls::new(11);
    let drawn = draw(&groups, "g0", &mut rolls);
    assert_eq!(drawn, vec![("meat".to_owned(), 1)], "ten levels down: {drawn:?}");
}
