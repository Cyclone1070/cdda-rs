//! What a martial art's buffs ask for before they are worth anything.
//!
//! Reference: `ma_buff::is_valid_character` (`martialarts.cpp:713-760`) and `struct ma_requirements` (`martialarts.h:102-130`). A style's static buff states its requirement **first** and its
//! numbers second: an unarmed-only buff wants an empty hand, a melee-only one wants a weapon the style allows, and a buff that names **neither** applies to neither - which is why the reference
//! loads both flags as `false` (`martialarts.cpp:250-251`) rather than as `true`.
//!
//! The port keeps the two facts that decide it - what is in the hand, and which categories the style allows - and leaves the weapon's own damage, the buffs a technique requires and the wall
//! adjacency to the rounds that port them. **Naming what is not read is the point**: a requirement the port ignores must not silently become an allowance.

use bevy_ecs::prelude::*;

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

/// What a bonus's scale is multiplied by.
///
/// Reference: `enum scaling_stat` (`bonuses.h:15-45`) and `effect_scaling` (`bonuses.cpp:318-372`). The reference switches over a **compile-time enum**; the port keeps the same cases with the
/// value already read from the creature, because a bonus is a number about a body and not a thing to look up later.
#[derive(Debug, Clone, PartialEq)]
pub enum Scaling {
    /// `STAT_NULL`, which is **the scale itself** - not zero.
    None,
    /// A character stat, by name and value: `str`, `dex`, `int`, `per`.
    Stat {
        /// Which stat, for the reader.
        name: String,
        /// What the creature's stat is.
        value: f64,
    },
    /// A skill, by name and level.
    Skill {
        /// Which skill, for the reader.
        name: String,
        /// What level the creature is at.
        level: f64,
    },
    /// A name the port does not read, which answers nothing rather than guessing.
    Unknown(String),
}

/// One bonus, as the data states it: a scale and what it scales with.
#[derive(Debug, Clone, PartialEq)]
pub struct Bonus {
    /// The number the data gives.
    pub scale: f64,
    /// What it is multiplied by.
    pub scaling: Scaling,
}

/// What one bonus is worth, given the body it applies to.
///
/// Reference: `effect_scaling::get` (`bonuses.cpp:318-372`). **`STAT_NULL` answers the scale itself**, which is the line a port gets wrong by defaulting to nothing.
#[must_use]
pub fn scaled(scale: f64, scaling: &Scaling) -> f64 {
    match scaling {
        Scaling::None => scale,
        Scaling::Stat { value, .. } => scale * value,
        Scaling::Skill { level, .. } => scale * level,
        Scaling::Unknown(_) => 0.0,
    }
}

/// The flat bonuses for one stat, summed.
///
/// Reference: `bonus_container::get_flat` (`bonuses.cpp:221-240`): the bonuses for the stat **add**, and a stat with none answers zero.
#[must_use]
pub fn flat_of(bonuses: &std::collections::BTreeMap<String, Vec<Bonus>>, stat: &str) -> f64 {
    bonuses
        .get(stat)
        .map_or(0.0, |found| found.iter().map(|bonus| scaled(bonus.scale, &bonus.scaling)).sum())
}

/// The multipliers for one stat, multiplied and then clamped at nothing.
///
/// Reference: `bonus_container::get_mult` (`bonuses.cpp:242-258`): it starts at **one**, multiplies, and answers `max( 0.0f, ret )` with the reference's own comment that "all relevant effects
/// require non-negative values". A stat with no multipliers therefore answers one, which is the identity.
#[must_use]
pub fn mult_of(bonuses: &std::collections::BTreeMap<String, Vec<Bonus>>, stat: &str) -> f64 {
    bonuses.get(stat).map_or(1.0, |found| {
        let product: f64 = found.iter().map(|bonus| scaled(bonus.scale, &bonus.scaling)).product();
        product.max(0.0)
    })
}

/// One move that has already qualified, and how many entries it is worth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The technique's id.
    pub id: String,
    /// Its `weighting`: how many entries it gets in the list the pick chooses from.
    pub weighting: i32,
}

/// Which move a striker makes, given the ones that qualified and one uniform roll.
///
/// Reference: `Character::pick_technique` (`melee.cpp:1406-1440`): every qualified technique is pushed once, then `weighting - 1` more times, and `random_entry` picks one uniformly - **so a
/// weighting of nothing still gets one entry**, because the first push happens whatever the weighting says. An empty list answers nothing, which the reference spells `tec_none`.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
pub fn pick(candidates: &[Candidate], roll: f64) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    let mut entries: Vec<&str> = Vec::new();
    for candidate in candidates {
        entries.push(&candidate.id);
        for _ in 1..candidate.weighting {
            entries.push(&candidate.id);
        }
    }
    let last = entries.len() - 1;
    let index = (roll.clamp(0.0, 1.0) * entries.len() as f64).floor() as usize;
    entries.get(index.min(last)).map(|id| (*id).to_owned())
}

/// What a move says about itself, as far as whether it is offered goes.
///
/// Reference: `ma_technique` (`martialarts.h:162-247`) - the flags `evaluate_technique` reads before it ever looks at the requirement.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TechniqueFlags {
    /// How many entries the move is worth. **Negative means a chance**: `one_in( |weighting| )` decides whether it is offered at all.
    pub weighting: i32,
    /// A placeholder the reference never offers.
    pub dummy: bool,
    /// A counter rather than an attack.
    pub defensive: bool,
    /// Only offered on a critical hit.
    pub crit_tec: bool,
    /// Offered whether or not the hit was critical.
    pub crit_ok: bool,
    /// Only offered during a reach attack.
    pub reach_tec: bool,
    /// Offered during a reach attack as well as an ordinary one.
    pub reach_ok: bool,
    /// Answers a dodge.
    pub dodge_counter: bool,
    /// Answers a block.
    pub block_counter: bool,
    /// Needs a loaded weapon.
    pub needs_ammo: bool,
    /// Takes the target's weapon away.
    pub disarms: bool,
    /// Takes the target's weapon for the striker.
    pub take_weapon: bool,
}

/// The moment a move is being offered for.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moment {
    /// Whether the blow is critical.
    pub crit: bool,
    /// Whether the attack reaches.
    pub reach_attacking: bool,
    /// Whether the striker is looking for a dodge counter.
    pub dodge_counter: bool,
    /// Whether the striker is looking for a block counter.
    pub block_counter: bool,
    /// Whether the weapon is loaded.
    pub loaded: bool,
    /// Whether the target carries a weapon.
    pub target_armed: bool,
    /// Whether the striker carries one.
    pub self_armed: bool,
    /// Whether there is a wall beside the striker.
    pub wall_adjacent: bool,
}

/// Whether a move is offered in this moment, given one uniform roll for its chance.
///
/// Reference: `Character::evaluate_technique` (`melee.cpp:1446-1550`), in the reference's own order, because the order is what a reader diffs against:
///
/// 1. a **dummy** is never offered;
/// 2. a **defensive** move is a counter, so it is offered only when a counter is being looked for, and the two kinds must match;
/// 3. a **reach** move needs a reach attack, and a reach attack needs a move that allows it;
/// 4. **criticality must match**: a move that is not `crit_ok` is offered only when the moment's criticality equals its own `crit_tec`;
/// 5. **ammunition** and the two weapon-taking rules;
/// 6. a **negative weighting** is a chance rather than a prohibition - `one_in( |weighting| )` - which is the line that looks like an exclusion and is not.
#[must_use]
pub fn offers(flags: &TechniqueFlags, moment: &Moment, roll: f64) -> bool {
    if flags.dummy {
        return false;
    }
    if flags.defensive && !(moment.dodge_counter && flags.dodge_counter || moment.block_counter && flags.block_counter) {
        return false;
    }
    if moment.dodge_counter != flags.dodge_counter || moment.block_counter != flags.block_counter {
        return false;
    }
    if flags.reach_tec && !moment.reach_attacking {
        return false;
    }
    if !(flags.reach_ok || flags.reach_tec) && moment.reach_attacking {
        return false;
    }
    if !flags.crit_ok && (moment.crit != flags.crit_tec) {
        return false;
    }
    if flags.needs_ammo && !moment.loaded {
        return false;
    }
    if flags.disarms && !moment.target_armed {
        return false;
    }
    if flags.take_weapon && (moment.self_armed || !moment.target_armed) {
        return false;
    }
    if flags.weighting < 0 {
        let chance = f64::from(flags.weighting.unsigned_abs());
        return chance > 0.0 && roll < 1.0 / chance;
    }
    true
}

/// A move as the store hands it over: what it is called and how it is offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Technique {
    /// Its id.
    pub id: String,
    /// The requirement it states for the hands.
    pub requirement: Requirement,
    /// The flags that decide whether it is offered at all.
    pub flags: TechniqueFlags,
    /// What it does to the body it lands on, beyond the damage.
    pub movement: AppliedMove,
}

/// A style a creature has learned: its id and the moves it offers.
///
/// Reference: `character_martial_arts` (`martialarts.h:445-460`) keeps the **selected style**, and the moves are the style's own set (`martialart::techniques`). The port keeps the style itself as
/// a component, because a creature's training is something it carries rather than a table to look up.
#[derive(Debug, Clone, PartialEq, Eq, Component)]
pub struct Practising {
    /// The style.
    pub style: Style,
}

/// One style, as far as picking a move needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Style {
    /// Which style.
    pub id: String,
    /// The moves it offers.
    pub techniques: Vec<Technique>,
}

impl Moment {
    /// A blow with nothing special about it: no critical, no reach, no counter, nothing loaded.
    #[must_use]
    pub fn ordinary() -> Self {
        Self {
            crit: false,
            reach_attacking: false,
            dodge_counter: false,
            block_counter: false,
            loaded: false,
            target_armed: false,
            self_armed: false,
            wall_adjacent: false,
        }
    }
}

/// Which move the striker makes at this target, or nothing when the style offers none.
///
/// Reference: `Character::pick_technique` (`melee.cpp:1406-1440`): the selected style's moves are filtered by `ma_technique::is_valid_character` and by `evaluate_technique`'s refusals, and one is
/// drawn by weight. **The store's own answer**: no style answers nothing, and a style whose every move is refused answers nothing too - which is the plain blow.
#[must_use]
pub fn pick_for(
    world: &mut bevy_ecs::prelude::World,
    striker: bevy_ecs::prelude::Entity,
    target: bevy_ecs::prelude::Entity,
    moment: &Moment,
    roll: f64,
) -> Option<String> {
    let style = world.get::<Practising>(striker)?.style.clone();
    // **What is in each hand, read from the store**: a `Wielding` link says the striker carries a weapon, and the weapon's own categories decide which moves allow it.
    let held = world.get::<crate::creature::Wielding>(striker).map(|wielded| wielded.0);
    let categories = held.map_or_else(Vec::new, |kind| crate::item_registry::weapon_categories_of(world, kind));
    let borrowed: Vec<&str> = categories.iter().map(String::as_str).collect();
    let hands = if held.is_some() { Hands::Weapon("a weapon") } else { Hands::Empty };
    let moment = Moment {
        self_armed: held.is_some(),
        target_armed: world.get::<crate::creature::Wielding>(target).is_some(),
        ..moment.clone()
    };
    let qualified: Vec<Candidate> = style
        .techniques
        .iter()
        .filter(|technique| offers(&technique.flags, &moment, roll))
        .filter(|technique| applies(&technique.requirement, hands, &borrowed))
        .map(|technique| Candidate {
            id: technique.id.clone(),
            // The reference's list pushes a move once and then `weighting - 1` more times, and `pick` does the same, so a weight of nothing stays one entry.
            weighting: technique.flags.weighting,
        })
        .collect();
    pick(&qualified, roll)
}

/// The data's styles, kept where a creature can learn one.
///
/// Reference: `martialart` objects live in the reference's own registry (`martialarts.cpp:71`), and a character's `martial_arts_data` names one by id. The port keeps them the same way every other
/// table is kept: a component on one entity, with a marker saying which table it is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Component)]
pub struct StyleTable {
    /// The styles by id.
    pub by_id: std::collections::BTreeMap<String, Style>,
}

/// Marks the stored style table.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheStyleTable;

/// Installs the data's styles, so a creature can learn one.
pub fn install(world: &mut bevy_ecs::prelude::World, styles: &cdda_content::martial_arts::Styles) {
    let mut by_id = std::collections::BTreeMap::new();
    for style in &styles.all {
        by_id.insert(style.id.clone(), from_content(style));
    }
    let holder = crate::singletons::singleton::<StyleTable, TheStyleTable>(world);
    world.entity_mut(holder).insert(StyleTable { by_id });
}

/// One of the data's styles, as the fight reads it.
///
/// **The requirement is the same type the buffs use**, because the reference states it with the same `ma_requirements`; a second type here would be a second implementation of one rule.
#[must_use]
pub fn from_content(style: &cdda_content::martial_arts::Style) -> Style {
    Style {
        id: style.id.clone(),
        techniques: style
            .techniques
            .iter()
            .map(|technique| Technique {
                id: technique.id.clone(),
                requirement: Requirement {
                    unarmed_allowed: technique.requirement.unarmed_allowed,
                    melee_allowed: technique.requirement.melee_allowed,
                    strictly_unarmed: technique.requirement.strictly_unarmed,
                    wall_adjacent: technique.requirement.wall_adjacent,
                    weapon_categories: technique.requirement.weapon_categories.clone(),
                },
                movement: AppliedMove {
                    effects: technique
                        .effects
                        .iter()
                        .map(|effect| MoveEffect {
                            id: effect.id.clone(),
                            duration: effect.duration,
                            permanent: effect.permanent,
                            on_damage: effect.on_damage,
                            chance: effect.chance,
                        })
                        .collect(),
                    down_dur: technique.down_dur,
                    stun_dur: technique.stun_dur,
                },
                flags: TechniqueFlags {
                    weighting: technique.weighting,
                    dummy: technique.dummy,
                    defensive: technique.defensive,
                    crit_tec: technique.crit_tec,
                    crit_ok: technique.crit_ok,
                    reach_tec: technique.reach_tec,
                    reach_ok: technique.reach_ok,
                    dodge_counter: technique.dodge_counter,
                    block_counter: technique.block_counter,
                    needs_ammo: technique.needs_ammo,
                    disarms: technique.disarms,
                    take_weapon: technique.take_weapon,
                },
            })
            .collect(),
    }
}

/// Teaches a creature a style the data defines, and answers whether it learned one.
///
/// Reference: `character_martial_arts::set_style` (`martialarts.cpp`): the style is named by id and looked up, and an id with nothing behind it is refused. **The refusal is the point**: a fighter
/// given an empty style and a fighter with none must not become the same thing here, because the pick would then answer nothing for the wrong reason.
pub fn learn_style(
    world: &mut bevy_ecs::prelude::World,
    creature: bevy_ecs::prelude::Entity,
    style_id: &str,
) -> bool {
    let Some(holder) = crate::singletons::holder::<TheStyleTable>(world) else {
        return false;
    };
    let Some(style) = world
        .get::<StyleTable>(holder)
        .and_then(|table| table.by_id.get(style_id).cloned())
    else {
        return false;
    };
    world.entity_mut(creature).insert(Practising { style });
    true
}

/// The move a striker made on its last blow, or nothing when it made the plain one.
///
/// Reference: `Character::melee_attack` carries the chosen technique with the blow it applies to (`melee.cpp:754`). The port's blow is a function rather than a struct, so the choice is **left on the
/// striker** where a turn, a test or a UI can read it - the same reason every other answer in this port is a component rather than a return value nobody keeps.
#[derive(Debug, Clone, Default, PartialEq, Eq, Component)]
pub struct LastMove(pub Option<String>);

/// One effect a move may leave on the body it lands on.
///
/// Reference: `tech_effect_data` (`martialarts.h:147-160`): an id, how long it lasts, whether it is permanent, whether it needs the blow to draw blood, and a chance **out of a hundred**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveEffect {
    /// Which effect.
    pub id: String,
    /// How long it lasts, in turns.
    pub duration: i64,
    /// Whether it never wears off.
    pub permanent: bool,
    /// Whether it only lands on a blow that drew blood.
    pub on_damage: bool,
    /// Its chance, out of a hundred.
    pub chance: i32,
}

/// What a move does to the body it lands on, beyond the damage.
///
/// Reference: `ma_technique`'s `tech_effects`, `down_dur` and `stun_dur` (`martialarts.h:147-243`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppliedMove {
    /// The effects it may leave.
    pub effects: Vec<MoveEffect>,
    /// How long it may knock the target down, or nothing.
    pub down_dur: i32,
    /// How long it may stun the target, or nothing.
    pub stun_dur: i32,
}

/// Applies a move's effects to the body it hit, answering which ones landed.
///
/// Reference: `melee.cpp:1759-1785`. Each effect lands when **`x_in_y( chance, 100 )`** holds **and** either the blow drew blood or the effect does not need it
/// (`di.total_damage() != 0 || !eff.on_damage`). Then `down_dur` lands `downed` and `stun_dur` lands `stunned`, each for a duration drawn from **one turn up to that many** - the reference's
/// `rng( 1_turns, from_turns( n ) )`, so a move that stuns never stuns for nothing.
#[must_use]
pub fn apply_move(
    world: &mut bevy_ecs::prelude::World,
    target: bevy_ecs::prelude::Entity,
    movement: &AppliedMove,
    damage_dealt: i32,
) -> Vec<String> {
    let mut landed = Vec::new();
    let mut rolls = crate::singletons::take_rolls(world);
    for effect in &movement.effects {
        if !(damage_dealt != 0 || !effect.on_damage) {
            continue;
        }
        // **The effect's own numbers are the registry's**, so an id the data never defined cannot land as a bare name with invented limits.
        let Some(kind) = crate::effect_registry::kind_of(world, &effect.id) else {
            continue;
        };
        // **The roll comes from the store's own dice**, never from a second source: a rule that took its own roll
        // would be a second source of truth for the same draw.
        let this_roll = rolls.as_mut().map_or(1.0, |dice| dice.rng_float(0.0, 1.0));
        if this_roll <= f64::from(effect.chance) / 100.0 {
            crate::applied_effects::add(
                world,
                target,
                crate::effects::ActiveEffect::new(&kind, effect.duration, 1),
            );
            landed.push(effect.id.clone());
        }
    }
    for (duration_turns, id) in [(movement.stun_dur, "stunned"), (movement.down_dur, "downed")] {
        if duration_turns <= 0 {
            continue;
        }
        // The reference draws from **one turn up to that many**, so a stunning move never stuns for nothing.
        let duration = i64::from(match rolls.as_mut() {
            Some(dice) => dice.rng(1, duration_turns),
            None => 1,
        });
        if let Some(kind) = crate::effect_registry::kind_of(world, id) {
            crate::applied_effects::add(
                world,
                target,
                crate::effects::ActiveEffect::new(&kind, duration, 1),
            );
            landed.push(id.to_owned());
        }
    }
    if let Some(dice) = rolls {
        crate::singletons::put_rolls(world, dice);
    }
    landed
}
