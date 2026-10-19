//! Where a JSON type goes, and the prototype a factory makes before it reads one.
//!
//! Two things about the reference cannot be read off the data: a few JSON types
//! are dispatched into a differently-named registry, and several factories insert
//! a null prototype the first time their loader runs.
//!
//! Both are recorded here with the line that does it, because a wrong entry here
//! silently moves prototypes between registries — the exact failure the id
//! comparison exists to catch.

/// The registry a JSON `type` string is loaded into.
///
/// Almost every type keeps its own name. The exceptions are dispatched by
/// `src/init.cpp` to a loader that writes into another registry; each is listed
/// with that loader.
#[must_use]
pub fn registry_for(json_type: &str) -> &str {
    match json_type {
        // `add( "conduct", &achievement::load_conduct )` — conducts are stored in
        // the achievement registry, not in one of their own.
        "conduct" => "achievement",
        other => other,
    }
}

/// The JSON member that carries an object's id in a registry.
///
/// The factory takes this as its second constructor argument, so it is per
/// registry rather than per type: `generic_factory<monfaction> faction_factory(
/// "MONSTER_FACTION", "name" )` is the only registry in the core content that
/// does not use `id`.
#[must_use]
pub fn id_member(registry: &str) -> &str {
    match registry {
        "MONSTER_FACTION" => "name",
        _ => "id",
    }
}

/// The null prototype a factory inserts before loading its first object, if any.
///
/// The reference does this inside the loader itself, guarded by "is the
/// registry still empty", so the null prototype is the first entry and exists
/// only when at least one object of that type is loaded.
#[must_use]
pub fn null_prototype(json_type: &str) -> Option<&'static str> {
    match json_type {
        // `mapdata.cpp:817-819`: `if( furniture_data.empty() ) insert( null_furniture_t() )`
        "furniture" => Some("f_null"),
        // `mapdata.cpp:825-827`: the same for terrain.
        "terrain" => Some("t_null"),
        // `species` has no loader of its own: `spec_null` comes from the
        // species table's own empty check, and is referenced by monsters that
        // name no species.
        "SPECIES" => Some("spec_null"),
        _ => None,
    }
}
