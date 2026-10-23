//! How much light a square lets through, and the second answer the reference keeps beside it.
//!
//! Reference: the `calc_transp` lambda inside `map::build_transparency_cache` (`src/lightmap.cpp:140-167`). One square's transparency is a small rule run twice over the same
//! facts: once with the fields counted and once without, because the game asks two different questions of it. "How much light gets here" wants the fields; "is there
//! something to see here at all" does not, or a smoked room would look like solid rock.

use crate::light::TRANSPARENCY_OPEN_AIR;
use crate::shadowcasting::SOLID;

/// One field's contribution to how opaque a square is.
///
/// Reference: `field_intensity_level`'s `transparent` and `translucency` (`lightmap.cpp:158-165`). A field is either entirely transparent - in which case it is skipped and
/// its translucency is irrelevant - or it multiplies the light by its translucency.
#[derive(Debug, Clone, Copy)]
pub struct FieldOpacity {
    /// Whether this field is see-through. A transparent field contributes nothing.
    pub transparent: bool,
    /// What a non-transparent field multiplies the light by.
    pub translucency: f32,
}

/// What the map knows about one square that changes its transparency.
///
/// Reference: the four things `calc_transp` reads - the terrain's and furniture's `transparent` flags, the `outside_cache` bit and the square's fields.
#[derive(Debug, Clone)]
pub struct SquareFacts {
    /// Whether the terrain is see-through.
    pub terrain_transparent: bool,
    /// Whether the furniture is see-through. Furniture that does not exist counts as transparent.
    pub furniture_transparent: bool,
    /// Whether the square is under open sky, and therefore in the weather.
    pub outside: bool,
    /// The fields lying on the square.
    pub fields: Vec<FieldOpacity>,
}

/// A square's transparency, in the two forms the reference caches.
///
/// Reference: the `std::pair` that `calc_transp` returns (`lightmap.cpp:142-167`) and the two caches it fills - `transparency_cache` and `transparent_cache_wo_fields`.
#[derive(Debug, Clone, Copy)]
pub struct Transparency {
    /// How much light gets through, fields included.
    pub value: f32,
    /// How much would get through with the fields removed.
    pub without_fields: f32,
}

impl Transparency {
    /// Whether anything at all gets through, which is what the reference's `wo_fields` bitset records.
    #[must_use]
    pub fn see_through(self) -> bool {
        self.without_fields > SOLID
    }
}

/// Computes one square's transparency.
///
/// Reference: `calc_transp` (`lightmap.cpp:140-167`), in its order - the order is visible in the answer, because the opacity test returns **both** values and never lets a
/// field make a wall less opaque:
///
/// 1. terrain **and** furniture must be transparent, or the answer is solid twice over;
/// 2. a square under open sky is multiplied by the weather's `sight_penalty`;
/// 3. every field that is not transparent multiplies the value by its translucency.
///
/// The second value is taken after step 2 and before step 3, which is what makes it "the same square without its smoke".
#[must_use]
pub fn transparency_of(facts: &SquareFacts, sight_penalty: f32) -> Transparency {
    if !(facts.terrain_transparent && facts.furniture_transparent) {
        return Transparency {
            value: SOLID,
            without_fields: SOLID,
        };
    }
    let mut value = TRANSPARENCY_OPEN_AIR;
    if facts.outside {
        value *= sight_penalty;
    }
    let without_fields = value;
    for field in &facts.fields {
        if field.transparent {
            continue;
        }
        value *= field.translucency;
    }
    Transparency {
        value,
        without_fields,
    }
}
