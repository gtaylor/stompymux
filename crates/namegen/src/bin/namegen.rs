//! `namegen`: print seeded mercenary outfit, person and callsign names for tuning the lists.
use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use stompymux_namegen::{Culture, Flavor, Lists, NameSpec};

/// Generate mercenary outfit, pilot and callsign names.
#[derive(Parser)]
#[command(name = "namegen", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Outfit names such as "Iron Hawk Irregulars".
    Outfit(Common),
    /// People's names in one culture.
    Person(Common),
    /// A roster: people sharing one culture with distinct family names.
    Roster(Common),
    /// Pilot callsigns.
    Callsign(Common),
    /// Summarize the word lists in use.
    Lists {
        /// Directory of TOML files overriding the embedded lists.
        #[arg(long)]
        lists: Option<PathBuf>,
    },
}

#[derive(Args)]
struct Common {
    /// How many names to print. With a seed, the first name is the one that seed gives alone.
    #[arg(short = 'n', long, default_value_t = 1)]
    count: usize,
    /// Random seed; omitted for a seed from the clock.
    #[arg(long)]
    seed: Option<u64>,
    /// Naming tradition for people and possessive outfit names.
    #[arg(long, value_enum)]
    culture: Option<Culture>,
    /// Outfit naming style.
    #[arg(long, value_enum)]
    flavor: Option<Flavor>,
    /// Directory of TOML files overriding the embedded lists.
    #[arg(long)]
    lists: Option<PathBuf>,
    /// Print JSON lines with the resolved spec instead of plain names.
    #[arg(long)]
    json: bool,
}

impl Common {
    fn lists(&self) -> Result<Lists> {
        match &self.lists {
            Some(dir) => Lists::from_dir(dir),
            None => Lists::embedded(),
        }
    }

    fn spec(&self) -> NameSpec {
        NameSpec {
            seed: self.seed,
            culture: self.culture,
            flavor: self.flavor,
        }
    }
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Outfit(common) => {
            let lists = common.lists()?;
            for spec in common.spec().series(common.count) {
                let outfit = lists.outfit_name(&spec);
                if common.json {
                    println!("{}", serde_json::to_string(&outfit)?);
                } else {
                    println!("{outfit}");
                }
            }
        }
        Command::Person(common) => {
            let lists = common.lists()?;
            for spec in common.spec().series(common.count) {
                let person = lists.person_name(&spec);
                if common.json {
                    println!(
                        "{}",
                        serde_json::json!({ "name": person, "spec": spec.resolve() })
                    );
                } else {
                    println!("{person}");
                }
            }
        }
        Command::Roster(common) => {
            let lists = common.lists()?;
            let spec = common.spec();
            let roster = lists.roster(&spec, common.count);
            if common.json {
                println!(
                    "{}",
                    serde_json::json!({ "roster": roster, "spec": spec.resolve() })
                );
            } else {
                for person in roster {
                    println!("{person}");
                }
            }
        }
        Command::Callsign(common) => {
            let lists = common.lists()?;
            let spec = common.spec();
            let callsigns = lists.callsigns(&spec, common.count);
            if common.json {
                println!(
                    "{}",
                    serde_json::json!({ "callsigns": callsigns, "spec": spec.resolve() })
                );
            } else {
                for callsign in callsigns {
                    println!("{callsign}");
                }
            }
        }
        Command::Lists { lists } => {
            let lists = match lists {
                Some(dir) => Lists::from_dir(&dir)?,
                None => Lists::embedded()?,
            };
            println!("patterns: {}", lists.outfit.patterns.len());
            for pattern in &lists.outfit.patterns {
                println!("  {:>3}  {}", pattern.weight, pattern.pattern.template);
            }
            for (name, list) in [
                ("adjectives", &lists.outfit.adjectives),
                ("nouns", &lists.outfit.nouns),
                ("suffixes", &lists.outfit.suffixes),
                ("colors", &lists.outfit.colors),
                ("places", &lists.outfit.places),
            ] {
                println!(
                    "{name}: {} common, {} house, {} periphery, {} pirate",
                    list.common.len(),
                    list.house.len(),
                    list.periphery.len(),
                    list.pirate.len()
                );
            }
            println!(
                "callsigns: {} words, {} prefixes",
                lists.callsigns.words.len(),
                lists.callsigns.prefixes.len()
            );
            for (culture, people) in &lists.people {
                println!(
                    "people/{}: {} given, {} family",
                    culture.name(),
                    people.given.len(),
                    people.family.len()
                );
            }
        }
    }
    Ok(())
}
