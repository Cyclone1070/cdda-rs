//! What a blow does to one vehicle part.
//!
//! Reference: `vehicle::damage_direct` (`vehicle.cpp:7925-7945`). Three rules in a row, and two of them are surprising enough that a port which skipped them would still look
//! right on a big hit:
//!
//! * **A blow below a threshold does nothing at all.** `tsh = min( 20, durability / 10 )`, and a hit smaller than that is returned unconsumed - not reduced, not rounded down,
//!   nothing. A 19-point blow on a car seat is a scratch the game does not track.
//! * **The part's flat armour is subtracted next**, by damage kind, from what is left.
//! * **`pure` damage is the exception to both** - the reference tests `type != damage_pure` for the threshold and `type->no_resist` for the armour, and the data marks `pure`
//!   `no_resist` (`data/json/damage_types.json`).
//!
//! The rule is a function of a part's data and its hit points, so it is testable without a world; the store that holds the hit points comes next.

use cdda_content::vehicles::PartSpec;

/// What became of a blow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The blow was below the part's threshold: it passed through and cost nothing.
    Ignored,
    /// The blow landed.
    Applied {
        /// How much damage reached the hit points after the armour.
        net: i32,
        /// The part's hit points afterwards.
        hp_after: i32,
        /// Whether that took the part to zero, which the reference calls broken - the part stays, and stops working.
        broke: bool,
    },
}

/// Applies one blow to one part.
///
/// `hp` is the part's hit points now, `amount` the damage being dealt, `kind` the damage type's id and `no_resist` whether that type ignores armour and thresholds - which is
/// the damage type's own fact, so the caller passes it rather than this rule guessing from the name.
#[must_use]
pub fn hit(part: &PartSpec, hp: i32, amount: i32, kind: &str, no_resist: bool) -> Outcome {
    // Reference: `int tsh = std::min( 20, vpi.durability / 10 ); if( dmg < tsh && type != damage_pure ) return dmg;` - the blow does nothing and costs nothing.
    let threshold = (part.durability / 10).min(20);
    if amount < threshold && !no_resist {
        return Outcome::Ignored;
    }
    // Reference: `if( !type->no_resist ) dmg -= std::min<int>( dmg, vpi.damage_reduction.at( type ) );` - armour shaves the blow, never adds to it.
    let armour = if no_resist {
        0
    } else {
        part.damage_reduction
            .get(kind)
            .copied()
            .map_or(0, |armour| armour.max(0.0) as i32)
    };
    let net = amount - amount.min(armour);
    // Reference: `mod_hp( vp, -dmg )` returns true when the part broke, and `is_broken()` is `damage >= max_damage` - the part stays in place, broken.
    let hp_after = (hp - net).max(0);
    Outcome::Applied {
        net,
        hp_after,
        broke: hp_after == 0,
    }
}
