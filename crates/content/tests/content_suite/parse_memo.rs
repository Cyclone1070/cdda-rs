//! The parse memo belongs to the process, not to the thread that filled it.
//!
//! Every loader walks the same 6,220 files, and the memo exists so the second walk is free. While it was a `thread_local!` the doc said "this process" and the gate's
//! test threads each paid for the whole tree: the suite was re-doing the work the memo was written to avoid.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::parse_count;

/// A file no other test writes, so the memo's answer is this test's own.
fn fixture(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("cdda-memo-{name}-{}.json", std::process::id()));
    std::fs::write(&path, br#"{"kind":"memo_probe","n":1}"#).expect("the fixture writes");
    path
}

#[test]
fn a_file_parsed_on_one_thread_is_not_parsed_again_on_another() {
    let path = fixture("cross-thread");
    let first = cdda_content::read_json(&path).expect("the fixture parses");
    assert!(
        matches!(&*first, cdda_json::Value::Object(root) if root.get("n").is_some()),
        "the fixture is what it says"
    );

    let seen = std::thread::spawn({
        let path = path.clone();
        move || {
            let before = parse_count();
            let value = cdda_content::read_json(&path).expect("and the second thread reads it");
            let parsed = parse_count() - before;
            (
                matches!(&*value, cdda_json::Value::Object(root) if root.get("n").is_some()),
                parsed,
            )
        }
    })
    .join()
    .expect("the thread runs");

    assert!(seen.0, "the second thread sees the same document");
    assert_eq!(
        seen.1, 0,
        "**the second thread parsed nothing: the memo is the process's**"
    );
}

#[test]
fn the_second_read_on_one_thread_parses_nothing() {
    let path = fixture("same-thread");
    let _ = cdda_content::read_json(&path).expect("the fixture parses");
    let before = parse_count();
    let _ = cdda_content::read_json(&path).expect("and again");
    assert_eq!(
        parse_count(),
        before,
        "**a hit costs no parse, on any thread**"
    );
}
