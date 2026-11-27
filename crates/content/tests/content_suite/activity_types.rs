//! The activity definitions the data holds.
//!
//! **Counts prove non-vacuity**: the data defines 138 `activity_type` objects with 138 unique ids. The fields read here are the ones a rule asks: the verb the player is told, what the
//! activity's moves are counted in, whether it can be resumed, whether it is a **multiple** activity (one that keeps looking for more work), and whether it roots the creature in
//! place.

#![allow(clippy::expect_used)]

use cdda_content::activity_types;

fn data_roots() -> Vec<std::path::PathBuf> {
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    vec![reference.join("core"), reference.join("json")]
}

#[test]
fn every_activity_in_the_data_is_read() {
    let roots = data_roots();
    let paths: Vec<&std::path::Path> = roots.iter().map(std::path::PathBuf::as_path).collect();
    let activities = activity_types::load_paths(&paths).expect("the activities load");
    assert_eq!(
        activities.kinds.len(),
        138,
        "the data defines 138 activities"
    );
    assert_eq!(activities.by_id.len(), 138, "all of them have unique ids");
}

/// **What one activity says about itself.** `ACT_MULTIPLE_BUTCHER` is the multiple-activity case: it keeps looking for work, cannot be resumed, and its moves are counted in
/// **neither** way - the actor spends what it spends, which is why the port's `Basis` needs a third answer and why this loader exists at all.
///
/// `ACT_RELOAD` is the plain speed case, `ACT_BIKERACK_RACKING` is time-based and rooted, and `ACT_READ` **omits** `can_resume` - which the reference defaults to **true**
/// (`activity_type.h:45`), not false. A loader that defaulted it to false would answer the opposite of the reference for 59 of the 138 activities.
#[test]
fn an_activity_says_what_it_is_counted_in_and_whether_it_resumes() {
    let roots = data_roots();
    let paths: Vec<&std::path::Path> = roots.iter().map(std::path::PathBuf::as_path).collect();
    let activities = activity_types::load_paths(&paths).expect("the activities load");
    let butcher = activities
        .by_id
        .get("ACT_MULTIPLE_BUTCHER")
        .expect("the butchering");
    assert_eq!(
        butcher.verb, "butchering",
        "**the verb the player is told**"
    );
    assert_eq!(
        butcher.based_on, "neither",
        "**its moves are counted in neither way**"
    );
    assert!(butcher.multi_activity, "**it keeps looking for more work**");
    assert!(!butcher.can_resume, "**and cannot be resumed**");
    assert!(!butcher.rooted, "**and does not root the creature**");
    let reload = activities.by_id.get("ACT_RELOAD").expect("the reloading");
    assert_eq!(
        reload.based_on, "speed",
        "**reloading is counted in speed**"
    );
    let racking = activities
        .by_id
        .get("ACT_BIKERACK_RACKING")
        .expect("the racking");
    assert_eq!(racking.based_on, "time", "**racking is counted in time**");
    assert!(racking.rooted, "**and roots the creature at the rack**");
    let reading = activities.by_id.get("ACT_READ").expect("the reading");
    assert!(
        reading.can_resume,
        "**an activity that does not say defaults to resumable**"
    );
    assert!(
        !reading.multi_activity,
        "**and is not a multiple activity**"
    );
}
