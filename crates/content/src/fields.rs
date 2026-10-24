//! Field prototypes: what each kind of field looks like at each intensity.
//!
//! Reference: `field_type` (`src/field_type.h`), reached through `--dump-fields`. A field's
//! number of levels is also its maximum intensity, which is what the runtime clamps to - so this
//! loader is what makes "intensity 3" mean something.

use cdda_core_types::rng::Rolls;
use cdda_json::{Object, Value};

/// One intensity of a field.
#[derive(Debug, Clone, PartialEq)]
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
    /// How much this intensity absorbs from a ray that gets through it.
    ///
    /// Reference: `field_intensity_level::translucency` (`src/field.h`), read by `calc_transp` (`lightmap.cpp:158-165`), which **multiplies** it into the square's value -
    /// a larger number absorbs more, and zero makes the square solid. The two flags are not redundant: smoke at intensity two is marked opaque yet has a translucency of
    /// ten, so it dims rather than blocks.
    pub translucency: f32,
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
    /// The least and most radiation this intensity adds to its square each processing turn.
    ///
    /// Reference: `field_intensity_level::extra_radiation_min` and `_max` (`src/field.h`), read by `field_processor_extra_radiation`
    /// (`src/map_field.cpp:664-673`): the processor rolls between them and adjusts the square's radiation. `fd_nuke_gas` is the only field in the data that radiates,
    /// and it does so at its three higher intensities - 0 to 1, 0 to 2 and 0 to 3.
    pub extra_radiation_min: i32,
    /// See [`FieldLevel::extra_radiation_min`].
    pub extra_radiation_max: i32,
    /// How far from the field they appear.
    pub spawn_radius: i32,
    /// The monster group to draw from, empty when it spawns nothing.
    pub spawn_group: String,
}

/// The turn one intensity level takes, read from the level's own data.
///
/// Reference: `field_type` registers its processors at load time (`field_type.cpp:350`), and each processor reads the numbers off
/// the level it runs at. Building the turn here rather than at the spawner keeps one source for those numbers - a rule handed its
/// constants by hand drifts while every test stays green.
impl FieldLevel {
    /// This level's turn.
    #[must_use]
    pub fn turn(&self) -> FieldTurn {
        FieldTurn {
            spawn_chance: self.spawn_chance,
            spawn_count: self.spawn_count,
            spawn_group: self.spawn_group.clone(),
            spawn_radius: self.spawn_radius,
            upgrade_chance: self.upgrade_chance,
            upgrade_duration: self.upgrade_duration,
        }
    }
}

/// The turn a field takes at a given intensity.
///
/// Reference: every processor asks `cur.get_intensity_level()` first (`map_field.cpp:764`), so an intensity-three field is **not**
/// the intensity-one field - the data gives each level its own chance, count and group, and using one level's numbers for all of
/// them spawns at the wrong rate on every field with more than one level.
///
/// Intensities start at one; a level the data does not have hands over nothing, which is the same refusal the reference makes
/// when intensity is capped at the type's maximum.
#[must_use]
pub fn turn_at(field: &FieldType, intensity: i32) -> Option<FieldTurn> {
    if intensity < 1 {
        return None;
    }
    let index = usize::try_from(intensity - 1).ok()?;
    field.levels.get(index).map(FieldLevel::turn)
}

/// One kind of field, with its levels in order (index 0 is intensity 1).
#[derive(Debug, Clone, PartialEq)]
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
    /// How long it takes this field to lose an intensity, in turns; zero means it never does on its own.
    ///
    /// Reference: `field_type::half_life` (`src/field_type.h:217`). This is the clock every field's decay is arithmetic on - a level is lost when the calendar passes
    /// the deadline the half-life sets - so a port that guessed it kept a mist alive for a million turns that the reference clears in ten.
    pub half_life: i64,
    /// Whether the half-life doubles with each intensity instead of staying fixed (`linear_half_life`, `src/field_type.h:223`).
    pub linear_half_life: bool,
    /// The field this one wanders into around itself, or `fd_null` when it wanders into nothing.
    ///
    /// Reference: `field_type::wandering_field` (`src/field_type.h`), read by `field_processor_wandering_field` (`src/map_field.cpp:675-690`). Three vents in the data
    /// wander - a gas vent into toxic gas, a smoke vent into smoke and the tindalos rift into its own gas - which is what makes a vent keep producing rather than
    /// produce once.
    pub wandering_field: String,
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

fn float_of(object: &Object, key: &str) -> Option<f64> {
    #[allow(clippy::cast_precision_loss)]
    match object.get(key)? {
        Value::Int(value) => Some(*value as f64),
        Value::Float(value) => Some(*value),
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
        let half_life = int_of(object, "half_life").unwrap_or(0);
        let linear_half_life = bool_of(object, "linear_half_life").unwrap_or(false);
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
                translucency: float_of(level, "translucency").map_or(0.0, |value| {
                    #[allow(clippy::cast_possible_truncation)]
                    let answer = value as f32;
                    answer
                }),
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
                extra_radiation_min: i32::try_from(int_of(level, "extra_radiation_min").unwrap_or(0))
                    .unwrap_or(0),
                extra_radiation_max: i32::try_from(int_of(level, "extra_radiation_max").unwrap_or(0))
                    .unwrap_or(0),
            });
        }
        fields.push(FieldType {
            id,
            priority: i32::try_from(priority).unwrap_or(0),
            underwater_age_speedup,
            outdoor_age_speedup,
            percent_spread: i32::try_from(percent_spread).unwrap_or(0),
            half_life,
            linear_half_life,
            wandering_field: str_of(object, "wandering_field").unwrap_or_default(),
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

pub use cdda_core_types::field::FieldTurn;
