//! A character's standing in one skill: the level, the experience behind it, and what training does to both.
//!
//! Reference: `SkillLevel` (`src/skill.h:130-253`, `src/skill.cpp:285-480`). Three things here are not what a reading of "experience points" suggests:
//!
//! * experience is stored **raw** and reported **scaled**: `exercise()` divides by `100 * ( level + 1 )^2`, so the same raw number is worth less at every level;
//! * levelling reaches the next level at `10000 * ( level + 1 )^2` points, and the excess is **spent** — dropped to zero — unless multi-level training is allowed, so a
//!   character who gains four levels' worth lands on one with nothing left over;
//! * the level jumps inside a **loop**, so one call can cross several levels when allowed, and the knowledge level follows the skill level up and resets the knowledge
//!   experience as it goes.
//!
//! Maximum skill is ten (`game_constants.h:87`), and `level()` reports the capped level while the arithmetic runs on the uncapped one — which is what lets rust take a
//! character back down from a level the cap was hiding.

/// The highest a skill can be trained to: `MAX_SKILL` (`src/game_constants.h:87`).
pub const MAX_SKILL: i32 = 10;

/// One character's standing in one skill (`src/skill.h:130-253`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkillLevel {
    level: i32,
    exercise: i32,
    knowledge_level: i32,
    knowledge_experience: i32,
    rust_accumulator: i32,
    last_practiced: i64,
}

impl SkillLevel {
    /// A character who has never practised it.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            level: 0,
            exercise: 0,
            knowledge_level: 0,
            knowledge_experience: 0,
            rust_accumulator: 0,
            last_practiced: 0,
        }
    }

    /// The level, capped: `level()` (`src/skill.h:154-156`).
    #[must_use]
    pub fn level(&self) -> i32 {
        self.level.min(MAX_SKILL)
    }

    /// Sets the level, raising the knowledge level with it: `level( int )` (`src/skill.h:157-163`).
    pub fn level_to(&mut self, level: i32) -> i32 {
        self.level = level;
        if self.level > self.knowledge_level {
            self.knowledge_level = self.level;
        }
        self.level()
    }

    /// The knowledge level, capped: `knowledgeLevel()` (`src/skill.h:165-167`).
    #[must_use]
    pub fn knowledge_level(&self) -> i32 {
        self.knowledge_level.min(MAX_SKILL)
    }

    /// The experience behind the level, raw: `_exercise` (`src/skill.h:133`).
    #[must_use]
    pub const fn raw_exercise(&self) -> i32 {
        self.exercise
    }

    /// The experience reported against the level: `exercise( bool )` (`src/skill.h:183-185`).
    ///
    /// The divisor is `100 * ( level + 1 )^2` and the level is the **capped** one, so a character at the cap reports against level eleven's divisor.
    #[must_use]
    pub fn exercise(&self, raw: bool) -> i32 {
        if raw {
            return self.exercise;
        }
        let level = self.level() + 1;
        if level == 0 {
            // The reference divides by zero here and dies of SIGFPE: negative experience at level zero takes the level to `-1`, and the divisor `100 * ( level + 1 )^2`
            // collapses. A port that reproduced the crash would be unusable, so it answers zero and `docs/parity.md` records the defect.
            return 0;
        }
        self.exercise / (100 * level * level)
    }

    /// The knowledge experience, reported the same way: `knowledgeExperience( bool )` (`src/skill.h:173-176`).
    #[must_use]
    pub fn knowledge_experience(&self, raw: bool) -> i32 {
        if raw {
            return self.knowledge_experience;
        }
        let level = self.knowledge_level() + 1;
        self.knowledge_experience / (100 * level * level)
    }

    /// The level and its progress as one number: `exercised_level()` (`src/skill.h:187-189`).
    #[must_use]
    pub fn exercised_level(&self) -> i32 {
        self.level() * self.level() * 100 + self.exercise(false)
    }

    /// Sets the experience, raw or in report units: `set_exercise` (`src/skill.cpp:444-448`).
    ///
    /// The non-raw form multiplies by the same divisor the reader divides by, so setting the report to one really is one point of report whatever the level. The change
    /// **always** allows crossing several levels, because that is what the reference passes (`on_exercise_change( true )`); the spending path is `train`'s.
    pub fn set_exercise(&mut self, value: i32, raw: bool) {
        let level = self.level() + 1;
        self.exercise = if raw { value } else { value * (100 * level * level) };
        self.on_exercise_change(true);
    }

    /// What a change in experience does to the level: `on_exercise_change` (`src/skill.cpp:450-480`).
    ///
    /// Negative experience does not clamp: it **removes a level** and parks at one point below the top of the level below (`10000 * level^2 - 1`). Positive experience
    /// crosses a level per `10000 * ( level + 1 )^2` points, keeping the remainder only when multi-level training is allowed.
    pub fn on_exercise_change(&mut self, allow_multilevel: bool) {
        if self.exercise < 0 {
            self.exercise = 100 * 100 * self.level * self.level - 1;
            self.level -= 1;
            return;
        }
        loop {
            let needed = 100 * 100 * (self.level + 1) * (self.level + 1);
            if self.exercise < needed {
                break;
            }
            self.exercise = if allow_multilevel {
                self.exercise - needed
            } else {
                0
            };
            self.level += 1;
            if self.level > self.knowledge_level {
                self.knowledge_level = self.level;
                self.knowledge_experience = 0;
            }
        }
        if self.rust_accumulator < 0 {
            self.rust_accumulator = 0;
        }
        if self.level() == self.knowledge_level() && self.exercise > self.knowledge_experience {
            self.knowledge_experience = self.exercise;
        }
        if self.knowledge_experience >= 100 * 100 * (self.knowledge_level + 1) * (self.knowledge_level + 1) {
            self.knowledge_experience = 0;
            self.knowledge_level += 1;
        }
    }

    /// Adds practice to the skill and its knowledge, then levels it: `train` (`src/skill.cpp:285-333`).
    ///
    /// Three branches decide how much of `amount` reaches the skill. When knowledge is **ahead** of practice the catch-up grows with the gap; when the two are level and
    /// knowledge experience is ahead, catch-up and knowledge both slow down in proportion to how far along the level the practice is, with a floor of one times the amount;
    /// otherwise both take the amount whole. Knowledge is then capped at ninety percent of catch-up — learning faster than practising "generates annoying problems", says the
    /// reference — and at zero once knowledge is at the maximum. `scaling` is the `SKILL_TRAINING_SPEED` option, passed in rather than read, and it is applied only when
    /// positive.
    ///
    /// The additions are **real** in the reference and the field is whole, so the sum truncates: a catch-up of 50.9 added to nothing is fifty. `rust_accumulator` goes
    /// **down** by the catch-up, which is the reference's own sign (`_rustAccumulator -= catchup_amount`).
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    pub fn train(
        &mut self,
        amount: i32,
        catchup_modifier: f32,
        knowledge_modifier: f32,
        scaling: f64,
        allow_multilevel: bool,
    ) {
        if amount < 0 {
            return;
        }
        let gap = f32::from(i16::try_from(self.knowledge_level().max(1)).unwrap_or(1))
            / f32::from(i16::try_from(self.level().max(1)).unwrap_or(1));
        let amount_real = amount as f32;
        let mut catchup = amount_real * catchup_modifier;
        let mut knowledge = amount_real * knowledge_modifier;
        if self.knowledge_level() > self.level() {
            catchup *= gap;
        } else if self.knowledge_level() == self.level() && self.knowledge_experience > self.exercise {
            let share = self.exercise as f32 / self.knowledge_experience as f32;
            catchup = amount_real * (catchup_modifier - share).max(1.0);
            knowledge = amount_real * (knowledge_modifier - 0.1 * share).max(1.0);
        } else {
            catchup = amount_real;
            knowledge = amount_real;
        }
        if knowledge > catchup * 0.9 {
            knowledge = catchup * 0.9;
        }
        if self.knowledge_level() >= MAX_SKILL {
            knowledge = 0.0;
        }
        #[allow(clippy::cast_possible_truncation)]
        let scaling_real = scaling as f32;
        if scaling > 0.0 {
            catchup *= scaling_real;
            knowledge *= scaling_real;
        }
        let before = self.exercise;
        self.exercise = (self.exercise as f32 + catchup) as i32;
        if self.exercise < 0 {
            // The reference undoes the addition and complains about an overflow; there is nothing to change and no message channel here.
            self.exercise = before;
            return;
        }
        self.rust_accumulator = (self.rust_accumulator as f32 - catchup) as i32;
        self.knowledge_experience = (self.knowledge_experience as f32 + knowledge) as i32;
        self.on_exercise_change(allow_multilevel);
        self.practice(self.last_practiced);
    }

    /// Adds book knowledge alone: `knowledge_train` (`src/skill.cpp:335-367`).
    ///
    /// The amount is scaled by `2 / ( gap + 1 )`, where the gap is how far knowledge already stands above practice — a teacher's knowledge counts too, and the best
    /// teacher is only somewhat more knowledgeable. The product truncates (`amount *= level_mult` on an `int`), the option's scaling is applied with `ceil`, and the
    /// knowledge experience crossing `10000 * ( knowledge level + 1 )^2` costs the whole of it and raises the knowledge level by exactly one — not in a loop, unlike
    /// the skill's own levelling.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    pub fn knowledge_train(&mut self, amount: i32, npc_knowledge: i32, scaling: f64) {
        let gap = if npc_knowledge > 0 {
            (npc_knowledge - self.knowledge_level()).max(1) as f32
        } else {
            (self.knowledge_level() - self.level()).max(1) as f32
        };
        let level_mult = 2.0 / (gap + 1.0);
        let mut amount = (amount as f32 * level_mult) as i32;
        if scaling > 0.0 {
            #[allow(clippy::cast_possible_truncation)]
            let scaled = (f64::from(amount) * scaling).ceil();
            amount = scaled as i32;
        }
        self.knowledge_experience += amount;
        if self.knowledge_experience >= 100 * 100 * (self.knowledge_level + 1) * (self.knowledge_level + 1) {
            self.knowledge_experience = 0;
            self.knowledge_level += 1;
        }
    }

    /// Marks the skill as practised now: `practice()` (`src/skill.cpp:425-428`).
    pub fn practice(&mut self, turn: i64) {
        self.last_practiced = turn;
    }

    /// When it was last practised.
    #[must_use]
    pub const fn last_practiced(&self) -> i64 {
        self.last_practiced
    }

    /// How much rust has built up.
    #[must_use]
    pub const fn rust_accumulator(&self) -> i32 {
        self.rust_accumulator
    }
}

/// How sharply a creature is paying attention, on the reference's hundred-point scale.
///
/// Reference: `Character::focus`, which every practice, reading and crafting gain is scaled by. A fresh survivor starts at a hundred, and a number below it means
/// the training is worth proportionally less.
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Focus(pub i32);

impl Default for Focus {
    fn default() -> Self {
        Focus(100)
    }
}

/// Scales a practice amount by the creature's focus.
///
/// Reference: `Character::adjust_for_focus` (`character.cpp:10172-10175`): `amount * ( get_effective_focus() / 100.0f )`. The port reads the focus component and
/// leaves the trait modifiers for the day traits exist here; a creature with no focus component is at the hundred a fresh survivor starts with.
#[must_use]
pub fn adjust_for_focus(world: &bevy_ecs::prelude::World, creature: bevy_ecs::prelude::Entity, amount: f32) -> f32 {
    let focus = world
        .get::<Focus>(creature)
        .map_or(Focus::default(), |focus| *focus);
    #[allow(clippy::cast_precision_loss)]
    let scale = focus.0 as f32 / 100.0;
    amount * scale
}

/// The focus a creature drifts towards, given how it feels.
///
/// Reference: `Character::calc_focus_equilibrium` (`character.cpp:3852-3920`) with no reading activity and no pain - the two modifiers the port has no store for -
/// so the whole answer is morale:
///
/// * **below -99** morale the equilibrium is 1, the floor;
/// * **between -99 and 50** each point of morale adds or takes a point, so fifty morale is a hundred and fifty focus;
/// * **above 50** the returns double every block - a hundred more morale buys the next fifty focus, then two hundred more, then four hundred - and the answer
///   **stops at 400**, which the reference calls a sanity check on its own arithmetic.
#[must_use]
pub fn focus_equilibrium(morale: i32) -> i32 {
    let mut equilibrium = 100;
    if morale < -99 {
        equilibrium = 1;
    } else if morale <= 50 {
        equilibrium += morale;
    } else {
        let mut block_multiplier = 1;
        let mut morale_left = morale;
        while equilibrium < 400 {
            if morale_left > 50 * block_multiplier {
                morale_left -= 50 * block_multiplier;
                equilibrium += 50;
                block_multiplier *= 2;
            } else {
                equilibrium += morale_left / block_multiplier;
                break;
            }
        }
    }
    equilibrium.clamp(1, 400)
}

/// The morale a creature effectively has, once pain is taken into account.
///
/// Reference: `Character::calc_focus_equilibrium` (`character.cpp:3868-3881`): pain is subtracted from morale - *"it's harder to rest your mind while your body hurts"* -
/// except for a masochist, who only minds it above twenty, and except for a creature that ignores pain outright (the reference's `CENOBITE`, or a caller that asks it to
/// be ignored).
#[must_use]
pub fn effective_morale(morale: i32, pain: i32, masochist: bool) -> i32 {
    if masochist {
        if pain > 20 {
            return morale - (pain - 20);
        }
        return morale;
    }
    morale - pain
}

/// Caps a focus equilibrium by how sleepy the creature is.
///
/// Reference: `Character::focus_equilibrium_sleepiness_cap` (`character.cpp:3838-3850`): a tired mind settles lower, and the more tired it is the lower the ceiling -
/// twenty at total exhaustion, forty at exhausted, sixty at dead tired, eighty at tired, and no ceiling at all below that.
#[must_use]
pub fn sleepiness_cap(equilibrium: i32, sleepiness: i32) -> i32 {
    // Reference: `sleepiness_levels` (`character.h:151-156`).
    const TIRED: i32 = 191;
    const DEAD_TIRED: i32 = 383;
    const EXHAUSTED: i32 = 575;
    const MASSIVE_SLEEPINESS: i32 = 1000;
    if sleepiness >= MASSIVE_SLEEPINESS && equilibrium > 20 {
        20
    } else if sleepiness >= EXHAUSTED && equilibrium > 40 {
        40
    } else if sleepiness >= DEAD_TIRED && equilibrium > 60 {
        60
    } else if sleepiness >= TIRED && equilibrium > 80 {
        80
    } else {
        equilibrium
    }
}

/// Moves a creature's focus a tenth of the way towards its equilibrium, which is what a minute of game time does.
///
/// Reference: `Character::update_mental_focus` (`character.cpp:3929-3934`), called from `reset_stats` on `once_every( 1_minutes )`, which adds ten times the gap
/// to the focus pool. The pool is kept in **hundredths** of a focus point, so the change is a tenth of the gap each minute, and the pool's own truncation is what
/// makes the arithmetic exact rather than approximately right.
pub fn update_mental_focus(world: &mut bevy_ecs::prelude::World, creature: bevy_ecs::prelude::Entity, morale: i32) {
    update_focus_towards(world, creature, focus_equilibrium(morale));
}

/// Moves a creature's focus a tenth of the way towards an equilibrium somebody else worked out.
///
/// The arithmetic is `update_mental_focus`'s own: the pool is kept in hundredths, so the change per minute is `10 * ( equilibrium - focus ) / 100`. The stage that has
/// already read the creature's pain, morale and sleepiness hands the answer in rather than making this function ask the same three questions again.
pub fn update_focus_towards(world: &mut bevy_ecs::prelude::World, creature: bevy_ecs::prelude::Entity, equilibrium: i32) {
    let Some(mut focus) = world.get_mut::<Focus>(creature) else {
        return;
    };
    let pool = focus.0.saturating_mul(100).saturating_add(10 * (equilibrium - focus.0));
    focus.0 = pool / 100;
}
