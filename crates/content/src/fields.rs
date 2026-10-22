//! Field prototypes: what each kind of field looks like at each intensity.
//!
//! Reference: `field_type` (`src/field_type.h`), reached through `--dump-fields`. A field's
//! number of levels is also its maximum intensity, which is what the runtime clamps to - so this
//! loader is what makes "intensity 3" mean something.

use cdda_core_types::rng::Rolls;
use cdda_json::{Object, Value};

/// One intensity of a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLevel {
    /// The displayed name.
    pub name: String,
    /// The ASCII glyph drawn for this intensity.
    pub sym: u32,
    /// The colour id drawn for this intensity.
    pub color: String,
    /// Whether standing in it is hazardous.
    pub dangerous: bool,
    /// Whether light passes through this intensity.
    pub transparent: bool,
    /// Extra movement cost to enter.
    pub move_cost: i32,
    /// The upgrade chance and period for this intensity, if it can grow on its own.
    pub upgrade_chance: i32,
    /// How often the upgrade chance is rolled, in turns.
    pub upgrade_duration: i64,
    /// The chance in a hundred that this intensity spawns a monster group.
    pub spawn_chance: i32,
    /// How many spawns to attempt.
    pub spawn_count: i32,
    /// How far from the field they appear.
    pub spawn_radius: i32,
    /// The monster group to draw from, empty when it spawns nothing.
    pub spawn_group: String,
}

/// One kind of field, with its levels in order (index 0 is intensity 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldType {
    /// The field's id, e.g. `fd_smoke`.
    pub id: String,
    /// Which field draws when several share a tile; higher wins.
    pub priority: i32,
    /// How much faster the field ages away under water, in turns.
    pub underwater_age_speedup: i64,
    /// How much faster it ages away outdoors, in turns.
    pub outdoor_age_speedup: i64,
    /// The chance in a hundred that this gas spreads to a neighbour each turn.
    pub percent_spread: i32,
    /// The intensities, weakest first.
    pub levels: Vec<FieldLevel>,
}

impl FieldType {
    /// The highest intensity this field can reach.
    #[must_use]
    pub fn max_intensity(&self) -> u8 {
        u8::try_from(self.levels.len()).unwrap_or(u8::MAX)
    }
}

fn int_of(object: &Object, key: &str) -> Option<i64> {
    match object.get(key)? {
        Value::Int(value) => Some(*value),
        _ => None,
    }
}

fn bool_of(object: &Object, key: &str) -> Option<bool> {
    match object.get(key)? {
        Value::Bool(value) => Some(*value),
        _ => None,
    }
}

fn str_of(object: &Object, key: &str) -> Option<String> {
    match object.get(key)? {
        Value::Str(value) => Some(value.clone()),
        _ => None,
    }
}

/// Reads the field dump.
///
/// # Errors
///
/// Returns the reason the dump could not be read: a shape it does not understand, a field with no
/// levels, or a level missing a piece the runtime needs.
pub fn from_dump(value: &Value) -> Result<Vec<FieldType>, String> {
    let Value::Object(root) = value else {
        return Err("the dump's root is not an object".to_string());
    };
    let Some(Value::Array(entries)) = root.get("prototypes") else {
        return Err("the dump has no prototypes array".to_string());
    };
    let mut fields = Vec::with_capacity(entries.len());
    for entry in entries {
        let Value::Object(object) = entry else {
            return Err("a field is not an object".to_string());
        };
        let Some(id) = str_of(object, "id") else {
            return Err("a field has no id".to_string());
        };
        let priority = int_of(object, "priority").unwrap_or(0);
        let underwater_age_speedup = int_of(object, "underwater_age_speedup").unwrap_or(0);
        let outdoor_age_speedup = int_of(object, "outdoor_age_speedup").unwrap_or(0);
        let percent_spread = int_of(object, "percent_spread").unwrap_or(0);
        let Some(Value::Array(levels)) = object.get("levels") else {
            return Err(format!("{id} has no levels"));
        };
        if levels.is_empty() {
            return Err(format!("{id} has no levels at all"));
        }
        let mut parsed = Vec::with_capacity(levels.len());
        for level in levels {
            let Value::Object(level) = level else {
                return Err(format!("a level of {id} is not an object"));
            };
            let sym = level.get("sym").and_then(|v| match v {
                Value::Int(code) => u32::try_from(*code).ok(),
                _ => None,
            });
            let Some(sym) = sym else {
                return Err(format!("a level of {id} has no glyph"));
            };
            parsed.push(FieldLevel {
                name: str_of(level, "name").unwrap_or_default(),
                sym,
                color: str_of(level, "color").unwrap_or_default(),
                dangerous: bool_of(level, "dangerous").unwrap_or(false),
                transparent: bool_of(level, "transparent").unwrap_or(true),
                move_cost: i32::try_from(int_of(level, "move_cost").unwrap_or(0)).unwrap_or(0),
                upgrade_chance: i32::try_from(int_of(level, "upgrade_chance").unwrap_or(0))
                    .unwrap_or(0),
                upgrade_duration: int_of(level, "upgrade_duration").unwrap_or(0),
                spawn_chance: i32::try_from(int_of(level, "spawn_chance").unwrap_or(0))
                    .unwrap_or(0),
                spawn_count: i32::try_from(int_of(level, "spawn_count").unwrap_or(0)).unwrap_or(0),
                spawn_radius: i32::try_from(int_of(level, "spawn_radius").unwrap_or(0))
                    .unwrap_or(0),
                spawn_group: str_of(level, "spawn_group").unwrap_or_default(),
            });
        }
        fields.push(FieldType {
            id,
            priority: i32::try_from(priority).unwrap_or(0),
            underwater_age_speedup,
            outdoor_age_speedup,
            percent_spread: i32::try_from(percent_spread).unwrap_or(0),
            levels: parsed,
        });
    }
    Ok(fields)
}

/// Finds a field by id.
#[must_use]
pub fn find<'a>(fields: &'a [FieldType], id: &str) -> Option<&'a FieldType> {
    fields.iter().find(|field| field.id == id)
}

/// The field ids the runtime uses, built from the dump.
///
/// An id is the field's position in the dump plus one, so zero stays free for the null field that
/// `add_field` refuses. The numbering is the port's own: the reference keys fields by string, and
/// a number is what lets a tile store one in a byte.
#[derive(Debug, Clone, Default)]
pub struct FieldRegistry {
    ids: std::collections::HashMap<String, u32>,
}

impl FieldRegistry {
    /// Builds the registry from a loaded field list.
    #[must_use]
    pub fn new(fields: &[FieldType]) -> Self {
        let mut ids = std::collections::HashMap::with_capacity(fields.len());
        for (index, field) in fields.iter().enumerate() {
            let Ok(id) = u32::try_from(index + 1) else {
                continue;
            };
            ids.insert(field.id.clone(), id);
        }
        Self { ids }
    }

    /// The id for a field name, if the reference defines it.
    #[must_use]
    pub fn id_of(&self, name: &str) -> Option<cdda_core_types::field::FieldId> {
        self.ids
            .get(name)
            .copied()
            .map(cdda_core_types::field::FieldId)
    }
}

/// What a monster-spawning field produces when its turn comes up.
///
/// Reference: `field_processor_monster_spawn` (`map_field.cpp:762-786`). The level's count and chance are tested **before any
/// dice are spent**, so a level with either at zero is free; then **one** `one_in(chance)` decides whether anything spawns at
/// all, and only then is the group asked, repeatedly, until the count is used up.
///
/// Returns the monsters' ids. The caller places them at random passable squares within the level's radius, which is the part of
/// the reference that needs a map rather than a table.
///
/// **Declared gap:** the reference asks `GetResultFromGroup` for a **batch** and passes its count in by reference, so a pack
/// entry can consume several points of the count and spawn several monsters at once. Packs are not ported, so a batch here is
/// always one monster.
pub fn spawn_from_field<'a>(
    spawn_chance: i32,
    spawn_count: i32,
    spawn_group: &str,
    groups: &'a [crate::monster_groups::MonsterGroup],
    rolls: &mut Rolls,
) -> Vec<&'a str> {
    let mut produced = Vec::new();
    if spawn_count <= 0 || spawn_chance <= 0 || spawn_group.is_empty() {
        return produced;
    }
    if !rolls.one_in(spawn_chance) {
        return produced;
    }
    let mut remaining = spawn_count;
    while remaining > 0 {
        match crate::monster_groups::pick_monster(spawn_group, groups, rolls) {
            Some(monster) => produced.push(monster),
            None => break,
        }
        remaining -= 1;
    }
    produced
}

/// Where one spawned monster lands: a square near the field, drawn the way the reference draws it.
///
/// Reference: `random_point( points_in_radius( p, radius ), passable )` (`map_field.cpp:777`). Three details matter and none is
/// the obvious reading:
///
/// * **The range is the bounding box**, not a circle - `points_in_radius` is `center - offset .. center + offset`
///   (`map_iterator.h:154`), so a square at the corner of the box is a legal spawn even though it is further away in a straight
///   line than the radius.
/// * **Ten optimistic tries** (`map_selector.cpp:59`), each spending three draws, before it enumerates the range.
/// * **The enumeration runs x fastest** (`map_iterator.h:44-58`), then y, then z, so the fallback list is row-major.
///
/// The predicate says whether a square will take a monster - in the reference, `passable_through`.
pub fn random_point_in_radius(
    center: (i32, i32),
    radius: i32,
    accepts: impl Fn(i32, i32) -> bool,
    rolls: &mut Rolls,
) -> Option<(i32, i32)> {
    let radius = radius.max(0);
    let (low_x, high_x) = (center.0 - radius, center.0 + radius);
    let (low_y, high_y) = (center.1 - radius, center.1 + radius);
    for _ in 0..10 {
        let x = rolls.rng(low_x, high_x);
        let y = rolls.rng(low_y, high_y);
        // The reference draws a z as well, with both bounds equal on a single-level search; the draw is spent regardless.
        let _z = rolls.rng(0, 0);
        if accepts(x, y) {
            return Some((x, y));
        }
    }
    let mut suitable = Vec::new();
    for y in low_y..=high_y {
        for x in low_x..=high_x {
            if accepts(x, y) {
                suitable.push((x, y));
            }
        }
    }
    if suitable.is_empty() {
        return None;
    }
    let index = rolls.rng(0, i32::try_from(suitable.len()).unwrap_or(1) - 1);
    usize::try_from(index)
        .ok()
        .and_then(|index| suitable.get(index).copied())
}

/// The whole spawn, in the reference's order: gate, group, square.
///
/// Reference: `field_processor_monster_spawn` (`map_field.cpp:762-786`). Whether anything spawns is decided **first**, so a
/// field that does not trigger spends exactly one die; only if it does are monsters picked, and only then placed. Composing the
/// three pieces here rather than at every call site is what keeps that order - a caller that picked the monster before rolling
/// the gate would produce a plausible world on different dice.
///
/// Returns each monster with the square it landed on. A monster that finds nowhere to stand is **not** produced, which is what
/// the reference does with an empty `random_point`.
#[allow(clippy::too_many_arguments)]
pub fn spawn_monsters_from_field<'a>(
    spawn_chance: i32,
    spawn_count: i32,
    spawn_group: &str,
    center: (i32, i32),
    radius: i32,
    groups: &'a [crate::monster_groups::MonsterGroup],
    accepts: impl Fn(i32, i32) -> bool,
    rolls: &mut Rolls,
) -> Vec<(&'a str, (i32, i32))> {
    let monsters = spawn_from_field(spawn_chance, spawn_count, spawn_group, groups, rolls);
    let mut placed = Vec::new();
    for monster in monsters {
        match random_point_in_radius(center, radius, &accepts, rolls) {
            Some(square) => placed.push((monster, square)),
            None => break,
        }
    }
    placed
}

/// The same spawn, asking a **level** what is on each square instead of a caller-supplied predicate.
///
/// The terrain table is passed as a function rather than a map so this stays a rule about spawning and not a second copy of the
/// loader: the caller says which terrain ids take a creature, and the level says which square holds which id.
#[allow(clippy::too_many_arguments)]
pub fn spawn_monsters_on_level<'a>(
    level: &cdda_core_types::level::Level,
    takes_creature: impl Fn(u32) -> bool,
    spawn_chance: i32,
    spawn_count: i32,
    spawn_group: &str,
    center: (i32, i32),
    radius: i32,
    groups: &'a [crate::monster_groups::MonsterGroup],
    rolls: &mut Rolls,
) -> Vec<(&'a str, (i32, i32))> {
    spawn_monsters_from_field(
        spawn_chance,
        spawn_count,
        spawn_group,
        center,
        radius,
        groups,
        |x, y| level.terrain_at(x, y).is_some_and(&takes_creature),
        rolls,
    )
}

/// What one intensity level of a field does on its turn, as far as the port models it yet.
///
/// Only the two processors with rules the port can already run are here: monster spawning, and the intensity upgrade that
/// competes with it for the same turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldTurn {
    /// The chance in a hundred that this level spawns a group.
    pub spawn_chance: i32,
    /// How many spawns to attempt.
    pub spawn_count: i32,
    /// The group to draw from.
    pub spawn_group: String,
    /// How far from the field the spawns appear.
    pub spawn_radius: i32,
    /// The chance in a hundred that the level upgrades instead.
    pub upgrade_chance: i32,
    /// How often the upgrade is rolled, in turns; zero disables it.
    pub upgrade_duration: i64,
}

/// One turn of one field: the three steps, in the reference's order.
///
/// Reference: `map::process_fields_in_submap` (`map_field.cpp:470-499`).
///
/// 1. A field with no intensity is **gone** - it is removed before anything runs.
/// 2. A field **created this turn is not processed**: it decays and nothing else, which is what gives a player standing in
///    freshly-lit fire a turn to step out. That is `if( cur.get_field_age() == 0_turns )`.
/// 3. Otherwise every processor the type registered runs, spawning among them, and the decay comes **last** - so a field that
///    spawns this turn is still at full strength while it does.
///
/// Returns the monsters the field spawned, with the square each landed on.
#[allow(clippy::too_many_arguments)]
pub fn run_field_turn<'a>(
    field: &mut cdda_core_types::field::FieldEntry,
    turn: &FieldTurn,
    center: (i32, i32),
    level: &cdda_core_types::level::Level,
    takes_creature: impl Fn(u32) -> bool,
    groups: &'a [crate::monster_groups::MonsterGroup],
    rolls: &mut Rolls,
    now: i64,
) -> Vec<(&'a str, (i32, i32))> {
    if !field.is_alive() {
        return Vec::new();
    }
    if field.age() == 0 {
        field.do_decay(now, rolls);
        return Vec::new();
    }
    let mut spawned = Vec::new();
    if turn.upgrade_chance > 0 && turn.upgrade_duration > 0 {
        // `calendar::once_every( duration ) && one_in( chance )`: the calendar gate is the caller's, the roll is ours.
        if now % turn.upgrade_duration == 0 && rolls.one_in(turn.upgrade_chance) {
            field.set_intensity(field.intensity() + 1);
        }
    }
    if turn.spawn_chance > 0 && turn.spawn_count > 0 {
        spawned = spawn_monsters_on_level(
            level,
            takes_creature,
            turn.spawn_chance,
            turn.spawn_count,
            &turn.spawn_group,
            center,
            turn.spawn_radius,
            groups,
            rolls,
        );
    }
    field.do_decay(now, rolls);
    spawned
}

/// One turn of every field on the level, in the reference's sweep order.
///
/// Reference: `map::process_fields_in_submap` (`map_field.cpp:445-499`) loops `locx` **outside** and `locy` inside, so the map is
/// visited column by column. `Level::field_squares` hands the squares back in exactly that order, so the sweep never sorts and the
/// dice are spent in the reference's sequence - the same trap that cost this port six rounds in map generation.
///
/// Each field is taken off the level, given its turn, and put back **only if it survived**, which is how the reference's removal of
/// a decayed field falls out of the same pass. The visitor is called once per processed square, which lets a test state the order
/// rather than infer it.
#[allow(clippy::too_many_arguments)]
pub fn run_level_field_turns<'a>(
    level: &mut cdda_core_types::level::Level,
    turn: &FieldTurn,
    takes_creature: impl Fn(u32) -> bool + Copy,
    groups: &'a [crate::monster_groups::MonsterGroup],
    rolls: &mut Rolls,
    now: i64,
    mut visited: impl FnMut((i32, i32)),
) -> Vec<(&'a str, (i32, i32))> {
    let mut spawned = Vec::new();
    for square in level.field_squares() {
        let Some(mut field) = level.take_field(square.0, square.1) else {
            continue;
        };
        visited(square);
        spawned.extend(run_field_turn(
            &mut field,
            turn,
            square,
            level,
            takes_creature,
            groups,
            rolls,
            now,
        ));
        if field.is_alive() {
            level.add_field(square.0, square.1, field);
        }
    }
    spawned
}
