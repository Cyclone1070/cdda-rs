#![allow(clippy::expect_used, clippy::indexing_slicing)]
//! Reading snippet categories, in every shape the data writes them.
//!
//! Reference: `/snippets/types` (`snippet.cpp`): a category holds `text`, which is one string, or a list of strings, or a list of objects with
//! `id`/`text`/`weight`. A line with no id is named after its category and its position, because `same_snippet` remembers a line by id.

use cdda_content::snippets::categories;

#[test]
fn a_category_reads_all_three_shapes_the_data_writes() {
    let value = cdda_json::parse(
        br#"[
            {"type":"snippet","category":"one_line","text":"a single line"},
            {"type":"snippet","category":"bare","text":["first","second"]},
            {"type":"snippet","category":"named","text":[
                {"id":"named_a","text":"with an id","weight":50},
                {"text":"without an id"}
            ]},
            {"type":"snippet","category":"one_line","text":"appended"}
        ]"#
        .as_slice(),
    )
    .expect("json");
    let read = categories(&value);
    assert_eq!(read.len(), 3, "three categories: {read:?}");

    let one_line = read
        .iter()
        .find(|(name, _)| name == "one_line")
        .expect("present");
    assert_eq!(
        one_line.1.len(),
        2,
        "a category written twice appends, the way the reference does"
    );
    assert_eq!(one_line.1.first().expect("a line").text, "a single line");
    assert_eq!(
        one_line.1.first().expect("a line").id,
        "one_line_0",
        "a line with no id is named after its category and position"
    );

    let bare = read
        .iter()
        .find(|(name, _)| name == "bare")
        .expect("present");
    assert_eq!(bare.1.len(), 2);
    assert_eq!(bare.1.get(1).expect("a line").text, "second");
    assert_eq!(
        bare.1.get(1).expect("a line").weight,
        100,
        "the default weight"
    );

    let named = read
        .iter()
        .find(|(name, _)| name == "named")
        .expect("present");
    assert_eq!(named.1.first().expect("a line").id, "named_a");
    assert_eq!(
        named.1.first().expect("a line").weight,
        50,
        "the weight the data wrote"
    );
    assert_eq!(
        named.1.get(1).expect("a line").id,
        "named_1",
        "the second line keeps its position even beside a named first line"
    );
}
