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
pub fn install(world: &mut World, kinds: &[ItemType], chip_resistance: &std::collections::BTreeMap<String, i32>) {
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
            // **Milligrams in, grams out.** The content's `ItemType::weight` is the reference's `units::mass` base, one milligram (`items.rs:38-40`), while a `Kind` weighs in grams because that is
            // what the reference's `item::weight()` reports. Copying the number across made every item in the game a thousand times too heavy - a 657 g rock weighed 657 kg - which only showed up in a
            // log line until a gate asked.
            weight_g: kind.weight / 1_000,
            pockets,
            thrown_damage: kind.thrown_damage,
            // Reference: `itype::melee_damage` (`item_factory.cpp:4109`) - the same map a swing reads, kept per damage type there and split here into the two the port's
            // `Damage` speaks. A weapon's cut and bash are carried apart because armour resists them apart.
            melee_bash: kind.melee.get("bash").copied().unwrap_or(0.0),
            melee_cut: kind.melee.get("cut").copied().unwrap_or(0.0),
            // The data's melee uses exactly three damage types - bash 1,354 times, cut 225 and stab 147 - so the port keeps those three and no more.
            melee_stab: kind.melee.get("stab").copied().unwrap_or(0.0),
            to_hit: kind.to_hit,
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
    // **The other three kind tables, in one pass** (round 1211's seam): a kind's flags, its chip resistance and the faults it can catch are all questions asked of the same number, and until this
    // ran the store answered every one of them with the missing-table answer - no flags, chip resistance 2, no faults - whatever the data said. Chip resistance is still the fallback here: it lives in
    // the prototypes dump rather than in the type records, and the dump's merge is the next slice.
    let mut facts: Vec<(crate::ItemTypeId, item_registry::KindFacts)> = Vec::new();
    for kind in kinds {
        let Some(number) = item_registry::id_of(world, &kind.id) else {
            continue;
        };
        facts.push((
            number,
            item_registry::KindFacts {
                flags: kind.flags.iter().cloned().collect(),
                qualities: kind.qualities.iter().map(|(quality, level)| (quality.clone(), *level)).collect(),
                // The dump's own figure, joined by id to the number the registry gave this kind; the reference's fallback of 2 when the dump does not mention it.
                chip_resistance: chip_resistance.get(&kind.id).copied().unwrap_or(2),
                faults: kind.faults.clone(),
                count_by_charges: kind.count_by_charges(),
                is_comestible: kind.is_comestible(),
            },
        ));
    }
    item_registry::install_kind_facts(world, &facts);
}
