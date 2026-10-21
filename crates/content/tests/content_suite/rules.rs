//! The rules the reference keeps in code rather than in data.
//!
//! Each of these moved prototypes between registries or added one the content
//! files do not contain, so each is pinned here with the line that does it.

use cdda_content::{id_member, null_prototype, registry_for};

#[test]
fn a_type_keeps_its_name_unless_the_loader_says_otherwise() {
    assert_eq!(registry_for("furniture"), "furniture");
    assert_eq!(registry_for("MONSTER"), "MONSTER");
    // `init.cpp`: conducts are loaded by the achievement loader, which stores
    // them in the achievement registry.
    assert_eq!(registry_for("conduct"), "achievement");
}

#[test]
fn the_id_member_is_per_registry() {
    assert_eq!(id_member("furniture"), "id");
    assert_eq!(id_member("item_category"), "id");
    // `generic_factory<monfaction> faction_factory( "MONSTER_FACTION", "name" )`.
    assert_eq!(id_member("MONSTER_FACTION"), "name");
}

#[test]
fn only_the_factories_that_insert_a_null_prototype_have_one() {
    assert_eq!(null_prototype("furniture"), Some("f_null"));
    assert_eq!(null_prototype("terrain"), Some("t_null"));
    assert_eq!(null_prototype("SPECIES"), Some("spec_null"));
    assert_eq!(null_prototype("item"), None);
    assert_eq!(null_prototype("MONSTER"), None);
}
