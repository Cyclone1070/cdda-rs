//! The overmap: the level above the map, one tile per 24 squares.
//!
//! Reference: `overmap` (`src/overmap.h`), a square grid `OMAPX` tiles on a side (`map_scale_constants.h`). The port
//! keeps the grid and its contents and nothing else yet: terrain placement, cities, roads and specials each arrive
//! as the rules that decide them are ported.
//!
//! A tile is addressed the way the reference addresses it - by its own coordinates, with the origin at a corner and
//! no wrapping - because everything above this layer (mapgen lookup, submap addressing) converts through these
//! numbers, and a wrap that exists nowhere in the reference would be a source of quiet wrongness.

use crate::rng::Rolls;

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

/// A city on the overmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct City {
    /// Where its crossroads are.
    pub x: i32,
    /// Where its crossroads are.
    pub y: i32,
    /// How far it reaches.
    pub size: i32,
}

/// How big a city turns out to be.
///
/// Reference: `place_cities` (`overmap.cpp`). The order of the rolls is the rule: first a size between one below the
/// setting and the largest allowed, then **one roll that decides which of four shapes it is** - a third tiny (a
/// third of the size), a third small (two thirds), then a half of what is left large (one and a half), and the rest
/// huge (double). The result is clamped to at least two, because a city of one is a crossroads with no buildings.
pub fn roll_city_size(rolls: &mut Rolls, settings: CitySettings) -> i32 {
    let largest = max_city_size(settings);
    let mut size = rolls.rng(settings.size - 1, largest);
    // The branch decides a fraction, and the fractions are applied in the order the reference rolls them.
    let shape = if rolls.rng(1, 3) == 1 {
        (1, 3) // tiny
    } else if rolls.rng(1, 2) == 1 {
        (2, 3) // small
    } else {
        // The third roll happens only when the first two said no, which is the reference's own order.
        if rolls.rng(1, 2) == 1 {
            (3, 2) // large
        } else {
            (2, 1) // huge
        }
    };
    size = size * shape.0 / shape.1;
    size.clamp(2, 55)
}

/// Where a city's crossroads land.
///
/// Reference: `rng( size - 1, OMAPX - size )` on each axis, which keeps a city from being drawn across the edge of
/// the map where it would be clipped.
#[must_use]
pub fn roll_city_position(rolls: &mut Rolls, size: i32, width: i32, height: i32) -> (i32, i32) {
    let x = rolls.rng(size - 1, width - size);
    let y = rolls.rng(size - 1, height - size);
    (x, y)
}

/// Places cities on an overmap, up to the number the settings ask for.
///
/// Reference: the placement loop in `place_cities`. A candidate is only taken if its square is still the default
/// terrain, which is how the reference avoids stacking cities; it gives up after fifty attempts, and each success
/// resets the count - so the loop is bounded by failures rather than by cities. The city's crossroads become a road
/// intersection, which is what the rest of generation looks for when it draws streets outward.
///
/// Returns the cities that were placed.
pub fn place_cities(
    overmap: &mut Overmap,
    settings: CitySettings,
    rolls: &mut Rolls,
    wanted: i32,
) -> Vec<City> {
    /// Reference: `MAX_PLACEMENT_ATTEMPTS`, fifty attempts without a success.
    const MAX_ATTEMPTS: i32 = 50;
    let mut placed = Vec::new();
    let mut attempts = 0;
    while placed.len() < usize::try_from(wanted.max(0)).unwrap_or(0) && attempts < MAX_ATTEMPTS {
        attempts += 1;
        let size = roll_city_size(rolls, settings);
        let (x, y) = roll_city_position(rolls, size, overmap.width(), overmap.height());
        if overmap.terrain(x, y) != Some(DEFAULT_OTER) {
            continue;
        }
        overmap.set_terrain(x, y, "road_nesw");
        placed.push(City { x, y, size });
        attempts = 0;
    }
    placed
}

/// The terrain a fresh overmap is filled with, which a city may only be placed on.
pub const DEFAULT_OTER: &str = "field";

/// Which way a street runs.
///
/// Reference: `om_direction::type` (`omdata.h`), whose four values are north, east, south and west **in that order**,
/// because the order is what rotation counts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
    /// Towards the top of the map.
    North,
    /// Towards the right.
    East,
    /// Towards the bottom.
    South,
    /// Towards the left.
    West,
}

impl Direction {
    /// All four, in the reference's order.
    pub const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    /// Where this direction sits in that order.
    #[must_use]
    pub fn index(self) -> i32 {
        match self {
            Self::North => 0,
            Self::East => 1,
            Self::South => 2,
            Self::West => 3,
        }
    }

    /// How far one step in this direction moves, in tiles.
    ///
    /// Reference: `om_direction::displace`, which rotates `(0, -dist)` - north is written first and is therefore
    /// the direction of decreasing y.
    #[must_use]
    pub fn displace(self, distance: i32) -> (i32, i32) {
        match self {
            Self::North => (0, -distance),
            Self::East => (distance, 0),
            Self::South => (0, distance),
            Self::West => (-distance, 0),
        }
    }

    /// The direction a quarter turn from this one, counted in the reference's order.
    #[must_use]
    fn rotate(self, steps: i32) -> Self {
        let turned = (self.index() + steps).rem_euclid(4);
        match turned {
            1 => Self::East,
            2 => Self::South,
            3 => Self::West,
            _ => Self::North,
        }
    }

    /// The direction a quarter turn to the left.
    ///
    /// Reference: `turn_left`, which is a rotation by **minus** a quarter of the four.
    #[must_use]
    pub fn turn_left(self) -> Self {
        self.rotate(-1)
    }

    /// The direction a quarter turn to the right.
    #[must_use]
    pub fn turn_right(self) -> Self {
        self.rotate(1)
    }
}

/// A straight run of tiles, from a starting point towards a direction.
///
/// Reference: `straight_path` (`overmap.cpp`). A run of `length` tiles **including the one it starts on**, each step
/// moving one tile in the direction. Nothing here checks the map: the caller that lays out a street decides where it
/// may go, and this is only the shape of the line once that is settled.
#[must_use]
pub fn straight_path(source: (i32, i32), direction: Direction, length: usize) -> Vec<(i32, i32)> {
    if length == 0 {
        return Vec::new();
    }
    let mut nodes = Vec::with_capacity(length);
    let (mut x, mut y) = source;
    for _ in 0..length - 1 {
        nodes.push((x, y));
        let (dx, dy) = direction.displace(1);
        x += dx;
        y += dy;
    }
    nodes.push((x, y));
    nodes
}

/// What a street needs to know about the map it is crossing.
///
/// Reference: the `overmap_connection` and the terrain itself. The port keeps them as two questions rather than a
/// connection object, because those two questions are all the rule asks: **is this tile already a road**, and **may
/// a road be laid here at all** (a river, a ravine, or anything the connection cannot serve says no).
pub struct StreetView<'a> {
    /// How many tiles the map is across.
    pub width: i32,
    /// How many tiles the map is down.
    pub height: i32,
    /// Whether a terrain is a road.
    pub is_road: &'a dyn Fn(&str) -> bool,
    /// Whether a road may be laid on a terrain.
    pub can_serve: &'a dyn Fn(&str) -> bool,
}

/// How many tiles from the edge a street keeps away.
///
/// Reference: `inbounds( pos, 1 )` in `lay_out_street` - streets do not approach the overmap's edge, because the
/// map that would continue them is a different overmap.
const STREET_MARGIN: i32 = 1;

fn inside_with_margin(x: i32, y: i32, width: i32, height: i32) -> bool {
    x >= STREET_MARGIN
        && y >= STREET_MARGIN
        && x < width - STREET_MARGIN
        && y < height - STREET_MARGIN
}

/// Lays out a street from a point in a direction, as far as the map allows.
///
/// Reference: `lay_out_street` (`overmap.cpp`). The order of the checks is the rule, so it is kept:
///
/// 1. If the tile **one past** the requested end could carry a road, the street is made one longer.
/// 2. Then it walks outwards, and stops at the first tile that is out of bounds (with a margin), that a road cannot
///    be laid on, that would run **alongside an existing road** (three road tiles among the eight around it, not
///    counting the tile it came from or the tile it is going to), or that **already is a road** once it has moved at
///    least one tile - a street that reaches an existing road joins it instead of crossing it.
///
/// Returns the tiles the street runs through, including the one it starts on. A run of one tile or less means the
/// street was not worth building, which the caller checks rather than this function deciding.
#[must_use]
pub fn lay_out_street(
    map: &Overmap,
    source: (i32, i32),
    direction: Direction,
    length: usize,
    view: &StreetView<'_>,
) -> Vec<(i32, i32)> {
    let mut wanted = length;
    let (sx, sy) = source;
    let (dx, dy) = direction.displace(1);
    let reach = i32::try_from(wanted).unwrap_or(i32::MAX);
    let (ex, ey) = (sx + dx * (reach + 1), sy + dy * (reach + 1));
    if inside_with_margin(ex, ey, view.width, view.height)
        && map
            .terrain(ex, ey)
            .is_some_and(|terrain| (view.can_serve)(terrain))
    {
        wanted += 1;
    }

    let mut laid = 0_usize;
    while laid < wanted {
        let step = i32::try_from(laid).unwrap_or(i32::MAX);
        let (x, y) = (sx + dx * step, sy + dy * step);
        if !inside_with_margin(x, y, view.width, view.height) {
            break;
        }
        let Some(terrain) = map.terrain(x, y) else {
            break;
        };
        if !(view.can_serve)(terrain) {
            break;
        }

        // A street that would run alongside another road is stopped before it makes a corridor.
        let mut collisions = 0;
        let mut collided = false;
        for i in -1..=1 {
            for j in -1..=1 {
                let (cx, cy) = (x + i, y + j);
                let ahead = (x + dx, y + dy);
                let behind = (x - dx, y - dy);
                if (cx, cy) == ahead || (cx, cy) == behind || (cx, cy) == (x, y) {
                    continue;
                }
                if map
                    .terrain(cx, cy)
                    .is_some_and(|there| (view.is_road)(there))
                {
                    collisions += 1;
                }
            }
            if collisions >= 3 {
                collided = true;
                break;
            }
        }
        if collided {
            break;
        }

        laid += 1;

        // Reaching a road that is already there: stop on it rather than beside it.
        if laid > 1 && (view.is_road)(terrain) {
            break;
        }
    }

    straight_path(source, direction, laid)
}

/// What a street tile turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreetTile {
    /// Plain road.
    Road,
    /// A road tile that a sub-street leaves from, so a four-way junction.
    Junction,
    /// A junction that also carries a manhole.
    Manhole,
}

/// Everything a growing town carries with it.
///
/// The reference passes the map, the connection, the city, the rolls and the set of placed buildings as five
/// separate arguments through every recursive call; the port gathers them once so the recursion says what it is
/// doing rather than repeating itself.
pub struct Town<'a> {
    /// The map being built on.
    pub map: &'a Overmap,
    /// What the street rule asks about the terrain.
    pub view: &'a StreetView<'a>,
    /// The dice, in one stream.
    pub rolls: &'a mut Rolls,
    /// The tiles the town has grown, in the order they were laid.
    pub tiles: &'a mut Vec<((i32, i32), StreetTile)>,
}

/// Grows a street, and the streets that leave it, across a map.
///
/// Reference: `build_city_street` (`overmap.cpp`). The shape is kept exactly:
///
/// * lay the street out with `lay_out_street` and give up if it is one tile or less;
/// * walk the tiles after the first, counting down a length `c` from the city's size;
/// * every `block_width` tiles - and only while `c` is at least two and past the last block - sprout a street
///   **left** and **right**, each `size - rng(1, 3)` long, where a length of one is bumped to two so no street is a
///   single-tile nub; those sub-streets alternate wide and thin blocks of their own;
/// * at a tile where a sub-street leaves, a junction may carry a manhole, one time in two;
/// * at the far end, if the city is still big and nothing was sprouted, turn a random way and grow again, and one
///   time in five grow the other way as well.
///
/// The tiles are reported rather than written, so a test can read the shape of a town without a terrain table.
pub fn build_city_street(
    town: &mut Town<'_>,
    source: (i32, i32),
    direction: Direction,
    size: i32,
    block_width: i32,
    depth: usize,
) {
    let map = town.map;
    let view = town.view;
    // A town that grew into itself would otherwise recurse until the stack runs out; the reference relies on the
    // shrinking `size` to terminate, and this is the same guard the group drawing uses.
    if depth > 6 || size < 2 {
        return;
    }

    let path = lay_out_street(
        map,
        source,
        direction,
        usize::try_from(size + 1).unwrap_or(1),
        view,
    );
    if path.len() <= 1 {
        return;
    }
    for tile in &path {
        town.tiles.push((*tile, StreetTile::Road));
    }

    let mut c = size;
    let mut croad = size;
    let new_width = if block_width == 2 {
        town.rolls.rng(3, 5)
    } else {
        2
    };

    for (index, tile) in path.iter().enumerate().skip(1) {
        c -= 1;
        if c >= 2 && c < croad - block_width {
            croad = c;
            let mut left = size - town.rolls.rng(1, 3);
            let mut right = size - town.rolls.rng(1, 3);
            if left == 1 {
                left += 1;
            }
            if right == 1 {
                right += 1;
            }
            build_city_street(
                town,
                *tile,
                direction.turn_left(),
                left,
                new_width,
                depth + 1,
            );
            build_city_street(
                town,
                *tile,
                direction.turn_right(),
                right,
                new_width,
                depth + 1,
            );
            // A junction is where the sub-streets leave, and one in two of them carries a manhole.
            let kind = if town.rolls.rng(1, 2) == 1 {
                StreetTile::Manhole
            } else {
                StreetTile::Junction
            };
            if let Some(entry) = town
                .tiles
                .iter_mut()
                .rev()
                .find(|(position, _)| position == tile && index > 0)
            {
                entry.1 = kind;
            }
        }
    }

    // At the edge of town, turn and grow again: small neighbourhoods rather than one long road.
    let shorter = size - town.rolls.rng(1, 3);
    let last = path.last().copied().unwrap_or(source);
    if shorter >= 2 && c == 0 {
        let turn = if town.rolls.rng(1, 2) == 1 {
            direction.turn_left()
        } else {
            direction.turn_right()
        };
        build_city_street(town, last, turn, shorter, block_width, depth + 1);
        if town.rolls.rng(1, 5) == 1 {
            build_city_street(
                town,
                last,
                turn.turn_right().turn_right(),
                shorter,
                block_width,
                depth + 1,
            );
        }
    }
}

/// Grows cities and their streets onto an overmap.
///
/// Reference: `place_cities` and the `build_city_street` calls it makes. Cities are placed first, then each grows
/// four streets from its crossroads - one in each direction, in the reference's own order of turning - and the tiles
/// they lay are written to the map as roads, junctions and manholes.
///
/// Returns how many cities were grown.
pub fn grow_town(
    overmap: &mut Overmap,
    settings: CitySettings,
    wanted: i32,
    rolls: &mut Rolls,
) -> usize {
    let is_road = |terrain: &str| terrain.starts_with("road");
    let can_serve =
        |terrain: &str| terrain != "river" && terrain != "ravine" && terrain != "ravine_edge";
    let placed = place_cities(overmap, settings, rolls, wanted);
    for city in &placed {
        let view = StreetView {
            width: overmap.width(),
            height: overmap.height(),
            is_road: &is_road,
            can_serve: &can_serve,
        };
        let mut grown = Vec::new();
        {
            let mut town = Town {
                map: overmap,
                view: &view,
                rolls,
                tiles: &mut grown,
            };
            for direction in Direction::ALL {
                build_city_street(&mut town, (city.x, city.y), direction, city.size, 2, 0);
            }
        }
        for (position, kind) in grown {
            let terrain = match kind {
                StreetTile::Road => "road",
                StreetTile::Junction => "road_nesw",
                StreetTile::Manhole => "road_nesw_manhole",
            };
            overmap.set_terrain(position.0, position.1, terrain);
        }
        // Once a city's streets are on the map, each tile is named after the neighbours it connects to.
        shape_roads(overmap);
    }
    placed.len()
}

/// Draws part of an overmap as text, one character per tile.
///
/// Not the game's view of anything - it is for looking at: a generated world is easier to judge as a picture than as
/// a count, and this is what makes a town's shape something a person can check.
#[must_use]
pub fn render(overmap: &Overmap, origin: (i32, i32), width: i32, height: i32) -> String {
    let mut out = String::new();
    for y in origin.1..origin.1 + height {
        for x in origin.0..origin.0 + width {
            let glyph = match overmap.terrain(x, y) {
                Some(terrain) if terrain.starts_with("road") => {
                    if terrain.contains("manhole") {
                        'M'
                    } else {
                        road_glyph(terrain).unwrap_or('#')
                    }
                }
                Some("forest") => 'F',
                Some("river") => '~',
                Some(_) => '.',
                None => ' ',
            };
            out.push(glyph);
        }
        out.push('\n');
    }
    out
}

/// Which of its four neighbours a road tile connects to.
///
/// The reference keeps this as a four-bit mask and names the terrain after the bits that are set - `road_nesw`,
/// `road_ne`, `road_ns` - which is why a generated town has a different glyph for every corner. The port computed
/// none of this before: it wrote one road glyph and a junction, so a picture of its world could not show a corner
/// even though the street rule had made one.
#[must_use]
pub fn road_mask(overmap: &Overmap, x: i32, y: i32) -> u8 {
    let mut mask = 0;
    for direction in Direction::ALL {
        let (dx, dy) = direction.displace(1);
        if overmap
            .terrain(x + dx, y + dy)
            .is_some_and(|terrain| terrain.starts_with("road"))
        {
            mask |= 1 << direction.index();
        }
    }
    mask
}

/// The terrain name for a mask, in the reference's own spelling.
#[must_use]
pub fn road_name(mask: u8) -> String {
    let mut name = String::from("road_");
    for (bit, letter) in [(0, 'n'), (1, 'e'), (2, 's'), (3, 'w')] {
        if mask & (1 << bit) != 0 {
            name.push(letter);
        }
    }
    name
}

/// The glyph a road terrain is drawn with, in the reference's box-drawing vocabulary.
#[must_use]
pub fn road_glyph(terrain: &str) -> Option<char> {
    let Some(bits) = terrain.strip_prefix("road_") else {
        return if terrain == "road" { Some('#') } else { None };
    };
    let north = bits.contains('n');
    let east = bits.contains('e');
    let south = bits.contains('s');
    let west = bits.contains('w');
    let glyph = match (north, east, south, west) {
        (true, false, true, false) => '│',
        (false, true, false, true) => '─',
        (true, true, false, false) => '└',
        (true, false, false, true) => '┘',
        (false, true, true, false) => '┌',
        (false, false, true, true) => '┐',
        (true, true, true, false) => '├',
        (true, false, true, true) => '┤',
        (false, true, true, true) => '┬',
        (true, true, false, true) => '┴',
        (true, true, true, true) => '┼',
        _ => '#',
    };
    Some(glyph)
}

/// Names every road tile after the neighbours it connects to.
///
/// Reference: `overmap_connection::pick_subtype_for`, which derives each tile's shape from the road that reaches it.
/// This is the shape pass over a grown town: it reads the map and renames the tiles, so a straight run becomes
/// `road_ns` and a corner becomes `road_ne`.
pub fn shape_roads(overmap: &mut Overmap) {
    let mut changes = Vec::new();
    for y in 0..overmap.height() {
        for x in 0..overmap.width() {
            let Some(terrain) = overmap.terrain(x, y) else {
                continue;
            };
            if !terrain.starts_with("road") {
                continue;
            }
            let mask = road_mask(overmap, x, y);
            if mask == 0 {
                continue;
            }
            let mut name = road_name(mask);
            if terrain.contains("manhole") {
                name.push_str("_manhole");
            }
            changes.push((x, y, name));
        }
    }
    for (x, y, name) in changes {
        overmap.set_terrain(x, y, &name);
    }
}

/// How much of the noise field becomes forest, and where the two kinds meet.
///
/// Reference: `overmap_forest_settings` - `noise_threshold_forest` and `noise_threshold_forest_thick`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForestSettings {
    /// Above this the field becomes forest.
    pub threshold: f32,
    /// Above this it becomes thick forest.
    pub threshold_thick: f32,
}

impl Default for ForestSettings {
    fn default() -> Self {
        Self {
            threshold: 0.25,
            threshold_thick: 0.35,
        }
    }
}

/// Turns open ground into woodland wherever the noise field is high enough.
///
/// Reference: `overmap::place_forests`. Two rules matter and both are easy to get subtly wrong: only tiles that are
/// **still the default terrain** are considered, so a forest never paints over a lake or a road, and the comparison
/// is against the noise **plus** the regional size adjustment, which shifts the whole field up or down for a region.
///
/// Returns how many tiles became forest.
#[allow(clippy::cast_precision_loss)]
pub fn place_forests(
    overmap: &mut Overmap,
    settings: ForestSettings,
    seed: u32,
    size_adjust: f32,
) -> usize {
    let mut planted = 0;
    let default = DEFAULT_OTER.to_string();
    for y in 0..overmap.height() {
        for x in 0..overmap.width() {
            if overmap.terrain(x, y) != Some(default.as_str()) {
                continue;
            }
            let noise = crate::noise::forest_noise(x, y, seed) + size_adjust;
            if noise > settings.threshold_thick {
                overmap.set_terrain(x, y, "forest_thick");
                planted += 1;
            } else if noise > settings.threshold {
                overmap.set_terrain(x, y, "forest");
                planted += 1;
            }
        }
    }
    planted
}
