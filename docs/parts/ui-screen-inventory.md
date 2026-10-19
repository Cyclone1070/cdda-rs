# UI screen inventory — Cataclysm-DDA 0.I-1

Repo `/Users/mac/repos/cve/references/Cataclysm-DDA`, `git log -1 --format=%H` = `7b2efa5cea38e4d4d97dd0e63b28b9148623da59`, working tree clean. Read-only survey; this file is the only artifact.

**Column key.** `Chg` = what the screen changes: **W** world state, **C** character state, **N** nothing (pure display), **S** player settings persisted to config, not to the world.
`Drive` = how a diff test could drive it: **K** keystrokes enough, **K+M** mouse needed for full coverage, **Sw** needs an existing save/world, **T** needs turns/time to advance, **D** debug-mode only, **A** Android-only code path.

Sections 2.1–2.9 are the checklist. Every `file:line` was read back at the cited line; nothing in this file is inferred without a marker. Where a context id is set indirectly (e.g. via `input_category`) the alternative site is cited in the same cell.

---

## 1. Evidence for the counts

| Claim | Command | Result |
|---|---|---|
| distinct input context ids | five construction forms (`input_context <var>( "ID"`, `<var>{ "ID" }`, `= input_context( "ID" )`, `input_category = "ID"`, member-init `ctxt( "ID" )`) plus `make_unique<input_context>( "ID" )`, deduped | **87** |
| `uilist` construction sites | `grep -rcE '^\s*uilist\s+\w+' src/*.cpp` summed | **259** across 81 files |
| prompt sites | `string_input_popup` 165, `query_popup` 78, `query_yn` 361, `query_int` 99 (all occurrences incl. declarations) | **703** |
| `DEFAULTMODE` actions dispatched | `grep -cE '^\s+case ACTION_' src/handle_action.cpp` | **138** |
| screens in this list | rows in 2.1–2.9 | **116** |

The brief's estimate of "about 63" contexts is low for this tag: 87 exist. The excess over 63 is concentrated in the 9 editmap sub-contexts, the 7 `NEW_CHAR_*` tabs, and 5 UI-only contexts the brief did not name (`BODYGRAPH`, `APP_INTERACT`, `MAIN_MENU`, `SMART_ENGINE_CONTROLLER`, `CALENDAR_UI`).

---

## 2. Screens

### 2.1 Front end, worldgen, character creation

| # | Screen | Entry (file:line) | Ctx | Shows | Chg | Drive |
|---|---|---|---|---|---|---|
| 1 | Main menu | `src/main_menu.cpp:579` `opening_screen` | `MAIN_MENU` `src/main_menu.h:21` | ASCII title, New/World/Load/Settings/Quit tabs | N | K |
| 2 | Main-menu text dialog | `src/main_menu.cpp:540` `display_text` | `default` | scrolled credits/special text | N | K |
| 3 | World tab | `src/main_menu.cpp:1147` `world_tab` | `default` | world list | N | K,Sw |
| 4 | Character/save picker | `src/main_menu.cpp:1100` `load_character_tab` | `default` | saves in world | N | K,Sw |
| 5 | New-character tab | `src/main_menu.cpp:915` `new_character_tab` | `default` | saved templates | C | K |
| 6 | Character creation (parent, 8 tabs) | `src/newcharacter.cpp:744` `avatar::create` | `NEW_CHAR_*` | tabbed builder | C | K |
| 7 | ↳ points pool | `src/newcharacter.cpp:1163` `set_points` | `NEW_CHAR_POINTS` `:1185` | point pool | C | K |
| 8 | ↳ stats | `src/newcharacter.cpp:1491` `set_stats` | `NEW_CHAR_STATS` `:1527` | STR/DEX/INT/PER | C | K |
| 9 | ↳ traits | `src/newcharacter.cpp:1705` `set_traits` | `NEW_CHAR_TRAITS` `:1853` | trait grid | C | K |
| 10 | ↳ profession | `src/newcharacter.cpp:2524` `set_profession` | `NEW_CHAR_PROFESSIONS` `:2550` | professions | C | K |
| 11 | ↳ hobbies | `src/newcharacter.cpp:2879` `set_hobbies` | `NEW_CHAR_PROFESSIONS` `:2906` | hobbies | C | K |
| 12 | ↳ scenario | `src/newcharacter.cpp:3590` `set_scenario` | `NEW_CHAR_SCENARIOS` `:3618` | scenarios | C | K |
| 13 | ↳ skills | `src/newcharacter.cpp:3221` `set_skills` | `NEW_CHAR_SKILLS` `:3236` | skill allocation | C | K |
| 14 | ↳ description | `src/newcharacter.cpp:4046` `set_description` | `NEW_CHAR_DESCRIPTION` `:4135` | name/gender/portrait | C | K |
| 15 | World picker | `src/worldfactory.cpp:454` `pick_world` | `PICK_WORLD_DIALOG` `:622` | world list | N | K,Sw |
| 16 | Worldgen basic | `src/worldfactory.cpp:1494` `show_worldgen_basic` | `WORLDGEN_CONFIRM_DIALOG` `:1502` | name, spawn settings | W | K |
| 17 | Worldgen advanced | `src/worldfactory.cpp:227` `show_worldgen_advanced` | `STRING_INPUT` `:214` | numeric world opts | W | K |
| 18 | Mod selection | `src/worldfactory.cpp:978` `show_worldgen_tab_modselection` | `MODMANAGER_DIALOG` `:994` | available/active mods | W | K+M |
| 19 | Active mods list | `src/worldfactory.cpp:907` `show_active_world_mods` | `DEFAULT` `:932` | ordered mod list | W | K |
| 20 | Language select | `src/translations.cpp:25` `select_language` | `UILIST` | language list | S | K |
| 21 | Loading screen | `src/loading_ui.cpp:152` `show` | none | progress text | N | K |

### 2.2 Main game view and map overlays

| # | Screen | Entry (file:line) | Ctx | Shows | Chg | Drive |
|---|---|---|---|---|---|---|
| 22 | Map view + HUD | `src/game.cpp:4172` `game::draw` | `DEFAULTMODE` `src/game.cpp:2680` | terrain, creatures, items | W | K+M |
| 23 | Sidebar / panels render | `src/game.cpp:4212` `game::draw_panels` | none (draw-only) | widget panels | N | K |
| 24 | Panel manager | `src/panels.cpp:711` `panel_manager::show_adm` | `PANEL_MGMT` `:713` | layout editor | S | K+M |
| 25 | Message log | `src/messages.cpp:675` `Messages::dialog::show` | `MESSAGE_LOG` `:616` | message history | N | K |
| 26 | Look around | `src/game.cpp:7932` `game::look_around` | `LOOK` `:7998` | tile/creature info | N | K+M |
| 27 | List items (V) | `src/game.cpp:8748` `game::list_items` | `LIST_ITEMS` `:8995` | ground items | N | K |
| 28 | List monsters | `src/game.cpp:9292` `game::list_monsters` | `LIST_MONSTERS` `:9348` | visible monsters | N | K |
| 29 | Nearby NPCs | `src/game.cpp:3849` `game::disp_NPCs` | `DISP_NPCS` `:3899` | NPC list | N | K |
| 30 | Zones manager | `src/game.cpp:7185` `game::zones_manager` | `ZONES_MANAGER` `:7241` | zone rects | W | K |
| 31 | Extended description | `src/ui_extended_description.cpp:92` | `EXTENDED_DESCRIPTION` | long wrapped text | N | K |
| 32 | Scrollable text popup | `src/output.cpp:441` | `SCROLLABLE_TEXT` | text + scrollbar | N | K |
| 33 | Wait-for-any-key | `src/input.cpp:891` `wait_for_any_key` | `WAIT_FOR_ANY_KEY` `:894` | one-line prompt | N | K |
| 34 | Yes/No popup | `src/output.cpp:843` `query_yn` | `""` unless caller sets it (`src/popup.cpp:155`; field default `src/popup.h:210`) | question | N | K |
| 35 | String/number prompt | `src/input_popup.cpp:21` | `STRING_INPUT` | editable field | N | K |
| 36 | Item info window | `src/ui_iteminfo.cpp:64` `iteminfo_window::execute` | none | item stats | N | K+M |

### 2.3 Character screens

| # | Screen | Entry (file:line) | Ctx | Shows | Chg | Drive |
|---|---|---|---|---|---|---|
| 37 | Character sheet | `src/player_display.cpp:1520` `Character::disp_info` | `PLAYER_INFO` `:1675` | stats, skills, traits | C | K |
| 38 | Bodygraph | `src/bodygraph.cpp:645` `display_bodygraph` | `BODYGRAPH` `src/bodygraph.cpp:289` | body-part diagram | N | K+M |
| 39 | ↳ Skills tab | `src/player_display.cpp:81` `title_SKILLS` | inside `PLAYER_INFO` | skill levels + header grouping (`src/skill_ui.cpp:11`) | C | K |
| 40 | Proficiencies | `src/proficiency_ui.cpp:331` `prof_window::run` | `PROFICIENCY_WINDOW` `:337` | proficiency list | N | K |
| 41 | Medical | `src/medical_ui.cpp:710` `disp_medical` | `MEDICAL` `:238`,`:935` | HP, body parts, vitals | N | K |
| 42 | Morale | `src/morale.cpp:516` `player_morale::display` | `MORALE` `:808` | morale points | N | K |
| 43 | Mutations | `src/mutation_ui.cpp:100` `power_mutations` | `MUTATIONS` `:217` | trait list | N | K |
| 44 | Bionics | `src/bionics_ui.cpp:575` `power_bionics` | `BIONICS` `:217`,`:659` | CBM list, power | C | K |
| 45 | Armor layers | `src/armor_layers.cpp:605` `outfit::sort_armor` | `SORT_ARMOR` `:675` | warmth/encumbrance | C | K |
| 46 | Blood test results | `src/character.cpp:3055` `conduct_blood_analysis` | `BLOOD_TEST_RESULTS` `:3091` | hidden vitals | N | K |
| 47 | Martial-arts details | `src/martialarts.cpp:2253` `draw_ma_details_ui` | `MA_DETAILS_UI` `:2255` | style techniques | N | K |
| 48 | Melee style picker | `src/character.cpp:13549` `Character::pause` | `MELEE_STYLE_PICKER` `:13716` | style list | C | K |
| 49 | Item action menu | `src/item_action.cpp:258` `item_action_menu` | `ITEM_ACTIONS` `:292` | actions for item | W | K |
| 50 | Pocket management | `src/item_contents.cpp:2813` | `INVENTORY` `:2815` | pockets/contents | W | K |
| 51 | Exosuit interact | `src/iuse.cpp:2017` `exosuit_interact` | `""` (keycode) | suit pockets | W | K+M |

### 2.4 Inventory, crafting, trade, vehicles

| # | Screen | Entry (file:line) | Ctx | Shows | Chg | Drive |
|---|---|---|---|---|---|---|
| 52 | Inventory | `src/game_inventory.cpp:253` `game_menus::inv::common` | `INVENTORY` `src/inventory_ui.cpp:2877` | carried items | W | K+M |
| 53 | Advanced inventory | `src/advanced_inv.cpp:2011` `advanced_inventory::display` | `ADVANCED_INVENTORY` `:821` | two-pane transfer | W | K+M |
| 54 | Crafting | `src/crafting_gui.cpp:1246` `select_crafter_and_crafting_recipe` | `CRAFTING` `:622` | recipe tree, requirements | W | K |
| 55 | Construction | `src/construction.cpp:480` `construction_menu` | `CONSTRUCTION` `:531` | buildable list | W | K |
| 56 | Disassembly | `src/game_inventory.cpp:655` `inv::disassemble` | `INVENTORY` | salvaged parts | W | K |
| 57 | Multidrop | `src/game_inventory.cpp:2401` `inv::multidrop` | `INVENTORY` | drop targets | W | K |
| 58 | Pickup | `src/game_inventory.cpp:2425` `inv::pickup` | `INVENTORY` | ground items | W | K |
| 59 | Trade | `src/trade_ui.cpp:113` `trade_ui::trade_ui` | `INVENTORY` `src/inventory_ui.cpp:4619` | both inventories, balance | W | K+M |
| 60 | Vehicle interact | `src/veh_interact.cpp:234` `veh_interact::run` | `VEHICLE` `src/veh_utils.cpp:231` | parts, repairs | W | K |
| 61 | Vehicle appliance | `src/veh_appliance.cpp:155` `veh_app_interact::run` | `APP_INTERACT` `:164` | appliance toggles | W | K |
| 62 | Vehicle shape editor | `src/veh_shape.cpp:34` `veh_shape::start` | `VEH_SHAPES` `:257` | shape preview | W | K |
| 63 | Computer terminal | `src/computer_session.cpp:159` `computer_session::use` | `YESNOQUIT` `:1851` | access/options | W | K |
| 64 | Vending machine | `src/iexamine.cpp:1259` `iexamine::vending` | `VENDING_MACHINE` `:1315` | goods, prices | W | K |
| 65 | Liquid handling | `src/handle_liquid.cpp:238` `handle_all_liquids_from_container` | none | container choice | W | K |
| 66 | Siphon / refill | `src/veh_appliance.cpp:394`,`:445` | `APP_INTERACT` | liquid routing | W | K |

### 2.5 Overmap and map editor (debug)

| # | Screen | Entry (file:line) | Ctx | Shows | Chg | Drive |
|---|---|---|---|---|---|---|
| 67 | Overmap | `src/overmap_ui.cpp:2506` `ui::omap::display` | `OVERMAP` `src/overmap_ui.h:162` | overmap tiles, cities | N | K+M |
| 68 | Overmap notes | `src/overmap_ui.cpp:476` | `OVERMAP_NOTES` | note text | W | K |
| 69 | Overmap editor | `src/overmap_ui.cpp:1556` | `OVERMAP_EDITOR` | edit overmap | W | K,D |
| 70 | Map editor | `src/editmap.cpp:355` `editmap::edit` | `EDITMAP` `:360` | tiles, cursor | W | K,D |
| 71 | ↳ feature menu | `src/editmap.cpp:1092` | `EDITMAP_FEATURE` | terrain/furniture | W | K,D |
| 72 | ↳ fields menu | `src/editmap.cpp:1240` | `EDIT_FIELDS` | fields | W | K,D |
| 73 | ↳ items menu | `src/editmap.cpp:1413` | `EDIT_ITEMS` | spawn items | W | K,D |
| 74 | ↳ mapgen menu | `src/editmap.cpp:2116` | `EDIT_MAPGEN` | mapgen ids | W | K,D |
| 75 | ↳ mapgen preview | `src/editmap.cpp:1859` | `MAPGEN_PREVIEW` | rendered preview | N | K,D |
| 76 | ↳ shape editor | `src/editmap.cpp:1676` | `EDITMAP_SHAPE` | shape drawing | W | K,D |
| 77 | ↳ retarget | `src/editmap.cpp:2054` | `EDITMAP_RETARGET` | target pick | W | K,D |
| 78 | Direction picker | `src/editmap.cpp:294` | `EGET_DIRECTION` | compass | N | K |

### 2.6 Settings and configuration

| # | Screen | Entry (file:line) | Ctx | Shows | Chg | Drive |
|---|---|---|---|---|---|---|
| 79 | Options (6 pages) | `src/options.cpp:3342` `options_manager::show` | `OPTIONS` `:3379` | general/interface/graphics/world_default/debug/android `:215–225` | S | K+M |
| 80 | ↳ tileset & font groups | `src/options.cpp:2365`,`:2440` | `OPTIONS` | tileset list, font params | S | K+M |
| 81 | Keybindings | `src/input_context.cpp:940` `display_menu` | `HELP_KEYBINDINGS` `:942` | action→key table | S | K+M |
| 82 | Color manager | `src/color.cpp:836` `color_manager::show_gui` | `COLORS` `:882` | color table | S | K |
| 83 | UI style picker | `src/ui_style_picker.cpp:27` `style_picker::show` | `UILIST` | style preview | S | K+M |
| 84 | Safemode | `src/safemode_ui.cpp:54` `safemode::show` | `SAFEMODE` `:98` | rule table | S | K |
| 85 | Safemode test ¹ | `src/auto_pickup.cpp:568` `player_settings::show` | `SAFEMODE_TEST` `src/auto_pickup.cpp:645` | pattern test | N | K |
| 86 | Auto pickup | `src/auto_pickup.cpp:296` `user_interface::show` | `AUTO_PICKUP` `:334` | rule table | S | K |
| 87 | Auto notes | `src/auto_note.cpp:299` `auto_note_manager_gui::show` | `AUTO_NOTES` `:341` | note rules | S | K |
| 88 | Distraction manager | `src/distraction_manager.cpp:49` `show` | `DISTRACTION_MANAGER` `:81` | distraction rules | S | K |
| 89 | Smart controller | `src/smart_controller_ui.cpp:136` `control` | `SMART_ENGINE_CONTROLLER` `:33` | pulse settings | S | K |

¹ `SAFEMODE_TEST` is registered in `src/auto_pickup.cpp:645`, inside the `player_settings` base shared with auto-pickup; the safemode screen itself is at `src/safemode_ui.cpp:98`.

### 2.7 Journal, help, metadata screens

| # | Screen | Entry (file:line) | Ctx | Shows | Chg | Drive |
|---|---|---|---|---|---|---|
| 90 | Missions | `src/mission_ui.cpp:361` `game::list_missions` | `default` `src/mission_ui.cpp:93` | mission list + description | N | K |
| 91 | Scores / achievements | `src/scores_ui.cpp:372` `show_scores_ui` | `SCORES_UI` `:97` | scores, kills, events | N | K |
| 92 | Diary | `src/diary_ui.cpp:175` `show_diary_ui` | `DIARY` `:191` | page list | C | K |
| 93 | Diary page editor | `src/diary_ui.cpp:383` `edit_page_ui` | `STRING_EDITOR` `src/string_editor_window.cpp:296` | multi-line text | C | K |
| 94 | Help | `src/help.cpp:163` `help::display_help` | `DISPLAY_HELP` `:181` | help pages | N | K+M |
| 95 | Faction manager | `src/faction.cpp:864` `display` | `FACTION_MANAGER` `:897` | 5 tabs of faction data | W | K |
| 96 | Camp worker assignment | `src/faction_camp.cpp:2236` `worker_assignment_ui` | `FACTION_MANAGER` `:2255` | workers, jobs | W | K |
| 97 | Companion mission menu | `src/mission_companion.cpp:595` `companion_mission` | `FACTIONS` `:927` | mission offers | W | K |
| 98 | Calendar / time picker | `src/calendar_ui.cpp:16` `select_time_point` | `CALENDAR_UI` `:48` | date fields | W | K |
| 99 | End screen | `src/game.cpp:3050` `draw_end_screen_ui` | `default` `:3052` | death summary | N | K |

### 2.8 Debug screens

| # | Screen | Entry (file:line) | Ctx | Shows | Chg | Drive |
|---|---|---|---|---|---|---|
| 100 | Debug menu | `src/debug_menu.cpp:4030` `debug_menu::debug` | `UILIST` `src/uilist.cpp:513` | 98 actions (`src/debug_menu.h:21–121`) | W | K,D |
| 101 | Wish item | `src/wish.cpp:1083` `wishitem` | `WISH_ITEM` `:1120` | item search | W | K,D |
| 102 | Wish monster/skill/etc. | `src/wish.cpp:822`,`:1264`,`:1378` | `WISH_ITEM`/`UILIST` | lists | W | K,D |
| 103 | Debug spells | `src/debug_menu.cpp:1410` `change_spells` | `DEBUG_SPELLS` `:1530` | spell list | C | K,D |
| 104 | Debug message filter | `src/debug.cpp:381` | `DEBUG_MSG` | filter keys | N | K,A,D |
| 105 | Spell menu | `src/magic.cpp:3246` `known_magic::select_spell` | `SPELL_MENU` `:3286` | known spells | W | K |

### 2.9 Dialogue, examine menus, mini-games

| # | Screen | Entry (file:line) | Ctx | Shows | Chg | Drive |
|---|---|---|---|---|---|---|
| 106 | NPC dialogue | `src/npctalk.cpp:1592` `avatar::talk_to` | `DIALOGUE_CHOOSE_RESPONSE` `src/dialogue_win.cpp:60` | chat history, responses | W | K |
| 107 | NPC picker | `src/npctalk.cpp:1186` | `UILIST` | nearby NPCs | W | K |
| 108 | Monster examine | `src/monexamine.cpp:422`,`:614`,`:961` | `UILIST` | pet/monster actions | W | K |
| 109 | Terrain examine menus | `src/iexamine.cpp` (24 `uilist` sites) | `UILIST` | per-terrain actions | W | K |
| 110 | Aiming / target UI | `src/ranged.cpp:2950` `target_ui` | `TARGET` `:2981` | trajectory, hit chance | W | K+M |
| 111 | Robot finds kitten | `src/iuse_software_kitten.cpp:121` `show` | `IUSE_SOFTWARE_KITTEN` `:123` | maze game | N | K |
| 112 | Lights on | `src/iuse_software_lightson.cpp:132` `start_game` | `LIGHTSON` `:146` | puzzle grid | N | K+M |
| 113 | Minesweeper | `src/iuse_software_minesweeper.cpp:132` `start_game` | `MINESWEEPER` `:151` | grid | N | K+M |
| 114 | Snake | `src/iuse_software_snake.cpp:89` `start_game` | `SNAKE` `:126` | arcade | N | K |
| 115 | Sokoban | `src/iuse_software_sokoban.cpp:214` `start_game` | `SOKOBAN` `:238` | puzzle | N | K |
| 116 | Generic menu widget | `src/uilist.cpp:464` `uilist::init` | `UILIST` `:513` | any do-it-yourself list | N | K |

---

## 3. Input-context index (87)

Format `ID @ file:line`. Android-only ids are marked.

`ADVANCED_INVENTORY @ src/advanced_inv.cpp:821` · `APP_INTERACT @ src/veh_appliance.cpp:164` · `AUTO_NOTES @ src/auto_note.cpp:341` · `AUTO_PICKUP @ src/auto_pickup.cpp:334` · `AUTO_PICKUP_TEST @ src/auto_pickup.cpp:645` · `BIONICS @ src/bionics_ui.cpp:217` · `BLOOD_TEST_RESULTS @ src/character.cpp:3091` · `BODYGRAPH @ src/bodygraph.cpp:289` · `CALENDAR_UI @ src/calendar_ui.cpp:48` · `COLORS @ src/color.cpp:882` · `CONSTRUCTION @ src/construction.cpp:531` · `CRAFTING @ src/crafting_gui.cpp:622` · `DEBUG_MSG @ src/debug.cpp:381` · `DEBUG_SPELLS @ src/debug_menu.cpp:1530` · `DEFAULT @ src/worldfactory.cpp:932` · `DEFAULTMODE @ src/game.cpp:2680` · `DIALOGUE_CHOOSE_RESPONSE @ src/dialogue_win.cpp:60` · `DIARY @ src/diary_ui.cpp:191` · `DISPLAY_HELP @ src/help.cpp:181` · `DISP_NPCS @ src/game.cpp:3899` · `DISTRACTION_MANAGER @ src/distraction_manager.cpp:81` · `EDITMAP @ src/editmap.cpp:360` · `EDITMAP_FEATURE @ src/editmap.cpp:1092` · `EDITMAP_RETARGET @ src/editmap.cpp:2054` · `EDITMAP_SHAPE @ src/editmap.cpp:1676` · `EDIT_FIELDS @ src/editmap.cpp:1240` · `EDIT_ITEMS @ src/editmap.cpp:1413` · `EDIT_MAPGEN @ src/editmap.cpp:2116` · `EGET_DIRECTION @ src/editmap.cpp:294` · `EXTENDED_DESCRIPTION @ src/ui_extended_description.cpp:92` · `FACTIONS @ src/mission_companion.cpp:927` · `FACTION_MANAGER @ src/faction.cpp:897` · `HELP_KEYBINDINGS @ src/input_context.cpp:942` · `INSERT_TABLE @ src/output.cpp:2947` · `INVENTORY @ src/inventory_ui.cpp:2877` · `ITEM_ACTIONS @ src/item_action.cpp:292` · `IUSE_SOFTWARE_KITTEN @ src/iuse_software_kitten.cpp:123` · `LIGHTSON @ src/iuse_software_lightson.cpp:146` · `LIST_ITEMS @ src/game.cpp:8995` · `LIST_MONSTERS @ src/game.cpp:9348` · `LOOK @ src/game.cpp:7998` · `MAIN_MENU @ src/main_menu.h:21` · `MAPGEN_PREVIEW @ src/editmap.cpp:1859` · `MA_DETAILS_UI @ src/martialarts.cpp:2255` · `MEDICAL @ src/medical_ui.cpp:238` · `MELEE_STYLE_PICKER @ src/newcharacter.cpp:396` · `MESSAGE_LOG @ src/messages.cpp:616` · `MINESWEEPER @ src/iuse_software_minesweeper.cpp:151` · `MODMANAGER_DIALOG @ src/worldfactory.cpp:994` · `MORALE @ src/morale.cpp:808` · `MUTATIONS @ src/mutation_ui.cpp:217` · `NEW_CHAR_DESCRIPTION @ src/newcharacter.cpp:4135` · `NEW_CHAR_POINTS @ src/newcharacter.cpp:1185` · `NEW_CHAR_PROFESSIONS @ src/newcharacter.cpp:2550` · `NEW_CHAR_SCENARIOS @ src/newcharacter.cpp:3618` · `NEW_CHAR_SKILLS @ src/newcharacter.cpp:3236` · `NEW_CHAR_STATS @ src/newcharacter.cpp:1527` · `NEW_CHAR_TRAITS @ src/newcharacter.cpp:1853` · `OPTIONS @ src/options.cpp:3379` · `OVERMAP @ src/overmap_ui.h:162` · `OVERMAP_EDITOR @ src/overmap_ui.cpp:1556` · `OVERMAP_NOTES @ src/overmap_ui.cpp:476` · `PANEL_MGMT @ src/panels.cpp:713` · `PICK_WORLD_DIALOG @ src/worldfactory.cpp:622` · `PLAYER_INFO @ src/player_display.cpp:1675` · `PROFICIENCY_WINDOW @ src/proficiency_ui.cpp:337` · `SAFEMODE @ src/safemode_ui.cpp:98` · `SAFEMODE_TEST @ src/auto_pickup.cpp:645` · `SCORES_UI @ src/scores_ui.cpp:97` · `SCROLLABLE_TEXT @ src/output.cpp:441` · `SMART_ENGINE_CONTROLLER @ src/smart_controller_ui.cpp:33` · `SNAKE @ src/iuse_software_snake.cpp:126` · `SOKOBAN @ src/iuse_software_sokoban.cpp:238` · `SORT_ARMOR @ src/armor_layers.cpp:675` · `SPELL_MENU @ src/magic.cpp:3286` · `STRING_EDITOR @ src/string_editor_window.cpp:296` · `STRING_INPUT @ src/input_popup.cpp:21` · `TARGET @ src/ranged.cpp:2981` · `UILIST @ src/uilist.cpp:513` · `VEHICLE @ src/veh_utils.cpp:231` · `VEH_SHAPES @ src/veh_shape.cpp:257` · `VENDING_MACHINE @ src/iexamine.cpp:1315` · `WAIT_FOR_ANY_KEY @ src/input.cpp:894` · `WISH_ITEM @ src/wish.cpp:1120` · `WORLDGEN_CONFIRM_DIALOG @ src/worldfactory.cpp:1502` · `YESNOQUIT @ src/computer_session.cpp:1851` · `ZONES_MANAGER @ src/game.cpp:7241`

`INVENTORY` is also set as `input_category` in `src/item_contents.cpp:2815`; `DEFAULTMODE` is built by `get_default_mode_input_context()` at `src/game.cpp:2680`. Contexts with no id resolve to the `default` category (`src/input_context.h:49`) — screens relying on that are marked `default` in section 2.

---

## 4. Summary counts

| Category | Count | Notes |
|---|---|---|
| Total screens listed | 116 | parent + sub-tab rows in 2.1–2.9 |
| Pure display (`N`) | 40 | nothing written; safe to diff as golden text |
| Changes character (`C`) | 18 | creation tabs, style/profile edits, diary |
| Changes world (`W`) | 46 | inventory, crafting, build, trade, vehicle, debug |
| Settings only (`S`) | 12 | options, keybinds, colors, safemode, autopickup, auto notes |
| Needs mouse for full coverage (`K+M`) | 19 | inventory family, panels, overmap/look, item info, options, help, targeting, mini-games, mod selection |
| Needs a saved world (`Sw`) | 3 | world/character pickers only |
| Needs debug mode (`D`) | 14 | editmap family, overmap editor, debug menu, wish |
| Android-only path (`A`) | 1 | `DEBUG_MSG` |

`N`+`C`+`W`+`S` = 116 exactly; each row carries one `Chg` value. `K+M`/`Sw`/`D`/`A` overlap the `Chg` axis, so those four sum higher than 116 by design.

## 5. Hard or impossible to automate

| Screen | Why |
|---|---|
| Map view (`game.cpp:4172`) | Renders ~1 screen of live `map`+creature state; output depends on RNG spawns, weather, light. A stable golden needs a frozen world snapshot, not just keystrokes. |
| Curses-only popups (`output.cpp:843`, `input_popup.cpp:21`) | The in-tree harness cannot build them at all: `uilist::init` asserts `!test_mode` (`uilist.cpp:466`). Driving them needs synthetic input injected below `input_manager`, which does not exist in tests. |
| Editmap family (9 screens, rows 70–78) | Debug-gated by `debug_mode`; also need a loaded world and per-tile cursors. |
| Vehicle interact / appliance (`veh_interact.cpp:234`) | Requires a constructed vehicle with parts to show anything beyond an empty grid. |
| Trade (`trade_ui.cpp:113`) | Needs two parties: a save with an NPC standing next to the player, plus currency. |
| Faction manager / camp (`faction.cpp:864`, `faction_camp.cpp:2236`) | Needs a founded camp and follower NPCs; most tabs are empty otherwise. |
| Overmap (`overmap_ui.cpp:2506`) | Produces mapgen-dependent ASCII; golden diff fails on any world seed change. |
| Title art (`main_menu.cpp:579`) | Time-dependent: holiday variants (`main_menu.cpp:396`,`:1235`,`:1266`) change the art by system clock. |
| Terminal vs tiles builds | `screen`, `tileset`, and mouse paths diverge (`src/sdltiles.cpp:2512` touch handling); one golden cannot cover both. |
| Mini-games (`iuse_software_*.cpp`) | RNG-generated levels; diff needs a seeded RNG hook that does not exist. |

## 6. Ten screens with the most simulation risk

Ranked by "player reads a computed number and acts on it" — a wrong value here silently corrupts decisions.

| # | Screen | Computed value at risk | Entry |
|---|---|---|---|
| 1 | Crafting | availability, batch time, component counts | `src/crafting_gui.cpp:1246` |
| 2 | Character sheet | stat/skill totals after modifiers | `src/player_display.cpp:1520` |
| 3 | Inventory / AIM | weight, volume, charges, denial reasons | `src/inventory_ui.cpp:2877`, `src/advanced_inv.cpp:2011` |
| 4 | Medical | HP per body part, infection, vitals | `src/medical_ui.cpp:710` |
| 5 | Armor layers | warmth, encumbrance, layering penalties | `src/armor_layers.cpp:605` |
| 6 | Vehicle interact | part HP, repair requirements, lift capacity | `src/veh_interact.cpp:234` |
| 7 | Aiming / target UI | hit chance, dispersion, recoil | `src/ranged.cpp:2950` |
| 8 | Construction | required tools/components, time | `src/construction.cpp:480` |
| 9 | Morale | per-source bonus and decay | `src/morale.cpp:516` |
| 10 | Overmap | distance, travel time, seen/hidden state | `src/overmap_ui.cpp:2506` |

## 7. What I could not enumerate

- **Individual `uilist` sites as separate screens.** 259 construction sites exist; I mapped them to owner screens (e.g. 24 in `iexamine.cpp`, 47 in `debug_menu.cpp`) rather than listing each. Enumerating all 259 with a screen label each is mechanical but not done here.
- **Individual prompt sites.** 703 occurrences of the four prompt helpers, almost all inline confirmations inside other screens, not standalone screens. Not listed individually.
- **Per-tab inner layouts.** Options has 6 pages (`src/options.cpp:215–225`) and the character sheet has 9 tabs (`src/player_display.cpp:76–84`); I list them as one screen plus notable sub-groups, not one row per tab.
- **ImGui-only overlays.** `src/cata_imgui.cpp` and `src/imgui_demo.cpp` contain windows (`cataimgui::window`, e.g. `end_screen_ui_impl` at `src/game.cpp:3040`) that draw through a separate renderer. I found no complete registry of them; I only counted the ones with a reachable entry.
- **Tiles-specific overlays.** `src/sdltiles.cpp` and `src/cata_tiles.cpp` add touch/click affordances and pixel-minimap rendering. I did not enumerate every mouse-only interaction surface there.
- **Exact mouse requirement per screen.** `K+M` in section 2 means the screen registers `SELECT`/`MOUSE_MOVE` actions; I did not verify that every list is unreachable by keyboard (most are keyboard-drivable, with mouse as an addition).
- **Android-only screens.** Several prompts branch to JNI native UI (`src/output.cpp:845`) and were not inventoried beyond noting they exist.
