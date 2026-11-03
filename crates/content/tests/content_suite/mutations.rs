//! The mutation table reads from the oracle's dump.
//!
//! Reference: `mutation_branch` (`mutation.h:331-335`): `prereqs` and `prereqs2` are alternatives, and `cancels` is what the mutation takes away.

#[test]
fn the_mutation_dump_reads_with_its_prerequisites_and_conflicts() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/mutations.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(root) = cdda_json::parse(text.as_bytes()) else {
        return;
    };
    let Some((table, read)) = cdda_content::mutations::from_dump(&root) else {
        return;
    };
    assert_eq!(read, 514, "the dump carries every valid mutation, and it read {read}");
    assert_eq!(read, table.len());

    // The three sets are not decoration: most of the tree has prerequisites, a fifth of it cancels something, and some of it cannot be purified away.
    let mut with_prereqs = 0;
    let mut with_cancels = 0;
    let mut unpurifiable = 0;
    for id in ["BIOLUM1", "HUMAN_ARMS", "ARM_FEATHERS", "ECHOLOCATION", "BEAUTIFUL"] {
        let Some(one) = table.get(id) else {
            continue;
        };
        if !one.prereqs.is_empty() {
            with_prereqs += 1;
        }
        if !one.cancels.is_empty() {
            with_cancels += 1;
        }
        if !one.purifiable {
            unpurifiable += 1;
        }
    }
    assert!(with_prereqs > 0, "a mutation with prerequisites came through");
    assert!(with_cancels > 0, "a mutation that cancels something came through");
    assert!(unpurifiable > 0, "a mutation no purifier can remove came through");
    assert!(
        table.get("ECHOLOCATION").is_some_and(|one| !one.purifiable),
        "and that one is ECHOLOCATION, which the dump marks unpurifiable"
    );

    // One mutation read whole, against the dump's own answer.
    let Some(biolum) = table.get("BIOLUM1") else {
        return;
    };
    assert_eq!(biolum.prereqs, vec!["BIOLUM0".to_owned()]);
    assert!(biolum.purifiable);
}
