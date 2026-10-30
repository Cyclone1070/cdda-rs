//! The body's parts and the numbers a blow's landing place is chosen from.
//!
//! Reference: `body_part_type` (`src/bodypart.h`), whose `hit_size` and `hit_difficulty` are what
//! `anatomy::select_body_part` weighs, and whose `LIMB_UPPER` / `LIMB_LOWER` flags decide what a short attacker
//! can reach. The port keeps the numbers and the two flags and nothing else: a part is not an entity, it is a
//! name a blow can land on.

/// One kind of body part, as the game states it.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyPartType {
    /// Its id, which is what an anatomy lists.
    pub id: String,
    /// Its name as a message would print it.
    pub name: String,
    /// How big a target it is.
    pub hit_size: f32,
    /// How much a better attack roll favours it.
    pub hit_difficulty: f32,
    /// Whether it is high enough that a short attacker cannot reach it.
    pub upper: bool,
    /// Whether it is low enough that a short attacker finds it easier.
    pub lower: bool,
}

/// Reads one prototype.
///
/// Reference: `body_part_type::load` (`bodypart.cpp`), which reads `id`, `name`, `hit_size`,
/// `hit_difficulty` and the part's flags.
///
/// # Errors
/// Returns the id when a prototype states no `id`.
pub fn read_prototype(
    object: &cdda_json::Object,
) -> Result<BodyPartType, crate::generated::LoadError> {
    let Some(id) = object.get("id").and_then(|value| match value {
        cdda_json::Value::Str(text) => Some(text.clone()),
        _ => None,
    }) else {
        return Err(crate::generated::LoadError {
            message: "a body part with no id".to_owned(),
        });
    };
    let name = match object.get("name") {
        Some(cdda_json::Value::Str(text)) => text.clone(),
        _ => id.clone(),
    };
    // The reference keeps these as `float`; a body part's size and difficulty are small numbers, and the
    // narrowing is the data's own precision rather than a loss the port is choosing.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let number = |member: &str| -> f32 {
        match object.get(member) {
            Some(cdda_json::Value::Float(found)) => *found as f32,
            Some(cdda_json::Value::Int(found)) => *found as f32,
            _ => 0.0,
        }
    };
    // The two flags are read as the dump states them - `upper` and `lower` booleans - rather than as the
    // `LIMB_UPPER`/`LIMB_LOWER` entries the game's own JSON carries. The dump writes the answer the reference
    // computed from those flags, which is the fact the rule needs and the one the port can be held to.
    let flag = |member: &str| -> bool {
        matches!(object.get(member), Some(cdda_json::Value::Bool(true)))
    };
    Ok(BodyPartType {
        name,
        hit_size: number("hit_size"),
        hit_difficulty: number("hit_difficulty"),
        upper: flag("upper"),
        lower: flag("lower"),
        id,
    })
}

/// Reads a list of prototypes, in the order the file states them.
///
/// # Errors
/// Returns the first prototype that could not be read.
pub fn load_objects(
    prototypes: &[cdda_json::Value],
) -> Result<Vec<BodyPartType>, crate::generated::LoadError> {
    let mut out = Vec::new();
    for value in prototypes {
        if let cdda_json::Value::Object(object) = value {
            out.push(read_prototype(object)?);
        }
    }
    Ok(out)
}
