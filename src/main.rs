use clap::{Parser, Subcommand};
use std::path::PathBuf;
use stompymux_rs::{config::Config, persistence, scripting::Scripts, server};
#[derive(Parser)]
#[command(version, about = "Rust StompyMUX foundation")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Serve {
        #[arg(long, default_value = "game")]
        game_dir: PathBuf,
        #[arg(long, default_value = "127.0.0.1")]
        listen_address: String,
        #[arg(long)]
        port: Option<u16>,
    },
    Check {
        #[arg(long, default_value = "game")]
        game_dir: PathBuf,
    },
    ImportLegacy {
        #[arg(long)]
        source: PathBuf,
        #[arg(long, default_value = "game")]
        game_dir: PathBuf,
    },
}
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Serve {
            game_dir,
            listen_address,
            port,
        } => {
            let c = Config::load(game_dir)?;
            let port = port.unwrap_or(c.int("server.port", 5555) as u16);
            server::serve(c, &format!("{listen_address}:{port}"), async {
                let mut terminate =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("install SIGTERM handler");
                tokio::select! { _=tokio::signal::ctrl_c()=>{}, _=terminate.recv()=>{} }
            })
            .await?;
        }
        Command::Check { game_dir } => {
            let c = Config::load(game_dir)?;
            let w = if c.database().exists() {
                persistence::load(&c.database())?
            } else {
                persistence::read_legacy(
                    &c.root
                        .join(c.string("database.game_database", "data/stompymux.db")),
                    &c,
                )?
            };
            w.validate(&c)?;
            let s = Scripts::new(&c, std::rc::Rc::new(std::cell::RefCell::new(w)))?;
            for warning in c.warnings.iter().chain(&s.warnings) {
                eprintln!("Warning: {warning}");
            }
            println!("Configuration, world references and Lua modules validated (read-only).");
        }
        Command::ImportLegacy { source, game_dir } => {
            let c = Config::load(game_dir)?;
            println!("{}", persistence::import(&source, &c)?);
        }
    }
    Ok(())
}
