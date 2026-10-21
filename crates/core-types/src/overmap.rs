//! The overmap: the level above the map, one tile per 24 squares.
//!
//! Reference: `overmap` (`src/overmap.h`), a square grid `OMAPX` tiles on a side (`map_scale_constants.h`). The port
//! keeps the grid and its contents and nothing else yet: terrain placement, cities, roads and specials each arrive
//! as the rules that decide them are ported.
//!
//! A tile is addressed the way the reference addresses it - by its own coordinates, with the origin at a corner and
//! no wrapping - because everything above this layer (mapgen lookup, submap addressing) converts through these
//! numbers, and a wrap that exists nowhere in the reference would be a source of quiet wrongness.

/// The tile at a position, as an index into the flat list.
///
/// Only called for positions already known to be inside the grid; the saturating arithmetic keeps a mistake from
/// becoming a wrapped index rather than a wrong tile.
fn index_of(x: i32, y: i32, width: i32) -> usize {
    let at = y.saturating_mul(width).saturating_add(x).max(0);
    usize::try_from(at).unwrap_or(0)
}

/// How many tiles an overmap is on a side.
///
/// Reference: `OMAPX` in `map_scale_constants.h`, with the reference asserting that the grid is square.
pub const OMAPX: i32 = 180;

/// What a square of the overmap holds.
///
/// The port stores the terrain as a number and keeps the names in a table beside it, for the same reason items are
/// numbered: the world is asked about a tile far more often than it is asked what a tile is called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overmap {
    width: i32,
    height: i32,
    tiles: Vec<u32>,
    names: Vec<String>,
    by_name: std::collections::BTreeMap<String, u32>,
}

impl Overmap {
    /// A new overmap filled with one terrain, which the reference fills with `field`.
    #[must_use]
    pub fn filled(terrain: &str) -> Self {
        let width = OMAPX;
        let height = OMAPX;
        let mut overmap = Self {
            width,
            height,
            tiles: Vec::new(),
            names: Vec::new(),
            by_name: std::collections::BTreeMap::new(),
        };
        let id = overmap.intern(terrain);
        let tiles = usize::try_from(width.saturating_mul(height).max(0)).unwrap_or(0);
        overmap.tiles = vec![id; tiles];
        overmap
    }

    /// The number for a terrain name, adding it to the table when it is new.
    fn intern(&mut self, name: &str) -> u32 {
        if let Some(id) = self.by_name.get(name) {
            return *id;
        }
        #[allow(clippy::cast_possible_truncation)]
        let id = self.names.len() as u32;
        self.names.push(name.to_owned());
        self.by_name.insert(name.to_owned(), id);
        id
    }

    /// How many tiles across.
    #[must_use]
    pub fn width(&self) -> i32 {
        self.width
    }

    /// How many tiles down.
    #[must_use]
    pub fn height(&self) -> i32 {
        self.height
    }

    /// Whether a tile is inside the grid.
    #[must_use]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    /// The terrain on a tile, if the tile is inside the grid.
    #[must_use]
    pub fn terrain_id(&self, x: i32, y: i32) -> Option<u32> {
        if !self.contains(x, y) {
            return None;
        }
        self.tiles.get(index_of(x, y, self.width)).copied()
    }

    /// The terrain's name on a tile.
    #[must_use]
    pub fn terrain(&self, x: i32, y: i32) -> Option<&str> {
        let id = self.terrain_id(x, y)?;
        self.names.get(id as usize).map(String::as_str)
    }

    /// Puts a terrain on a tile, naming it if the table has never seen it.
    pub fn set_terrain(&mut self, x: i32, y: i32, terrain: &str) {
        if !self.contains(x, y) {
            return;
        }
        let id = self.intern(terrain);
        let at = index_of(x, y, self.width);
        if let Some(slot) = self.tiles.get_mut(at) {
            *slot = id;
        }
    }

    /// How many tiles hold a given terrain.
    #[must_use]
    pub fn count(&self, terrain: &str) -> usize {
        let Some(id) = self.by_name.get(terrain).copied() else {
            return 0;
        };
        self.tiles.iter().filter(|tile| **tile == id).count()
    }
}

/// The world settings that decide how many cities an overmap holds.
///
/// Reference: the `CITY_SIZE`, `CITY_SPACING`, `OVERMAP_MAXIMUM_URBANITY` and forestosity settings
/// (`options.cpp`, whose defaults are 8, 4 and 0). Kept as a plain struct so a test can state a world instead of
/// reaching for global options.
#[derive(Debug, Clone, Copy)]
pub struct CitySettings {
    /// `CITY_SIZE`: how big a city is, where zero means no cities at all.
    pub size: i32,
    /// `CITY_SPACING`: how far apart they are kept.
    pub spacing: i32,
    /// How built up the world is, which raises the largest city.
    pub urbanity: i32,
    /// How wooded the world is, which lowers it.
    pub forestosity: i32,
    /// The ceiling on how far urbanity may grow a city.
    pub maximum_urbanity: i32,
}

impl Default for CitySettings {
    fn default() -> Self {
        // The reference's own defaults.
        Self {
            size: 8,
            spacing: 4,
            urbanity: 0,
            forestosity: 0,
            maximum_urbanity: 0,
        }
    }
}

/// The largest a city may grow under these settings.
///
/// Reference: `place_cities` (`overmap.cpp`). The adjustment may not push the size below two, and the result may
/// not fall below `CITY_SIZE` - the reference says in a comment that "funny things happen" otherwise, which is the
/// kind of note worth keeping rather than rediscovering.
#[must_use]
pub fn max_city_size(settings: CitySettings) -> i32 {
    let size_adjust = settings
        .urbanity
        .saturating_sub(settings.forestosity / 2)
        .min(-settings.size + 2);
    let grown = settings
        .size
        .saturating_add(size_adjust)
        .min(settings.size.saturating_mul(settings.maximum_urbanity));
    grown.max(settings.size)
}

/// How much of the overmap cities cover, from the spacing.
///
/// Reference: `1.0 / 2^spacing` - each step of spacing halves the share of the map that is urban.
#[must_use]
pub fn city_coverage_ratio(spacing: i32) -> f64 {
    1.0 / 2.0_f64.powi(spacing.clamp(0, 30))
}

/// How many tiles one city and its surroundings take up.
#[must_use]
pub fn tiles_per_city(size: i32, largest: i32) -> f64 {
    let across = f64::from(size * 2 + 1);
    let down = f64::from(largest * 2 + 1);
    across * down * 3.0 / 4.0
}

/// How many cities belong on an overmap, before the remainder roll.
///
/// Reference: `omts_per_overmap * city_map_coverage_ratio / omts_per_city`, with the fraction then decided by
/// `roll_remainder`. `draw` is that decision, a uniform value in `[0, 1)`, supplied by the caller for the same
/// reason every other rule takes its dice from the caller.
#[must_use]
pub fn cities_for(settings: CitySettings, draw: f64) -> i32 {
    if settings.size <= 0 {
        return 0;
    }
    let largest = max_city_size(settings);
    let coverage = city_coverage_ratio(settings.spacing);
    let per_city = tiles_per_city(settings.size, largest);
    if per_city <= 0.0 {
        return 0;
    }
    let exact = f64::from(OMAPX * OMAPX) * coverage / per_city;
    let whole = exact.floor();
    let fraction = exact - whole;
    let count = if fraction > 0.0 && draw < fraction {
        whole + 1.0
    } else {
        whole
    };
    // The count is a whole number by construction; the clamp keeps a wild setting from wrapping it.
    let bounded = count.clamp(0.0, f64::from(i32::MAX));
    #[allow(clippy::cast_possible_truncation)]
    let cities = bounded as i32;
    cities
}
