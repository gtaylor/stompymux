//! Local, reproducible autopilot acceptance; no Python runtime is required.
#[path = "autopilot_acceptance/compare.rs"]
mod compare;
#[path = "autopilot_acceptance/evidence.rs"]
mod evidence;
#[path = "autopilot_acceptance/runner.rs"]
mod runner;
#[cfg(test)]
#[path = "autopilot_acceptance/tests.rs"]
mod tests;
use anyhow::{Result, ensure};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Capture {
        #[arg(long)]
        output: PathBuf,
    },
    Run {
        #[arg(long)]
        baseline: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 4)]
        jobs: usize,
        /// Optional existing directory for correctness fixtures only; CPU fixtures use normal storage.
        #[arg(long)]
        encounter_temp_root: Option<PathBuf>,
        #[arg(long)]
        behavior_only: bool,
        /// Exact trace hashes and per-case reasons for intended reference differences.
        #[arg(long)]
        reviewed_changes: Option<PathBuf>,
    },
    Compare {
        #[arg(value_enum)]
        kind: Kind,
        before: PathBuf,
        after: PathBuf,
    },
    Episodes {
        summary: PathBuf,
        #[arg(required = true)]
        traces: Vec<PathBuf>,
    },
    /// Summarize committed pursuit transitions without inferring causality.
    Timeline { trace: PathBuf },
    /// Review exact intended reference differences without rerunning or replacing measurements.
    Finalize {
        #[arg(long)]
        run: PathBuf,
        #[arg(long)]
        reviewed_changes: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}
#[derive(Clone, Copy, ValueEnum)]
enum Kind {
    Movement,
    Adversarial,
    Pursuit,
    Trace,
    Cpu,
}
fn main() -> Result<()> {
    let result = match Args::parse().command {
        Command::Timeline { trace } => evidence::timeline(&trace)?,
        Command::Capture { output } => return runner::capture(&output),
        Command::Finalize {
            run,
            reviewed_changes,
            output,
        } => return runner::finalize(&run, &reviewed_changes, &output),
        Command::Run {
            baseline,
            output,
            jobs,
            encounter_temp_root,
            behavior_only,
            reviewed_changes,
        } => {
            return runner::run(
                &baseline,
                &output,
                jobs,
                behavior_only,
                reviewed_changes.as_deref(),
                encounter_temp_root.as_deref(),
            );
        }
        Command::Episodes { summary, traces } => {
            evidence::episodes(&compare::read(&summary)?, &traces)?
        }
        Command::Compare {
            kind,
            before,
            after,
        } => match kind {
            Kind::Trace => evidence::traces(&before, &after)?,
            Kind::Cpu => evidence::cpu(&before, &after)?,
            _ => compare::compare(
                match kind {
                    Kind::Movement => "movement",
                    Kind::Adversarial => "adversarial",
                    _ => "pursuit",
                },
                &compare::read(&before)?,
                &compare::read(&after)?,
            )?,
        },
    };
    println!("{}", serde_json::to_string_pretty(&result)?);
    ensure!(
        result
            .get("failures")
            .and_then(|v| v.as_array())
            .is_none_or(|v| v.is_empty()),
        "Acceptance comparison failed"
    );
    Ok(())
}
