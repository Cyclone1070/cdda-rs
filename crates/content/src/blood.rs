//! What a monster bleeds, and what it splatters into.
//!
//! Reference: `mtype::bloodType` (`mtype.cpp:431-454`) and `mtype::gibType` (`mtype.cpp:456-470`). Both are **ladders**, and every rung is a fact about the monster rather than about
//! its field data: a flag first, then what it is made of, and finally - for blood only - the species' own bleed type. The port keeps the ladder in the reference's order because that order
//! is what makes an acid-blooded corpse bleed acid and not blood.

/// The three blood flags and the warmth that decides the ordinary case, gathered so the ladder takes one argument.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BloodTraits {
    /// `ACID_BLOOD`: it bleeds acid. Checked first, and it wins over everything below.
    pub acid: bool,
    /// `BILE_BLOOD`: it bleeds bile.
    pub bile: bool,
    /// `ARTHROPOD_BLOOD`: it bleeds an invertebrate's blood.
    pub arthropod: bool,
    /// `WARM`: a warm fleshy thing bleeds ordinary blood; a cold one falls through to its species.
    pub warm: bool,
}

/// The field a monster bleeds, as an id, or `None` when the ladder falls through.
///
/// Reference: `mtype::bloodType` (`mtype.cpp:431-454`), in its own order:
///
/// ```text
/// ACID_BLOOD -> fd_acid; BILE_BLOOD -> fd_bile; ARTHROPOD_BLOOD -> fd_blood_invertebrate;
/// veggy -> fd_blood_veggy; iflesh -> fd_blood_insect; WARM && flesh -> fd_blood; else get_bleed_type()
/// ```
///
/// **The order is the rule**: an acid-blooded monster made of flesh bleeds **acid**, not blood. A port that checked the material first would be right about most monsters and wrong about
/// exactly the ones a player notices.
///
/// `None` stands for `get_bleed_type` (`mtype.cpp:412-422`), which needs the species' `bleeds` field and the monster's `bleed_rate` - neither is in the dump yet, so the port says it does
/// not know rather than inventing `fd_blood`.
#[must_use]
pub fn blood_field(traits: &BloodTraits, materials: &[&str]) -> Option<&'static str> {
    if traits.acid {
        return Some("fd_acid");
    }
    if traits.bile {
        return Some("fd_bile");
    }
    if traits.arthropod {
        return Some("fd_blood_invertebrate");
    }
    if materials.contains(&"veggy") {
        return Some("fd_blood_veggy");
    }
    if materials.contains(&"iflesh") {
        return Some("fd_blood_insect");
    }
    if traits.warm && materials.contains(&"flesh") {
        return Some("fd_blood");
    }
    None
}

/// The field a monster splatters into, as an id, or `None` for `fd_null`.
///
/// Reference: `mtype::gibType` (`mtype.cpp:456-470`):
///
/// ```text
/// MOLLUSK species -> fd_gibs_invertebrate; veggy -> fd_gibs_veggy; iflesh -> fd_gibs_insect;
/// flesh -> fd_gibs_flesh; else fd_null
/// ```
///
/// **Not every material gibs**: steel, bone, stone and the rest fall to `fd_null`, which is the reference's own comment ("There are other materials not listed here") and not an oversight.
#[must_use]
pub fn gib_field(materials: &[&str], in_mollusk_species: bool) -> Option<&'static str> {
    if in_mollusk_species {
        return Some("fd_gibs_invertebrate");
    }
    if materials.contains(&"veggy") {
        return Some("fd_gibs_veggy");
    }
    if materials.contains(&"iflesh") {
        return Some("fd_gibs_insect");
    }
    if materials.contains(&"flesh") {
        return Some("fd_gibs_flesh");
    }
    None
}

/// The blood a monster falls through to when its flags and materials say nothing: its **species'** bleed type.
///
/// Reference: `mtype::get_bleed_type` (`mtype.cpp:412-422`):
///
/// ```text
/// if( bleed_rate > 0 )
///     for( const species_id &s : species )
///         if( !s->bleeds.is_empty() ) return s->bleeds;
/// return fd_null;
/// ```
///
/// **Two things make this a rule rather than a lookup.** The rate is a **gate**: zero means the monster never bleeds its species' blood, however the species is written. And the species are
/// walked **in order, first non-empty wins** - a monster with two bleeding species takes the first, not the most specific and not the strongest.
///
/// `None` is `fd_null`, which is the reference's answer for a monster with no bleeding species at all.
#[must_use]
pub fn bleed_field<'a>(
    bleed_rate: i32,
    species: &[&'a str],
    bleeds_of: impl Fn(&str) -> Option<&'a str>,
) -> Option<&'a str> {
    if bleed_rate <= 0 {
        return None;
    }
    let found = species.iter().find_map(|kind| bleeds_of(kind).filter(|field| !field.is_empty()))?;
    // `fd_null` is the reference's own "no field" id and the data spells it out for species that bleed nothing (`species_type::bleeds` defaults to it). Returning it would have the
    // splatter look up a field the registry does not define, so the rule answers `None` - the same answer, said in the port's own voice.
    if found == "fd_null" { None } else { Some(found) }
}
