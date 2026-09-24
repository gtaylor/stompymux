//! Production-heartbeat ground-autopilot benchmark.

use clap::Parser;
use stompymux_rs::{
    AutopilotBenchmarkOptions as BenchmarkOptions, AutopilotBenchmarkReport as BenchmarkReport,
    AutopilotBenchmarkScenario as Scenario, run_autopilot_benchmark as run,
};

#[derive(Debug, Parser)]
#[command(about = "Run the isolated ground-autopilot heartbeat benchmark")]
struct Args {
    /// Warmup heartbeats per repetition.
    #[arg(long, default_value_t = 30)]
    warmup: usize,
    /// Measured heartbeats per repetition.
    #[arg(long, default_value_t = 600)]
    ticks: usize,
    /// Independent isolated repetitions.
    #[arg(long, default_value_t = 5)]
    repetitions: usize,
    /// Number of attached controllers (the acceptance workload uses 100).
    #[arg(long, default_value_t = 100)]
    controllers: usize,
    /// Fixed fixture seed.
    #[arg(long, default_value_t = 0x5eed_1000_1000_0001)]
    seed: u64,
    /// Collect inclusive CPU attribution (do not use for acceptance timing).
    #[arg(long)]
    detailed: bool,
    /// Use direct pursuit as a same-executable control in isolated benchmarks.
    #[arg(long)]
    direct_pursuit: bool,
    /// Write one deterministic gameplay checksum per tick to this JSONL file.
    #[arg(long)]
    trace: Option<std::path::PathBuf>,
    /// Select a battlefield geometry; all retains the acceptance protocol.
    #[arg(long, default_value = "all", value_parser = ["all", "open", "obstacles", "moving_congestion", "moving_pursuit"])]
    scenario: String,
    /// Select a firing policy; all retains the acceptance protocol.
    #[arg(long, default_value = "all", value_parser = ["all", "hold", "opportunistic"])]
    fire: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let report = run(&BenchmarkOptions {
        warmup_ticks: args.warmup,
        measured_ticks: args.ticks,
        repetitions: args.repetitions,
        seed: args.seed,
        controllers: args.controllers,
        detailed: args.detailed,
        direct_pursuit: args.direct_pursuit,
        trace: args.trace,
        scenario: match args.scenario.as_str() {
            "open" => Some(Scenario::Open),
            "obstacles" => Some(Scenario::Obstacles),
            "moving_congestion" => Some(Scenario::MovingCongestion),
            "moving_pursuit" => Some(Scenario::MovingPursuit),
            _ => None,
        },
        fire: match args.fire.as_str() {
            "hold" => Some(false),
            "opportunistic" => Some(true),
            _ => None,
        },
    })
    .await?;
    print_report(&report);
    Ok(())
}

fn print_report(report: &BenchmarkReport) {
    println!(
        "scenario,fire,repetitions,warmup_ticks,measured_ticks,p50_heartbeat_ms,p95_heartbeat_ms,max_heartbeat_ms,p50_autopilot_ms,p95_autopilot_ms,max_autopilot_ms,p95_navigation_ms,p95_observation_ms,p50_persistence_ms,p95_persistence_ms,max_persistence_ms,expansions,max_controller_expansions,peak_search_records,replans,completed,blocked,shots,p95_completion_tick,min_enabled,max_service_delay_ticks,renewals,p95_movement_ms,p95_combat_ms,peak_trace_entries,peak_trace_cells"
    );
    for result in &report.results {
        if result.diagnostics.calls.iter().any(|&calls| calls != 0) {
            eprintln!(
                "diagnostics: {} fire={} categories=contacts,geometry,sensors,illumination,readiness,selection,shots {}",
                result.scenario,
                result.opportunistic_fire,
                serde_json::to_string(&result.diagnostics).expect("diagnostics serialize")
            );
        }
        println!(
            "{},{},{},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{},{}",
            result.scenario,
            result.opportunistic_fire,
            result.repetitions,
            result.warmup_ticks,
            result.measured_ticks,
            result.p50_heartbeat.as_secs_f64() * 1000.0,
            result.p95_heartbeat.as_secs_f64() * 1000.0,
            result.max_heartbeat.as_secs_f64() * 1000.0,
            result.p50_autopilot.as_secs_f64() * 1000.0,
            result.p95_autopilot.as_secs_f64() * 1000.0,
            result.max_autopilot.as_secs_f64() * 1000.0,
            result.p95_navigation.as_secs_f64() * 1000.0,
            result.p95_observation.as_secs_f64() * 1000.0,
            result.p50_persistence.as_secs_f64() * 1000.0,
            result.p95_persistence.as_secs_f64() * 1000.0,
            result.max_persistence.as_secs_f64() * 1000.0,
            result.expansions,
            result.max_controller_expansions,
            result.peak_search_records,
            result.replans,
            result.completed_orders,
            result.blocked_orders,
            result.autonomous_shots,
            result
                .completion_latency_ticks
                .map_or_else(|| "-".to_owned(), |ticks| ticks.to_string()),
            result.minimum_enabled_controllers,
            result.maximum_service_delay_ticks,
            result.workload_renewals,
            result.p95_movement.as_secs_f64() * 1000.0,
            result.p95_combat.as_secs_f64() * 1000.0,
            result.peak_trace_entries,
            result.peak_trace_cells,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_default_to_full_protocol_and_reject_unknown_values() {
        let defaults = Args::try_parse_from(["autopilot-bench"]).unwrap();
        assert_eq!(
            (defaults.scenario.as_str(), defaults.fire.as_str()),
            ("all", "all")
        );
        let selected = Args::try_parse_from([
            "autopilot-bench",
            "--scenario",
            "moving_congestion",
            "--fire",
            "opportunistic",
        ])
        .unwrap();
        assert_eq!(selected.scenario, "moving_congestion");
        assert_eq!(selected.fire, "opportunistic");
        assert!(Args::try_parse_from(["autopilot-bench", "--scenario", "unknown"]).is_err());
        assert!(Args::try_parse_from(["autopilot-bench", "--fire", "unknown"]).is_err());
    }
}
