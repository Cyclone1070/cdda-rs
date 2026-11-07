//! How long cutting up a corpse takes.
//!
//! Reference: `butcher_time_to_cut` (`butchery.cpp:382-487`). The chain is short and every link is the reference's own number: the body's **size** sets the base in moves, the tool's
//! **quality factor** takes a percentage off, the **action** multiplies what is left, and a **quartered** corpse is quicker again by four.
//!
//! **The quality factor stops mattering at seventy-five**, because the reference multiplies by `max( 25, 100 - factor )` rather than by the percentage alone - a floor that keeps a corpse from
//! ever becoming free. Two things are named rather than invented: the proficiency penalties a butcher's practice buys (`prof_butchering_basic` and friends, which need the practice **fraction**
//! the port does not keep per proficiency) and the crafting helpers the player brings.

use crate::CreatureSize;

/// Which cut is being made, which decides the multiplier on the base time.
///
/// Reference: `enum class butcher_type` (`butchery.h`): a quick cut, a bleed, a full butchery, a field dressing, a skinning, a quartering, a dismembering, and a dissection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButcherAction {
    /// A quick cut for what is easy to reach.
    Quick,
    /// Bleeding a corpse out.
    Bleed,
    /// The whole job: gutting, skinning, and the rest.
    Full,
    /// Gutting in the field, which the full job does faster when it is already done.
    FieldDress,
    /// Taking the hide.
    Skin,
    /// Cutting the body into quarters, which makes it carryable.
    Quarter,
    /// Cutting the body into pieces, without taking anything.
    Dismember,
    /// Looking for what the body's anatomy says is there.
    Dissect,
}

/// How many moves cutting up a corpse takes.
///
/// Reference: `butcher_time_to_cut` (`butchery.cpp:382-487`), in the reference's own order: size, then quality, then action, then the quartered flag. The action multipliers are applied
/// **after** the quality factor, which is why a quality-ten blade still saves more on a full butchery than on a quick cut in absolute terms.
///
/// `field_dressed` is the corpse's own `FIELD_DRESS` flag: a body gutted in the field is quicker to finish. **The table is in turns and the answer is in moves.** `150 * max( 25, 100 - factor )` with no blade is `15000` moves - a hundred and fifty turns - which is the reference's own arithmetic and
/// the reason its comment calls the table "time (roughly) in turns" while the function returns moves.
#[must_use]
pub fn time_to_cut(
    size: CreatureSize,
    quality: i32,
    action: ButcherAction,
    field_dressed: bool,
    quartered: bool,
) -> i32 {
    let base = match size {
        CreatureSize::Tiny => 150,
        CreatureSize::Small => 300,
        CreatureSize::Medium => 450,
        CreatureSize::Large => 600,
        CreatureSize::Huge => 1800,
    };
    let mut time = base * (100 - quality).clamp(25, 100);
    time = match action {
        ButcherAction::Quick | ButcherAction::Bleed => time,
        // **A full butchery is four times the base once the corpse has been field dressed, and six times when it has not** (`butchery.cpp:431-436`). The reference also multiplies by six when the
        // dressing **failed**; the port keeps no failed flag yet, so a failed dressing is named rather than guessed.
        ButcherAction::Full => {
            if field_dressed {
                time * 4
            } else {
                time * 6
            }
        }
        ButcherAction::FieldDress | ButcherAction::Skin => time * 2,
        // A quartering and a dismembering are quicker than they look, but never quicker than their own floors (`:445-455`).
        ButcherAction::Quarter => (time / 4).max(1200),
        ButcherAction::Dismember => (time / 10).max(600),
        ButcherAction::Dissect => time * 6,
    };
    if quartered {
        time /= 4;
    }
    time
}
