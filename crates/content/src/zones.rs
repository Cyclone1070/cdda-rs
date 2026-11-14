//! Zone types: the catalogue of areas a character can mark out.
//!
//! Reference: `zone_type` (`src/clzones.h:40-72`) and its loader (`src/clzones.cpp:157-166`) — an id, a translated name and description, the **field** the map draws the
//! zone with, whether a character may own one, and whether the interface hides it.
//!
//! The one thing about this registry that does not read the way it looks is the JSON tag: the data writes zone types with `"type": "LOOT_ZONE"` (`src/init.cpp:370`), not
//! `"zone_type"`, so a loader that looks for its own name finds nothing and a gate that counts what it found is happy with zero.

use cdda_json::{Object, Value};
use std::collections::{BTreeMap, BTreeSet};

/// One zone type the data defines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneType {
    /// Its id, such as `LOOT_FOOD`.
    pub id: String,
    /// The name a player sees.
    pub name: String,
    /// What the zone is for, as one sentence.
    pub description: String,
    /// The field the map draws the zone with, such as `fd_food_loot_zone`.
    pub display_field: String,
    /// Whether a character may own the zone, as opposed to an NPC faction or the camp.
    pub can_be_personal: bool,
    /// Whether the interface leaves the zone out of its list.
    pub hidden: bool,
}

/// Why the zone types could not be loaded.
#[derive(Debug)]
pub struct LoadError {
    /// The problem.
    pub message: String,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LoadError {}

/// Loads every zone type, in the reference's file order.
///
/// # Errors
///
/// Fails when a directory cannot be walked. A zone type defined twice is **not** an error: the reference's factory lets a mod redefine one, and the last definition wins,
/// which is what the data relies on for the zones it overrides.
pub fn load_paths(dirs: &[&std::path::Path]) -> Result<BTreeMap<String, ZoneType>, LoadError> {
    let mut out: BTreeMap<String, ZoneType> = BTreeMap::new();
    for dir in dirs {
        let files = crate::json_files_in_load_order(dir).map_err(|error| LoadError {
            message: format!("{}: {error}", dir.display()),
        })?;
        for path in files {
            let Some(value) = crate::read_json(&path) else {
                continue;
            };
            collect(&value, &mut out);
        }
    }
    Ok(out)
}

fn collect(value: &Value, out: &mut BTreeMap<String, ZoneType>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect(item, out);
            }
        }
        Value::Object(object) => {
            if !matches!(object.get("type"), Some(Value::Str(text)) if text == "LOOT_ZONE") {
                return;
            }
            let Some(Value::Str(id)) = object.get("id") else {
                return;
            };
            let zone = ZoneType {
                id: id.clone(),
                name: name_of(object, "name").unwrap_or_else(|| id.clone()),
                description: name_of(object, "description").unwrap_or_default(),
                display_field: match object.get("display_field") {
                    Some(Value::Str(text)) => text.clone(),
                    _ => String::new(),
                },
                can_be_personal: matches!(object.get("can_be_personal"), Some(Value::Bool(true))),
                hidden: matches!(object.get("hidden"), Some(Value::Bool(true))),
            };
            if let Some(previous) = out.get_mut(id) {
                *previous = zone;
            } else {
                out.insert(id.clone(), zone);
            }
        }
        _ => {}
    }
}

/// A translated string, which the data writes either plainly or as a `{"str": ...}` object.
///
/// The plural forms (`str_sp`, `str_pl`) are not read: a zone's name and description never use them in this data set, and a loader that guessed would put the wrong
/// form in front of a player.
fn name_of(object: &Object, key: &str) -> Option<String> {
    match object.get(key) {
        Some(Value::Str(text)) => Some(text.clone()),
        Some(Value::Object(inner)) => match inner.get("str") {
            Some(Value::Str(text)) => Some(text.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// One zone a character has marked out: a rectangle, a type, and the faction that owns it.
///
/// Reference: `zone_data` (`src/clzones.h:347-542`). The rectangle's two corners are **inclusive** on every axis, and the corners may be given in either order — the reference
/// builds an `inclusive_cuboid` from the pair rather than from a point and a size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneData {
    /// The name the player gave the zone.
    pub name: String,
    /// Its type's id, such as `LOOT_FOOD`.
    pub zone_type: String,
    /// The faction that owns it. A query names the faction it is asking for, and a zone of another faction is simply not there.
    pub faction: String,
    /// What the player asked for: everything **but** the rectangle.
    ///
    /// **This does not change containment.** `has_inside` is the rectangle whether the flag is set or not; the inversion is applied by whoever reads the zones, and a port
    /// that folded it into the rectangle would answer "outside" where the reference answers "inside" and its complement.
    pub invert: bool,
    /// Whether the player has the zone switched on. The manager's lookups **do not read it** — a switched-off zone still contains its squares; the flag stops actions from
    /// using it.
    pub enabled: bool,
    /// One corner, as an absolute square.
    pub start: (i32, i32, i32),
    /// The other corner, inclusive.
    pub end: (i32, i32, i32),
}

impl ZoneData {
    /// Whether the square is inside the zone's rectangle.
    ///
    /// Reference: `zone_data::has_inside` (`src/clzones.h:533-542`), which asks `inclusive_cuboid( start, end ).contains( p )` — so both corners and every square between
    /// them are inside, and the corners are the same set whichever way round the player drew them.
    #[must_use]
    pub fn has_inside(&self, at: (i32, i32, i32)) -> bool {
        let (sx, ex) = ordered(self.start.0, self.end.0);
        let (sy, ey) = ordered(self.start.1, self.end.1);
        let (sz, ez) = ordered(self.start.2, self.end.2);
        (sx..=ex).contains(&at.0) && (sy..=ey).contains(&at.1) && (sz..=ez).contains(&at.2)
    }
}

/// The pair of bounds a rectangle spans, whichever way round the corners were given.
fn ordered(a: i32, b: i32) -> (i32, i32) {
    (a.min(b), a.max(b))
}

/// The zones a character has marked out, in the order they were drawn.
///
/// Reference: `zone_manager` (`src/clzones.h:545-700`). The list's **order** is load-bearing and the reference reads it from both ends:
///
/// - [`ZoneManager::zone_at`] walks it forwards, so the answer is the **first** zone drawn that contains the square;
/// - [`ZoneManager::bottom_zone`] walks it backwards, so the answer is the **last** one drawn;
/// - [`ZoneManager::loot_zone_at`] also walks backwards, and only considers types whose id starts with `LOOT`.
///
/// The three are not interchangeable, and the fixture in `corpus/zones/probe.json` overlaps its zones precisely so that a port which conflated them is caught.
#[derive(Debug, Clone, Default)]
pub struct ZoneManager {
    zones: Vec<ZoneData>,
}

impl ZoneManager {
    /// An empty manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every zone, in the order they were drawn, whichever faction owns them.
    #[must_use]
    pub fn zones(&self) -> &[ZoneData] {
        &self.zones
    }

    /// Marks out a zone. It goes on the end of the list, which is what makes the order the queries depend on.
    pub fn add(&mut self, zone: ZoneData) {
        self.zones.push(zone);
    }

    /// Forgets every zone.
    pub fn clear(&mut self) {
        self.zones.clear();
    }

    /// The zones of one type that contain the square, in the order they were drawn.
    ///
    /// Reference: `zone_manager::get_zones_at` (`src/clzones.cpp:1123-1140`). It iterates **forwards**, which is why its first answer is the oldest zone rather than the
    /// newest — a player who draws a second food zone over the first keeps being answered by the first.
    #[must_use]
    pub fn zones_at(&self, at: (i32, i32, i32), zone_type: &str, faction: &str) -> Vec<&ZoneData> {
        self.zones
            .iter()
            .filter(|zone| {
                zone.faction == faction && zone.zone_type == zone_type && zone.has_inside(at)
            })
            .collect()
    }

    /// The **first** zone of one type that contains the square.
    ///
    /// Reference: `zone_manager::get_zone_at` (`src/clzones.cpp:1112-1121`), which is `get_zones_at(...).front()`.
    #[must_use]
    pub fn zone_at(
        &self,
        at: (i32, i32, i32),
        zone_type: &str,
        faction: &str,
    ) -> Option<&ZoneData> {
        self.zones_at(at, zone_type, faction).into_iter().next()
    }

    /// The **last** zone of any type that contains the square.
    ///
    /// Reference: `zone_manager::get_bottom_zone` (`src/clzones.cpp:1380-1396`), which walks the list from its end. This is the zone the interface draws on top, and the one
    /// the game answers with when the question is "what am I standing in".
    #[must_use]
    pub fn bottom_zone(&self, at: (i32, i32, i32), faction: &str) -> Option<&ZoneData> {
        self.zones
            .iter()
            .rev()
            .find(|zone| zone.faction == faction && zone.has_inside(at))
    }

    /// The **last** loot zone that contains the square.
    ///
    /// Reference: `zone_manager::get_zone_at( where, loot_only, faction )` (`src/clzones.cpp:1356-1378`). "Loot" is decided by the **prefix of the type's id**, not by a flag:
    /// `z.get_type().str().substr( 0, 4 ) == "LOOT"`. So a zone of a future type called `LOOTAGE` would count, and one called `AUTO_EAT` would not.
    #[must_use]
    pub fn loot_zone_at(&self, at: (i32, i32, i32), faction: &str) -> Option<&ZoneData> {
        self.zones.iter().rev().find(|zone| {
            zone.faction == faction && zone.zone_type.starts_with("LOOT") && zone.has_inside(at)
        })
    }
}

impl ZoneManager {
    /// Every square a type of zone covers, for one faction — the set the cached questions are asked of.
    ///
    /// Reference: `zone_manager::cache_data` (`src/clzones.cpp:892-917`), which walks every zone and inserts each square of its rectangle under `type + faction`.
    ///
    /// **It skips a zone that is switched off**, and that is the one place in the manager where `enabled` matters. The lookups that walk the zone list
    /// ([`ZoneManager::zones_at`], [`ZoneManager::bottom_zone`]) return a switched-off zone; the ones that read this set do not.
    ///
    /// The reference builds this once per mutation and can answer from a set that is a turn out of date; the port builds it on demand, so its answer is always the current
    /// one — which is what the reference's next `cache_data` produces.
    #[must_use]
    pub fn points_of(&self, zone_type: &str, faction: &str) -> BTreeSet<(i32, i32, i32)> {
        let mut points = BTreeSet::new();
        for zone in &self.zones {
            if !zone.enabled || zone.faction != faction || zone.zone_type != zone_type {
                continue;
            }
            let (sx, ex) = ordered(zone.start.0, zone.end.0);
            let (sy, ey) = ordered(zone.start.1, zone.end.1);
            let (sz, ez) = ordered(zone.start.2, zone.end.2);
            for z in sz..=ez {
                for y in sy..=ey {
                    for x in sx..=ex {
                        points.insert((x, y, z));
                    }
                }
            }
        }
        points
    }

    /// Whether any zone of a type covers the square, counting only zones that are switched on.
    ///
    /// Reference: `zone_manager::has` (`src/clzones.cpp:1041-1047`), which asks the cached set. Note that this is **not** [`ZoneManager::zones_at`] being empty: a
    /// switched-off zone contains the square and is not in the set.
    #[must_use]
    pub fn has(&self, at: (i32, i32, i32), zone_type: &str, faction: &str) -> bool {
        self.points_of(zone_type, faction).contains(&at)
    }

    /// Whether any zone of a type covers a square within `range` of this one, counting only zones that are switched on.
    ///
    /// Reference: `zone_manager::has_near` (`src/clzones.cpp:1049-1063`), which measures `square_dist` — the **larger** of the two axis distances, so a range of one covers
    /// the eight neighbours and a diagonal is as far as a straight line. `square_dist` on three-dimensional points ignores the level, so a zone on the floor above counts;
    /// the fixture is flat and does not distinguish that.
    #[must_use]
    pub fn has_near(
        &self,
        at: (i32, i32, i32),
        range: i32,
        zone_type: &str,
        faction: &str,
    ) -> bool {
        self.points_of(zone_type, faction)
            .iter()
            .any(|point| square_dist(*point, at) <= range)
    }
}

/// The larger of the two axis distances, which is how the reference measures "near".
///
/// Reference: `square_dist( const tripoint &, const tripoint & )` (`src/line.cpp`), which takes the maximum of the x and y differences and **ignores z**.
fn square_dist(a: (i32, i32, i32), b: (i32, i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}
