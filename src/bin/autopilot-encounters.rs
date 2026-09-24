//! Deterministic combat movement comparison runner, independent of CPU acceptance workloads.
use clap::Parser;
#[derive(Parser)]
struct Args {
    /// Select the established suite, adversarial opponents, or both.
    #[arg(long, default_value = "existing", value_parser = ["existing", "adversarial", "pursuit", "pursuit_extended", "all"])]
    suite: String,
    /// Compare against direct pursuit, only in the pursuit suite.
    #[arg(long)]
    direct_pursuit: bool,
    /// Bounded experimental policy, restricted to the isolated pursuit suite.
    #[arg(long, default_value="g", value_parser=["control", "a", "b", "c", "d", "e", "f", "g"])]
    pursuit_policy: String,
    /// Pursuit fire mode selection.
    #[arg(long, default_value="all", value_parser=["all","hold","fire"])]
    fire: String,
    #[arg(long, default_value_t = 180)]
    ticks: usize,
    #[arg(long, default_value_t = 3)]
    seeds: u8,
    /// First pursuit seed; established movement and adversarial suites start at one.
    #[arg(long, default_value_t = 1)]
    seed_start: u8,
    #[arg(long)]
    scenario: Option<String>,
    /// Write a detached CSV summary in addition to JSON on stdout.
    #[arg(long)]
    csv: Option<std::path::PathBuf>,
    #[arg(long)]
    trace: Option<std::path::PathBuf>,
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let existing = [
        "approach",
        "long_approach",
        "close",
        "behind",
        "corner",
        "obstacle_pursuit",
        "fallback",
        "moving",
        "attack_move",
        "jammed",
    ];
    if let Some(name) = args.scenario.as_deref() {
        anyhow::ensure!(
            (matches!(args.suite.as_str(), "existing" | "all") && existing.contains(&name))
                || (matches!(args.suite.as_str(), "adversarial" | "all")
                    && stompymux_rs::AUTOPILOT_ADVERSARIAL_SCENARIOS.contains(&name))
                || (matches!(args.suite.as_str(), "pursuit" | "all")
                    && stompymux_rs::AUTOPILOT_PURSUIT_SCENARIOS.contains(&name))
                || (matches!(args.suite.as_str(), "pursuit_extended" | "all")
                    && stompymux_rs::AUTOPILOT_PURSUIT_EXTENDED_SCENARIOS.contains(&name)),
            "Unknown scenario for selected suite"
        );
    }
    let mut results = Vec::<serde_json::Value>::new();
    if matches!(args.suite.as_str(), "existing" | "all")
        && args
            .scenario
            .as_deref()
            .is_none_or(|s| existing.contains(&s))
    {
        results.extend(
            serde_json::to_value(
                stompymux_rs::run_autopilot_encounters(
                    args.ticks,
                    args.seeds,
                    args.scenario.as_deref(),
                    args.trace.as_deref(),
                )
                .await?,
            )?
            .as_array()
            .unwrap()
            .iter()
            .cloned(),
        );
    }
    if matches!(args.suite.as_str(), "adversarial" | "all")
        && args
            .scenario
            .as_deref()
            .is_none_or(|s| stompymux_rs::AUTOPILOT_ADVERSARIAL_SCENARIOS.contains(&s))
    {
        let path = args.trace.as_ref().map(|p| {
            if args.suite == "all" {
                p.with_extension("adversarial.jsonl")
            } else {
                p.clone()
            }
        });
        results.extend(
            serde_json::to_value(
                stompymux_rs::run_autopilot_adversarial(
                    args.ticks,
                    args.seeds,
                    args.scenario.as_deref(),
                    path.as_deref(),
                )
                .await?,
            )?
            .as_array()
            .unwrap()
            .iter()
            .cloned(),
        );
    }
    for (suite, scenarios, extended) in [
        ("pursuit", stompymux_rs::AUTOPILOT_PURSUIT_SCENARIOS, false),
        (
            "pursuit_extended",
            stompymux_rs::AUTOPILOT_PURSUIT_EXTENDED_SCENARIOS,
            true,
        ),
    ] {
        if (args.suite == suite || args.suite == "all")
            && args
                .scenario
                .as_deref()
                .is_none_or(|s| scenarios.contains(&s))
        {
            let path = args.trace.as_ref().map(|p| {
                if args.suite == "all" {
                    p.with_extension(format!("{suite}.jsonl"))
                } else {
                    p.clone()
                }
            });
            let fire = match args.fire.as_str() {
                "hold" => Some(false),
                "fire" => Some(true),
                _ => None,
            };
            results.extend(
                serde_json::to_value(
                    stompymux_rs::run_autopilot_pursuit(
                        args.ticks,
                        args.seeds,
                        args.scenario.as_deref(),
                        path.as_deref(),
                        args.direct_pursuit,
                        fire,
                        match args.pursuit_policy.as_str() {
                            "a" => stompymux_rs::AutopilotPursuitPolicy::A,
                            "b" => stompymux_rs::AutopilotPursuitPolicy::B,
                            "c" => stompymux_rs::AutopilotPursuitPolicy::C,
                            "d" => stompymux_rs::AutopilotPursuitPolicy::D,
                            "e" => stompymux_rs::AutopilotPursuitPolicy::E,
                            "f" => stompymux_rs::AutopilotPursuitPolicy::F,
                            "g" => stompymux_rs::AutopilotPursuitPolicy::G,
                            _ => stompymux_rs::AutopilotPursuitPolicy::Control,
                        },
                        args.seed_start,
                        extended,
                    )
                    .await?,
                )?
                .as_array()
                .unwrap()
                .iter()
                .cloned(),
            );
        }
    }
    if let Some(path) = args.csv {
        use std::io::Write;
        let rows = serde_json::to_value(&results)?;
        let rows = rows.as_array().unwrap();
        let keys: std::collections::BTreeSet<_> = rows
            .iter()
            .flat_map(|row| row.as_object().unwrap().keys())
            .collect();
        let mut file = std::fs::File::create(path)?;
        writeln!(
            file,
            "{}",
            keys.iter()
                .map(|k| k.as_str())
                .collect::<Vec<_>>()
                .join(",")
        )?;
        for row in rows {
            let fields: Vec<_> = keys
                .iter()
                .map(|key| match &row[*key] {
                    serde_json::Value::Null => String::new(),
                    serde_json::Value::String(value) => {
                        format!("\"{}\"", value.replace('"', "\"\""))
                    }
                    value => value.to_string(),
                })
                .collect();
            writeln!(file, "{}", fields.join(","))?;
        }
    }
    println!("{}", serde_json::to_string_pretty(&results)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suite_defaults_and_validation() {
        assert_eq!(
            Args::try_parse_from(["encounters"]).unwrap().suite,
            "existing"
        );
        assert!(Args::try_parse_from(["encounters", "--suite", "unknown"]).is_err());
        assert_eq!(
            Args::try_parse_from(["encounters", "--suite", "all"])
                .unwrap()
                .suite,
            "all"
        );
    }
}
