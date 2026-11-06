//! How a body's condition and load change what its parts are worth.
//!
//! Reference: `bodypart::wound_adjusted_limb_value`, `encumb_adjusted_limb_value` and `skill_adjusted_limb_value` (`src/bodypart.cpp:967-996`), which
//! `bodypart::get_limb_score` (`:998-1018`) applies in that order before summing a creature's score.

/// A hurt part is worth less in proportion, and only below three quarters of its health.
///
/// Reference: `bodypart::wound_adjusted_limb_value` (`bodypart.cpp:967-976`) - above three quarters of its health a part is worth its whole figure, and below it the figure is scaled by
/// `health / 0.75`, so a part at a **quarter** of its health is worth **a third** of what it was.
#[test]
fn a_hurt_part_is_worth_less() {
    use cdda_sim::limbs::wound_adjusted_limb_value;

    #[allow(clippy::float_cmp)]
    {
        assert_eq!(wound_adjusted_limb_value(1.0, 100, 100), 1.0, "a whole part is worth its whole figure");
        assert_eq!(wound_adjusted_limb_value(1.0, 76, 100), 1.0, "**and three quarters is still whole**");
    }
    assert!((wound_adjusted_limb_value(1.0, 75, 100) - 1.0).abs() < 1e-9, "the threshold itself is whole");
    assert!(
        (wound_adjusted_limb_value(1.0, 50, 100) - 2.0 / 3.0).abs() < 1e-9,
        "**half health leaves two thirds**"
    );
    assert!(
        (wound_adjusted_limb_value(1.0, 25, 100) - 1.0 / 3.0).abs() < 1e-9,
        "and a quarter of the health is a third of the score"
    );
    assert!((wound_adjusted_limb_value(0.7, 0, 100) - 0.0).abs() < 1e-9, "a ruined part is worth nothing");
}

/// Load costs a part a twentieth per point past its threshold, and an overloaded limb is worth nothing.
///
/// Reference: `bodypart::encumb_adjusted_limb_value` (`bodypart.cpp:978-990`): **zero** when the limb is over its limit, otherwise `val * 19 / ( 19 + enc / 3 )` past the part's threshold -
/// a curve that starts at one, is a **twentieth** down at three points, and never reaches zero, which is the reference's own comment.
#[test]
fn a_loaded_part_is_worth_less() {
    use cdda_sim::limbs::encumb_adjusted_limb_value;

    #[allow(clippy::float_cmp)]
    {
        assert_eq!(encumb_adjusted_limb_value(1.0, 0, 0, false), 1.0, "nothing in the way");
        assert_eq!(encumb_adjusted_limb_value(1.0, 3, 3, false), 1.0, "**and up to the threshold is free**");
    }
    assert!(
        (encumb_adjusted_limb_value(1.0, 6, 3, false) - 19.0 / 20.0).abs() < 1e-9,
        "**three points over the threshold is a twentieth**"
    );
    assert_eq!(
        encumb_adjusted_limb_value(1.0, 99, 3, true),
        0.0,
        "an overencumbered limb is worth nothing at all"
    );
    // The curve never reaches zero in the reference's own arithmetic, which is why the over-limit flag exists.
    assert!(encumb_adjusted_limb_value(1.0, 100_000, 3, false) > 0.0);
}

/// Skill never lowers a part's figure, and the reference's own arithmetic says why.
///
/// Reference: `bodypart::skill_adjusted_limb_value` (`bodypart.cpp:992-996`):
///
/// ```text
/// const float mitigated_score = val * ( 1.0f + ( skill / 10.0f ) );
/// return std::min( val, mitigated_score );
/// ```
///
/// **The minimum is taken against a figure that is larger than `val` for every non-negative skill**, so the answer is always `val` - the function does nothing. It reads like a penalty and is
/// inert, which is exactly the kind of thing a port must copy rather than "fix": the reference's behaviour is the behaviour, and a port that made skill help would answer differently from the
/// game it is a port of. It is recorded here so that nobody has to rediscover it.
#[test]
fn skill_never_lowers_a_limb_value() {
    use cdda_sim::limbs::skill_adjusted_limb_value;

    #[allow(clippy::float_cmp)]
    {
        assert_eq!(skill_adjusted_limb_value(1.0, 0), 1.0, "a beginner");
        assert_eq!(skill_adjusted_limb_value(1.0, 5), 1.0, "**and a master: the reference's minimum keeps the value**");
    }
    assert_eq!(skill_adjusted_limb_value(0.7, 10), 0.7, "whatever figure is asked about");
}
