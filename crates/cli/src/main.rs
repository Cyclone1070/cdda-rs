//! The headless game: a turn loop with no window, and a way to look at what it did.
//!
//! This is the QA and vibe-coding surface. It runs the same store and the same schedule the game
//! will, so a scenario can be played, dumped and diffed without a screen:
//!
//! ```text
//! cdda-cli play --turns 10 --seed 42      run ten turns and print what each one changed
//! cdda-cli store                          what the world holds, by kind
//! cdda-cli square 5 5 0                   everything standing on one square
//! cdda-cli index                          whether the index agrees with the store
//! ```
//!
//! A command that cannot be understood prints the usage and exits with status 2, so a script can
//! tell "you asked for something odd" from "the world is broken".

use bevy_ecs::prelude::*;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        usage();
        std::process::exit(2);
    };
    let rest: Vec<String> = args.collect();
    match command.as_str() {
        "play" => play(&rest[..]),
        "store" => {
            let mut world = World::new();
            load_demo_world(&mut world);
            print!("{}", cdda_sim::inspect::store_report(&world));
        }
        "square" => square(&rest[..]),
        "cast" => cast(&rest[..]),
        "craft" => craft(&rest[..]),
        "playtest" => playtest(&rest[..]),
        "shot" => shot(&rest[..]),
        "qa" => qa(),
        "index" => index(),
        other => {
            eprintln!("unknown command `{other}`");
            usage();
            std::process::exit(2);
        }
    }
}

fn usage() {
    eprintln!(
        "cdda-cli — the headless game\n\
         \n\
         commands:\n\
         \x20 play [--turns N] [--seed S]   run turns and report what each one changed\n\
         \x20 cast --spell <id>               cast a spell at what is next to the avatar\n\
         \x20 store                          what the world holds, by kind\n\
         \x20 square X Y Z                   everything standing on one square\n\
         \x20 index                          whether the index agrees with the store\n\
         \x20 shot FILE [--radius N]          draw the view around the fire and write it as a PNG\n\
         \x20 qa                             run every check and print the table"
    );
}

/// A world to run in: one submap of floor, which is the smallest thing that is a level.
fn load_demo_world(world: &mut World) -> &mut World {
    let mut level = cdda_core_types::level::Level::new();
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(world, &level);
    // A floor that feeds and a fire on it, so a turn has something to do: an empty world takes turns
    // that change nothing, which is a poor way to see whether the loop is wired up.
    for y in 0..12 {
        for x in 0..12 {
            cdda_sim::fire_facts::set_fire_facts(
                world,
                (x, y, 0),
                cdda_sim::TerrainFacts {
                    flammable: true,
                    ..cdda_sim::TerrainFacts::default()
                },
            );
        }
    }
    let mut entry = cdda_core_types::field::FieldEntry::new(3, 1_000_000, false, true);
    entry.set_age(500);
    entry.set_intensity(2);
    world.spawn((
        cdda_sim::Field {
            id: cdda_sim::FIRE,
            entry,
            priority: 0,
        },
        cdda_sim::Position { x: 6, y: 6, z: 0 },
    ));
    cdda_sim::PositionIndex::rebuild(world);
    world
}

fn play(args: &[String]) {
    let turns = number(args, "--turns").unwrap_or(10);
    let seed = number(args, "--seed").unwrap_or(1);
    let mut world = World::new();
    load_demo_world(&mut world);
    let avatar = give_the_avatar_a_body(&mut world);
    match cdda_cli::install_item_data(&mut world) {
        Some(kinds) => println!("the store knows {kinds} kinds of thing"),
        None => println!("the store knows no things: the game data was not found"),
    }
    match cdda_cli::install_field_data(&mut world) {
        Some(fields) => println!("the store knows {fields} kinds of field"),
        None => println!("the store knows no fields: the game data was not found"),
    }
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(seed));
    cdda_sim::turn::set_scent_source(&mut world, (6, 6), 0);
    match &avatar {
        Some(body) => println!("the avatar has {body} parts of its body"),
        None => println!("the avatar has no body: the corpus did not describe one"),
    }
    let me = {
        let mut query = world.query_filtered::<Entity, With<cdda_sim::Player>>();
        query.iter(&world).next()
    };
    if let Some(me) = me {
        match cdda_cli::dress_the_avatar(&mut world, me) {
            Some(worn) => println!("the avatar wears {worn}"),
            None => println!("the avatar wears nothing: the game data was not found"),
        }
    }
    for turn in 1..=turns {
        let before = cdda_sim::inspect::world_fingerprints(&world);
        cdda_sim::turn::run_turn(&mut world);
        let after = cdda_sim::inspect::world_fingerprints(&world);
        let changed = cdda_sim::inspect::squares_that_differ(&before, &after);
        println!(
            "turn {turn}: {} square(s) changed{}",
            changed.len(),
            if changed.is_empty() {
                String::new()
            } else {
                format!(", first at {changed:?}")
            }
        );
    }
}

/// Puts the avatar in the demo world with the body the corpus describes.
///
/// The reference gives a character its anatomy once, at load (`anatomy::finalize`), and every later rule - which
/// part a blow lands on, what armour covers it - reads that. A port that only gave bodies to tests would have a game
/// where a person is an unarmoured point, so the demo world builds one from the same two corpus files the gates use.
///
/// Returns the number of parts, or nothing when the corpus is not where the binary expects it.
fn give_the_avatar_a_body(world: &mut World) -> Option<usize> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let anatomies_text = std::fs::read_to_string(root.join("anatomy-parts.json")).ok()?;
    let parts_text = std::fs::read_to_string(root.join("anatomy.json")).ok()?;
    let cdda_json::Value::Array(anatomies) = cdda_json::parse(anatomies_text.as_bytes()).ok()? else {
        return None;
    };
    let parts_document = cdda_json::parse(parts_text.as_bytes()).ok()?;
    let cdda_json::Value::Object(root_object) = parts_document else {
        return None;
    };
    let Some(cdda_json::Value::Array(parts)) = root_object.get("parts") else {
        return None;
    };
    let anatomies = cdda_content::anatomies::load_objects(&anatomies).ok()?;
    let parts = cdda_content::body_parts::load_objects(parts).ok()?;
    let body = cdda_sim::creature::body_named("human_anatomy", &anatomies, &parts)?;

    let avatar = cdda_sim::spawn_player(world, (0, 0), 80, 0);
    let count = body.parts.len();
    world.entity_mut(avatar).insert(body);
    Some(count)
}

/// Casts a spell from the game's own data at whatever is nearest the avatar.
///
/// A QA verb rather than a rule: the rule is `cdda_sim::spellcasting`, and this exists so a spell can be exercised
/// from a terminal the way `play` exercises a turn.
fn cast(args: &[String]) {
    let spell = flag(args, "--spell").unwrap_or_else(|| "bio_taser_spell".to_owned());
    #[allow(clippy::cast_possible_wrap)]
    let level = flag(args, "--level")
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(0);
    let mut world = World::new();
    load_demo_world(&mut world);
    let avatar = cdda_sim::spawn_player(&mut world, (6, 6), 80, 0);
    let torso = cdda_sim::hit_location::BodyPart {
        name: "torso".to_owned(),
        hit_size: 36.0,
        hit_difficulty: 1.0,
        upper: false,
        lower: false,
    };
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(1));
    let zombie = cdda_sim::spawn_monster(&mut world, (7, 6), cdda_sim::MonsterKindId(0), 30);
    world
        .entity_mut(zombie)
        .insert(cdda_sim::creature::Body { parts: vec![torso] });
    match cdda_cli::cast_at_nearest(&mut world, avatar, &spell, level) {
        Some(said) => println!("{said}"),
        None => println!("no such spell, or nothing in reach"),
    }
}

fn craft(args: &[String]) {
    let recipe = args.first().cloned().unwrap_or_default();
    let turns = i32::try_from(number(args, "--turns").unwrap_or(1_000)).unwrap_or(i32::MAX);
    let mut world = World::new();
    load_demo_world(&mut world);
    cdda_cli::install_item_data(&mut world);
    cdda_cli::install_field_data(&mut world);
    let me = {
        let mut query = world.query_filtered::<Entity, With<cdda_sim::Player>>();
        query.iter(&world).next()
    };
    match me.and_then(|me| cdda_cli::craft_a_recipe(&mut world, me, &recipe, turns)) {
        Some(line) => println!("{line}"),
        None => println!("no recipe named {recipe}, or no survivor to make it"),
    }
}

fn playtest(args: &[String]) {
    let Some(path) = args.first() else {
        eprintln!("playtest needs a file");
        std::process::exit(2);
    };
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("cannot read `{path}`: {error}");
            std::process::exit(2);
        }
    };
    match cdda_cli::run(&text) {
        Ok(outcome) => {
            for line in &outcome.lines {
                println!("{line}");
            }
            for failure in &outcome.failures {
                println!("FAILED: {failure}");
            }
            println!(
                "{}\n",
                if outcome.passed() {
                    "the scenario passed"
                } else {
                    "the scenario failed"
                }
            );
            std::process::exit(i32::from(!outcome.passed()));
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}

fn qa() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut world = World::new();
    load_demo_world(&mut world);
    let checks = vec![
        cdda_cli::qa::corpus(&root.join("corpus")),
        cdda_cli::qa::scenarios(&root.join("scenarios")),
        cdda_cli::qa::oracle_sessions(),
        cdda_cli::qa::index_matches_the_store(&mut world),
    ];
    print!("{}", cdda_cli::qa::report(&checks));
    let green = checks.iter().all(cdda_cli::qa::Check::green);
    println!(
        "\n{}",
        if green {
            "all checks green"
        } else {
            "something is not green"
        }
    );
    std::process::exit(i32::from(!green));
}

fn shot(args: &[String]) {
    let Some(path) = args.first() else {
        eprintln!("shot needs a file to write");
        std::process::exit(2);
    };
    let radius = number(args.get(1..).unwrap_or(&[]), "--radius").unwrap_or(15);
    let mut world = World::new();
    load_demo_world(&mut world);
    let radius = i32::try_from(radius).unwrap_or(15);
    cdda_sim::turn::run_turn(&mut world);
    let capture = cdda_cli::capture::draw(&mut world, (6, 6), radius);
    if let Err(error) = std::fs::write(path, capture.image.png()) {
        eprintln!("cannot write `{path}`: {error}");
        std::process::exit(2);
    }
    println!(
        "wrote {path}: {} by {} pixels, {} squares across, {} of them lit",
        capture.image.width,
        capture.image.height,
        capture.squares_across,
        capture.lit_squares
    );
}

fn square(args: &[String]) {
    let mut numbers = args.iter().filter_map(|arg| arg.parse::<i32>().ok());
    let (Some(x), Some(y), z) = (numbers.next(), numbers.next(), numbers.next().unwrap_or(0)) else {
        eprintln!("square needs X and Y, and takes Z");
        std::process::exit(2);
    };
    let mut world = World::new();
    load_demo_world(&mut world);
    print!("{}", cdda_sim::inspect::square_report(&world, (x, y, z)));
}

fn index() {
    let mut world = World::new();
    load_demo_world(&mut world);
    let positioned: Vec<(Entity, (i32, i32, i32))> = world
        .query::<(Entity, &cdda_sim::Position)>()
        .iter(&world)
        .map(|(entity, position)| (entity, (position.x, position.y, position.z)))
        .collect();
    let mut missing = 0;
    for (entity, square) in &positioned {
        if !cdda_sim::singletons::index(&world)
            .is_some_and(|index| index.at_level(square.0, square.1, square.2).contains(entity))
        {
            missing += 1;
        }
    }
    println!(
        "{} positioned thing(s), {missing} of them not found where they stand",
        positioned.len()
    );
    std::process::exit(i32::from(missing != 0));
}

/// Reads `--name value` from a command line, answering `None` when it is absent or unreadable.
/// The word after a flag, if the flag is there.
fn flag(args: &[String], name: &str) -> Option<String> {
    let mut found = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == name {
            found = rest.next().cloned();
        }
    }
    found
}

fn number(args: &[String], name: &str) -> Option<u32> {
    let mut found = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == name {
            found = rest.next().and_then(|value| value.parse().ok());
        }
    }
    found
}
