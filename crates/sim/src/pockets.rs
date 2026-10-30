//! Whether a thing fits in a pocket.
//!
//! Reference: `item_pocket::can_contain` (`item_pocket.cpp:1435-1610`), whose checks a survivor feels in this order -
//! is there room left, is it too heavy, is it too big for the one thing this pocket takes, and does it beat the pocket's
//! shape. The port keeps the four that decide whether a thing goes in; it leaves out what only matters once a pocket
//! already holds something (mixing liquids, gases, ammo types), because a pocket holding nothing is the state a pickup
//! starts from and the rest needs contents the port cannot yet inspect.

/// A pocket, as much of it as decides what goes in.
///
/// Reference: the part of `pocket_data` that `can_contain` reads. `content::pockets::PocketSpec` holds all of it; this
/// is the slice the rule needs, so a test can build one without the whole record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pocket {
    /// How much room it has, in millilitres.
    pub capacity_ml: i32,
    /// How much one thing may weigh, in grams; zero when the pocket does not care.
    pub max_weight_g: i32,
    /// The biggest single thing it takes, in millilitres; [`None`] when it does not care.
    pub max_item_ml: Option<i32>,
    /// Whether it holds exactly one thing, like a holster.
    pub holster: bool,
}

/// What a pocket already holds, as far as the fit rule cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Contents {
    /// How many things are in it.
    pub count: i32,
    /// How much room they take, in millilitres.
    pub volume_ml: i32,
    /// What they weigh, in grams.
    pub weight_g: i32,
}

/// Why a thing did or did not go in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fits {
    /// It goes in.
    Yes,
    /// It is bigger than the pocket's shape allows, whatever room is left.
    TooBigForThePocket,
    /// There is not enough room left.
    NoRoom,
    /// It is too heavy for the pocket.
    TooHeavy,
    /// The pocket takes one thing and already has it.
    AlreadyFull,
}

/// Whether a thing fits in a pocket that already holds something.
///
/// Reference: `can_contain`, in the order it tests - the holster first (`item_pocket.cpp:1545`), then weight
/// (`:1559`), then volume (`:1564`), then the one-item limit.
#[must_use]
pub fn fits(pocket: Pocket, held: Contents, thing_volume_ml: i32, thing_weight_g: i32) -> Fits {
    if pocket.holster && held.count > 0 {
        return Fits::AlreadyFull;
    }
    if pocket.max_item_ml.is_some_and(|most| thing_volume_ml > most) {
        return Fits::TooBigForThePocket;
    }
    if pocket.max_weight_g > 0 && held.weight_g + thing_weight_g > pocket.max_weight_g {
        return Fits::TooHeavy;
    }
    if held.volume_ml + thing_volume_ml > pocket.capacity_ml {
        return Fits::NoRoom;
    }
    Fits::Yes
}
