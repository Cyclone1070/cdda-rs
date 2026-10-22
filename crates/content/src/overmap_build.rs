//! Putting buildings into a town.
//!
//! Reference: the pair of calls at the end of `overmap::build_city_street` - `place_building` on each side of every
//! street tile, once the dice say so - and the retry loop inside `overmap::place_building`, which asks the city for a
//! building up to **ten times** until one fits.
//!
//! Everything this needs already existed: the shape (`overmap_special`), the ground it may stand on
//! (`overmap_special::OvermapLocation`), the test for whether it fits (`overmap::can_place_shape`), the bins to draw
//! from (`overmap_special::CitySettings`) and the weighted pick (`overmap::WeightedList`). What was missing was the
//! loop that puts them in the reference's order.

use cdda_core_types::overmap::{
    BUILDING_CHANCE, BuildingKind, Direction, Overmap, Placement, building_kind, building_site,
    can_place_shape, feather_pool, nesw_weights,
};
use cdda_core_types::rng::Rolls;
use cdda_core_types::weighted::WeightedList;

use crate::overmap_special::{
    CitySettings, OvermapLocation, OvermapSpecial, resolve_region_terrain,
};

/// How many times the reference retries a building that does not fit before giving up on the square.
pub const PLACEMENT_RETRIES: usize = 10;

/// One building that was put into the world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    /// The special's id.
    pub id: String,
    /// The square the special's origin sits on.
    pub at: (i32, i32),
    /// How the shape was turned.
    pub rotation: Direction,
    /// How many tiles it wrote.
    pub tiles: usize,
}

/// What happened while building a town.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildReport {
    /// The buildings that went in, in the order they were placed.
    pub built: Vec<Built>,
    /// The squares that got nothing, and the last rule that refused them.
    pub refused: Vec<((i32, i32), Placement)>,
}

impl BuildReport {
    /// How many tiles the town's buildings cover.
    #[must_use]
    pub fn tiles(&self) -> usize {
        self.built.iter().map(|building| building.tiles).sum()
    }
}

/// The bin a kind of building draws from.
fn bin(city: &CitySettings, kind: BuildingKind) -> &[(String, i32)] {
    match kind {
        BuildingKind::Shop => &city.shops,
        BuildingKind::Park => &city.parks,
        BuildingKind::House => &city.houses,
    }
}

/// Finds a special by id.
fn find<'a>(specials: &'a [OvermapSpecial], id: &str) -> Option<&'a OvermapSpecial> {
    specials.iter().find(|special| special.id == id)
}

/// Whether a special may appear again, given what the town has already used.
fn still_allowed(special: &OvermapSpecial, placed: &[String]) -> bool {
    let once_only = special
        .flags
        .iter()
        .any(|flag| flag == "CITY_UNIQUE" || flag == "OVERMAP_UNIQUE" || flag == "GLOBALLY_UNIQUE");
    !(once_only && placed.iter().any(|used| used == &special.id))
}

/// Whether a special belongs to a town this size.
fn suits_size(special: &OvermapSpecial, town_size: i32) -> bool {
    town_size >= special.city_sizes.0 && town_size <= special.city_sizes.1
}

/// Whether a terrain counts as any of the locations a special names.
fn ground_test<'a>(
    special: &'a OvermapSpecial,
    locations: &'a [OvermapLocation],
) -> impl Fn(&str) -> bool + 'a {
    move |terrain: &str| {
        special.locations.iter().any(|wanted| {
            locations
                .iter()
                .find(|location| &location.id == wanted)
                .is_some_and(|location| location.test(terrain, &[]))
        })
    }
}

/// Puts buildings beside a town's streets.
///
/// The order is the reference's: for every street tile, **left then right**, each side rolled against
/// `BUILDING_CHANCE`; the square one off the street must be free; the spot's distance from the town's middle picks
/// which list to draw from; and a drawn building is retried up to ten times until one fits the ground and the town's
/// size.
///
/// `centre` and `size` are the town's own, because the distance that picks the list is measured from it.
#[allow(clippy::too_many_arguments)]
pub fn build_town(
    overmap: &mut Overmap,
    specials: &[OvermapSpecial],
    locations: &[OvermapLocation],
    city: &CitySettings,
    streets: &[(i32, i32)],
    towards: Direction,
    centre: (i32, i32),
    size: i32,
    rolls: &mut Rolls,
) -> BuildReport {
    let mut report = BuildReport::default();
    let mut placed: Vec<String> = Vec::new();

    for street in streets {
        for side in [towards.turn_left(), towards.turn_right()] {
            if rolls.one_in(BUILDING_CHANCE) {
                continue;
            }
            let Some((square, facing)) = building_site(overmap, *street, side) else {
                report.refused.push((*street, Placement::WrongGround));
                continue;
            };
            let dx = f64::from(square.0 - centre.0);
            let dy = f64::from(square.1 - centre.1);
            #[allow(clippy::cast_possible_truncation)]
            let distance = ((dx * dx + dy * dy).sqrt() as i32) * 100 / size.max(1);
            let kind = building_kind(
                distance,
                city.shop_radius,
                city.shop_sigma,
                city.park_radius,
                city.park_sigma,
                rolls,
            );
            let list = WeightedList::from_entries(bin(city, kind).iter().cloned());
            let mut verdict = Placement::Nameless;
            for _ in 0..PLACEMENT_RETRIES {
                let Some(id) = list.pick(rolls).cloned() else {
                    break;
                };
                let Some(special) = find(specials, &id) else {
                    continue;
                };
                if !suits_size(special, size) || !still_allowed(special, &placed) {
                    continue;
                }
                let tiles: Vec<(i32, i32)> = special
                    .overmaps
                    .iter()
                    .filter(|tile| tile.offset.2 == 0)
                    .map(|tile| (tile.offset.0, tile.offset.1))
                    .collect();
                let ground = ground_test(special, locations);
                verdict = can_place_shape(overmap, &tiles, square, facing, &ground);
                if verdict.is_fits() {
                    for tile in &special.overmaps {
                        if tile.offset.2 != 0 {
                            continue;
                        }
                        let (rx, ry) = facing.rotate_point((tile.offset.0, tile.offset.1));
                        overmap.set_terrain(square.0 + rx, square.1 + ry, &tile.terrain);
                    }
                    placed.push(special.id.clone());
                    report.built.push(Built {
                        id: special.id.clone(),
                        at: square,
                        rotation: facing,
                        tiles: tiles.len(),
                    });
                    break;
                }
            }
            if !verdict.is_fits() {
                report.refused.push((square, verdict));
            }
        }
    }
    report
}

/// The ground a forest chunk gets when only the groundcover pass runs.
///
/// This is the **first** of `mapgen_forest`'s passes and deliberately not all of it: the four feature layers - trees,
/// shrubs, clutter, water - run afterwards and overwrite most of what this produces. It exists because it is the only
/// pass whose input the port already has end to end: the edge curve, the pull of each neighbour, the six-ticket
/// raffle, and the palettes the tickets resolve through.
///
/// Reference: `get_feathered_groundcover` called for every square of the chunk.
#[allow(
    clippy::too_many_arguments,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::indexing_slicing
)]
pub fn forest_ground_pass(
    side: i32,
    perimeter: &[i32],
    self_palette: &[(String, i32)],
    default_palette: &[(String, i32)],
    layers: &[crate::overmap_special::ForestComponent],
    palettes: &std::collections::BTreeMap<String, Vec<(String, i32)>>,
    max_factor: f32,
    factor: f32,
    neighbour_factors: [f32; 4],
    rolls: &mut Rolls,
) -> Vec<String> {
    let mut ground = Vec::with_capacity((side * side) as usize);
    // The reference steps the perimeter back for groundcover by half the average depth and lets the four neighbours
    // compete with this square's own biome, which holds three tickets.
    let margin = -2.0;
    let self_weight = 3.0;
    for y in 0..side {
        for x in 0..side {
            // The reference scales the four neighbours by the square's own sparseness and leaves this square's own
            // ticket at a fixed three, so the scaling is not symmetric and cannot be dropped without letting the
            // forest claim more of the chunk than the reference gives it.
            let weights = nesw_weights((x, y), side, perimeter, margin, [true; 4])
                .map(|weight| weight * factor);
            let net: f32 = weights.iter().sum();
            let pool = feather_pool(
                weights,
                self_weight,
                net,
                max_factor,
                factor,
                neighbour_factors,
            );
            let total: f32 = pool.iter().map(|weight| weight.max(0.0)).sum();
            let mut ticket = rolls.rng_float(0.0, f64::from(total));
            let mut chosen = 4; // this square's own biome, the fallback when the arithmetic leaves nothing else
            for index in 0..6 {
                ticket -= f64::from(pool[index].max(0.0));
                if ticket <= 0.0 {
                    chosen = index;
                    break;
                }
            }
            // Ticket 4 is this square's own biome; the four neighbour tickets and the empty one resolve through the
            // region's default ground. That is right for the case that matters here - a forest next to open ground,
            // where the neighbours own no forest biome - and it is the difference between a chunk that is mostly
            // grass, as the reference's is, and one that is mostly forest floor, as my first version produced.
            if chosen == 4 {
                // This square's own biome runs its layers and only falls back to ground when none of them rolled:
                // the reference's `forest_biome::pick` followed by `get_feathered_groundcover`.
                if let Some(feature) = pick_forest_feature(layers, rolls) {
                    // A feature can be **furniture** - a boulder, or the region's own forest furniture palette - and
                    // furniture does not replace the ground under it. Those squares keep their ground cover, which is
                    // what the reference does by setting the terrain and the furniture separately.
                    if !feature.starts_with('f') {
                        ground.push(resolve_region_terrain(palettes, &feature, rolls));
                        continue;
                    }
                }
            }
            let palette = if chosen == 4 {
                self_palette
            } else {
                default_palette
            };
            let entry = WeightedList::from_entries(palette.iter().cloned());
            let picked = entry.pick(rolls).cloned().unwrap_or_default();
            ground.push(resolve_region_terrain(palettes, &picked, rolls));
        }
    }
    ground
}

/// What one of a forest's layers puts on a square, if anything.
///
/// Reference: `forest_biome::pick` in `regional_settings.cpp`. The layers run **in sequence order**, and each one gets a
/// single `one_in(chance)` roll: the first layer that succeeds contributes **one** feature and the rest do not run at
/// all. If no layer succeeds the square keeps its groundcover - which is how a forest ends up being mostly floor with
/// trees standing in it rather than a solid stand of trunks.
///
/// The roll consumes **one** die per layer tried, in sequence order, so a wrong order here does not merely change what
/// grows: it changes the dice the next square receives.
#[must_use]
pub fn pick_forest_feature(
    layers: &[crate::overmap_special::ForestComponent],
    rolls: &mut Rolls,
) -> Option<String> {
    for layer in layers {
        if rolls.one_in(layer.chance) {
            let entries = WeightedList::from_entries(layer.types.iter().cloned());
            // The layer's list is `(terrain id, weight)` pairs, and the pick borrows from it, so the pair is cloned
            // out rather than the id alone.
            return entries.pick(rolls).cloned();
        }
    }
    None
}
