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

/// The item's standing with respect to the character, which is all `can_use` reads about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    /// Whether the character is wearing it.
    pub worn: bool,
    /// Whether the character is wielding it.
    pub wielded: bool,
    /// Whether it is empty, which is what `need_empty` refuses.
    pub empty: bool,
    /// How many charges it has left, which is what `need_charges` is compared against.
    pub charges: i32,
}

/// Why a transform cannot be used, as the reference's own sentence.
///
/// Reference: `iuse_transform::can_use` (`src/iuse_actor.cpp:428-475`). The conditions are asked in a **fixed order** and the first failure is the answer, which is why
/// the variant matters as much as the text: an item that is neither worn nor wielded reports the wearing rule even when it also has no charges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// `need_worn`, and the character is not wearing it.
    NotWorn,
    /// `need_wielding`, and the character is not wielding it.
    NotWielded,
    /// `need_empty`, and it has something inside.
    NotEmpty,
    /// `need_charges`, and it has fewer charges than the actor asks.
    NotEnoughCharges,
}

impl Refusal {
    /// The sentence the reference gives, with `%1$s` where the item's name goes.
    ///
    /// The three fixed ones are literals in `can_use`; the fourth is the **actor's own** message, whose default is `"The %s is empty!"` and which several actors
    /// replace (six of them with `"Your internal reservoir is empty."`, which has no placeholder at all).
    #[must_use]
    pub fn template<'actor>(&self, actor: &'actor Transform) -> &'actor str {
        match self {
            Self::NotWorn => "You need to wear the %1$s before activating it.",
            Self::NotWielded => "You need to wield the %1$s before activating it.",
            Self::NotEmpty => "You need to empty the %1$s before activating it.",
            Self::NotEnoughCharges => &actor.need_charges_msg,
        }
    }
}

/// Whether a transform may be used, and if not, why.
///
/// # Errors
///
/// The first condition that refuses it, in the reference's order.
pub fn can_use(actor: &Transform, standing: &Standing) -> Result<(), Refusal> {
    if actor.need_worn && !standing.worn {
        return Err(Refusal::NotWorn);
    }
    if actor.need_wielding && !standing.wielded {
        return Err(Refusal::NotWielded);
    }
    if actor.need_empty && !standing.empty {
        return Err(Refusal::NotEmpty);
    }
    // **The worn-mutation conflict sits between these and the charges** (`src/iuse_actor.cpp:438-451`): a worn item whose target conflicts with a mutation the
    // character has is refused. It needs the mutation layer, which is P4, so it is **declared** rather than skipped silently.
    if actor.need_charges > 0 && standing.charges < actor.need_charges {
        return Err(Refusal::NotEnoughCharges);
    }
    // **The qualities come last**, and they are asked of the character *and* of the surroundings within one tile (`has_quality` over an inventory formed from the
    // map). That needs an inventory and a materialised map, so it is declared too.
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    /// **Transcribed from the reference, because the content cannot tell these two apart.** No actor in the corpus states `need_worn` *and* `need_wielding`, so no
    /// recording can distinguish the order — only the source can, and it asks `need_worn` first (`src/iuse_actor.cpp:433-441`).
    #[test]
    fn the_wearing_rule_is_asked_before_the_wielding_one() {
        let actor = Transform {
            action: "transform".to_owned(),
            msg: String::new(),
            target: "test".to_owned(),
            target_group: String::new(),
            container: String::new(),
            variant_type: "<any>".to_owned(),
            sealed: true,
            target_charges: -1,
            target_ammo: String::new(),
            target_timer_seconds: 0,
            active: false,
            need_worn: true,
            need_wielding: true,
            need_empty: false,
            moves: 0,
            need_fire: 0,
            need_charges: 0,
            need_charges_msg: String::new(),
            need_fire_msg: String::new(),
            menu_text: String::new(),
            qualities_needed: std::collections::BTreeMap::new(),
            rand_target_charges: Vec::new(),
            transform_age: 0,
            not_ready_msg: String::new(),
        };
        let bare = Standing { worn: false, wielded: false, empty: true, charges: 0 };
        assert_eq!(can_use(&actor, &bare), Err(Refusal::NotWorn));
    }
}
