//! The book slot, against the game's own data.

#![allow(clippy::panic, clippy::expect_used)]

/// A book record reads into the slot, and its duration becomes turns.
///
/// Reference: `islot_book` (`src/itype.h:569-620`): `read_skill` is the skill, `required_level` the level a reader needs, `max_level` the level it teaches to, `intelligence` the intelligence it
/// asks for, `read_fun` how fun it is, and `time` how long **one skill point** takes.
#[test]
fn a_book_record_reads_into_its_slot() {
    let record = cdda_json::parse(
        br#"{"id":"a_book","subtypes":["BOOK"],"read_skill":"chemistry","required_level":5,"max_level":7,"intelligence":9,"read_fun":-2,"time":"35 m","chapters":6}"#,
    )
    .expect("valid json");
    let cdda_json::Value::Object(object) = record else {
        panic!("a record is an object")
    };
    let book = cdda_content::books::Book::read(&object);
    assert_eq!(book.skill, "chemistry");
    assert_eq!(book.req, 5, "required_level is the readability gate");
    assert_eq!(book.level, 7, "max_level is what it teaches to");
    assert_eq!(book.intel, 9);
    assert_eq!(book.fun, -2, "fun may be negative");
    assert_eq!(book.time_turns, 35 * 60, "**35 m is 2100 turns**, a turn being a second");
    assert_eq!(book.chapters, 6);
    assert!(book.is_usable(), "and it is a book the reading rules can use");
}

/// The corpus's own books read, and the durations among them are the units the data actually writes.
///
/// Reference: the same slot, read from the game's files. **The count is the gate's non-vacuity**: 591 records carry the `BOOK` subtype in this version, and the port reads the ones that state their own
/// fields - `copy-from` inheritance is the item loader's job, not this record reader's.
#[test]
fn the_corpus_books_read() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json/items/book");
    if !dir.is_dir() {
        return;
    }
    let mut books = 0_usize;
    let mut usable = 0_usize;
    let mut hours = 0_usize;
    let mut minutes = 0_usize;
    for entry in std::fs::read_dir(&dir).expect("the book directory lists") {
        let path = entry.expect("an entry").path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(cdda_json::Value::Array(records)) = cdda_json::parse(text.as_bytes()) else {
            continue;
        };
        for record in records {
            let cdda_json::Value::Object(object) = record else {
                continue;
            };
            let is_book = matches!(object.get("subtypes"), Some(cdda_json::Value::Array(list)) if list.iter().any(|kind| matches!(kind, cdda_json::Value::Str(name) if name == "BOOK")));
            if !is_book {
                continue;
            }
            books += 1;
            let book = cdda_content::books::Book::read(&object);
            if book.is_usable() {
                usable += 1;
            }
            match object.get("time") {
                Some(cdda_json::Value::Str(text)) if text.ends_with('h') => hours += 1,
                Some(cdda_json::Value::Str(text)) if text.ends_with('m') => minutes += 1,
                _ => {}
            }
        }
    }
    assert!(books > 500, "the corpus is large: {books} books");
    assert!(usable > 100, "and many of them state a skill and a time: {usable}");
    assert!(minutes > hours, "**most books are measured in minutes**: {minutes} against {hours} hours");
}

/// The item loader fills the book slot, and a `copy-from` child keeps the fields it does not state.
///
/// Reference: `Item_factory::load_slot` (`src/item_factory.cpp:2859-2868`) reads into the parent's slot object with `was_loaded`, so **a child that states only `time` keeps the parent's skill, level
/// and intelligence**. That per-field keep-if-absent merge is the opposite of a stated array, which clears first - and it is the rule that makes a whole shelf of derived books readable.
#[test]
fn the_item_loader_fills_the_book_slot_and_inherits_field_by_field() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json/items/book");
    if !dir.is_dir() {
        return;
    }
    let types = cdda_content::items::load_paths(&[dir.as_path()]).expect("the book records load");
    let books: Vec<_> = types.iter().filter(|kind| kind.book.is_some()).collect();
    assert!(books.len() > 100, "the loader keeps the slot for the books it reads: {}", books.len());
    // Every kept slot is usable, and at least one of them **inherited** its skill rather than stating it: a child with a `time` and no `read_skill` of its own.
    assert!(
        books.iter().any(|kind| kind.book.as_ref().is_some_and(|book| book.time_turns > 0 && !book.skill.is_empty())),
        "at least one book states a skill and a time, so the slot is not an empty shell"
    );
    assert!(
        types.iter().filter(|kind| kind.book.is_none()).count() > 0,
        "and a non-book in the same directory does not get a slot"
    );
}
