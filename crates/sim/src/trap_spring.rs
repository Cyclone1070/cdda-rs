//! **What happens when a creature steps on a trap.**
//!
//! Reference: `trap::trigger` (`trap.cpp`) reached from `map::creature_on_trap`, and the `eocs` action in particular (`trapfunc::eocs`, `trapfunc.cpp:382-400`). That one is
//! small and is the shape this module takes: build a dialogue with the creature as the speaker, hand it the trap's own square in the invocation's context under the name
//! **`trap_location`**, run each of the trap's programs, and **remove the trap**.
//!
//! Two details are the rule rather than incidentals:
//!
//! - the square goes into the **context** (`write_var_value( var_type::context, "trap_location", ... )`), which is why the telepad's own program adjusts and teleports the
//!   value the trap handed it;
//! - the trap is removed **whether or not** the programs did anything, because a sprung trap is gone - and the removal is the last thing the reference does.
//!
//! Only traps whose action is `eocs` are sprung here. The named actions (`bubble`, `tripwire`, the eighty-odd in `trapfunc.cpp`) are their own rules and are not ported;
//! a trap with one is left alone rather than half-triggered, and `docs/parity.md` names it.

use bevy_ecs::prelude::*;

/// Springs the trap under a creature, and answers whether there was one to spring.
///
/// Reference: `trapfunc::eocs` (`trapfunc.cpp:382-400`) called from `trap::trigger`. Nothing happens when the square carries no trap, when the trap's action is not `eocs`,
/// or when the data names no programs - in each case the trap stays where it is, because a trap the port cannot act on is better left set than silently removed.
pub fn spring(world: &mut World, who: Entity, square: (i32, i32, i32)) -> bool {
    spring_with(world, who, square, true)
}

/// The same, and whether the creature **may dodge** it.
///
/// Reference: `map::creature_on_trap( Creature &c, bool may_avoid )` (`map.cpp:9881`), called with `true` for the player (`game.cpp:11361`) and `false` for monsters
/// (`game.cpp:12221`). A monster does not notice traps, so its call skips the avoidance branch entirely.
pub fn spring_with(
    world: &mut World,
    who: Entity,
    square: (i32, i32, i32),
    may_avoid: bool,
) -> bool {
    let Some(kind) = crate::square::trap_at(world, square) else {
        return false;
    };
    let Some(trap) = crate::trap_registry::get(world, &kind) else {
        return false;
    };
    // **One function per action**, because each action is its own rule with its own reference: the dispatcher below decides *which* rule applies, and each rule says what
    // it does. A single function holding all three was 155 lines and clippy was right to object - the rule is the reading, not the length.
    match trap.action.as_deref() {
        Some("map_regen") => return rebuild_the_tile(world, who, square, &trap),
        Some("spell") => return cast_its_spell(world, who, square, &trap, &kind),
        Some("glass") => return broken_glass(world, who, square),
        Some("shotgun") => return shotgun_trap(world, who, square, &kind),
        _ => {}
    }
    if trap.action.as_deref() != Some("eocs") || trap.eocs.is_empty() {
        return false;
    }
    // **A benign trap is not a trap.** Reference: `map::maybe_trigger_trap` returns before doing anything for a `benign` trap (`map.cpp:9923-9926`) - a cot is a trap in the
    // data and nothing should happen to whoever lies on it.
    if trap.benign {
        return false;
    }
    // **Some traps only care about the player.** Reference: `map.cpp:9928-9930` skips the trigger when the trap carries `AVATAR_ONLY` and the creature is not the avatar.
    if trap.flags.contains("AVATAR_ONLY") && crate::missions::avatar(world) != Some(who) {
        return false;
    }
    // **A rider is above the trap.** Reference: `map::creature_on_trap` returns at once for a character `in_vehicle` or gliding (`map.cpp:9886-9888`) - the wheels, not the
    // passenger, meet the trap, and a vehicle's own collisions are a different rule.
    if world.get::<crate::vehicles::Riding>(who).is_some() {
        return false;
    }
    // **A trap that is noticed is stepped over.** Reference: `map.cpp:9932-9939` - the roll is skipped for `UNDODGEABLE` traps and for a creature that may not avoid, and a
    // trap that is dodged is announced and remembered unless it is always invisible.
    if !trap.flags.contains("UNDODGEABLE")
        && may_avoid
        && crate::trap_avoidance::avoid_trap(world, who, &trap, square)
    {
        if !trap.always_invisible && !crate::trap_avoidance::knows(world, who, square) {
            crate::messages::push(world, &format!("You've spotted a {}!", trap.name));
            crate::trap_avoidance::remember(world, who, square);
        }
        return false;
    }
    // **The trap says what happened**, in the words the data gave it. Reference: `map.cpp:9941-9944` - the line is shown when the trap is not `always_invisible` and the data
    // supplied **both** halves (`has_trigger_msg`, `trap.h:258-260`). The player hears `trigger_message_u`; an NPC or monster hears `trigger_message_npc` with `<npcname>`
    // replaced by its own name (`Creature::replace_with_npc_name`, `creature.cpp:3408-3412`).
    if !trap.always_invisible
        && let (Some(player), Some(other)) =
            (trap.trigger_u.as_deref(), trap.trigger_npc.as_deref())
    {
        let line = if crate::missions::avatar(world) == Some(who) {
            player.to_owned()
        } else {
            let name = world
                .get::<crate::creature::Creature>(who)
                .map_or("Someone", |creature| creature.name.as_str());
            other.replace("<npcname>", name)
        };
        crate::messages::push(world, &line);
    }
    // Reference: `dialogue d( get_talker_for( critter ), nullptr ); write_var_value( var_type::context, "trap_location", &d, trap_location );`
    let previous = crate::dialogue_context::values(world, who);
    let mut context = previous.clone();
    context.insert(
        "trap_location".to_owned(),
        format!("{},{},{}", square.0, square.1, square.2),
    );
    crate::dialogue_context::put(world, who, context);
    for program in &trap.eocs {
        crate::eoc_registry::run_program(world, who, None, program, 0);
    }
    crate::dialogue_context::restore(world, who, Some(previous));
    // **The trap is gone**, which the reference does after the programs whatever they did.
    crate::square::remove_trap(world, square);
    true
}

/// **A trap that rebuilds its tile**, the data's third most common action (seventeen traps).
///
/// Reference: `trapfunc::map_regen` (`trapfunc.cpp:1522-1547`): only a **character** sets it off - `dynamic_cast<Character *>` answers nothing for a monster - it says "Your
/// surroundings shift!", the trap is removed **before** the mapgen runs, and the update lands on the character's own overmap tile.
fn rebuild_the_tile(
    world: &mut World,
    who: Entity,
    square: (i32, i32, i32),
    trap: &cdda_content::traps::Trap,
) -> bool {
    if !crate::npcs::is_npc(world, who) && crate::missions::avatar(world) != Some(who) {
        return false;
    }
    let Some(update) = trap.map_regen.clone() else {
        return false;
    };
    crate::messages::push(world, "Your surroundings shift!");
    crate::square::remove_trap(world, square);
    let Some(at) = world.get::<crate::Position>(who).copied() else {
        return false;
    };
    let tile = crate::overmap::tile_of(at.x, at.y);
    let place = (
        tile.0 * crate::overmap::SQUARES_PER_OMT,
        tile.1 * crate::overmap::SQUARES_PER_OMT,
        at.z,
    );
    let _written = crate::mapgen_update::apply(world, &update, place);
    let _fields = crate::mapgen_update::place_fields(world, &update, place);
    true
}

/// **A trap that casts a spell**, the data's most common action (thirty-four traps).
///
/// Reference: `trapfunc::cast_spell` (`trapfunc.cpp:1568-1606`): the trap is removed **before** the cast - a teleport would leave the trap's square pointing at somewhere the
/// creature no longer is - and the spell is cast with the creature as its caster and the trap's own square as the centre. `UNCONSUMED` keeps the trap set, so a trap meant to
/// keep casting does.
fn cast_its_spell(
    world: &mut World,
    who: Entity,
    square: (i32, i32, i32),
    trap: &cdda_content::traps::Trap,
    kind: &str,
) -> bool {
    let Some(spell) = trap.spell.clone() else {
        return false;
    };
    let Some(prototype) = crate::spell_types::find(world, &spell.id) else {
        return false;
    };
    // Reference: `fake_spell::get_spell` clamps the trap's level to the spell's ceiling (`magic.cpp:3571-3600`); the floor is the level itself, so the clamp only
    // ever lowers it, and a spell with no ceiling keeps the level the data gave.
    let ceiling = spell
        .max_level
        .or_else(|| {
            prototype
                .max_level
                // A ceiling is a count of levels, and the data writes it whole; a fractional one truncates, which is what the reference's own `int` does.
                .and_then(as_whole_level)
        })
        .unwrap_or(spell.level);
    let level = spell.level.min(ceiling);
    let unconsumed = trap.flags.contains("UNCONSUMED");
    if !unconsumed {
        crate::square::remove_trap(world, square);
    }
    // Reference: `PROXIMITY` clears every trap of the same kind in the 3x3 around the spring, so one trigger does not set off nine (`trapfunc.cpp:1590-1600`).
    if trap.flags.contains("PROXIMITY") {
        for x in square.0 - 1..=square.0 + 1 {
            for y in square.1 - 1..=square.1 + 1 {
                let at = (x, y, square.2);
                if crate::square::trap_at(world, at).as_deref() == Some(kind) {
                    crate::square::remove_trap(world, at);
                }
            }
        }
    }
    let _cast = crate::spell_effects::apply(world, who, &prototype, level, square);
    true
}

/// **A trap the walker may step past**: broken glass, the first action whose rule reads the body.
///
/// Reference: `trapfunc::glass` (`trapfunc.cpp:151-175`): only a creature sets it off; whether it steps on the shards at all is `x_in_y( occupied_tile_fraction( size ), 1 )`,
/// so a tiny creature walks between them nine times in ten; the cut is `max( 0, rng( -10, 5 ) )`; a cut that lands spends `speed * 0.8` moves and is dealt to a random
/// **ground-contact** part; and the trap says "glass cracking!" and is removed.
fn broken_glass(world: &mut World, who: Entity, square: (i32, i32, i32)) -> bool {
    let size = world
        .get::<crate::creature::Creature>(who)
        .map_or(crate::CreatureSize::Medium, |creature| creature.size);
    let Some(mut rolls) = crate::singletons::take_rolls(world) else {
        return false;
    };
    // Whether it steps on the shards at all: `x_in_y( occupied_tile_fraction( size ), 1.0f )`, so a tiny creature walks between them nine times in ten.
    let stepped = rolls.x_in_y(crate::ballistics::occupied_tile_fraction(size), 1.0);
    // The cut is `max( 0, rng( -10, 5 ) )` - drawn whether or not the glass is stepped on, because the reference draws it after the message.
    let cut = if stepped { rolls.rng(-10, 5).max(0) } else { 0 };
    let part = if stepped {
        ground_contact_part(world, who, &mut rolls)
    } else {
        None
    };
    crate::singletons::put_rolls(world, rolls);
    if !stepped {
        return false;
    }
    let line = if crate::missions::avatar(world) == Some(who) {
        "You step on some glass!".to_owned()
    } else {
        format!("{} steps on some glass!", name_of(world, who))
    };
    crate::messages::push(world, &line);
    // Reference: `c->mod_moves( -c->get_speed() * 0.8 )` - the flinch, before the damage, and only for a cut that landed.
    if cut > 0 {
        #[allow(clippy::cast_precision_loss)]
        let cost = f64::from(crate::creature::speed_of(world, who)) * 0.8;
        crate::creature::spend_moves(world, who, cost);
        // What the part the glass reached stops: `get_armor_type( cut, bp )` (`character_armor.cpp:70-86`).
        let stops = part.as_deref().map_or(0, |hit| {
            crate::creature::armour_on(world, who, hit, Some("cut"))
        });
        let through = (cut - stops).max(0);
        #[allow(clippy::cast_precision_loss)]
        crate::creature::hurt(world, who, through as f32);
    }
    crate::sounds::make_a_sound_of_kind(
        world,
        square,
        1,
        "glass cracking!",
        crate::sounds::SoundCategory::Combat,
    );
    crate::square::remove_trap(world, square);
    true
}

/// **A shotgun trap fires**, the largest of the actions the port had no rule for and the only one that shoots.
///
/// Reference: `trapfunc::shotgun` (`trapfunc.cpp:566-667`). The sound is `MAX_VIEW_DISTANCE` loud - a shotgun is heard across the map. A character triggers it and is told so;
/// how many shells fire is a roll the trap's own kind limits (`tr_shotgun_2` can fire two, `tr_shotgun_1` one); a shell that is not dodged hits a part by a ten-sided roll -
/// a foot once in twenty, a leg three times in ten, the torso five times, the head once - for `rng( 40, 60 )` bullet damage per shell. A monster's own chance of two shells
/// depends on how big it is. Then the trap is gone and its gun and its string of shells are on the ground.
fn shotgun_trap(world: &mut World, who: Entity, square: (i32, i32, i32), kind: &str) -> bool {
    let Some(mut rolls) = crate::singletons::take_rolls(world) else {
        return false;
    };
    // Reference: `sounds::sound( p, MAX_VIEW_DISTANCE, ..., "Kerblam!" )` - the loudest thing a trap does, and it happens whether or not anyone is hit.
    crate::sounds::make_a_sound_of_kind(
        world,
        square,
        cdda_core_types::light::MAX_VIEW_DISTANCE,
        "Kerblam!",
        crate::sounds::SoundCategory::Combat,
    );
    // The ridden creature hears about its mount first (`trapfunc.cpp:573-575`).
    let name = world
        .get::<crate::creature::Creature>(who)
        .map_or_else(String::new, |creature| creature.name.clone());
    if world.get::<crate::vehicles::Riding>(who).is_some() {
        crate::messages::push(world, &format!("Your {name} triggers a shotgun trap!"));
    }
    let is_person = crate::npcs::is_npc(world, who) || crate::missions::avatar(world) == Some(who);
    // The announcement, in the reference's two voices (`trapfunc.cpp:576-577`).
    if is_person {
        let line = if crate::missions::avatar(world) == Some(who) {
            "You trigger a shotgun trap!".to_owned()
        } else {
            format!("{name} triggers a shotgun trap!")
        };
        crate::messages::push(world, &line);
    }
    // How many shells: a character's strength raises the chance of two, and the trap's own kind caps it; a monster's size sets its chance (`trapfunc.cpp:580-640`).
    let mut shots;
    if is_person {
        let strength = crate::creature::effective_strength(world, who);
        let two = rolls.one_in(8) || rolls.one_in((20 - strength).max(1));
        shots = if two { 2 } else { 1 };
    } else {
        let chance = match world
            .get::<crate::creature::Creature>(who)
            .map_or(crate::CreatureSize::Medium, |creature| creature.size)
        {
            crate::CreatureSize::Tiny => 100,
            crate::CreatureSize::Small => 16,
            crate::CreatureSize::Medium => 12,
            crate::CreatureSize::Large => 8,
            crate::CreatureSize::Huge => 2,
        };
        shots = if rolls.one_in(8) || rolls.one_in(chance) {
            2
        } else {
            1
        };
    }
    // Reference: `if( here.tr_at( p ) != tr_shotgun_2 ) shots = 1;` - only the double-barrelled trap can fire twice.
    if kind != "tr_shotgun_2" {
        shots = 1;
    }
    let bullet = rolls.rng(40 * shots, 60 * shots);
    // A character may dodge it: `rng( 5, 50 ) > you->get_dodge()` (`trapfunc.cpp:587`). A monster does not dodge, and the reference gives it the torso.
    // Reference: `rng( 5, 50 ) > you->get_dodge()` (`trapfunc.cpp:587`) - and `get_dodge` is never zero for a person, because a character's base dodge is the limb score its
    // body states. A creature with no `Dodge` component answers the same, which is what makes a fixture without a body dodge everything.
    let dodge = world
        .get::<crate::creature::Dodge>(who)
        .map_or(0, |dodge| dodge.points);
    let dodged = is_person && rolls.rng(5, 50) > dodge;
    let aimed = if dodged {
        None
    } else if is_person {
        Some(person_part(&mut rolls))
    } else {
        Some("torso".to_owned())
    };
    crate::singletons::put_rolls(world, rolls);
    if let Some(part) = aimed {
        if is_person {
            let line = if crate::missions::avatar(world) == Some(who) {
                format!("Your {} is hit!", part.replace('_', " "))
            } else {
                format!("{name}'s {} is hit!", part.replace('_', " "))
            };
            crate::messages::push(world, &line);
        } else {
            crate::messages::push(world, &format!("A shotgun fires and hits the {name}!"));
        }
        let stops = crate::creature::armour_on(world, who, &part, Some("bullet"));
        let through = (bullet - stops).max(0);
        #[allow(clippy::cast_precision_loss)]
        crate::creature::hurt(world, who, through as f32);
    } else {
        let line = if crate::missions::avatar(world) == Some(who) {
            "You dodge the shot!".to_owned()
        } else {
            format!("{name} dodges the shot!")
        };
        crate::messages::push(world, &line);
    }
    // **What is left behind**, which is the trap's own gun and a string of shells (`trapfunc.cpp:660-665`). The sawn-off is the single-barrelled trap's.
    let gun = if kind == "tr_shotgun_1" {
        "shotgun_s"
    } else {
        "shotgun_d"
    };
    for dropped in [gun, "string_36"] {
        if let Some(item) = crate::item_registry::id_of(world, dropped) {
            crate::drop_item(world, (square.0, square.1), item, 1);
        }
    }
    crate::square::remove_trap(world, square);
    true
}

/// The part a shotgun's single shell hits: a ten-sided roll, as `switch( rng( 1, 10 ) )` reads it (`trapfunc.cpp:589-612`).
fn person_part(rolls: &mut cdda_core_types::rng::Rolls) -> String {
    match rolls.rng(1, 10) {
        1 => {
            if rolls.one_in(2) {
                "foot_l".to_owned()
            } else {
                "foot_r".to_owned()
            }
        }
        2..=4 => {
            if rolls.one_in(2) {
                "leg_l".to_owned()
            } else {
                "leg_r".to_owned()
            }
        }
        10 => "head".to_owned(),
        _ => "torso".to_owned(),
    }
}

/// A level from the data's own number, which is written whole.
///
/// Reference: `spell_type::max_level` is an `int` (`magic.h`), so the reference never holds a fraction to begin with - a fractional one in the data truncates here for
/// the same reason it would there.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a level is a small count; the reference's own int truncates the same way"
)]
fn as_whole_level(number: f64) -> Option<i32> {
    (number.abs() < f64::from(i32::MAX)).then(|| i32::try_from(number as i64).unwrap_or_default())
}

/// A part that touches the ground - a leg or a foot - chosen the way `random_entry` chooses.
///
/// Reference: `Creature::get_ground_contact_bodyparts( false )` (`creature.cpp:2972-2999`) keeps the parts whose type is a foot; the port's body parts carry their
/// `primary_limb_type`, so the set is a filter rather than a table. A creature with no legs - a monster with no stated body - answers nothing, and the damage then lands on
/// the creature itself, which is where the port keeps every wound.
fn ground_contact_part(
    world: &World,
    creature: Entity,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> Option<String> {
    let parts = world.get::<crate::creature::Body>(creature)?.parts.clone();
    let ground: Vec<String> = parts
        .iter()
        .filter(|part| {
            part.primary_limb_type
                .as_deref()
                .is_some_and(|kind| kind == "leg" || kind == "foot")
        })
        .map(|part| part.name.clone())
        .collect();
    if ground.is_empty() {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    let top = i32::try_from(ground.len()).unwrap_or(1) - 1;
    let at = usize::try_from(rolls.rng(0, top)).unwrap_or(0);
    ground.get(at).cloned()
}

/// What a message calls a creature: its name when it is not the player.
fn name_of(world: &World, creature: Entity) -> String {
    world
        .get::<crate::creature::Creature>(creature)
        .map_or_else(String::new, |creature| creature.name.clone())
}
