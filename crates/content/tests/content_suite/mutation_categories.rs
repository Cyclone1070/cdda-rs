//! The mutation categories carry the numbers a mutation's removal and a threshold are decided by.
//!
//! Reference: `mutation_category_trait` (`mutation.h:520-560`): `base_removal_chance` is the percent a starting trait is removed at, and `threshold_min` the strength a
//! threshold needs.

#[test]
fn the_category_dump_reads_with_its_removal_chance_and_threshold() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/mutation-categories.json"
    );
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(root) = cdda_json::parse(text.as_bytes()) else {
        return;
    };
    let Some((table, read)) = cdda_content::mutations::categories_from_dump(&root) else {
        return;
    };
    assert_eq!(read, 29, "every mutation category is in the dump, and it read {read}");
    assert_eq!(read, table.len());

    // HUMAN never lets go of its starting traits, and every category wants the same strength for a threshold; URSINE is the ordinary third.
    assert!(
        table.get("HUMAN").is_some_and(|one| one.base_removal_chance == 0),
        "the human baseline never removes a starting trait"
    );
    assert!(
        table.get("URSINE").is_some_and(|one| one.base_removal_chance == 33),
        "a beast category removes one a third of the time"
    );
    assert!(
        table.get("HUMAN").is_some_and(|one| one.threshold_min == 2200),
        "and a threshold wants 2200 of category strength"
    );
}
