//! The noises of one turn: who they provoke, and how long they last.
//!
//! Reference: `sounds::sound` and `sounds::process_sounds` (`src/sounds.cpp:287-336, 460-490`). Two facts decide most of the behaviour
//! and both are easy to lose in a port: a **negative** volume records nothing at all (the reference bails after a debug message), and
//! what a turn records is read once and then gone - the queue is a turn's worth of noise, not a history.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::sounds::{SoundCategory, SoundQueue};

#[test]
fn only_four_categories_provoke_a_monster() {
    // `is_provocative` (`src/sounds.cpp:287-313`) answers true for exactly these four; everything else - including a shout at the top of
    // the list - is noise a monster ignores.
    let provoking = [
        SoundCategory::Speech,
        SoundCategory::ElectronicSpeech,
        SoundCategory::Alert,
        SoundCategory::Order,
    ];
    for category in provoking {
        assert!(category.is_provocative(), "{category:?} should provoke");
    }
    let ignored = [
        SoundCategory::Background,
        SoundCategory::Weather,
        SoundCategory::Sensory,
        SoundCategory::Music,
        SoundCategory::Movement,
        SoundCategory::Activity,
        SoundCategory::DestructiveActivity,
        SoundCategory::Alarm,
        SoundCategory::Combat,
    ];
    for category in ignored {
        assert!(
            !category.is_provocative(),
            "{category:?} should not provoke"
        );
    }
    assert_eq!(
        SoundCategory::ALL.len(),
        13,
        "the reference has thirteen categories today"
    );
}

#[test]
fn a_negative_volume_records_nothing_and_a_zero_does() {
    // The guard is `vol < 0`, not `vol <= 0`: silence is a legitimate noise, and the reference only refuses a negative one.
    let mut queue = SoundQueue::default();
    assert!(!queue.record((1, 1), -1, SoundCategory::Combat), "refused");
    assert!(
        queue.record((1, 1), 0, SoundCategory::Combat),
        "a silent noise is still a noise"
    );
    assert_eq!(queue.len(), 1);
}

#[test]
fn the_queue_holds_one_turn_and_then_forgets() {
    let mut queue = SoundQueue::default();
    queue.record((3, 4), 10, SoundCategory::Combat);
    queue.record((5, 6), 20, SoundCategory::Speech);
    let drained = queue.drain();
    assert_eq!(drained.len(), 2, "both noises were there");
    assert_eq!(drained[0].square, (3, 4), "in the order they happened");
    assert_eq!(drained[1].category, SoundCategory::Speech);
    assert!(queue.is_empty(), "and the queue is spent");
    assert!(queue.drain().is_empty(), "draining again finds nothing");
}

#[test]
fn a_noise_carries_its_volume_and_whether_it_provokes() {
    let mut queue = SoundQueue::default();
    queue.record((0, 0), 12, SoundCategory::Order);
    queue.record((0, 0), 12, SoundCategory::Music);
    let drained = queue.drain();
    assert_eq!(drained[0].volume, 12);
    assert!(drained[0].provokes);
    assert!(!drained[1].provokes, "music is noise a monster ignores");
}
