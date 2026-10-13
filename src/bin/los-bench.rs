//! Line-of-sight and perception micro-benchmark: a seeded battlefield of hills, woods, water,
//! buildings and smoke with units of both chassis on two teams, timing raw terrain traces and
//! full contact refreshes so changes to the LOS and perception code can be compared.

use clap::Parser;
use std::path::Path;
use std::time::{Duration, Instant};
use stompymux_rs::{
    Config, HexCoordinate, Kind, MapAsset, ObjectId, Power, Terrain, UnitTemplate, UnitTemplateExt,
    World, battle_contact_observers, battle_hex_visible, battle_unit_terrain_los,
    create_battle_map, persistence, place_battle_unit, refresh_battle_contacts,
    visible_battle_contacts,
};

#[derive(Debug, Parser)]
#[command(
    about = "Time terrain line-of-sight traces and contact refreshes on a seeded battlefield"
)]
struct Args {
    /// Units placed on the map, alternating chassis and team.
    #[arg(long, default_value_t = 60)]
    units: usize,
    /// Map width and height in hexes.
    #[arg(long, default_value_t = 60)]
    size: u16,
    /// Contact refreshes to time after one warmup refresh.
    #[arg(long, default_value_t = 20)]
    refreshes: usize,
    /// Passes over every ordered unit pair for raw terrain traces.
    #[arg(long, default_value_t = 3)]
    passes: usize,
    /// Terrain and placement seed.
    #[arg(long, default_value_t = 0x1055_5eed)]
    seed: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let root = copy_game_root()?;
    let config = Config::load(root.path())?;
    let mut world = persistence::load(&config.database()).await?;
    let (map, units) = build_battlefield(&config, &mut world, &args)?;

    // Raw terrain traces over every ordered pair.
    let pairs: Vec<(ObjectId, ObjectId)> = units
        .iter()
        .flat_map(|&a| units.iter().map(move |&b| (a, b)))
        .filter(|(a, b)| a != b)
        .collect();
    let mut blocked = 0_usize;
    let started = Instant::now();
    for _ in 0..args.passes {
        for &(a, b) in &pairs {
            if battle_unit_terrain_los(&world, a, b)?.blocked {
                blocked += 1;
            }
        }
    }
    let traces = pairs.len() * args.passes;
    let trace_time = started.elapsed();

    // Hex visibility from every unit to a sweep of hexes, as hex shots and artillery ask it.
    let targets: Vec<_> = (0..args.size)
        .step_by(3)
        .flat_map(|x| (0..args.size).step_by(3).map(move |y| (x, y)))
        .collect();
    let started = Instant::now();
    let mut hex_visible = 0_usize;
    for &unit in &units {
        for &(x, y) in &targets {
            let coordinate = HexCoordinate {
                x: i32::from(x),
                y: i32::from(y),
            };
            if battle_hex_visible(&world, unit, coordinate)? {
                hex_visible += 1;
            }
        }
    }
    let hex_traces = units.len() * targets.len();
    let hex_time = started.elapsed();

    // Full perception refreshes, as the server heartbeat runs them.
    let observers = battle_contact_observers(&world);
    refresh_battle_contacts(&mut world, &observers)?;
    let mut samples = Vec::with_capacity(args.refreshes);
    for _ in 0..args.refreshes {
        let started = Instant::now();
        refresh_battle_contacts(&mut world, &observers)?;
        samples.push(started.elapsed());
    }
    samples.sort();
    let contacts: usize = units
        .iter()
        .map(|&id| visible_battle_contacts(&world, id).map_or(0, |c| c.len()))
        .sum();

    println!("metric,value");
    println!("map,{}", map.0);
    println!("units,{}", units.len());
    println!("unit_traces,{traces}");
    println!("unit_trace_blocked,{blocked}");
    println!(
        "unit_trace_us,{:.3}",
        trace_time.as_secs_f64() * 1e6 / traces as f64
    );
    println!("hex_traces,{hex_traces}");
    println!("hex_trace_visible,{hex_visible}");
    println!(
        "hex_trace_us,{:.3}",
        hex_time.as_secs_f64() * 1e6 / hex_traces as f64
    );
    println!("refreshes,{}", samples.len());
    println!(
        "refresh_p50_ms,{:.3}",
        percentile(&samples, 50).as_secs_f64() * 1e3
    );
    println!(
        "refresh_p95_ms,{:.3}",
        percentile(&samples, 95).as_secs_f64() * 1e3
    );
    println!(
        "refresh_max_ms,{:.3}",
        samples.last().copied().unwrap_or_default().as_secs_f64() * 1e3
    );
    println!("contacts,{contacts}");
    Ok(())
}

/// The sorted sample at `percent` of the way through.
fn percentile(sorted: &[Duration], percent: usize) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let index = (sorted.len() * percent).div_ceil(100).saturating_sub(1);
    sorted[index.min(sorted.len() - 1)]
}

/// A seeded map of rolling hills with scattered woods, lakes, buildings and smoke, and the
/// requested units on two teams, every one running and placed on open ground.
fn build_battlefield(
    config: &Config,
    world: &mut World,
    args: &Args,
) -> anyhow::Result<(ObjectId, Vec<ObjectId>)> {
    let map = world.create(config, "los-benchmark".into(), Kind::Room);
    create_battle_map(
        world,
        map,
        "los-benchmark",
        MapAsset::from_cells(&map_source(args.size, args.seed))?,
    )?;
    let templates = [
        ("JR7-D", include_str!("../../game/units/JR7-D.toml")),
        (
            "Demolisher",
            include_str!("../../game/units/Demolisher.toml"),
        ),
    ];
    let mut rng = args.seed ^ 0xa5a5_5a5a_1234_5678;
    let mut units = Vec::with_capacity(args.units);
    let mut taken = std::collections::BTreeSet::new();
    for index in 0..args.units {
        let id = world.create(config, format!("los-benchmark-{index}"), Kind::Thing);
        let (reference, source) = templates[index % templates.len()];
        UnitTemplate::parse(reference, source)?.create(world, id)?;
        world.btech.edit_unit(id, |unit| {
            unit.set_power(Power::Off);
        })?;
        // Open ground only, so placement never needs a surface rule.
        let (x, y) = loop {
            let x = (next_random(&mut rng) % u64::from(args.size)) as i64;
            let y = (next_random(&mut rng) % u64::from(args.size)) as i64;
            if taken.contains(&(x, y)) {
                continue;
            }
            let tile = world.btech.maps()[&map].hex(x, y)?;
            if tile.terrain() == Terrain::Clear && tile.level() <= 4 {
                taken.insert((x, y));
                break (x, y);
            }
        };
        place_battle_unit(world, id, map, x, y)?;
        world.btech.edit_unit(id, |unit| {
            unit.set_power(Power::Running);
        })?;
        stompymux_rs::set_battle_unit_signature(
            world,
            id,
            stompymux_rs::UnitSignature {
                team: if index % 2 == 0 { 1 } else { 2 },
                hidden: false,
                illuminated: false,
            },
        )?;
        units.push(id);
    }
    Ok((map, units))
}

/// Cell notation for a varied battlefield: a smooth height field with woods, lakes,
/// buildings and a little smoke sprinkled by the seed.
fn map_source(size: u16, seed: u64) -> String {
    let mut rng = seed;
    let mut source = format!("{size} {size}\n");
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (f64::from(x), f64::from(y));
            let height =
                ((fx / 7.0).sin() + (fy / 9.0).cos() + ((fx + fy) / 13.0).sin()) * 1.4 + 2.0;
            let level = height.clamp(0.0, 6.0) as u8;
            let roll = next_random(&mut rng) % 100;
            let symbol = match roll {
                0..=11 => Terrain::LightWoods.symbol(),
                12..=17 => Terrain::HeavyWoods.symbol(),
                18..=19 => Terrain::UltraHeavyWoods.symbol(),
                20..=24 if level <= 1 => Terrain::Water.symbol(),
                25..=27 => Terrain::Building.symbol(),
                28 => Terrain::Smoke.symbol(),
                29 => Terrain::PlantedFields.symbol(),
                30..=32 => Terrain::Rough.symbol(),
                _ => '.',
            };
            let digit = if symbol == Terrain::Water.symbol() {
                1
            } else if symbol == Terrain::Building.symbol() {
                1 + (roll % 3) as u8
            } else {
                level
            };
            source.push(symbol);
            source.push(char::from(b'0' + digit));
        }
        source.push('\n');
    }
    source
}

fn next_random(state: &mut u64) -> u64 {
    *state ^= *state << 7;
    *state ^= *state >> 9;
    *state
}

/// Copy the test fixture game root with the read-only asset trees the server needs, never
/// the repository's own database.
fn copy_game_root() -> anyhow::Result<tempfile::TempDir> {
    let directory = tempfile::tempdir()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    copy_tree(&repository.join("tests/fixtures/game"), directory.path())?;
    for asset in ["lua", "units", "maps"] {
        copy_tree(
            &repository.join("game").join(asset),
            &directory.path().join(asset),
        )?;
    }
    Ok(directory)
}

fn copy_tree(source: &Path, target: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let destination = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &destination)?;
        } else {
            std::fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}
