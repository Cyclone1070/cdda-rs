# Entity Map — Actors, Vehicles, World-Scale, Scripting

Repo: `/Users/mac/repos/cve/references/Cataclysm-DDA` @ tag `0.I-1`, commit `7b2efa5cea38e4d4d97dd0e63b28b9148623da59` (verified `git log -1 --format=%H`).
All paths below are relative to `src/`. Member counts are **measured** with a brace-matching extractor that strips comments/strings and joins multi-line declarations; they count data members only (functions, enums, `using`, static constants of non-state type excluded).

## Legend

`entity` = individually addressable, exists in world, can be pointed at · `prototype` = immutable shared data-loaded definition · `component` = plain data describing one aspect of an entity · `cache/index` = derived, rebuildable, never authoritative · `system` = logic only · `scaffolding` = build/IO/helper, no simulation meaning.

---

## 1. Core actor hierarchy

| Class | Decl | State held (grouped, counts) | Verdict | Port note |
|---|---|---|---|---|
| `viewer` | viewer.h:11 | no game state (perception interface) | **system** | Base of `Creature` (creature.h:255); stays a system/mixin. |
| `Creature` | creature.h:255 | identity/pos 2 (`location`:803, `facing`:308); body 1 (`body` map:814); effects 5 (`effects` pimpl:1234, `scheduled_effects`:1235, `terminating_effects`:1236, `damage_over_time_map`:1238, `values`:1241); combat bonuses 17 (`moves`:1223 … `throw_resist`:1266); ownership 3 (`killer`*:1224, `lifespan_end`:1226, `summoner` opt id:1227); counters 4 (`last_updated`:1268, `fake`:1270, `archery_aim_counter`:1285, `times_combatted_player`:1288); pain 1 (:1357); statics 6 | **mixed → split** | Split as below. |
| `Creature` → *state* half | creature.h:1223-1357 | 41 data members total | **entity** | No id of its own. See §4. |
| `Creature` → *statics* | creature.h:260,769-772,1304 | `size_map`, 4 `cmat_*` sets, `dispersion_for_even_chance_of_good_hit` | **prototype** (const lookup) | Static const tables; move to data registry. |
| `Character` | character.h:520 | 92 data members: attributes 9 (:561-576), needs/health 12, activity 6 (:2778-2782), inventory/bionics/MA 5 pimpls (:2783-2795), identity 1 (`id`:4124), caches 6, missions 3, misc | **mixed → split** | See split rows. |
| `Character` → *state* | character.h:520-4229 | 92 members (measured) | **entity** | Has `character_id`; the only actor class with one. |
| `Character` → *derived caches* | character.h:1414, 1224, 2829/3282 | `trait_flag_cache` map:1414; `cached_dead_state`:1224; `moncam_cache`:3282; `cached_organic_size`:582; `max_power_level_cached`:4128 | **cache/index** | All must be rebuilt on load; mark non-serialized. |
| `avatar` | avatar.h:77 | 24 members: missions 4 raw `mission*` (:395,399,403,407), memory 2 (:387,411), translocators 1 (:263), followers 1 (:375), aim cache 2 (:377,435), identity 1 (`save_id`:385), misc 13 | **entity** (singleton) | `save_id` string is a second, ad-hoc identity — unify. |
| `monster` | monster.h:80 | 54 members: hp/type 2 (`hp`:629, `type`*:566), AI 12 (:534-596), items 7 `value_ptr<item>` (:544-548), mount 2 (:541,542), missions 2 (:563,565), timers 5, path 3 (:644,647,648) | **entity** | **No id at all.** See §4. |
| `npc` | npc.h:756 | 59 members: id 2 (`idz`:1312, `unique_id`:1468), attitude/mission 4 (:1324,1325,1388,1389), goals 8 (:1345-1366), companion mission 7 (:1375-1387), social 6 (:1391-1397), caches 4 (:1330,1332,1393,1406), misc | **entity** | Two competing identities (`character_id` + `unique_id`). |
| `npc` → *caches* | npc.h:1330,1332,1393,1406 | `ai_cache` (20 fields, npc.h:527-571), `goal_cache`:1332, `mem_combat` (12 fields, npc.h:567-579), `confident_range_cache`:1406 | **cache/index** | Explicitly non-serialized; rebuild each turn. |
| `standard_npc` | npc.h:1472 | test/demo subclass | **scaffolding** | Drop from port. |
| `npc_template` | npc.h:1482 | data-loaded NPC preset | **prototype** | Loaded via `npc_template::load` npc.cpp:313. |
| `creature_tracker` | creature_tracker.h:24 | 7 members: `active_npc` list:147, `monsters_list`:148, `monsters_by_location`:150, `removed_this_turn_`:156, `dirty_`:161, `zone_tick_`:162, `zone_number_`:163, `creatures_by_zone_and_faction_`:164 | **cache/index** | Pure derived index over world entities. `rebuild_cache()` creature_tracker.cpp. |

---

## 2. Character sub-systems (stateful helpers held by value)

| Class | Decl | State (counts) | Verdict | Port note |
|---|---|---|---|---|
| `effects_map` | effect.h:258 | 0 own; inherits `map<efftype_id, map<bodypart_id, effect>>` | **component** (container) | Body-part-keyed effect store. |
| `effect` | effect.h:263 | 8: `eff_type`*:443, `duration`:444, `bp`:445, `permanent`:446, `intensity`:447, `start_time`:448, `source`:449, static `null_effect`:288 | **component** | Raw `const effect_type*` back-pointer to prototype. |
| `effect_type` | effect.h:104 | 46 | **prototype** | Stored in global `std::map<efftype_id, effect_type> effect_types` effect.cpp:58. |
| `bodypart` | bodypart.h:437 | 12: `id`:440, hp 2 (:442,443), wetness:445, temps 2 (:446,447), frostbite:448, heal 3 (:450-452), `encumb_data`:454, `mut_drench`:456 | **component** | `bodypart_str_id id` — part identity is the *type* id, not an instance id. |
| `body_part_type` | bodypart.h:168 | 64 | **prototype** | `generic_factory<body_part_type>` bodypart.cpp:101. |
| `anatomy` | anatomy.h:20 | 6: `unloaded_bps`:23, `cached_bps`:24, `size_sum`:26, `id`:32, `src`:33, `was_loaded`:34 | **mixed → split** | |
| `anatomy` → definition | anatomy.h:32-34 | id + src + was_loaded | **prototype** | `generic_factory<anatomy>` anatomy.cpp:33. |
| `anatomy` → resolved list | anatomy.h:23-26 | `cached_bps`, `size_sum` | **cache/index** | Int-id resolution of str-ids; rebuild on load. |
| `player_morale` | morale.h:20 | 10: `points`:162, `body_parts`:176, `no_body_part`:177, `mutations`:197, `level`/`level_is_valid` (mutable):200,201, prozac 2, `perceived_pain`:205, `radiation`:206 | **mixed → split** | |
| `player_morale` → inputs | morale.h:162-206 | points + modifiers | **component** | Belongs to Character as a component. |
| `player_morale` → `level` | morale.h:200-201 | mutable lazy scalar | **cache/index** | Explicit lazy-init cache. |
| `morale_point` | morale.h:79 | 7 | **component** | |
| `disease_type` | disease.h:17 | 10 | **prototype** | `generic_factory<disease_type>` disease.cpp:8. |
| `mutation_branch` | mutation.h:173 | 96 | **prototype** | `generic_factory<mutation_branch> trait_factory` mutation_data.cpp:52. |
| `mutation_variant`, `mut_attack`, `mut_transform`, `dream`, `mutation_category_trait` | mutation.h:152,58,82,41,514 | data-only payloads | **component** | Nested inside `mutation_branch` prototype. |
| `bionic` | bionics.h:195 | `bionic_id id`:200 (type), `uid`/`parent_uid`:255,256 (instance), `charge_timer`:201, `invlet`, `incapacitated_time` | **component** | **Has a real instance id** (`bionic_uid`, bionics.h:197) — the only component in scope that does. See §4. |
| `bionic_collection` | bionics.h:263 | 0 own; `public std::vector<bionic>` | **component** (container) | Held via `pimpl<bionic_collection>` character.h:2794. |
| `bionic_data` | bionics.h:30 | prototype fields | **prototype** | |
| `ma_technique` | martialarts.h:162 | 40 | **prototype** | `generic_factory<ma_technique>` martialarts.cpp:71. |
| `martialart` | martialarts.h:325 | 46 | **prototype** | `generic_factory<martialart>` martialarts.cpp:72. |
| `ma_buff` | martialarts.h:247 | 18 | **prototype** | |
| `weakpoint` | weakpoint.h:132 | 13 | **prototype** | Loaded via `weakpoints_factory` weakpoint.cpp:45. |
| `weakpoints` | weakpoint.h:174 | 5 | **prototype** (set) | `generic_factory<weakpoints>` weakpoint.cpp:45. |
| `weakpoint_families` | weakpoint.h:116 | 1 | **prototype** | |
| `known_magic` | magic.h:695 | 10: `spellbook`:699, `spells_to_invlets`:701, `invlets_to_spells`:703, `favorites`:705, mana 2 (:707,709), `casting_ignore`:713, caster adj 3 (:766-768) | **mixed → split** | |
| `known_magic` → spells/mana | magic.h:699,705-713 | authoritative | **component** | Component of Character. |
| `known_magic` → `invlets_to_spells` | magic.h:703 | reverse map of `spells_to_invlets`:701 | **cache/index** | Rebuilt by `build_invlets_to_spells()` magic.cpp:3193. |
| `spell` | magic.h:449 | 14: `type` id:454, `experience`:457, temp modifiers 11 (:462-472), `alt_message`:476 | **component** (instance) | Instance state; type is prototype. |
| `spell_type` | magic.h:191 | 122 | **prototype** | `generic_factory<spell_type> spell_factory` magic.cpp:278. |
| `enchantment` | magic_enchantment.h:160 | 26 | **prototype** | |
| `enchant_cache` | magic_enchantment.h:304 | 14 maps of accumulated modifiers | **cache/index** | Rebuilt by `Character::recalculate_enchantment_cache()` character.h:1579. |
| `relic` | relic.h:199 | 8 | **component** | Item-carried state. |
| `teleporter_list` | magic_teleporter_list.h:16 | 1 | **component** | Avatar-only. |

---

## 3. World-scale: vehicles, overmap, factions, missions, scripting

| Class | Decl | State (counts) | Verdict | Port note |
|---|---|---|---|---|
| `vehicle` | vehicle.h:818 | 120 members: parts 3 (`parts`:2243, `edges`:2247, `relative_parts`:2332); **part-index caches 26** (:2295-2321); motion 12 (:2379-2432); dirty flags 11 (:2438-2489); fuel 2 (:2337,2338); misc 60 | **mixed → split** | Largest offender; see split rows. |
| `vehicle` → *identity/parts/motion* | vehicle.h:2243, 2330, 2379-2409 | `parts`, `type`:2330, `sm_pos`:2379, `pos`:2392, `velocity`:2394, `om_id`:2406 | **entity** | `om_id` is an int assigned by the overmap — see §4. |
| `vehicle` → *part-index vectors* | vehicle.h:2295-2321 | 26 `std::vector<int>` | **cache/index** | Comment vehicle.h:2293-2294: "rebuilt in `vehicle::refresh()`"; `refresh()` clears them vehicle.cpp. |
| `vehicle` → *dirty/cached scalars* | vehicle.h:2229-2240, 2357-2366, 2438-2450 | `mass_cache`:2357, `pivot_cache`:2359, `occupied_points`:2240, `coefficient_*`:2229-2231, 8 `*_dirty` flags | **cache/index** | Guarded by `mutable bool *_dirty`. |
| `vehicle` → *effects/enchant* | vehicle.h:899, 901 | `enchantment_cache`:899, `effects`:901 | **component** / **cache** | Effects are state; enchant cache derived. |
| `vehicle_part` | vehicle.h:260 | 29 | **component** | Index-addressed within `vehicle::parts`. |
| `turret_data` / `turret_cpu` | vehicle.h:599 / :245 | 2 / 2 (`unique_ptr<npc> brain`:249) | **scaffolding** (view) / **component** | `turret_data` holds raw `vehicle*`+`vehicle_part*` (vehicle.h:695,696). |
| `vpart_info` | veh_type.h:241 | 58 | **prototype** | `friend class generic_factory<vpart_info>` veh_type.h:466. |
| `vehicle_prototype` | veh_type.h:487 | 8 | **prototype** | |
| `vehicle_group`, `VehiclePlacement`, `VehicleFunction*` | vehicle_group.h:32,85,106 | spawn tables | **prototype** | |
| `veh_interact` | veh_interact.h:55 | UI state; raw `vehicle* veh`:131, `const vehicle_part* sel_vehicle_part`:88 | **scaffolding** | Pure UI; do not port state. |
| `overmap` | overmap.h:244 | 26: layers 1 (`layer` array:431), `cities`:401, `camps`:400, `vehicles`:399, `npcs`:422, `zg` mongroups:391, `monster_map`:482, `radios`:398, `rivers`:402, `connections_out`:403, `scents`:432, `city_tiles`:388, `settings`*:458, `predecessors_`:444, `mapgen_arg_storage`:449 + `mapgen_args_index`:450 | **mixed → split** | |
| `overmap` → *authoritative world data* | overmap.h:388-482 | layers, cities, camps, vehicles, npcs, zg, monster_map, radios | **entity** (chunk aggregate) | Not individually addressable; found by `point_abs_om` key. |
| `overmap` → *derived indexes* | overmap.h:450, 444, 388 | `mapgen_args_index` (ptr into `mapgen_arg_storage`), `predecessors_`, `city_tiles` | **cache/index** | `mapgen_args_index[loc]` written overmap.cpp:7259. |
| `map_layer` | overmap.h:130 | 5: `terrain`, `visible`, `explored` mdarray + `notes`:134, `extras`:135 | **component** | |
| `om_note` / `om_map_extra` / `om_vehicle` / `radio_tower` | overmap.h:61 / 68 / 73 / 95 | 4 / 2 / 2 / 5 | **component** | `om_vehicle` has no id — keyed by `std::map<int, om_vehicle>` overmap.h:399. |
| `overmapbuffer` | overmapbuffer.h:151 | 8: `overmaps` map of `unique_ptr<overmap>`:581, `known_non_existing`:586, `last_requested_overmap`*:588, `placed_unique_specials`:590, `unique_special_count`:593, counts 2 (:595,597) | **mixed → split** | |
| `overmapbuffer` → *ownership/registry* | overmapbuffer.h:581 | `unordered_map<point_abs_om, unique_ptr<overmap>>` | **system** (world store) | Sole owner of overmap chunks. |
| `overmapbuffer` → *lookup caches* | overmapbuffer.h:586-597 | `known_non_existing`, `last_requested_overmap`, counts | **cache/index** | |
| `city` | city.h:16 | 8: `id`:25, `was_loaded`:26, `database_id`:28, `pos_om`:30, `pos`:32, `population`:34, `size`:35, `name`:36 | **mixed → split** | Loaded from JSON *and* pushed at worldgen (overmap.cpp:6147). |
| `city` → *prototype fields* | city.h:25-28 | `id`, `was_loaded`, `database_id` | **prototype** | `generic_factory<city> city_factory` city.cpp:14-17. |
| `city` → *placement* | city.h:30-36 | `pos_om`, `pos`, `population`, `size`, `name` | **entity** | Stored by value in `overmap::cities` — pointer-unstable. |
| `mongroup` | mongroup.h:121 | 10: `type`:123, `monsters` (by value!):129, `abs_pos`:134, `population`:139, `target`:142, `nemesis_target`:144, `interest`:146, `dying`:149, `horde`:151, `behaviour`:160 | **entity** | Stores `std::vector<monster>` **by value** — monsters owned by group, not tracker. |
| `MonsterGroup` / `MonsterGroupManager` | mongroup.h:93 / 238 | 10 / 7 | **prototype** / **system** | Manager holds spawn tables. |
| `faction` | faction.h:170 | 3: `opinion_of`:195, `validated`:196, `members` map:197 | **entity** | `faction_id id` faction.h:149 lives in `faction_template` base. |
| `faction_template` | faction.h:117 | 18 | **prototype** | |
| `faction_manager` | faction.h:200 | 1: `std::map<faction_id, faction> factions` | **system** (registry) | Owns factions by value → pointer-unstable. |
| `mission` | mission.h:271 | 22: `type`*:288, `uid`:295, targets 6, ids 4, `deadline`:319, status:289 | **entity** | Has `int uid` from `g->assign_mission_id()` mission.cpp:50. |
| `mission_type` | mission.h:175 | 19 | **prototype** | `generic_factory<mission_type>` missiondef.cpp:267. |
| `basecamp` | basecamp.h:157 | 20: `omt_pos`:504, `bb_pos`:507, `expansions`:508, `assigned_npcs`:505, `camp_map`:510, `_inv`:520, `salt_water_pipes` raw ptrs:200, resources 3, misc | **entity** | Identified only by `omt_pos` (basecamp.h:174). |
| `basecamp_map` | basecamp.h:146 | 1: `unique_ptr<map> map_`:150 | **scaffolding** (map handle) | Owning pointer to a sub-map. |
| `dialogue` | dialogue.h:277 | 10 | **scaffolding** (transient) | Conversation context; not persisted. |
| `talk_topic` / `talk_response` / `json_talk_topic` | dialogue.h:98 / 169 / 503 | 1 / — / — | **prototype** | JSON dialogue graph. |
| `dialogue_chatbin` | dialogue_chatbin.h:15 | NPC conversation state | **component** | Member of npc npc.h:1395. |
| `conditional_t` | condition.h:80 | 1: `std::function<bool(const_dialogue&)> condition`:99 | **prototype** (compiled predicate) | Holds a closure, not game state. |
| `effect_on_condition` | effect_on_condition.h:49 | 13 | **prototype** | |
| `player_activity` | player_activity.h:33 | 24: `type`:36, `actor` clone_ptr:37, `targets` item_locations:62, `monsters` weak_ptrs:67, `coords`:65, `values`:63, `str_values`:64, progress 5 | **component** | `clone_ptr<activity_actor>` owns polymorphic behavior. |
| `activity_actor` (+ 61 subclasses in activity_actor_definitions.h) | activity_actor.h:18 | 0 own members (pure interface) | **system** | Stateless base; state lives in `player_activity`. |
| `mapgen_palette` / `mapgen_function_json` / `jmapgen_*` | mapgen.h:292 / 477 / 194 (+31 subclasses mapgen.cpp) | palette 6, json fn 4, `jmapgen_piece` 1 (`repeat`:217) | **prototype** | Loaded mapgen definitions. |
| `mapgendata` | mapgendata.h:115 | 25 | **scaffolding** (per-call context) | Holds `::mission *mission_`:122. |
| `regional_settings` | regional_settings.h:323 | 18 | **prototype** | `overmap::settings` is a raw `const regional_settings*` overmap.h:458. |
| `map_extra` | map_extras.h:42 | 9 | **prototype** | |

---

## 4. Identity — how each entity is found today

| Entity | Identity today | Evidence | What a stable id would replace |
|---|---|---|---|
| `Character` (npc/avatar) | `character_id` = thin `int` wrapper, `-1` = invalid | character_id.h:9-35; member character.h:4124 | Already an id, but **not stable across saves**: `next_npc_id` is reset to 1 on new game (game.cpp:805) and assigned by counter (game.cpp:4485-4489). Replace with UUID + persisted allocator. |
| `npc` lookup | Linear scan of `overmap::npcs` comparing `getID()` | overmap.cpp `find_npc` loops `for( const auto &guy : npcs ) if( guy->getID() == id )` | O(n) scan per lookup; needs a map index `character_id → npc*`. |
| `npc` (second identity) | `std::string unique_id` | npc.h:1468; `find_npc_by_unique_id` overmap.cpp | Duplicate identity path — two ways to name the same object. |
| `monster` | **none** — addressed only by pointer or tracker slot | monster.h:80-680 has no id member (measured 54 members, none is an id) | Biggest gap. `creature_tracker::temporary_id()` returns the *vector index* (creature_tracker.cpp) and is documented valid only "until monsters are added or removed". |
| `monster` cross-references | `character_id mounted_player_id`, `character_id dragged_foe_id` | monster.h:542,543 | Partial ids already exist for character links only. |
| `vehicle` | `int om_id`, assigned as `om->vehicles.size() + 1` with a collision loop | vehicle.h:2406; overmapbuffer.cpp:783-791 | Id is **per-overmap and derived from container size** — collides after removal, not globally unique. |
| `vehicle` (persisted) | `std::map<int, om_vehicle>` keyed by that int, storing only `p` + `name` | overmap.h:399, om_vehicle overmap.h:73-76 | Overmap keeps a shadow record with no back-link; position must be kept in sync manually (overmapbuffer.cpp:744-746). |
| `city` | `city_id` (= `string_id<city>`) + `int database_id` + position | city.h:25,28; type_id.h:53 | `operator==` accepts **any** of id/database_id/position as identity (city.h:45-48) — ambiguous. |
| `basecamp` | `tripoint_abs_omt omt_pos` only | basecamp.h:174-176, member :504 | No id; camp is found by `get_camps_near(location, radius)` overmapbuffer.h:511. Moving a camp breaks identity. |
| `mission` | `int uid` from `g->assign_mission_id()` | mission.h:295; mission.cpp:50 | Counter-based, game-session scoped. |
| `faction` | `faction_id` (`string_id`) | faction.h:149 | Already stable string id — good model. |
| `mongroup` | none; stored as multimap value keyed by position | overmap.h:391 `std::multimap<tripoint_om_sm, mongroup> zg` | Groups have no handle; hordes cannot be referred to individually. |
| `effect` | none — keyed by `(efftype_id, bodypart_id)` in a nested map | effect.h:258-261, `effects` creature.h:1234 | Two effects of same type on same limb cannot coexist; no per-instance handle. |
| `bodypart` | `bodypart_str_id id` — the *type* id, not an instance id | bodypart.h:440 | Works only because there is ≤1 instance per type per creature. |
| `spell` | `spell_id type` — type only | magic.h:454 | Two castings of one spell share identity. |
| `bionic` | `bionic_uid uid` + `parent_uid` (unsigned int) | bionics.h:197,255,256; `find_bionic_by_uid()` bionics.cpp:2178 | **Best-existing model in scope**: a per-instance counter id, resolved by linear search over the collection. Port as a first-class id, not a counter. |

---

## 5. Cross-reference inventory (who points at whom)

**Raw pointers — non-owning, no lifetime guarantee**

| From | Member | To | Evidence |
|---|---|---|---|
| `Creature` | `killer` | `Creature` | creature.h:1224 |
| `Creature` | `summoner` (optional id) | `character_id` | creature.h:1227 |
| `monster` | `mounted_player` | `Character` | monster.h:541 |
| `monster` | `type` | `const mtype` (prototype) | monster.h:566 |
| `npc` | `known_stolen_item` | `item` | npc.h:1364 |
| `Character` | `prof` | `const profession` | character.h:584 |
| `Character` | `hobbies` set | `const profession` | character.h:585 |
| `effect` | `eff_type` | `const effect_type` (prototype) | effect.h:443 |
| `mission` | `type` | `const mission_type` (prototype) | mission.h:288 |
| `overmap` | `settings` | `const regional_settings` | overmap.h:458 |
| `overmapbuffer` | `last_requested_overmap` | `overmap` (owned elsewhere) | overmapbuffer.h:588 |
| `overmapbuffer::city_reference` | `city` | `struct city` inside `overmap::cities` | overmapbuffer.h:85 |
| `overmapbuffer::camp_reference` | `camp` | `basecamp` inside `overmap::camps` | overmapbuffer.h:99 |
| `vehicle::towing_data` | `towing`, `towed_by` | `vehicle` (both directions) | vehicle.h:202,203 |
| `turret_data` | `veh`, `part` | `vehicle`, `vehicle_part` | vehicle.h:695,696 |
| `veh_interact` | `veh`, `sel_vehicle_part`, `sel_vpart_info` | `vehicle`, `vehicle_part`, `vpart_info` | veh_interact.h:88,89,131 |
| `basecamp` | `salt_water_pipes` | `expansion_salt_water_pipe` | basecamp.h:200 |
| `mapgendata` | `mission_` | `::mission` | mapgendata.h:122 |
| `follower_rules_ui_impl` | `guy` | `npc` | npctalk_rules.h:37 |
| `avatar` | `active_mission`, `active_missions`, `completed_missions`, `failed_missions` | `mission` | avatar.h:395,399,403,407 |

**Smart pointers — owning**

| From | Member | To | Kind | Evidence |
|---|---|---|---|---|
| `creature_tracker` | `monsters_list` | `monster` | `shared_ptr_fast` | creature_tracker.h:148 |
| `creature_tracker` | `monsters_by_location` | `monster` | `shared_ptr_fast` (2nd owner) | creature_tracker.h:150 |
| `creature_tracker` | `active_npc` | `npc` | `shared_ptr_fast` | creature_tracker.h:147 |
| `creature_tracker` | `removed_this_turn_` | `monster` | `shared_ptr_fast` (keeps dead alive) | creature_tracker.h:156 |
| `creature_tracker` | `creatures_by_zone_and_faction_` | `Creature` | `shared_ptr_fast` (3rd owner) | creature_tracker.h:164 |
| `overmap` | `npcs` | `npc` | `shared_ptr_fast` (2nd owner) | overmap.h:422 |
| `overmapbuffer` | `overmaps` | `overmap` | `unique_ptr` (sole owner) | overmapbuffer.h:581 |
| `avatar` | `shadow_npc` | `npc` | `unique_ptr` | avatar.h:432 |
| `avatar` | `player_map_memory`, `a_diary` | `map_memory`, `diary` | `unique_ptr` | avatar.h:387,411 |
| `vehicle::turret_cpu` | `brain` | `npc` | `unique_ptr` | vehicle.h:249 |
| `vehicle` | `active_autodrive_controller` | `autodrive_controller` | `shared_ptr` | vehicle.h:2371 |
| `player_activity` | `actor` | `activity_actor` | `clone_ptr` (deep copy) | player_activity.h:37 |
| `basecamp` | `camp_map` | `map` | `unique_ptr` | basecamp.h:150 |
| `mapgen_palette` | palette entries | `const jmapgen_piece` | `shared_ptr_fast` | mapgen.h:303 |

**Weak pointers — cycle-breakers**

| From | Member | To | Evidence |
|---|---|---|---|
| `npc` | `chosen_mount` | `monster` | npc.h:781 |
| `Character` | `last_target` | `Creature` | character.h:2834 |
| `player_activity` | `monsters` | `monster` | player_activity.h:67 |
| `npc_short_term_cache` | `target`, `ally`, `hostile_guys`, `neutral_guys`, `friends` | `Creature` | npc.h:532,534,552,553,554 (comment npc.h:531 "avoid circular references") |

**Value-embedded entities (no pointer, no handle)**

| Owner | Member | Embedded | Evidence |
|---|---|---|---|
| `overmap` | `cities` | `std::vector<city>` by value | overmap.h:401 |
| `overmap` | `camps` | `std::vector<basecamp>` by value | overmap.h:400 |
| `overmap` | `zg` | `std::multimap<tripoint_om_sm, mongroup>` by value | overmap.h:391 |
| `overmap` | `monster_map` | `std::unordered_multimap<tripoint_om_sm, monster>` by value | overmap.h:482 |
| `mongroup` | `monsters` | `std::vector<monster>` by value | mongroup.h:129 |
| `Creature` | `body` | `std::map<bodypart_str_id, bodypart>` by value | creature.h:814 |
| `Creature` | `effects` | `pimpl<effects_map>` by value | creature.h:1234 |

---

## 6. Derived indexes that must be rebuilt

| Index | Owner | Rebuild trigger | Evidence |
|---|---|---|---|
| `monsters_by_location` | `creature_tracker` | `rebuild_cache()` on add/remove/move | creature_tracker.h:150; creature_tracker.cpp |
| `creatures_by_zone_and_faction_` | `creature_tracker` | `dirty_ = true` via `invalidate_reachability_cache()` | creature_tracker.h:133,161-165 |
| 26 part-index vectors | `vehicle` | `vehicle::refresh()` clears then repopulates | vehicle.h:2293-2321; vehicle.cpp `refresh()` |
| `mass_cache`, `pivot_cache`, `occupied_points`, `coefficient_*` | `vehicle` | `*_dirty` flags | vehicle.h:2357-2366, 2438-2450 |
| `enchantment_cache` | `Character` | `recalculate_enchantment_cache()` | character.h:1579, 933 |
| `trait_flag_cache` | `Character` | `.clear()` on mutation change | character.h:1414; character.cpp:636, 7931 |
| `moncam_cache`, `cached_dead_state`, `cached_organic_size` | `Character` | recompute on demand | character.h:3282,1224,582 |
| `ai_cache`, `goal_cache`, `mem_combat`, `confident_range_cache` | `npc` | per-turn AI refresh (`regen_ai_cache()` npc.h:991) | npc.h:1330,1332,1393,1406 |
| `invlets_to_spells` | `known_magic` | `build_invlets_to_spells()` | magic.h:703,788; magic.cpp:3193 |
| `level`, `level_is_valid` | `player_morale` | lazy recompute | morale.h:200-201 |
| `mapgen_args_index` | `overmap` | rewritten when special placed | overmap.h:450; overmap.cpp:7259 |
| `predecessors_`, `city_tiles` | `overmap` | worldgen / city growth | overmap.h:444,388 |
| `cached_bps`, `size_sum` | `anatomy` | str-id → int-id resolution | anatomy.h:23-26 |

---

## 7. Ambiguity list

| Class | Why unclear | Reading that would settle it |
|---|---|---|
| `city` | Simultaneously a JSON prototype (`generic_factory<city>` city.cpp:14-17) and a worldgen-created placement (overmap.cpp:6147). | Whether `city::get_all()` instances are ever mutated after load, and whether worldgen cities bypass the factory. |
| `mongroup` | Holds `std::vector<monster>` **by value** (mongroup.h:129) yet is also a spawn descriptor — entity and prototype in one. | How `overmap::zg` groups transfer monsters into `creature_tracker` (overmap.cpp:4546-4576) and whether embedded monsters are ever individually addressed. |
| `basecamp` | An entity with heavy mutable state, but with no id and stored by value in a `std::vector` (overmap.h:400) that reallocates. | Whether anything persists a `basecamp*` across turns; `overmapbuffer::camp_reference` (overmapbuffer.h:99) suggests yes. |
| `vehicle` | `om_id` (vehicle.h:2406) looks like identity but is a per-overmap container-size counter (overmapbuffer.cpp:783-791). Whether it is truly the vehicle's identity or just a cache key is unresolved. | Whether any code compares two vehicles by `om_id`, or relies on it across overmap boundaries. |
| `overmap` | Entity vs. chunk aggregate: it owns `cities`, `camps`, `vehicles`, `npcs` but is itself only addressable by `point_abs_om`. | Whether gameplay code ever treats an overmap as a single world object vs. a spatial container. |
| `activity_actor` | Base class holds 0 data (activity_actor.h:18-125) but its 61 subclasses hold the actual activity state; `player_activity` also holds parallel `targets`/`coords`/`values` (player_activity.h:62-66). | Whether subclass state or `player_activity` fields are authoritative — read one full actor (e.g. `craft_activity_actor` activity_actor_definitions.h:1147). |
| `bionic` | Resolved: has both a type id (`bionic_id`) and an instance `uid` (bionics.h:200,255), but is stored by value in a `std::vector` (bionics.h:263) so the uid is the only stable handle. | Confirmed by `find_bionic_by_uid()` bionics.cpp:2178 — no ambiguity remains, listed for the port because it is the identity model to copy. |
| `npc` identity | Two ids coexist (`character_id` from base + `unique_id` npc.h:1468) with separate lookup functions. | Which one survives save/load round-trips and which is regenerated. |
| `effect` | Has `source` (:449) and `start_time` (:448) — instance-ish — but is stored in a map keyed by type+limb, so it has no handle. | Whether any code needs to refer to a specific effect instance later (e.g. for removal by handle). |

---

## 8. Files read fully vs. skimmed

**Read fully:** `character_id.h`, `creature_tracker.h`, `city.h`, `viewer.h` (class region), `condition.h` (struct region), `activity_actor.h` (class region), `bodypart.h` (bodypart/body_part_type regions), `effect.h` (effects_map/effect regions), `bionics.h` (bionic/bionic_collection regions), `npc.h` (cache structs 527-600, member block 1311-1468), `creature.h` (member blocks 1221-1372, 795-980), `basecamp.h` (identity accessors), `overmap.h` (61-170, 380-490).

**Skimmed (via measured extraction + targeted grep only):** `character.h` (4239 lines), `vehicle.h` (2500+), `overmap.cpp`, `magic.h`, `mutation.h`, `martialarts.h`, `veh_type.h`, `mission.h`, `dialogue.h`, `mapgen.cpp`, `regional_settings.h`, `mapgendata.h`, `monster.h` body, `avatar.h` body, `bionics.cpp`.

**Not read:** `suffer.cpp`, `needs` (no dedicated file — needs live in `character.h`), `wound` (no dedicated file), `projectile.h`/`dispersion.h`/`ranged.h`/`melee.h` beyond class extraction, `anatomy.cpp`, `faction_camp.*`, `mission_companion.*`, `overmap_connection/location/noise`, `vehicle_autodrive.cpp`, `vehicle_display.cpp`, `veh_appliance.*`, `veh_shape.*`.
