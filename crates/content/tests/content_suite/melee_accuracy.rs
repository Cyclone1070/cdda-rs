//! What an item's `to_hit` actually means: an object of four qualities, summed on the reference's own scale.
//!
//! Reference: `melee_accuracy` (`item_factory.cpp:3929-3968`) is a collection of int values summed to give an item's melee accuracy. `base_acc` is **-2**, and the three offsets
//! are -1, -2 and -2, so the sum starts at **-7** before the four values are added. An item that states nothing at all gets `grip WEAPON` (3), `length HAND` (0), `surface ANY`
//! (2) and `balance NEUTRAL` (2) - which is **0**, not -2. The -2 default only applies to a record with no `to_hit` member at all.

#![allow(clippy::expect_used, clippy::panic)]

/// Loads one item record out of a scratch directory and answers its `to_hit`.
fn to_hit_of(record: &str) -> i32 {
    // A fresh directory per record: the loader walks a directory, and a directory reused between cases is a directory whose contents the last case chose.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "cdda-melee-accuracy-{}",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    std::fs::write(dir.join("items.json"), record.as_bytes()).expect("the record is written");
    let loaded = cdda_content::items::load_paths(&[dir.as_path()]).expect("the record loads");
    loaded.first().expect("one item").to_hit
}

/// The four values on the reference's scale, and the crowbar from the game's own data.
#[test]
fn an_items_accuracy_is_the_sum_of_its_four_qualities() {
    // Every quality at its default: -7 + 3 + 0 + 2 + 2.
    let bare = to_hit_of(r#"[{"type": "GENERIC", "id": "test_default", "name": "x", "to_hit": {}}]"#);
    assert_eq!(bare, 0, "an unstated to_hit object is worth nothing, not -2");

    // The crowbar's own record: grip solid (2), length short (1), surface any (2), balance neutral (2) -> -7 + 7.
    let crowbar = to_hit_of(
        r#"[{"type": "GENERIC", "id": "test_crowbar", "name": "x",
             "to_hit": {"grip": "solid", "length": "short", "surface": "any", "balance": "neutral"}}]"#,
    );
    assert_eq!(crowbar, 0, "the crowbar the game ships aims neither well nor badly");

    // A weapon in the hand and balanced well: grip weapon (3), length hand (0), surface every (3), balance good (3) -> -7 + 9.
    let fine = to_hit_of(
        r#"[{"type": "GENERIC", "id": "test_fine", "name": "x",
             "to_hit": {"grip": "weapon", "length": "hand", "surface": "every", "balance": "good"}}]"#,
    );
    assert_eq!(fine, 2, "a well made weapon aims two points better than a plain one: {fine}");

    // A clumsy thing held by its point: grip bad (0), length hand (0), surface point (0), balance clumsy (0) -> -7.
    let clumsy = to_hit_of(
        r#"[{"type": "GENERIC", "id": "test_clumsy", "name": "x",
             "to_hit": {"grip": "bad", "length": "hand", "surface": "point", "balance": "clumsy"}}]"#,
    );
    assert_eq!(clumsy, -7, "and a bad one aims seven points worse");

    // A plain number is still a number, and a record with no member at all keeps the reference's -2 default.
    let numbered = to_hit_of(r#"[{"type": "GENERIC", "id": "test_number", "name": "x", "to_hit": 3}]"#);
    assert_eq!(numbered, 3, "a stated number is used as it stands");
    let silent = to_hit_of(r#"[{"type": "GENERIC", "id": "test_silent", "name": "x"}]"#);
    assert_eq!(silent, -2, "and silence is the reference's -2");
}
