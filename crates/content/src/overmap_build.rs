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
/// Draws one terrain name from a weighted palette.
fn pick_terrain(palette: &[(String, i32)], rolls: &mut Rolls) -> String {
    let list = WeightedList::from_entries(palette.iter().cloned());
    list.pick(rolls).cloned().unwrap_or_default()
}

/// The ground-cover margin: half the average depth of a border, in squares.
const MARGIN: f32 = -2.0;

/// Reference: `get_feathered_groundcover` called for every square of the chunk.
#[allow(
    clippy::too_many_arguments,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
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
    neighbour_biomes: [Option<&crate::overmap_special::ForestMapgen>; 4],
    rolls: &mut Rolls,
) -> Vec<String> {
    let mut ground = Vec::with_capacity((side * side) as usize);
    let self_weight = 3.0;

    // One raffle. The reference runs **two** of these per square - one to pick a feature, and, when that yields no
    // terrain, a second one stepped back by the ground-cover margin to pick the ground - so the two are the same
    // arithmetic with a different offset, and the dice are spent twice.
    let owns_biome = neighbour_biomes.map(|biome| biome.is_some());
    let neighbour_factors = neighbour_biomes.map(|biome| {
        biome.map_or(0.0, |biome| {
            f32::from(i16::try_from(biome.sparseness_adjacency_factor).unwrap_or(0))
        })
    });
    let raffle = |rolls: &mut Rolls, square: (i32, i32), depth_offset: f32| -> usize {
        let weights = nesw_weights(square, side, perimeter, depth_offset, owns_biome)
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
        // The reference draws this raffle with a raw 32-bit value and walks the tickets; a real-valued draw consumes a
        // different amount of the engine's state and puts every later square on different dice.
        cdda_core_types::overmap::pick_real_ticket(rolls.rng_bits(), &pool)
    };

    for y in 0..side {
        for x in 0..side {
            let point = (x, y);
            // Ticket 4 is this square's own biome; the four neighbour tickets and the empty one fall through to the
            // ground raffle. A neighbour without a biome is **not** divided by its own depth, because it does not
            // feather at all.
            let first = raffle(rolls, point, 0.0);
            if first == 4
                && let Some(feature) = pick_forest_feature(layers, rolls)
                // A feature can be **furniture** - a boulder, or the region's own forest furniture palette - and
                // furniture does not replace the ground under it; the reference keeps the ground and sets the
                // furniture on top.
                && !feature.starts_with('f')
            {
                ground.push(resolve_region_terrain(palettes, &feature, rolls));
                continue;
            }
            // No terrain yet, so the ground gets its own raffle, stepped back by the ground-cover margin - half the
            // average depth - so that ground reaches further out than the features standing on it.
            let second = raffle(rolls, point, MARGIN);
            let picked = if second == 4 {
                pick_terrain(self_palette, rolls)
            } else if let Some(biome) = neighbour_biomes.get(second).copied().flatten() {
                // A neighbour that owns a biome contributes **its own** ground. Its groundcover is a list of palette
                // names, so it is drawn from and then resolved to a real terrain.
                let list = WeightedList::from_entries(biome.groundcover.iter().cloned());
                list.pick(rolls).cloned().unwrap_or_default()
            } else {
                pick_terrain(default_palette, rolls)
            };
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

/// Turns the placeholder names in a generated chunk into the terrain the region's palettes stand for.
///
/// Reference: `resolve_regional_terrain_and_furniture` in `mapgen_functions.cpp`. Map generation writes names like
/// `t_region_groundcover_forest` into the map and never resolves them; a **separate pass over every square of the chunk**
/// does that afterwards, drawing one weighted pick per placeholder. So the terrain a finished chunk holds is not the
/// terrain its generator chose - it is the generator's choice **plus** a second draw, and a port that stops after the
/// generator produces a chunk of placeholders.
///
/// The pass is deliberately separate here too: it is a step a caller can run on any chunk, in the order the reference
/// runs it, rather than something hidden inside generation.
pub fn resolve_chunk_placeholders(
    cells: &mut [String],
    palettes: &std::collections::BTreeMap<String, Vec<(String, i32)>>,
    rolls: &mut Rolls,
) -> usize {
    let mut resolved = 0;
    for cell in cells.iter_mut() {
        if palettes.contains_key(cell) {
            *cell = crate::overmap_special::resolve_region_terrain(palettes, cell, rolls);
            resolved += 1;
        }
    }
    resolved
}
