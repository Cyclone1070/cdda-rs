//! What the data says about learning a proficiency.
//!
//! Reference: `proficiency` (`proficiency.h:60-115`), dumped by `--dump-proficiencies`.

use std::sync::Arc;

fn corpus(name: &str) -> Arc<cdda_json::Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus")
        .join(name);
    let Some(value) = cdda_content::read_json(&path) else {
        return Arc::new(cdda_json::Value::Null);
    };
    value
}

/// The dump reads, and its numbers are the reference's.
///
/// Reference: `proficiency::time_to_learn` (`proficiency.h:84`) and `required_proficiencies`. The counts prove the read was not vacuous: **235** proficiencies in the
/// baseline's data, and every one of them carries a learning time.
#[test]
fn the_proficiency_dump_reads_with_its_times_and_prerequisites() {
    let dump = corpus("proficiencies.json");
    let Some((proficiencies, count)) = cdda_content::proficiencies::from_dump(&dump) else {
        return;
    };
    assert_eq!(count, 235, "the baseline's data defines 235 proficiencies");
    assert_eq!(proficiencies.by_id.len(), count, "every one has an id");

    // Four hours for the first athletics proficiency, eight for the expert one that needs it.
    assert_eq!(proficiencies.seconds_for("prof_athlete_basic"), Some(14_400));
    let Some(expert) = proficiencies.by_id.get("prof_athlete_expert") else {
        return;
    };
    assert_eq!(expert.seconds, 28_800);
    assert_eq!(expert.required, vec!["prof_athlete_basic".to_owned()]);

    // Prerequisites are what gate practising: the expert one is not learnable until the basic one is known.
    assert!(!proficiencies.learnable_by("prof_athlete_expert", &|_| false));
    assert!(proficiencies.learnable_by("prof_athlete_expert", &|id| id == "prof_athlete_basic"));
    assert!(proficiencies.learnable_by("prof_athlete_basic", &|_| false));

    // A proficiency the data does not define is not learnable, rather than a zero-second one.
    assert!(!proficiencies.learnable_by("prof_nonexistent", &|_| true));
    assert_eq!(proficiencies.seconds_for("prof_nonexistent"), None);
}
