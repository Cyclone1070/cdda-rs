//! What a martial art's buffs ask for before they are worth anything.
//!
//! Reference: `ma_buff::is_valid_character` (`martialarts.cpp:713-760`) and `struct ma_requirements` (`martialarts.h:102-130`). A style's static buff states its requirement **first** and its
//! numbers second: an unarmed-only buff wants an empty hand, a melee-only one wants a weapon the style allows, and a buff that names **neither** applies to neither - which is why the reference
//! loads both flags as `false` (`martialarts.cpp:250-251`) rather than as `true`.
//!
//! The port keeps the two facts that decide it - what is in the hand, and which categories the style allows - and leaves the weapon's own damage, the buffs a technique requires and the wall
//! adjacency to the rounds that port them. **Naming what is not read is the point**: a requirement the port ignores must not silently become an allowance.

/// What a creature has in its hands, which a requirement asks about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hands<'a> {
    /// Nothing at all: the reference's `!u.is_armed()`.
    Empty,
    /// One weapon, by a name for the message, and the categories it belongs to.
    Weapon(&'a str),
}

/// What one buff or technique requires of the hands holding it.
///
/// Reference: `struct ma_requirements` (`martialarts.h:102-130`). **The two flags default to false**, so a buff that says nothing is in play for nobody; the categories are the reference's
/// `weapon_categories_allowed`, and an empty list allows any weapon category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    /// Whether it works with an empty hand.
    pub unarmed_allowed: bool,
    /// Whether it works with a melee weapon.
    pub melee_allowed: bool,
    /// Whether the empty hand must be truly empty, ignoring a style that forces unarmed (`strictly_unarmed`).
    pub strictly_unarmed: bool,
    /// Whether it only works with a wall next to the striker.
    pub wall_adjacent: bool,
    /// The weapon categories it allows. Empty allows any weapon category.
    pub weapon_categories: Vec<String>,
}

/// Whether a requirement is met by what is in hand and what the style allows.
///
/// Reference: `ma_buff::is_valid_character` (`martialarts.cpp:713-760`), reduced to the two facts this port keeps: `valid_unarmed` is `unarmed_allowed && !is_armed`, and `valid_melee` is
/// `melee_allowed` with a weapon whose categories the requirement allows. **A requirement with neither flag is met by nothing**, which is the reference's own default.
#[must_use]
pub fn applies(requirement: &Requirement, hands: Hands<'_>, weapon_categories: &[&str]) -> bool {
    match hands {
        Hands::Empty => requirement.unarmed_allowed,
        Hands::Weapon(_) => {
            requirement.melee_allowed
                && (requirement.weapon_categories.is_empty()
                    || requirement
                        .weapon_categories
                        .iter()
                        .any(|allowed| weapon_categories.iter().any(|held| held == allowed)))
        }
    }
}
