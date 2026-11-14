//! The ammo effect registry, and the refusal that comes with it.
//!
//! `corpus/items/ammo-effect-registry.json` is `--dump-ammo-effects-registry`: the 82 effects the content defines. An ammunition's `"effects"` are typed ids, so the
//! reference refuses one it does not know (`ammo_effect_str_id`, `src/ammo_effect.h:16-32`) — and the port has to refuse it too, or a typo becomes "no effect",
//! which for a round means no fire, no smoke, and no fence to stop it.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::ammo_effects;
use cdda_json::Value;
use std::collections::BTreeSet;

#[test]
fn the_registry_holds_every_effect_the_content_defines() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/items/ammo-effect-registry.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let Value::Object(root) = cdda_json::parse(&bytes).expect("valid json") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(effects)) = root.get("effects") else {
        panic!("the dump holds effects")
    };
    let recorded: BTreeSet<String> = effects
        .iter()
        .map(|effect| {
            let Value::Object(effect) = effect else {
                panic!("an effect")
            };
            match effect.get("id") {
                Some(Value::Str(id)) => id.clone(),
                other => panic!("an effect id: {other:?}"),
            }
        })
        .collect();
    assert_eq!(recorded.len(), 82, "the content defines 82 effects");

    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let ours = ammo_effects::load_paths(&paths).expect("the effects load");
    assert_eq!(ours, recorded, "the port holds the same 82 effects");
}

#[test]
fn the_one_effect_id_the_content_uses_without_defining_it_is_still_accepted() {
    // Four ids appear among ammunition effects without an `ammo_effect` definition, and **all four are flags**: `rock` lists `ALLOWS_BODY_BLOCK`,
    // `HURT_WHEN_WIELDED`, and `STREAM_GAS_INSECTICIDAL`, and the 0.I obsoletion migration extends `CASELESS_ROUNDS` onto defunct 20x66 rounds
    // (`data/json/flags.json:183` defines it as a flag). The reference loads every one of them without complaint, because an `ammo_effect_str_id` is a string and
    // nothing validates it at load (`src/ammo_effect.h:16-32`). A port that refused unknown ids would therefore be **stricter than the reference** and would not
    // load the content at all, so the gate pins the slack rather than wishing it away.
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let known = ammo_effects::load_paths(&paths).expect("the effects load");
    let items =
        cdda_content::items::load_paths(&paths).expect("the items load, unknown effect and all");
    let unknown: BTreeSet<String> = items
        .iter()
        .flat_map(|item| item.ammo_effects.iter().cloned())
        .filter(|effect| !known.contains(effect))
        .collect();
    assert_eq!(
        unknown,
        [
            "ALLOWS_BODY_BLOCK",
            "CASELESS_ROUNDS",
            "HURT_WHEN_WIELDED",
            "STREAM_GAS_INSECTICIDAL"
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        "exactly four flag names sit among the content's ammunition effects, and they load anyway"
    );
}
