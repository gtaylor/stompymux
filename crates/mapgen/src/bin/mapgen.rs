//! `mapgen`: generate stompymux battlefield map files from flags or a JSON/TOML spec.
//!
//! `mapgen generate` writes a map file; `mapgen schema` prints the spec's JSON Schema for
//! tool definitions; `mapgen biomes` lists every biome with its defaults. Flags override the
//! matching fields of a `--spec` file, and `--settlement` flags add to its settlements.
use anyhow::{Context, Result};
use clap::{
    Args, Parser, Subcommand,
    builder::{PossibleValue, PossibleValuesParser, TypedValueParser},
};
use std::{fs, io::Read, path::PathBuf};
use stompymux_mapgen::{
    Amount, BattleMapFlag, Biome, MapSize, MapSpec, Relief, SettlementSpec, biome_catalog,
    generate, spec_schema,
};

/// Generate BattleTech battlefield maps for stompymux.
#[derive(Parser)]
#[command(name = "mapgen", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a map file.
    Generate(Generate),
    /// Print the JSON Schema for map specs (the --spec file format).
    Schema,
    /// List every biome with its description and defaults.
    Biomes {
        /// Print JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
}

/// Options for `mapgen generate`. Unset flags take the spec file's value, then the biome's.
#[derive(Args)]
struct Generate {
    /// JSON or TOML map spec to start from; `-` reads standard input.
    #[arg(long, value_name = "FILE")]
    spec: Option<PathBuf>,
    /// Where to write the map file; standard output by default.
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
    /// Write a JSON report of what was built here; `-` prints it to standard error.
    #[arg(long, value_name = "FILE")]
    report: Option<PathBuf>,
    /// Random seed; the same spec and seed always produce the same map.
    #[arg(long)]
    seed: Option<u64>,
    #[arg(long, value_enum)]
    biome: Option<Biome>,
    #[arg(long, value_enum)]
    size: Option<MapSize>,
    /// Width in hexes; overrides --size.
    #[arg(long)]
    width: Option<u16>,
    /// Height in hexes; overrides --size.
    #[arg(long)]
    height: Option<u16>,
    #[arg(long, value_enum)]
    relief: Option<Relief>,
    #[arg(long, value_enum)]
    water: Option<Amount>,
    #[arg(long, value_enum)]
    woods: Option<Amount>,
    #[arg(long, value_enum)]
    rough: Option<Amount>,
    /// Permanent fire and smoke.
    #[arg(long, value_enum)]
    fire: Option<Amount>,
    /// Rivers crossing the map (0 to 8).
    #[arg(long)]
    rivers: Option<u8>,
    /// Freeze lakes and rivers (true or false).
    #[arg(long)]
    frozen: Option<bool>,
    /// Add a settlement: SIZE[@POSITION][,OPTION...]. Sizes: outpost, hamlet, village, town,
    /// city, metropolis. Positions: center, north, northeast, east, southeast, south,
    /// southwest, west, northwest, random. Options: walled, open, kind=civilian|industrial|
    /// military|ruins, layout=grid|organic|scattered|compound, count=N, name=NAME, at=X:Y.
    /// Example: --settlement city@center,walled --settlement village,count=3
    #[arg(long = "settlement", value_name = "SPEC")]
    settlements: Vec<SettlementSpec>,
    /// Highways crossing the map edge to edge (0 to 8).
    #[arg(long)]
    through_roads: Option<u8>,
    /// Do not link settlements with roads.
    #[arg(long)]
    no_connecting_roads: bool,
    /// Road width in hexes (1 to 3).
    #[arg(long)]
    road_width: Option<u8>,
    /// Gravity in percent of standard.
    #[arg(long)]
    gravity: Option<u8>,
    /// Temperature in degrees Celsius.
    #[arg(long, allow_hyphen_values = true)]
    temperature: Option<i8>,
    /// Map flag; repeat for several. Replaces the biome's default flags.
    #[arg(long = "flag", value_parser = map_flag_parser())]
    flags: Vec<BattleMapFlag>,
}

/// Accept each map flag's name, listing every flag and its description in `--help`.
fn map_flag_parser() -> impl TypedValueParser<Value = BattleMapFlag> {
    let names =
        BattleMapFlag::ALL.map(|flag| PossibleValue::new(flag.name()).help(flag.description()));
    PossibleValuesParser::new(names)
        .map(|name| BattleMapFlag::parse(&name).expect("the parser only accepts flag names"))
}

impl Generate {
    /// The spec file's contents with every given flag applied on top.
    fn spec(&self) -> Result<MapSpec> {
        let mut spec = match &self.spec {
            None => MapSpec::default(),
            Some(path) => {
                let text = if path.as_os_str() == "-" {
                    let mut text = String::new();
                    std::io::stdin().read_to_string(&mut text)?;
                    text
                } else {
                    fs::read_to_string(path)
                        .with_context(|| format!("reading {}", path.display()))?
                };
                MapSpec::parse(&text)?
            }
        };
        macro_rules! take {
            ($($field:ident),*) => {
                $(if let Some(value) = self.$field { spec.$field = Some(value); })*
            };
        }
        take!(
            seed, biome, size, width, height, relief, water, woods, rough, fire, rivers, frozen
        );
        if self.width.is_none() && self.height.is_none() && self.size.is_some() {
            (spec.width, spec.height) = (None, None);
        }
        spec.settlements.extend(self.settlements.iter().cloned());
        let roads = spec.roads.get_or_insert_with(Default::default);
        if self.no_connecting_roads {
            roads.connect_settlements = Some(false);
        }
        if let Some(through) = self.through_roads {
            roads.through_roads = Some(through);
        }
        if let Some(width) = self.road_width {
            roads.width = Some(width);
        }
        let environment = spec.environment.get_or_insert_with(Default::default);
        if let Some(gravity) = self.gravity {
            environment.gravity = Some(gravity);
        }
        if let Some(temperature) = self.temperature {
            environment.temperature = Some(temperature);
        }
        if !self.flags.is_empty() {
            environment.flags = Some(self.flags.clone());
        }
        Ok(spec)
    }

    fn run(&self) -> Result<()> {
        let generated = generate(&self.spec()?)?;
        let file = generated.to_toml()?;
        match &self.output {
            Some(path) => {
                fs::write(path, &file).with_context(|| format!("writing {}", path.display()))?;
            }
            None => print!("{file}"),
        }
        for warning in &generated.report.warnings {
            eprintln!("warning: {warning}");
        }
        if let Some(path) = &self.report {
            let report = serde_json::to_string_pretty(&generated.report)?;
            if path.as_os_str() == "-" {
                eprintln!("{report}");
            } else {
                fs::write(path, report).with_context(|| format!("writing {}", path.display()))?;
            }
        }
        Ok(())
    }
}

fn main() -> std::process::ExitCode {
    match run(Cli::parse()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Generate(generate) => generate.run(),
        Command::Schema => {
            println!("{}", serde_json::to_string_pretty(&spec_schema())?);
            Ok(())
        }
        Command::Biomes { json: true } => {
            println!("{}", serde_json::to_string_pretty(&biome_catalog())?);
            Ok(())
        }
        Command::Biomes { json: false } => {
            for info in biome_catalog() {
                let name = serde_json::to_value(info.biome)?;
                println!(
                    "{:<10} {}",
                    name.as_str().unwrap_or_default(),
                    info.description
                );
            }
            Ok(())
        }
    }
}
