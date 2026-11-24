//! Which monster a group produces, and how often.
//!
//! The spawn path draws from a group by weight and, when an entry names **another group**, picks again inside it - so what a
//! spawn point finally produces is never a group id, whatever the weights were. Both facts are cheap to state and easy to get
//! wrong: a port that ignored nestings would spawn nothing from a third of the table, and one that ignored weights would
//! spawn the rare shoggoth as often as the common blob.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss
)]

use std::collections::BTreeMap;

use cdda_content::monster_groups::{load_paths, pick_monster};
use cdda_core_types::rng::Rolls;

fn load() -> Vec<cdda_content::monster_groups::MonsterGroup> {
    let dirs = [std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")];
    let dirs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    load_paths(&dirs).expect("the groups load")
}

#[test]
fn a_weighted_group_produces_its_monsters_in_the_proportions_the_weights_give() {
    // `GROUP_GOO` is the worked example in the data: two blob-small entries of 200 and 100, a blob of 693, a large blob of
    // five, a brain and a shoggoth of one each. Six entries, a thousand weight in total, so a tenth of the draws are small
    // blobs and seven in a thousand are blobs.
    let groups = load();
    let goo = groups
        .iter()
        .find(|group| group.id == "GROUP_GOO")
        .expect("GROUP_GOO");
    let mut rolls = Rolls::new(2024);
    let draws = 20_000;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for _ in 0..draws {
        let picked = pick_monster("GROUP_GOO", &groups, &mut rolls).expect("a monster");
        *counts.entry(picked).or_insert(0) += 1;
    }
    let share = |name: &str| counts.get(name).copied().unwrap_or(0) as f64 / f64::from(draws);
    assert!(
        (share("mon_blob") - 0.693).abs() < 0.03,
        "blob share {:.3}",
        share("mon_blob")
    );
    assert!(
        (share("mon_blob_small") - 0.300).abs() < 0.03,
        "small blob share {:.3}",
        share("mon_blob_small")
    );
    assert!(
        share("mon_shoggoth") < 0.01,
        "the shoggoth stays rare: {:.4}",
        share("mon_shoggoth")
    );
    let _ = goo;
}

#[test]
fn a_group_of_groups_never_produces_a_group_id() {
    // Nesting has to resolve to a monster, however many layers deep: a spawn point that produced `GROUP_ZOMBIE` instead of a
    // zombie would place nothing at all.
    let groups = load();
    // Only groups whose every nested target is defined: the data contains a few pointers to groups a mod removes, and a
    // port that invented a monster for those would be worse than one that finds nothing.
    let known: std::collections::BTreeSet<&str> =
        groups.iter().map(|group| group.id.as_str()).collect();
    let nested: Vec<&cdda_content::monster_groups::MonsterGroup> = groups
        .iter()
        .filter(|group| {
            !group.entries.is_empty()
                && group.entries.iter().all(|entry| match &entry.kind {
                    cdda_content::monster_groups::EntryKind::Group(id) => {
                        known.contains(id.as_str())
                    }
                    cdda_content::monster_groups::EntryKind::Monster(_) => false,
                })
        })
        .collect();
    assert!(
        !nested.is_empty(),
        "the data has groups made only of other groups"
    );
    let mut rolls = Rolls::new(7);
    for group in nested.iter().take(8) {
        let picked = pick_monster(&group.id, &groups, &mut rolls).expect("a monster");
        assert!(
            !groups.iter().any(|other| other.id == picked),
            "{picked} is a group, not a monster"
        );
    }
}

#[test]
fn the_same_seed_picks_the_same_monster() {
    let groups = load();
    let mut first = Rolls::new(99);
    let mut second = Rolls::new(99);
    let a = pick_monster("GROUP_ZOMBIE", &groups, &mut first);
    let b = pick_monster("GROUP_ZOMBIE", &groups, &mut second);
    assert_eq!(a, b);
}

#[test]
fn a_group_that_does_not_exist_produces_nothing() {
    let groups = load();
    let mut rolls = Rolls::new(1);
    assert_eq!(
        pick_monster("GROUP_NOT_IN_THE_DATA", &groups, &mut rolls),
        None
    );
}

/// **A corpse is chosen uniformly over the group's flattened list, not by weight.** Reference: `jmapgen_corpse::apply` (`mapgen.cpp:2965-2974`), which calls
/// `GetMonstersFromGroup( group, true )` - every entry's monster, nested groups expanded, nothing dropped or repeated by weight - and then `random_entry_ref`, a uniform pick
/// over that vector.
///
/// The test uses the same `GROUP_GOO` the weighted test uses, where `mon_shoggoth` carries weight one in a thousand: by weight it would appear about once in a thousand draws,
/// and uniformly it appears about one draw in six. The threshold is wide enough that neither the seed nor the corpus can decide the answer.
#[test]
fn a_corpse_is_chosen_uniformly_over_the_flattened_group() {
    let groups = load();
    let flat = cdda_content::monster_groups::flatten("GROUP_GOO", &groups);
    assert_eq!(flat.len(), 6, "six entries, and no nested group to expand");
    let mut rolls = Rolls::new(99);
    let draws = 600;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for _ in 0..draws {
        let picked = cdda_content::monster_groups::pick_flat("GROUP_GOO", &groups, &mut rolls)
            .expect("a monster");
        *counts.entry(picked).or_insert(0) += 1;
    }
    let shoggoths = counts.get("mon_shoggoth").copied().unwrap_or(0);
    assert!(
        shoggoths > 50,
        "**a weight-one monster is chosen about one draw in six, not one in a thousand**: {shoggoths} of {draws}"
    );
    // **Six entries, five names**: `GROUP_GOO` lists `mon_blob_small` twice (weights 200 and 100), and the reference's flatten keeps both - so the name is twice as likely as
    // the others, which is the data's two entries and not its weights. This is the duplication the flatten preserves, seen from the pick's side.
    assert_eq!(
        flat.iter().filter(|name| *name == "mon_blob_small").count(),
        2
    );
    assert_eq!(
        counts.len(),
        5,
        "**five distinct monsters, because one of the six entries names a monster the list already had**"
    );
}
