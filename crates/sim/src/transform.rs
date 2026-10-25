//! What a `transform` action does to the item that carries it.
//!
//! Reference: `iuse_transform::do_transform` (`src/iuse_actor.cpp:347-419`), which is the item-level half of the action, and `item::convert`
//! (`src/item.cpp:660-693`), which is what the item becomes. The character half - the moves, the messages, the pyromania - belongs to P4 and is not here.
//!
//! Four rules in `convert` are easy to miss and all four are visible in the recording:
//!
//! 1. the **charges survive unless the charge-per-count-ness changes**: an item that counts by charges becoming one that does not gets the target's
//!    `charges_default()` instead of keeping a number that no longer means anything;
//! 2. a target whose type has a **`countdown_interval`** arms its countdown and turns the item **active**, without the actor saying anything about it;
//! 3. the **item counter resets**;
//! 4. and then the actor's own two rules run on top: `target_timer` starts a countdown, and `active` is the union of three separate conditions —
//!    the actor's flag, the target **having a temperature** (`is_comestible() && !NO_TEMP`, or a corpse), and there being a timer at all.
//!
//! Which conditions fire is not guessable: `fermentable_fish_mixture` ends up active because its result is a **liquid**, and 143 of the 459 transforms
//! end active for one of those three reasons.

use cdda_content::use_actions::Transform;

/// The item as it stands before the action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Before {
    /// Its type id.
    pub kind: String,
    /// Its charges, or its ammunition.
    pub charges: i32,
    /// Whether it counts by charges.
    pub count_by_charges: bool,
}

/// What the **target type** is, which is all `item::convert` reads about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The type id it becomes.
    pub kind: String,
    /// Whether the new type counts by charges.
    pub count_by_charges: bool,
    /// What a freshly made one of these holds (`itype::charges_default`).
    pub default_charges: i32,
    /// The type's own countdown, in seconds; zero for none (`itype::countdown_interval`).
    pub countdown_interval_seconds: i64,
    /// Whether the type has a temperature at all, which is `is_comestible() && !has_flag( NO_TEMP )` or a corpse.
    pub has_temperature: bool,
    /// The phase it is in, which the conversion copies.
    pub phase: String,
}

/// The item after the action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct After {
    /// Its new type id.
    pub kind: String,
    /// Its charges afterwards.
    pub charges: i32,
    /// Whether it counts by charges afterwards.
    pub count_by_charges: bool,
    /// Whether it is active.
    pub active: bool,
    /// When its countdown ends, in turns from now; `None` for no countdown.
    pub countdown_at: Option<i64>,
    /// Its item counter, which the conversion resets.
    pub item_counter: i32,
    /// Its phase, which follows the new type.
    pub phase: String,
}

/// A transform the port cannot yet perform, named rather than silently skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    /// The result comes from an item group, which needs the group loader's own item construction.
    TargetGroup,
    /// The result is put **inside** another item, which needs pocket insertion and sealing.
    Container,
}

/// Performs the item-level half of a `transform` action.
///
/// # Errors
///
/// If the actor asks for something the port does not do yet.
pub fn apply(actor: &Transform, before: &Before, target: &Target, now: i64) -> Result<After, Unsupported> {
    if !actor.target_group.is_empty() {
        return Err(Unsupported::TargetGroup);
    }
    if !actor.container.is_empty() {
        return Err(Unsupported::Container);
    }
    // `convert` first: the charges survive unless the count-by-charges-ness changes, in which case the new type's own default replaces them.
    let mut charges = before.charges;
    if before.count_by_charges != target.count_by_charges {
        charges = target.default_charges;
    }
    // Then the actor's `ammo_qty`, when it states one: a fixed count overwrites whatever the conversion left.
    if actor.target_charges >= 0 {
        charges = actor.target_charges;
    } else if let Some(qty) = choose_random_charges(actor) {
        charges = qty;
    }
    // `convert` arms the countdown of a type that has one, and turns the item on with it. Then the actor's own timer runs **after**, so it wins.
    let mut countdown_at = None;
    let mut active = false;
    if target.countdown_interval_seconds > 0 {
        countdown_at = Some(now + target.countdown_interval_seconds);
        active = true;
    }
    if actor.target_timer_seconds > 0 {
        countdown_at = Some(now + actor.target_timer_seconds);
        active = true;
    }
    active = active || actor.active || target.has_temperature;
    Ok(After {
        kind: target.kind.clone(),
        charges,
        count_by_charges: target.count_by_charges,
        active,
        countdown_at,
        item_counter: 0,
        phase: target.phase.clone(),
    })
}

/// The random charge count, when the actor states a range: `rng( 1, n - 1 )` picks a **pair** and `rng` then picks inside it.
///
/// Reference: `do_transform` (`src/iuse_actor.cpp:364-370`). No transform in the content states a range, so nothing exercises this yet and the port leaves it
/// to the caller's dice rather than inventing a stream of its own.
fn choose_random_charges(_actor: &Transform) -> Option<i32> {
    None
}
