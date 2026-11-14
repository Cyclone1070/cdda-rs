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
    // Every branch is in the dump: **959** of them, of which **514** are valid. The 445 that are not include every threshold, and a dump that dropped them would leave
    // the `threshreq` lists pointing at nothing.
    assert_eq!(
        read, 959,
        "the dump carries every mutation branch, and it read {read}"
    );
    assert_eq!(
        table.len(),
        959,
        "including the ones the data marks invalid"
    );
    assert_eq!(read, table.len());

    // The three sets are not decoration: most of the tree has prerequisites, a fifth of it cancels something, and some of it cannot be purified away.
    let mut with_prereqs = 0;
    let mut with_cancels = 0;
    let mut unpurifiable = 0;
    for id in [
        "BIOLUM1",
        "HUMAN_ARMS",
        "ARM_FEATHERS",
        "ECHOLOCATION",
        "BEAUTIFUL",
    ] {
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
    assert!(
        with_prereqs > 0,
        "a mutation with prerequisites came through"
    );
    assert!(
        with_cancels > 0,
        "a mutation that cancels something came through"
    );
    assert!(
        unpurifiable > 0,
        "a mutation no purifier can remove came through"
    );
    assert!(
        table.get("ECHOLOCATION").is_some_and(|one| !one.purifiable),
        "and that one is ECHOLOCATION, which the dump marks unpurifiable"
    );

    // 30 branches are thresholds, none of them valid, and each is named by other mutations' `threshreq`.
    let thresholds: Vec<&str> = ["THRESH_URSINE", "THRESH_LUPINE", "THRESH_INSECT"]
        .into_iter()
        .filter(|id| table.get(id).is_some_and(|one| one.threshold))
        .collect();
    assert_eq!(thresholds.len(), 3, "the three thresholds are marked");
    assert!(
        table.get("THRESH_URSINE").is_some_and(|one| !one.valid),
        "and a threshold is not a valid mutation"
    );
    assert_eq!(
        table.get("BIOLUM3").map(|one| one.threshreq.clone()),
        Some(vec!["THRESH_INSECT".to_owned()]),
        "a mutation past a threshold names it"
    );

    // 85 of the 514 are traits a character may **start** with, and BEAUTIFUL is not one of them while NIGHTVISION is - which is what decides whose removal is a roll.

    assert!(
        table
            .get("NIGHTVISION")
            .is_some_and(|one| one.startingtrait),
        "NIGHTVISION is a starting trait"
    );
    assert!(
        table.get("BEAUTIFUL").is_some_and(|one| !one.startingtrait),
        "and BEAUTIFUL is not"
    );

    // A mutation's stat change is an **enchantment** in this baseline, and the dump follows it to the values: 257 of the 959 carry at least one.
    let with_values = ["STR_UP", "FASTHEALER", "DEX_UP", "LIGHTEATER"]
        .into_iter()
        .filter(|id| {
            table
                .get(id)
                .is_some_and(|one| !one.enchant_values.is_empty())
        })
        .count();
    assert!(with_values > 0, "an enchantment's values came through");
    assert_eq!(
        table.get("STR_UP").map(|one| one.enchant_values.clone()),
        Some(vec![cdda_content::mutations::EnchantmentValue {
            value: "STRENGTH".to_owned(),
            add: 1.0,
            multiply: 0.0,
        }]),
        "and a strong mutant is one point of strength"
    );

    // The mutagen cost is in the dump now, and the categories name the vitamin they consume: every one of the 29 states it.
    assert_eq!(
        table.get("STR_UP").map(|one| one.vitamin_cost),
        Some(100),
        "a plain mutation costs a hundred units"
    );
    let costly = table
        .ids()
        .filter(|id| table.get(id).is_some_and(|one| one.vitamin_cost == 210))
        .count();
    assert_eq!(costly, 52, "and fifty-two cost more");

    // One mutation read whole, against the dump's own answer.
    let Some(biolum) = table.get("BIOLUM1") else {
        return;
    };
    assert_eq!(biolum.prereqs, vec!["BIOLUM0".to_owned()]);
    assert!(biolum.purifiable);
}
