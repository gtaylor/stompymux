//! Isolated build/run orchestration with immutable evidence directories.
use super::{compare, evidence};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
static INTERRUPTED: AtomicBool = AtomicBool::new(false);
const BINS: [&str; 2] = ["autopilot-encounters", "autopilot-bench"];
const PURSUIT: [&str; 8] = [
    "lateral",
    "distant",
    "retreat",
    "reversals",
    "circuit",
    "short_occlusions",
    "expiry",
    "intercept_move",
];
const EXTENDED: [&str; 6] = [
    "slow_crossing",
    "fast_crossing",
    "opposite_crossing",
    "stop_start",
    "gradual_turns",
    "damaged_pursuit",
];
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
pub fn write(path: &Path, value: &Value) -> Result<()> {
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))?;
    Ok(())
}
fn unique(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(path).with_context(|| format!("Output must not exist: {}", path.display()))?;
    Ok(())
}
fn output(program: &str, args: &[&str]) -> Result<String> {
    let o = Command::new(program)
        .args(args)
        .current_dir(root())
        .output()?;
    ensure!(o.status.success(), "{program} failed");
    Ok(String::from_utf8(o.stdout)?)
}
/// All children get private process groups, making interruption terminate descendants too.
fn command(
    program: &Path,
    args: &[String],
    stdout: &Path,
    stderr: &Path,
    tmp: Option<&Path>,
) -> Result<()> {
    ensure!(!INTERRUPTED.load(Ordering::SeqCst), "Interrupted");
    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(root())
        .stdout(Stdio::from(fs::File::create(stdout)?))
        .stderr(Stdio::from(fs::File::create(stderr)?));
    if let Some(tmp) = tmp {
        cmd.env("TMPDIR", tmp);
    }
    #[cfg(unix)]
    cmd.process_group(0);
    let mut child = cmd
        .spawn()
        .with_context(|| format!("Spawn {}", program.display()))?;
    loop {
        if INTERRUPTED.load(Ordering::SeqCst) {
            #[cfg(unix)]
            {
                let _ = nix::sys::signal::killpg(
                    nix::unistd::Pid::from_raw(child.id() as i32),
                    nix::sys::signal::Signal::SIGKILL,
                );
            }
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("Interrupted; partial evidence retained");
        }
        if let Some(status) = child.try_wait()? {
            ensure!(
                status.success(),
                "{} exited {status}; see {}",
                program.display(),
                stderr.display()
            );
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn signal_handler() {
    std::thread::spawn(|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            #[cfg(unix)]
            {
                let mut term =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .unwrap();
                tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
            INTERRUPTED.store(true, Ordering::SeqCst);
        });
    });
}
fn build(path: &Path) -> Result<()> {
    command(
        Path::new("cargo"),
        &[
            "build",
            "--release",
            "--target-dir",
            "target",
            "--bin",
            BINS[0],
            "--bin",
            BINS[1],
        ]
        .map(str::to_owned),
        &path.join("build.stdout"),
        &path.join("build.log"),
        None,
    )?;
    for bin in BINS {
        fs::copy(root().join("target/release").join(bin), path.join(bin))?;
    }
    Ok(())
}
fn provenance(path: &Path) -> Result<()> {
    write(
        &path.join("policy-capabilities.json"),
        &json!({"encounters":true,"benchmark":true,"pursuit_policy":"adaptive"}),
    )?;
    let diff = output("git", &["diff", "HEAD", "--binary"])?;
    fs::write(path.join("source.patch"), diff)?;
    let files = output(
        "git",
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "src",
            "Cargo.toml",
            "Cargo.lock",
            "justfile",
            "design/autopilot/pursuit-extended-fixtures.json",
        ],
    )?;
    let mut sources = serde_json::Map::new();
    for file in files.lines() {
        let p = root().join(file);
        if p.is_file() {
            let snapshot = path.join("source").join(file);
            fs::create_dir_all(snapshot.parent().context("Source parent")?)?;
            fs::copy(&p, &snapshot)?;
            sources.insert(file.into(), json!(evidence::hash(&p)?));
        }
    }
    let mut hashes = serde_json::Map::new();
    for bin in BINS {
        hashes.insert(bin.into(), json!(evidence::hash(&path.join(bin))?));
    }
    write(
        &path.join("manifest.json"),
        &json!({"schema":1,"revision":output("git",&["rev-parse","HEAD"])?.trim(),"compiler":output("rustc",&["-Vv"])? ,"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,"cpu":fs::read_to_string("/proc/cpuinfo").ok(),"governor":fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor").ok(),"rustflags":std::env::var("RUSTFLAGS").ok(),"build":"cargo build --release --bin autopilot-encounters --bin autopilot-bench","executables":hashes,"source_hashes":sources,"fixture_inputs":fixture_hashes()?,"patch_sha256":evidence::hash(&path.join("source.patch"))?}),
    )
}
fn verify(path: &Path) -> Result<()> {
    let m = compare::read(&path.join("manifest.json"))?;
    ensure!(m["schema"] == 1, "Unsupported baseline manifest");
    for bin in BINS {
        ensure!(
            m["executables"][bin] == evidence::hash(&path.join(bin))?,
            "Executable hash mismatch: {bin}"
        );
    }
    ensure!(
        m["patch_sha256"] == evidence::hash(&path.join("source.patch"))?,
        "Source provenance mismatch"
    );
    if !m["fixture_inputs"].is_null() {
        ensure!(
            m["fixture_inputs"] == fixture_hashes()?,
            "Fixture inputs differ from capture"
        );
    }
    if let Some(sources) = m["source_hashes"].as_object() {
        for (file, hash) in sources {
            let relative = Path::new(file);
            ensure!(
                relative
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_))),
                "Unsafe source path"
            );
            ensure!(
                *hash == evidence::hash(&path.join("source").join(relative))?,
                "Source snapshot mismatch: {file}"
            );
        }
    }
    Ok(())
}
pub fn capture(path: &Path) -> Result<()> {
    signal_handler();
    unique(path)?;
    let path = fs::canonicalize(path)?;
    fs::copy(std::env::current_exe()?, path.join("acceptance-tool"))?;
    write(
        &path.join("status.json"),
        &json!({"state":"incomplete","stage":"capture"}),
    )?;
    build(&path)?;
    provenance(&path)?;
    write(
        &path.join("status.json"),
        &json!({"state":"complete","stage":"capture"}),
    )
}
#[derive(Clone)]
struct Task {
    label: String,
    suite: String,
    scenario: Option<String>,
    ticks: usize,
    seeds: usize,
    start: usize,
    direct: bool,
    exe: PathBuf,
}
fn tasks(exe: &Path, label: &str) -> Vec<Task> {
    let mut tasks = Vec::new();
    let mut push = |suite: &str, scenario: Option<&str>, ticks, start, direct| {
        tasks.push(Task {
            label: format!(
                "{label}-{}{}",
                scenario.unwrap_or(suite),
                if direct { "-direct" } else { "" }
            ),
            suite: suite.into(),
            scenario: scenario.map(str::to_owned),
            ticks,
            seeds: 3,
            start,
            direct,
            exe: exe.to_owned(),
        });
    };
    for suite in ["existing", "adversarial"] {
        push(suite, None, 240, 1, false);
    }
    for scenario in [
        "late_clearance",
        "alternating_clearance",
        "waiting_controllers",
    ] {
        push("adversarial", Some(scenario), 240, 1, false);
    }
    for scenario in PURSUIT {
        push("pursuit", Some(scenario), 900, 1, false);
        if label == "candidate" {
            push("pursuit", Some(scenario), 900, 1, true);
        }
    }
    for scenario in EXTENDED {
        push("pursuit_extended", Some(scenario), 900, 4, false);
        if label == "candidate" {
            push("pursuit_extended", Some(scenario), 900, 4, true);
        }
    }
    tasks
}
/// Only correctness jobs use this optional location. Timed CPU jobs never call it.
fn encounter_directory(temp_root: Option<&Path>) -> Result<tempfile::TempDir> {
    Ok(match temp_root {
        Some(root) => tempfile::Builder::new()
            .prefix("autopilot-")
            .tempdir_in(root)?,
        None => tempfile::tempdir()?,
    })
}

fn execute(task: &Task, path: &Path, temp_root: Option<&Path>) -> Result<()> {
    validate_capture_policy(task.exe.parent().context("Executable directory")?)?;
    let args = vec![
        "--suite".into(),
        task.suite.clone(),
        "--ticks".into(),
        task.ticks.to_string(),
        "--seeds".into(),
        task.seeds.to_string(),
        "--seed-start".into(),
        task.start.to_string(),
        "--trace".into(),
        path.join(format!("{}.jsonl", task.label))
            .to_string_lossy()
            .into_owned(),
    ];
    let mut args = args;
    if let Some(s) = &task.scenario {
        args.extend(["--scenario".into(), s.clone()]);
    }
    if task.direct {
        args.push("--direct-pursuit".into());
    }
    let requested = "adaptive";
    args.extend([
        "--pursuit-policy".into(),
        requested.into(),
        "--policy-metadata".into(),
        path.join(format!("{}.policy.json", task.label))
            .to_string_lossy()
            .into_owned(),
    ]);
    let tmp = encounter_directory(temp_root)?;
    write(
        &path.join(format!("{}.command.json", task.label)),
        &json!({"program":task.exe,"args":args}),
    )?;
    command(
        &task.exe,
        &args,
        &path.join(format!("{}.json", task.label)),
        &path.join(format!("{}.log", task.label)),
        Some(tmp.path()),
    )?;
    let rows = compare::read(&path.join(format!("{}.json", task.label)))?;
    validate_policy_metadata(
        &compare::read(&path.join(format!("{}.policy.json", task.label)))?,
        requested,
    )?;
    let kind = if task.suite == "existing" {
        "movement"
    } else if task.suite == "adversarial" {
        "adversarial"
    } else {
        "pursuit"
    };
    let indexed = compare::indexed(&rows, kind)?;
    let mut cases = std::collections::BTreeSet::new();
    for row in indexed.values() {
        if kind == "pursuit" && !task.label.starts_with("reference-") {
            ensure!(
                row["pursuit_policy"] == "Adaptive",
                "Encounter pursuit policy mismatch"
            );
        }
        let scenario = row["scenario"].as_str().context("Missing scenario")?;
        let legal = if let Some(expected) = &task.scenario {
            scenario == expected
        } else if task.suite == "existing" {
            [
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
            ]
            .contains(&scenario)
        } else {
            [
                "crossing",
                "duel",
                "occluded",
                "bottleneck",
                "pursuers",
                "passage",
                "no_passing_space",
                "mobility_damage",
                "mixed_arcs",
            ]
            .contains(&scenario)
        };
        ensure!(legal, "Unexpected scenario {scenario}");
        if kind == "pursuit" {
            ensure!(row["fire"].is_boolean(), "Invalid fire mode");
        }
        let seed = number_usize(row, "seed")?;
        ensure!(
            (task.start..task.start + task.seeds).contains(&seed),
            "Unexpected seed"
        );
        ensure!(
            ["mech", "tracked", "wheeled", "hover"]
                .contains(&row["chassis"].as_str().context("Missing chassis")?),
            "Unexpected chassis"
        );
        cases.insert((
            row["scenario"].to_string(),
            row["chassis"].to_string(),
            seed,
            if kind == "pursuit" {
                row["fire"].to_string()
            } else {
                String::new()
            },
        ));
    }
    let expected = if task.suite == "existing" {
        120
    } else if task.suite == "adversarial" && task.scenario.is_none() {
        108
    } else if kind == "pursuit" {
        24
    } else {
        12
    };
    ensure!(
        cases.len() == expected,
        "Incomplete matrix {}: {} != {expected}",
        task.label,
        cases.len()
    );
    evidence::validate_trace(&path.join(format!("{}.jsonl", task.label)), &rows, kind)?;
    Ok(())
}
fn number_usize(row: &Value, k: &str) -> Result<usize> {
    Ok(row[k].as_u64().context("Expected integer")? as usize)
}
/// Fail closed when benchmark metadata cannot prove the requested runtime policy.
pub(crate) fn validate_policy_metadata(policy: &Value, expected: &str) -> Result<()> {
    ensure!(
        policy["requested"] == expected
            && policy["resolved"] == expected
            && policy["verified_each_tick"] == true,
        "CPU pursuit policy mismatch"
    );
    Ok(())
}

/// Require a capture that explicitly identifies the supported runtime policy.
pub(crate) fn validate_capture_policy(path: &Path) -> Result<()> {
    let capabilities = compare::read(&path.join("policy-capabilities.json"))
        .context("Capture a fresh reference with explicit Adaptive policy metadata")?;
    ensure!(
        capabilities["encounters"] == true
            && capabilities["benchmark"] == true
            && capabilities["pursuit_policy"] == "adaptive",
        "Unsupported reference policy; capture a fresh Adaptive reference"
    );
    Ok(())
}
fn batch(tasks: &[Task], path: &Path, jobs: usize, temp_root: Option<&Path>) -> Result<()> {
    let index = AtomicUsize::new(0);
    let errors = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..jobs {
            scope.spawn(|| {
                loop {
                    if INTERRUPTED.load(Ordering::SeqCst) {
                        break;
                    }
                    let i = index.fetch_add(1, Ordering::SeqCst);
                    let Some(task) = tasks.get(i) else {
                        break;
                    };
                    eprintln!("acceptance: {}", task.label);
                    if let Err(e) = execute(task, path, temp_root) {
                        errors
                            .lock()
                            .unwrap()
                            .push(format!("{}: {e:#}", task.label));
                    }
                }
            });
        }
    });
    let errors = errors.into_inner().unwrap();
    ensure!(
        !INTERRUPTED.load(Ordering::SeqCst),
        "Interrupted; partial evidence retained"
    );
    ensure!(errors.is_empty(), "Encounter errors: {}", errors.join("\n"));
    Ok(())
}
fn gate(path: &Path, name: &str, result: Value, gates: &mut Vec<Value>) -> Result<()> {
    write(&path.join(format!("{name}.comparison.json")), &result)?;
    gates.push(json!({"name":name,"passed":result["failures"].as_array().context("Missing failures")?.is_empty()}));
    Ok(())
}
/// Run complete validation; status remains incomplete if the process is interrupted.
pub fn run(
    baseline: &Path,
    path: &Path,
    jobs: usize,
    behavior_only: bool,
    reviewed_changes: Option<&Path>,
    encounter_temp_root: Option<&Path>,
) -> Result<()> {
    signal_handler();
    ensure!(jobs > 0, "jobs must be positive");
    let encounter_temp_root = encounter_temp_root.map(fs::canonicalize).transpose()?;
    if let Some(root) = &encounter_temp_root {
        ensure!(
            root.is_dir(),
            "Encounter temporary root must be a directory"
        );
    }
    verify(baseline)?;
    validate_capture_policy(baseline)?;
    let baseline = fs::canonicalize(baseline)?;
    unique(path)?;
    let path = fs::canonicalize(path)?;
    write(
        &path.join("run-settings.json"),
        &json!({"jobs":jobs,"encounter_temp_root":encounter_temp_root,"cpu_temp_root":"system default"}),
    )?;
    fs::copy(std::env::current_exe()?, path.join("acceptance-tool"))?;
    write(
        &path.join("reference-manifest.json"),
        &compare::read(&baseline.join("manifest.json"))?,
    )?;
    write(
        &path.join("status.json"),
        &json!({"state":"incomplete","stage":"checks","performance_evaluated":false}),
    )?;
    let reviews = reviewed_changes
        .map(compare::read)
        .transpose()?
        .unwrap_or_else(|| json!({}));
    write(&path.join("reviewed-changes.json"), &reviews)?;
    let result = run_inner(
        &baseline,
        &path,
        jobs,
        behavior_only,
        &reviews,
        encounter_temp_root.as_deref(),
    );
    if let Err(e) = &result {
        write(
            &path.join("status.json"),
            &json!({"state":if INTERRUPTED.load(Ordering::SeqCst) { "incomplete" } else { "failed" },"error":format!("{e:#}"),"performance_evaluated":path.join("cpu.comparison.json").exists() && path.join("moving.comparison.json").exists()}),
        )?;
    }
    result
}
fn run_inner(
    baseline: &Path,
    path: &Path,
    jobs: usize,
    behavior_only: bool,
    reviews: &Value,
    encounter_temp_root: Option<&Path>,
) -> Result<()> {
    let fixture_inputs = fixture_hashes()?;
    write(&path.join("fixture-inputs.json"), &fixture_inputs)?;
    let candidate = path.join("candidate");
    fs::create_dir(&candidate)?;
    build(&candidate)?;
    provenance(&candidate)?;
    command(
        Path::new("just"),
        &["checks".into()],
        &path.join("checks.stdout"),
        &path.join("checks.log"),
        None,
    )?;
    write(
        &path.join("status.json"),
        &json!({"state":"incomplete","stage":"encounters","performance_evaluated":false}),
    )?;
    let mut all = tasks(&baseline.join(BINS[0]), "reference");
    all.extend(tasks(&candidate.join(BINS[0]), "candidate"));
    all.extend(tasks(&candidate.join(BINS[0]), "replay"));
    batch(&all, path, jobs, encounter_temp_root)?;
    ensure!(
        fixture_inputs == fixture_hashes()?,
        "Fixture inputs changed during encounters"
    );
    let mut gates = Vec::new();
    for task in tasks(&candidate.join(BINS[0]), "candidate")
        .iter()
        .filter(|t| !t.direct)
    {
        let suffix = task.label.strip_prefix("candidate-").unwrap();
        let read = |label: &str| compare::read(&path.join(format!("{label}-{suffix}.json")));
        let kind = if task.suite == "existing" {
            "movement"
        } else if task.suite == "adversarial" {
            "adversarial"
        } else {
            "pursuit"
        };
        // Per-scenario pursuit comparisons omit the aggregate lateral gain for predictive references.
        if kind != "pursuit" {
            gate(
                path,
                &format!("behavior-{suffix}"),
                compare::compare(kind, &read("reference")?, &read("candidate")?)?,
                &mut gates,
            )?;
        }
        let replay = evidence::traces(
            &path.join(format!("replay-{suffix}.jsonl")),
            &path.join(format!("candidate-{suffix}.jsonl")),
        )?;
        gate(path, &format!("replay-{suffix}"), replay, &mut gates)?;
        gate(
            path,
            &format!("replay-summary-{suffix}"),
            json!({"failures": if read("candidate")? == read("replay")? { vec![] } else { vec!["Candidate replay summaries differ"] }}),
            &mut gates,
        )?;
        let changes = evidence::traces(
            &path.join(format!("reference-{suffix}.jsonl")),
            &path.join(format!("candidate-{suffix}.jsonl")),
        )?;
        write(
            &path.join(format!("reference-{suffix}.trace-diff.json")),
            &changes,
        )?;
        let reviewed = reviewed_trace(&changes, &reviews[suffix]);
        gate(
            path,
            &format!("reference-{suffix}"),
            json!({"failures": if reviewed { vec![] } else {vec!["Reference trace differences need exact hashes and per-case reasons"]}}),
            &mut gates,
        )?;
    }
    for (suite, scenarios) in [
        ("pursuit", PURSUIT.as_slice()),
        ("pursuit_extended", EXTENDED.as_slice()),
    ] {
        let mut direct = Vec::new();
        let mut predicted = Vec::new();
        let mut traces = Vec::new();
        for s in scenarios {
            direct.extend(
                compare::read(&path.join(format!("candidate-{s}-direct.json")))?
                    .as_array()
                    .context("Summary array")?
                    .clone(),
            );
            predicted.extend(
                compare::read(&path.join(format!("candidate-{s}.json")))?
                    .as_array()
                    .context("Summary array")?
                    .clone(),
            );
            traces.push(path.join(format!("candidate-{s}.jsonl")));
        }
        gate(
            path,
            suite,
            compare::compare("pursuit", &json!(direct), &json!(predicted))?,
            &mut gates,
        )?;
        write(
            &path.join(format!("{suite}-episodes.json")),
            &evidence::episodes(&json!(predicted), &traces)?,
        )?;
    }
    if !behavior_only {
        write(
            &path.join("status.json"),
            &json!({"state":"incomplete","stage":"cpu","performance_evaluated":false}),
        )?;
        for workload in ["cpu", "moving"] {
            for (label, dir) in [("reference", baseline), ("candidate", candidate.as_path())] {
                validate_capture_policy(dir)?;
                let requested = "adaptive";
                let mut args = ["--warmup", "35", "--ticks", "60", "--repetitions", "3"]
                    .map(str::to_owned)
                    .to_vec();
                if workload == "moving" {
                    args.extend(["--scenario".into(), "moving_pursuit".into()]);
                }
                args.extend([
                    "--pursuit-policy".into(),
                    requested.into(),
                    "--policy-metadata".into(),
                    path.join(format!("{workload}-{label}.policy.json"))
                        .to_string_lossy()
                        .into_owned(),
                ]);
                let tmp = tempfile::tempdir()?;
                let before_machine = machine_sample();
                command(
                    &dir.join(BINS[1]),
                    &args,
                    &path.join(format!("{workload}-{label}.csv")),
                    &path.join(format!("{workload}-{label}.log")),
                    Some(tmp.path()),
                )?;
                {
                    let policy =
                        compare::read(&path.join(format!("{workload}-{label}.policy.json")))?;
                    validate_policy_metadata(&policy, requested)?;
                }
                write(
                    &path.join(format!("{workload}-{label}.environment.json")),
                    &json!({"program":dir.join(BINS[1]),"args":args,"before":before_machine,"after":machine_sample()}),
                )?;
            }
            let cpu_result = evidence::cpu(
                &path.join(format!("{workload}-reference.csv")),
                &path.join(format!("{workload}-candidate.csv")),
            )?;
            let expected_scenarios = if workload == "cpu" {
                vec!["open", "obstacles", "moving_congestion"]
            } else {
                vec!["moving_pursuit"]
            };
            let cases = cpu_result["cases"].as_array().context("CPU cases")?;
            ensure!(
                cases.len() == expected_scenarios.len() * 2,
                "Incomplete CPU matrix"
            );
            for case in cases {
                ensure!(
                    expected_scenarios
                        .contains(&case["scenario"].as_str().context("CPU scenario")?),
                    "Unexpected CPU scenario"
                );
                for row in [&case["before"], &case["after"]] {
                    ensure!(
                        compare::number(row, "repetitions")? == 3.0
                            && compare::number(row, "warmup_ticks")? == 35.0
                            && compare::number(row, "measured_ticks")? == 60.0,
                        "Unexpected CPU protocol"
                    );
                }
            }
            gate(path, workload, cpu_result, &mut gates)?;
        }
    }
    let passed = gates.iter().all(|g| g["passed"] == true);
    ensure!(
        fixture_inputs == fixture_hashes()?,
        "Fixture inputs changed during acceptance"
    );
    write(
        &path.join("report.json"),
        &json!({"passed":passed,"performance_evaluated":!behavior_only,"gates":gates}),
    )?;
    let mut md = format!(
        "# Autopilot acceptance\n\nPassed: {passed}. Performance evaluated: {}.\n\n| Gate | Passed |\n| --- | --- |\n",
        !behavior_only
    );
    for g in &gates {
        md.push_str(&format!(
            "| {} | {} |\n",
            g["name"].as_str().unwrap(),
            g["passed"]
        ));
    }
    if !behavior_only {
        md.push_str("\n| Workload | Fire | Autopilot p50 before / after | Autopilot p95 before / after | Heartbeat p95 before / after |\n| --- | --- | --- | --- | --- |\n");
        for workload in ["cpu", "moving"] {
            let report = compare::read(&path.join(format!("{workload}.comparison.json")))?;
            for row in report["cases"].as_array().context("CPU cases")? {
                let a = &row["after"];
                let b = &row["before"];
                md.push_str(&format!(
                    "| {} | {} | {} / {} | {} / {} | {} / {} |\n",
                    row["scenario"].as_str().unwrap(),
                    row["fire"],
                    b["p50_autopilot_ms"],
                    a["p50_autopilot_ms"],
                    b["p95_autopilot_ms"],
                    a["p95_autopilot_ms"],
                    b["p95_heartbeat_ms"],
                    a["p95_heartbeat_ms"]
                ));
            }
        }
    }
    md.push_str("\nReference trace differences are retained separately and require review for intended behavior changes. Raw artifacts and command logs accompany this report.\n");
    fs::write(path.join("report.md"), md)?;
    write(
        &path.join("status.json"),
        &json!({"state":if passed{"complete"}else{"failed"},"performance_evaluated":!behavior_only}),
    )?;
    write(&path.join("artifact-hashes.json"), &artifact_hashes(path)?)?;
    ensure!(passed, "Acceptance gates failed; see report.md");
    Ok(())
}

/// Record observed conditions without changing host governor or affinity.
fn machine_sample() -> Value {
    json!({"utc":chrono::Utc::now().to_rfc3339(),"cpu0_khz":fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq").ok(),"governor":fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor").ok(),"load":fs::read_to_string("/proc/loadavg").ok(),"ac_online":fs::read_to_string("/sys/class/power_supply/AC/online").ok()})
}

/// These are the harness's isolated fixture inputs, never the live game database.
fn fixture_hashes() -> Result<Value> {
    fn visit(relative: &Path, hashes: &mut serde_json::Map<String, Value>) -> Result<()> {
        for entry in fs::read_dir(root().join(relative))? {
            let entry = entry?;
            let path = relative.join(entry.file_name());
            let kind = entry.file_type()?;
            ensure!(
                !kind.is_symlink(),
                "Fixture symlink needs an explicit snapshot policy"
            );
            if kind.is_dir() {
                visit(&path, hashes)?;
            } else if kind.is_file() {
                hashes.insert(
                    path.to_string_lossy().into_owned(),
                    json!(evidence::hash(&root().join(path))?),
                );
            }
        }
        Ok(())
    }
    let mut hashes = serde_json::Map::new();
    for path in ["tests/fixtures/game", "game/lua", "game/units", "game/maps"] {
        visit(Path::new(path), &mut hashes)?;
    }
    Ok(Value::Object(hashes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encounter_storage_is_isolated_and_does_not_change_default_storage() {
        let parent = tempfile::tempdir().unwrap();
        let marker = parent.path().join("retain");
        fs::write(&marker, "parent-owned").unwrap();
        let first = encounter_directory(Some(parent.path())).unwrap();
        let second = encounter_directory(Some(parent.path())).unwrap();
        assert!(first.path().starts_with(parent.path()));
        assert_ne!(first.path(), second.path());
        fs::write(first.path().join("fixture"), "isolated").unwrap();
        first.close().unwrap();
        assert!(marker.exists());
        assert!(second.path().exists());
        let ordinary = encounter_directory(None).unwrap();
        assert!(!ordinary.path().starts_with(parent.path()));
    }

    #[test]
    fn output_and_provenance_fail_closed() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("run");
        unique(&p).unwrap();
        assert!(unique(&p).is_err());
        assert!(verify(&p).is_err());
        for name in BINS {
            fs::write(p.join(name), b"fake").unwrap();
        }
        fs::write(p.join("source.patch"), b"patch").unwrap();
        write(&p.join("manifest.json"),&json!({"schema":1,"executables":{"autopilot-encounters":evidence::hash(&p.join(BINS[0])).unwrap(),"autopilot-bench":evidence::hash(&p.join(BINS[1])).unwrap()},"patch_sha256":evidence::hash(&p.join("source.patch")).unwrap()})).unwrap();
        verify(&p).unwrap();
        fs::write(p.join(BINS[0]), b"changed").unwrap();
        assert!(verify(&p).is_err());
    }
    #[test]
    #[cfg(unix)]
    fn subprocess_failure_and_interruption_are_errors() {
        let d = tempfile::tempdir().unwrap();
        let out = d.path().join("out");
        let err = d.path().join("err");
        assert!(
            command(
                Path::new("/bin/sh"),
                &["-c".into(), "exit 7".into()],
                &out,
                &err,
                None
            )
            .is_err()
        );
        let interrupter = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(200));
            INTERRUPTED.store(true, Ordering::SeqCst);
        });
        assert!(
            command(
                Path::new("/bin/sh"),
                &["-c".into(), "sleep 30 & wait".into()],
                &out,
                &err,
                None
            )
            .is_err()
        );
        interrupter.join().unwrap();
        INTERRUPTED.store(false, Ordering::SeqCst);
    }
}

/// Reviews acknowledge only the exact measured changes; stale reviews cannot hide regressions.
fn reviewed_trace(changes: &Value, review: &Value) -> bool {
    if changes["first_difference"].is_null() {
        return true;
    }
    if changes["reference_sha256"] != review["reference_sha256"]
        || changes["candidate_sha256"] != review["candidate_sha256"]
    {
        return false;
    }
    let Some(cases) = changes["changed_cases"].as_object() else {
        return false;
    };
    !cases.is_empty()
        && cases.keys().all(|k| {
            review["reasons"][k]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty())
        })
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn finalization_cannot_waive_other_gates_or_modify_measurements() {
        let d = tempfile::tempdir().unwrap();
        let run = d.path().join("run");
        fs::create_dir(&run).unwrap();
        fs::copy(
            std::env::current_exe().unwrap(),
            run.join("acceptance-tool"),
        )
        .unwrap();
        let reviews = d.path().join("reviews.json");
        write(&reviews, &json!({})).unwrap();
        let report = json!({"passed":false,"performance_evaluated":true,"gates":[{"name":"cpu","passed":false}]});
        write(&run.join("report.json"), &report).unwrap();
        write(
            &run.join("artifact-hashes.json"),
            &artifact_hashes(&run).unwrap(),
        )
        .unwrap();
        assert!(finalize(&run, &reviews, &d.path().join("waived")).is_err());
        let mut report = report;
        report["gates"][0]["passed"] = json!(true);
        write(&run.join("report.json"), &report).unwrap();
        assert!(finalize(&run, &reviews, &d.path().join("tampered")).is_err());
        write(
            &run.join("artifact-hashes.json"),
            &artifact_hashes(&run).unwrap(),
        )
        .unwrap();
        assert!(finalize(&run, &reviews, &run.join("nested")).is_err());
        let output = d.path().join("reviewed");
        finalize(&run, &reviews, &output).unwrap();
        assert!(finalize(&run, &reviews, &output).is_err());
        assert_eq!(
            compare::read(&run.join("report.json")).unwrap()["passed"],
            false
        );
        assert_eq!(
            compare::read(&output.join("report.json")).unwrap()["passed"],
            true
        );
    }

    #[test]
    fn review_requires_exact_hashes_and_each_case_reason() {
        let changes = json!({"first_difference":1,"reference_sha256":"a","candidate_sha256":"b","changed_cases":{"case-one":{},"case-two":{}}});
        let mut review = json!({"reference_sha256":"a","candidate_sha256":"b","reasons":{"case-one":"bounded-lead repair"}});
        assert!(!reviewed_trace(&changes, &review));
        review["reasons"]["case-two"] = json!("same repair");
        assert!(reviewed_trace(&changes, &review));
        review["candidate_sha256"] = json!("stale");
        assert!(!reviewed_trace(&changes, &review));
    }
}

/// Seal measurement files; mutable progress status is not part of the measurement proof.
fn artifact_hashes(root: &Path) -> Result<Value> {
    fn visit(root: &Path, relative: &Path, out: &mut serde_json::Map<String, Value>) -> Result<()> {
        for entry in fs::read_dir(root.join(relative))? {
            let entry = entry?;
            let path = relative.join(entry.file_name());
            if path == Path::new("artifact-hashes.json") || path == Path::new("status.json") {
                continue;
            }
            let kind = entry.file_type()?;
            ensure!(!kind.is_symlink(), "Artifact symlink");
            if kind.is_dir() {
                visit(root, &path, out)?;
            } else if kind.is_file() {
                out.insert(
                    path.to_string_lossy().into_owned(),
                    json!(evidence::hash(&root.join(path))?),
                );
            }
        }
        Ok(())
    }
    let mut hashes = serde_json::Map::new();
    visit(root, Path::new(""), &mut hashes)?;
    Ok(Value::Object(hashes))
}
/// Append a reviewed verdict without modifying raw measurements or waiving other gates.
pub fn finalize(run: &Path, reviews: &Path, path: &Path) -> Result<()> {
    let run = fs::canonicalize(run)?;
    let parent = path.parent().context("Missing output parent")?;
    let parent = fs::canonicalize(if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    })?;
    ensure!(
        !parent.starts_with(&run),
        "Reviewed output must be outside the sealed run"
    );
    ensure!(
        compare::read(&run.join("artifact-hashes.json"))? == artifact_hashes(&run)?,
        "Measurement artifacts changed"
    );
    ensure!(
        evidence::hash(&run.join("acceptance-tool"))? == evidence::hash(&std::env::current_exe()?)?,
        "Use the preserved acceptance-tool executable to finalize this run"
    );
    let reviews = compare::read(reviews)?;
    let mut report = compare::read(&run.join("report.json"))?;
    for gate in report["gates"].as_array_mut().context("Missing gates")? {
        let name = gate["name"]
            .as_str()
            .context("Missing gate name")?
            .to_owned();
        if gate["passed"] == true {
            continue;
        }
        let suffix = name
            .strip_prefix("reference-")
            .context("Non-reference acceptance gate failed; review cannot waive it")?;
        let changes = compare::read(&run.join(format!("reference-{suffix}.trace-diff.json")))?;
        ensure!(
            reviewed_trace(&changes, &reviews[suffix]),
            "Incomplete or stale review for {suffix}"
        );
        gate["passed"] = json!(true);
        gate["reviewed"] = json!(true);
    }
    ensure!(
        !report["gates"].as_array().unwrap().is_empty(),
        "Empty acceptance report"
    );
    unique(path)?;
    report["passed"] = json!(true);
    report["source_run"] = json!(run);
    report["artifact_manifest_sha256"] = json!(evidence::hash(&run.join("artifact-hashes.json"))?);
    write(&path.join("report.json"), &report)?;
    write(&path.join("reviewed-changes.json"), &reviews)?;
    fs::write(
        path.join("report.md"),
        format!(
            "# Reviewed autopilot acceptance\n\nAll evaluated gates passed. Performance evaluated: {}.\n\nMeasurements: `{}`. Original measurement files and verdict remain unchanged. Exact per-case reasons are in `reviewed-changes.json`; no behavioral, replay, or CPU failure can be waived here.\n",
            report["performance_evaluated"],
            run.display()
        ),
    )?;
    Ok(())
}
