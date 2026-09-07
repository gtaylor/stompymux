//! On-demand Unix process metrics, normalized before rendering and independently fallible.
use crate::{config::Config, logging::Category};
use nix::{
    sys::resource::{RLIM_INFINITY, Resource, UsageWho, getrlimit, getrusage},
    unistd::{SysconfVar, sysconf},
};
use std::time::Duration;

/// A descriptor limit is not a count of currently unused descriptors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    Unlimited,
    Finite(u64),
}

/// Missing host measurements are distinct from measured zero.
#[derive(Default, Debug)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub page_bytes: Option<u64>,
    pub cpu: [Option<Duration>; 2],
    pub peak_rss_bytes: Option<u64>,
    /// Shared, private and stack integrals, in platform-native units.
    pub integrals: [Option<u64>; 3],
    /// Major, minor and swap counters.
    pub faults: [Option<u64>; 3],
    pub blocks: [Option<u64>; 2],
    /// IPC messages, not network socket bytes.
    pub ipc: [Option<u64>; 2],
    pub switches: [Option<u64>; 2],
    pub signals: Option<u64>,
    pub descriptors: Option<[Limit; 2]>,
    pub errors: Vec<String>,
}

/// Normalize getrusage's platform-dependent RSS unit with checked arithmetic.
fn rss_bytes(value: i64, apple: bool) -> Option<u64> {
    u64::try_from(value)
        .ok()?
        .checked_mul(if apple { 1 } else { 1024 })
}

/// Sample process-wide resource usage; individual failures preserve other observations.
pub fn collect() -> ProcessSnapshot {
    let mut s = ProcessSnapshot {
        pid: std::process::id(),
        ..Default::default()
    };
    match sysconf(SysconfVar::PAGE_SIZE) {
        Ok(value) => s.page_bytes = value.and_then(|v| u64::try_from(v).ok()),
        Err(error) => s.errors.push(format!("page size: {error}")),
    }
    match getrlimit(Resource::RLIMIT_NOFILE) {
        Ok((soft, hard)) => {
            s.descriptors = Some([soft, hard].map(|n| {
                if n == RLIM_INFINITY {
                    Limit::Unlimited
                } else {
                    Limit::Finite(n)
                }
            }))
        }
        Err(error) => s.errors.push(format!("descriptor limits: {error}")),
    }
    match getrusage(UsageWho::RUSAGE_SELF) {
        Err(error) => s.errors.push(format!("resource usage: {error}")),
        Ok(usage) => {
            s.cpu = [usage.user_time(), usage.system_time()].map(|t| {
                Some(
                    Duration::from_secs(u64::try_from(t.tv_sec()).ok()?)
                        + Duration::from_micros(u64::try_from(t.tv_usec()).ok()?),
                )
            });
            s.peak_rss_bytes = rss_bytes(usage.max_rss(), cfg!(target_vendor = "apple"));
            s.faults = [
                usage.major_page_faults(),
                usage.minor_page_faults(),
                usage.full_swaps(),
            ]
            .map(|v| u64::try_from(v).ok());
            s.blocks = [usage.block_reads(), usage.block_writes()].map(|v| u64::try_from(v).ok());
            s.switches = [
                usage.voluntary_context_switches(),
                usage.involuntary_context_switches(),
            ]
            .map(|v| u64::try_from(v).ok());
            // Linux leaves these getrusage fields unmaintained, so zero would be misleading.
            if cfg!(target_os = "linux") {
                s.faults[2] = None;
            } else {
                s.integrals = [
                    usage.shared_integral(),
                    usage.unshared_data_integral(),
                    usage.unshared_stack_integral(),
                ]
                .map(|v| u64::try_from(v).ok());
                s.ipc = [usage.ipc_receives(), usage.ipc_sends()].map(|v| u64::try_from(v).ok());
                s.signals = u64::try_from(usage.signals()).ok();
            }
        }
    }
    s
}

fn metric(value: Option<u64>) -> String {
    value.map_or_else(|| "unavailable".into(), |v| v.to_string())
}

impl ProcessSnapshot {
    /// Fixed resource categories with explicit units and no invented network statistics.
    pub fn render(&self) -> String {
        let cpu = self.cpu.map(|v| {
            v.map_or_else(
                || "unavailable".into(),
                |v| format!("{:.6}", v.as_secs_f64()),
            )
        });
        let limits = self
            .descriptors
            .map_or(["unavailable".into(), "unavailable".into()], |v| {
                v.map(|v| match v {
                    Limit::Unlimited => "unlimited".into(),
                    Limit::Finite(n) => n.to_string(),
                })
            });
        [
            format!("Process ID:  {}", self.pid),
            format!("Page size:   {} bytes", metric(self.page_bytes)),
            format!("CPU time:    {} user, {} system (seconds)", cpu[0], cpu[1]),
            format!("Peak RSS:    {} bytes", metric(self.peak_rss_bytes)),
            format!(
                "Integral mem: {} shared, {} private, {} stack (host units)",
                metric(self.integrals[0]),
                metric(self.integrals[1]),
                metric(self.integrals[2])
            ),
            format!(
                "Page faults: {} major, {} minor, {} swapouts",
                metric(self.faults[0]),
                metric(self.faults[1]),
                metric(self.faults[2])
            ),
            format!(
                "Block I/O:   {} reads, {} writes (operations)",
                metric(self.blocks[0]),
                metric(self.blocks[1])
            ),
            format!(
                "IPC messages: {} received, {} sent",
                metric(self.ipc[0]),
                metric(self.ipc[1])
            ),
            format!(
                "Context switches: {} voluntary, {} involuntary",
                metric(self.switches[0]),
                metric(self.switches[1])
            ),
            format!("Signals:     {}", metric(self.signals)),
            format!("Descriptor limits: {} soft, {} hard", limits[0], limits[1]),
        ]
        .join("\n")
    }
}

/// Await one isolated worker under the existing write budget; no world state is borrowed.
async fn report_with(
    c: &Config,
    sample: impl FnOnce() -> ProcessSnapshot + Send + 'static,
) -> String {
    match tokio::time::timeout(
        Duration::from_millis(c.runtime.write_timeout_ms),
        tokio::task::spawn_blocking(sample),
    )
    .await
    {
        Ok(Ok(snapshot)) => {
            for error in &snapshot.errors {
                c.log(&[Category::Problems], "SYS", "USAGE", error);
            }
            snapshot.render()
        }
        error => {
            c.log(
                &[Category::Problems],
                "SYS",
                "USAGE",
                format!("Process report failed: {error:?}"),
            );
            "Unable to collect process statistics before the report deadline.".into()
        }
    }
}

/// Public process-report entry point shared by interactive and queued execution.
pub async fn process_report(c: &Config) -> String {
    report_with(c, collect).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn units_unavailable_zero_and_limits() {
        assert_eq!(rss_bytes(7, false), Some(7168));
        assert_eq!(rss_bytes(7, true), Some(7));
        assert_eq!(rss_bytes(-1, false), None);
        assert_eq!(rss_bytes(i64::MAX, false), None);
        let snapshot = ProcessSnapshot {
            faults: [Some(0), Some(7), None],
            descriptors: Some([Limit::Unlimited, Limit::Finite(2048)]),
            ..Default::default()
        };
        let text = snapshot.render();
        assert!(text.contains("0 major, 7 minor, unavailable swapouts"));
        assert!(text.contains("unlimited soft, 2048 hard"));
        assert!(!text.contains("Network"));
        let actual = collect();
        assert_eq!(actual.pid, std::process::id());
        assert!(actual.page_bytes.is_some());
    }
    #[tokio::test]
    async fn partial_failure_and_timeout() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("stompymux.toml"),
            "[runtime]\nwrite_timeout_ms=10\n",
        )
        .unwrap();
        let c = Config::load(d.path()).unwrap();
        let text = report_with(&c, || ProcessSnapshot {
            pid: 42,
            errors: vec!["injected failure".into()],
            ..Default::default()
        })
        .await;
        assert!(text.contains("42") && text.contains("unavailable"));
        let text = report_with(&c, || {
            std::thread::sleep(Duration::from_millis(50));
            ProcessSnapshot::default()
        })
        .await;
        assert!(text.contains("deadline"));
        c.logger.shutdown(&c).await.unwrap();
    }
}
