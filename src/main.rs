//! Server CLI and graceful process shutdown.
use clap::{Parser, Subcommand};
use std::{net::IpAddr, path::PathBuf};
use stompymux_rs::{config::Config, server};
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
        #[arg(long)]
        listen_address: Option<IpAddr>,
        #[arg(long)]
        port: Option<u16>,
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
            let c = Config::load(game_dir)?.with_listener_overrides(listen_address, port)?;
            server::serve(c, async {
                let mut terminate =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("install SIGTERM handler");
                tokio::select! { _=tokio::signal::ctrl_c()=>{}, _=terminate.recv()=>{} }
            })
            .await?;
        }
    }
    Ok(())
}
