//! How well a character can make out fine detail.
//!
//! Reference: `Character::fine_detail_vision_mod()` (`character.cpp:2916-2947`), which the reading rules multiply time by and the crafting rules gate on. It is **a multiplier, so smaller is
//! better**: one is "no penalty" and eleven is "you cannot see".

/// How much harder fine work is, from the light on the character and the light around them.
///
/// Reference: `Character::fine_detail_vision_mod()`:
///
/// ```text
/// if( is_blind() || ( ( boomered || darkness ) && !PER_SLIME_OK ) ) return 11.0;
/// float own_light     = std::max( 1.0f, LIGHT_AMBIENT_LIT - active_light() - 2.0f );
/// float ambient_light = std::max( 1.0f, LIGHT_AMBIENT_LIT - ambient_light_at( p ) + 1.0f );
/// return std::min( own_light, ambient_light );
/// ```
///
/// **Light the character carries is worth three more than ambient light**: the `-2` against the `+1` means a lamp is treated as three points brighter than the same brightness on the floor, because
/// they can point it where they need it. **Both answers are floored at one**, so more light than `LIGHT_AMBIENT_LIT` (ten, `lightmap.h:19`) never makes the work easier than it already is - there is no
/// bonus for standing in the sun.
#[must_use]
pub fn fine_detail_vision_mod(impaired: bool, active_light: f32, ambient_light: f32) -> f32 {
    /// `LIGHT_AMBIENT_LIT` (`lightmap.h:19`): the light level at which the penalty is gone.
    const LIGHT_AMBIENT_LIT: f32 = 10.0;
    if impaired {
        return 11.0;
    }
    let own_light = (LIGHT_AMBIENT_LIT - active_light - 2.0).max(1.0);
    let around = (LIGHT_AMBIENT_LIT - ambient_light + 1.0).max(1.0);
    own_light.min(around)
}
