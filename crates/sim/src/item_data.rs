//! The game's own item data, turned into the registry the store answers from.
//!
//! Reference: `item_factory::load_item` reads every item type once and the game asks that table ever after. The split
//! in the port is a crate boundary rather than a class: `cdda-content` reads the JSON and knows what an `item` is,
//! `cdda-sim` knows what a survivor can do with one, and this module is the one place the two meet.
//!
//! It is a module rather than a method because the conversion is not either side's business: content must not know
//! about pockets as the simulation needs them, and the simulation must not know how JSON is spelled.

use bevy_ecs::prelude::*;
use cdda_content::items::ItemType;
use cdda_content::pockets::PocketSpec;

use crate::item_registry::{self, Kind};
use crate::pockets::Pocket;

/// The pockets of an item, as the fit rule wants them.
///
/// Reference: `itype::pockets` -> `pocket_data`. The four fields kept are the four `item_pocket::can_contain` reads when
/// a pocket is empty; the rest of the record (sealing, restrictions, ablative plates) is for rules that do not exist
/// here yet, and carrying it would be a lie dressed as completeness.
#[must_use]
pub fn pockets_of(specs: &[PocketSpec]) -> Vec<Pocket> {
    specs
        .iter()
        .map(|spec| Pocket {
            #[allow(clippy::cast_possible_truncation)]
            capacity_ml: spec.max_volume_ml as i32,
            #[allow(clippy::cast_possible_truncation)]
            max_weight_g: spec.max_weight_grams as i32,
            #[allow(clippy::cast_possible_truncation)]
            max_item_ml: spec.max_item_volume_ml.map(|most| most as i32),
            holster: spec.holster,
        })
        .collect()
}

/// Installs the registry from the game's item types.
///
/// The ids are numbers in the order the data lists them, which is the reference's own scheme (`itype_id`), and the
/// pockets of each are kept so a survivor can put something in them.
pub fn install(world: &mut World, kinds: &[ItemType]) {
    // The pockets have to outlive the borrowed `Kind` list, so they are built first and held in one place.
    let pockets: Vec<Vec<Pocket>> = kinds
        .iter()
        .map(|kind| pockets_of(&kind.pockets))
        .collect();
    let borrowed: Vec<Kind<'_>> = kinds
        .iter()
        .zip(&pockets)
        .map(|(kind, pockets)| Kind {
            id: &kind.id,
            volume_ml: kind.volume,
            weight_g: kind.weight,
            pockets,
            thrown_damage: kind.thrown_damage,
            // Reference: `itype::melee_damage` (`item_factory.cpp:4109`) - the same map a swing reads, kept per damage type there and split here into the two the port's
            // `Damage` speaks. A weapon's cut and bash are carried apart because armour resists them apart.
            melee_bash: kind.melee.get("bash").copied().unwrap_or(0.0),
            melee_cut: kind.melee.get("cut").copied().unwrap_or(0.0),
            // The data's melee uses exactly three damage types - bash 1,354 times, cut 225 and stab 147 - so the port keeps those three and no more.
            melee_stab: kind.melee.get("stab").copied().unwrap_or(0.0),
        })
        .collect();
    item_registry::set_item_types(world, &borrowed);
    // The qualities come from the same records and are installed right after the ids exist: the reference keeps them on `itype` (`item_factory.cpp`), and a
    // world that loaded item data but not its qualities answers every requirement as if nothing had a quality.
    let mut held: Vec<(crate::ItemTypeId, Vec<(String, i32)>)> = Vec::new();
    for kind in kinds {
        let Some(number) = item_registry::id_of(world, &kind.id) else {
            continue;
        };
        let qualities: Vec<(String, i32)> = kind
            .qualities
            .iter()
            .map(|(quality, level)| (quality.clone(), *level))
            .collect();
        if !qualities.is_empty() {
            held.push((number, qualities));
        }
    }
    let borrowed: Vec<(crate::ItemTypeId, &[(String, i32)])> = held
        .iter()
        .map(|(kind, qualities)| (*kind, qualities.as_slice()))
        .collect();
    item_registry::set_item_qualities(world, &borrowed);
}
