//! What the data says about one square: its terrain, the furniture standing on it, and the two questions
//! the movement rules ask of both.

use bevy_ecs::prelude::*;
use cdda_content::mapdata::MapData;
use std::collections::BTreeMap;

/// Every terrain and furniture prototype the data defines, by id.
///
/// Reference: `map_data_common_t` and its subclasses (`src/mapdata.h`) are a registry the reference reaches
/// through globals (`ter_id::obj()`, `furn_str_id::obj()`). The port keeps the same records on one entity, so
/// a rule asks the store instead of a global - and the answers are the loader's, not a second copy.
#[derive(Component, Debug, Clone, Default)]
pub struct MapDataTable(pub BTreeMap<String, MapData>);

/// Marks the entity carrying [`MapDataTable`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheMapData;

/// Installs the map data every square rule reads.
pub fn set_data(world: &mut World, entries: impl IntoIterator<Item = MapData>) {
    let table = MapDataTable(
        entries
            .into_iter()
            .map(|data| (data.id.clone(), data))
            .collect(),
    );
    let entity = crate::singletons::singleton::<MapDataTable, TheMapData>(world);
    world.entity_mut(entity).insert(table);
}

/// The whole table, cloned, for a rule that reads several entries.
#[must_use]
pub fn table(world: &World) -> Option<MapDataTable> {
    let holder = crate::singletons::holder::<TheMapData>(world)?;
    world.get::<MapDataTable>(holder).cloned()
}

/// The terrain under a square and the furniture on it, as the data describes them.
///
/// Reference: a square's rules are `ter( p ).obj()` and `furn( p ).obj()` (`src/map.cpp`), with `f_null`
/// standing for no furniture at all. `None` means the world has no map data, which is a setup fault rather
/// than a rule.
#[must_use]
pub fn at(world: &World, square: (i32, i32, i32)) -> Option<SquareData> {
    let table = table(world)?;
    let terrain_name =
        crate::terrain_types::name_of(world, crate::square::terrain_at(world, square)?)?;
    let terrain = table.0.get(&terrain_name)?.clone();
    let furniture = crate::square::furniture_at(world, square)
        .and_then(|code| crate::furniture_types::name_of(world, code))
        .filter(|name| name != "f_null")
        .and_then(|name| table.0.get(&name).cloned());
    Some(SquareData { terrain, furniture })
}

/// One square's terrain and furniture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SquareData {
    /// The ground.
    pub terrain: MapData,
    /// What stands on it, if anything.
    pub furniture: Option<MapData>,
}

impl SquareData {
    /// Whether either half carries a flag.
    ///
    /// Reference: `map::has_flag_ter_or_furn` (`src/map.cpp:2005-2026`) asks the furniture first and the terrain
    /// second, and answers true if either has it.
    #[must_use]
    pub fn has_flag(&self, flag: &str) -> bool {
        self.terrain.flags.contains(flag)
            || self
                .furniture
                .as_ref()
                .is_some_and(|furniture| furniture.flags.contains(flag))
    }

    /// What the square costs to walk through, in the reference's units.
    ///
    /// Reference: `map::move_cost_ter_furn` (`src/map.cpp:2429-2452`): a terrain whose cost is zero makes the square
    /// impassable whatever stands on it, furniture with a **negative** cost does the same, and otherwise the two costs
    /// add. `2` is a plain floor, which is why the collision rule tests for it by name.
    #[must_use]
    pub fn move_cost(&self) -> i32 {
        if self.terrain.move_cost == 0 {
            return 0;
        }
        if self
            .furniture
            .as_ref()
            .is_some_and(|furniture| furniture.move_cost < 0)
        {
            return 0;
        }
        let mut cost = self.terrain.move_cost.max(0);
        if let Some(furniture) = &self.furniture {
            cost += furniture.move_cost.max(0);
        }
        cost
    }

    /// Whether nothing can walk through the square.
    ///
    /// Reference: `map::impassable_ter_furn` (`src/map.cpp:2456-2459`) is `!passable_ter_furn( p )`, and `passable_ter_furn` is
    /// `move_cost_ter_furn( p ) != 0` (`src/map.cpp:2461-2464`). There is **no `PASSABLE` flag in play** - a wall is impassable because its
    /// move cost is zero, which is why this asks the cost and not the flags.
    #[must_use]
    pub fn impassable(&self) -> bool {
        self.move_cost() == 0
    }

    /// Whether a hit could break something here, and it is not a plain floor.
    ///
    /// Reference: `map::is_bashable_ter_furn` (`src/map.cpp:3175-3189`) - furniture with a bash block, or terrain with
    /// one that is **not** `bash_below` (a floor has a bash block so that a fall through it can break it, and it is not
    /// something a vehicle hits).
    #[must_use]
    pub fn bashable(&self) -> bool {
        let furniture_bashable = self
            .furniture
            .as_ref()
            .is_some_and(|furniture| furniture.bash.is_some());
        let terrain_bashable = self
            .terrain
            .bash
            .as_ref()
            .is_some_and(|bash| !bash.bash_below);
        furniture_bashable || terrain_bashable
    }
}
