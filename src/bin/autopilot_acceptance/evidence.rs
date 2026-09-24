//! Streaming trace verification, episode attribution, and CPU guard parsing.
use super::compare::{indexed, key, number};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};

pub fn hash(path: &Path) -> Result<String> {
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut b = [0; 65536];
    loop {
        let n = f.read(&mut b)?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(h.finalize().iter().map(|v| format!("{v:02x}")).collect())
}
/// Compare exact lines, rejecting empty, truncated and invalid JSON traces.
pub fn traces(a: &Path, b: &Path) -> Result<Value> {
    let reference_sha256 = hash(a)?;
    let candidate_sha256 = hash(b)?;
    let mut a = BufReader::new(File::open(a)?);
    let mut b = BufReader::new(File::open(b)?);
    let mut count = 0;
    let mut first = None;
    let mut changed = BTreeMap::<String, Value>::new();
    loop {
        let mut x = String::new();
        let mut y = String::new();
        let nx = a.read_line(&mut x)?;
        let ny = b.read_line(&mut y)?;
        if nx == 0 && ny == 0 {
            break;
        }
        for line in [&x, &y] {
            if !line.is_empty() {
                ensure!(line.ends_with('\n'), "Truncated trace");
                let _: Value = serde_json::from_str(line)?;
            }
        }
        count += 1;
        if x != y {
            first.get_or_insert(count);
            for line in [&x, &y] {
                if line.is_empty() {
                    continue;
                }
                let row: Value = serde_json::from_str(line)?;
                if row.get("scenario").is_some() {
                    let identity = key(
                        &row,
                        if row.get("fire").is_some() {
                            "pursuit"
                        } else {
                            "movement"
                        },
                    )?;
                    changed
                        .entry(identity)
                        .or_insert_with(|| json!({"first_tick":row["tick"],"first_record":count}));
                }
            }
        }
    }
    ensure!(count > 0, "Empty trace");
    Ok(
        json!({"records":count,"first_difference":first,"changed_cases":changed,"reference_sha256":reference_sha256,"candidate_sha256":candidate_sha256,"failures":if first.is_some(){vec!["trace mismatch"]}else{vec![]}}),
    )
}
const COUNTERS: [&str; 4] = [
    "reversals",
    "replans",
    "prediction_ticks",
    "prediction_fallbacks",
];
/// Assign monotone committed counters to observed contact episodes.
pub fn episodes(summary: &Value, paths: &[std::path::PathBuf]) -> Result<Value> {
    let mut rows = indexed(summary, "pursuit")?;
    let mut previous: BTreeMap<String, (u64, [u64; 4])> = BTreeMap::new();
    let mut previous_ready = BTreeMap::<String, u64>::new();
    for row in rows.values_mut() {
        row["event_responses"] = Value::Array(row.get("script_events").and_then(Value::as_array).into_iter().flatten().map(|event| json!({"requested_tick":event[0],"event":event[1],"first_opportunity_delay":null})).collect());
        row["outside_contact_counters"] = json!({});
        for c in COUNTERS {
            row["outside_contact_counters"][c] = json!(0);
        }
        let mut last = None;
        for e in row["episodes"].as_array_mut().context("Missing episodes")? {
            for c in COUNTERS {
                e[c] = json!(0);
            }
            let start = e["start"].as_u64().context("Invalid episode start")?;
            let end = e["end"].as_u64().context("Invalid episode end")?;
            ensure!(
                end >= start && last.is_none_or(|l| start > l),
                "Invalid episode ordering"
            );
            e["contact_gap_ticks"] = last.map(|l| json!(start - l - 1)).unwrap_or(Value::Null);
            e["regain_opportunity_ticks"] = e["first_ready"]
                .as_u64()
                .map(|t| t.checked_sub(start).context("Opportunity before episode"))
                .transpose()?
                .map(|v| json!(v))
                .unwrap_or(Value::Null);
            last = Some(end);
        }
    }
    for path in paths {
        for line in BufReader::new(File::open(path)?).lines() {
            let record: Value = serde_json::from_str(&line?)?;
            let k = key(&record, "pursuit")?;
            let row = rows.get_mut(&k).context("Unknown trace case")?;
            let tick = record["tick"].as_u64().context("Invalid tick")?;
            let (old_tick, old) = previous.get(&k).copied().unwrap_or_default();
            ensure!(tick == old_tick + 1, "Missing/duplicate tick {k}");
            let mut values = [0; 4];
            let index = row["episodes"].as_array().unwrap().iter().position(|e| {
                e["start"].as_u64().unwrap() <= tick && e["end"].as_u64().unwrap() >= tick
            });
            let target = if let Some(i) = index {
                &mut row["episodes"][i]
            } else {
                &mut row["outside_contact_counters"]
            };
            for (i, c) in COUNTERS.iter().enumerate() {
                values[i] = record["result"][c].as_u64().context("Missing counter")?;
                let delta = values[i]
                    .checked_sub(old[i])
                    .context("Counter went backwards")?;
                target[c] = json!(target[c].as_u64().unwrap() + delta);
            }
            let ready = record["result"]["ready_ticks"]
                .as_u64()
                .context("Missing readiness counter")?;
            let old_ready = previous_ready.insert(k.clone(), ready).unwrap_or(0);
            ensure!(ready >= old_ready, "Readiness counter went backwards");
            if ready > old_ready {
                for event in row["event_responses"]
                    .as_array_mut()
                    .context("Event responses")?
                {
                    let start = event["requested_tick"]
                        .as_u64()
                        .context("Invalid event tick")?;
                    if tick >= start && event["first_opportunity_delay"].is_null() {
                        event["first_opportunity_delay"] = json!(tick - start);
                    }
                }
            }
            previous.insert(k, (tick, values));
        }
    }
    for (k, row) in &rows {
        let (tick, _) = previous.get(k).context("Missing trace case")?;
        ensure!(
            *tick == row["ticks"].as_u64().context("Invalid summary ticks")?,
            "Incomplete trace {k}"
        );
        for c in COUNTERS {
            let total = row["episodes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e[c].as_u64().unwrap())
                .sum::<u64>()
                + row["outside_contact_counters"][c].as_u64().unwrap();
            ensure!(
                Some(total) == row[c].as_u64(),
                "Counter attribution mismatch {k}/{c}"
            );
        }
    }
    Ok(Value::Array(rows.into_values().collect()))
}
fn csv_rows(path: &Path) -> Result<BTreeMap<(String, bool), Value>> {
    let mut reader = csv::Reader::from_path(path)?;
    let headers = reader.headers()?.clone();
    for required in [
        "scenario",
        "fire",
        "repetitions",
        "warmup_ticks",
        "measured_ticks",
        "p50_autopilot_ms",
        "p95_autopilot_ms",
        "p50_heartbeat_ms",
        "p95_heartbeat_ms",
        "min_enabled",
        "max_controller_expansions",
        "peak_search_records",
        "max_service_delay_ticks",
    ] {
        ensure!(
            headers.iter().filter(|h| *h == required).count() == 1,
            "Missing/duplicate CPU column {required}"
        );
    }
    let mut rows = BTreeMap::new();
    for row in reader.deserialize::<BTreeMap<String, String>>() {
        let row = row?;
        let scenario = row.get("scenario").context("Missing scenario")?.clone();
        let fire = row.get("fire").context("Missing fire")?.parse::<bool>()?;
        let mut value = json!({});
        for (k, v) in row {
            if k == "scenario" || k == "fire" || v == "-" {
                value[&k] = json!(v);
            } else {
                let n = v.parse::<f64>()?;
                ensure!(n.is_finite() && n >= 0.0, "Invalid CSV value");
                value[&k] = json!(n);
            }
        }
        ensure!(
            rows.insert((scenario, fire), value).is_none(),
            "Duplicate CPU case"
        );
    }
    ensure!(!rows.is_empty(), "Empty CPU matrix");
    Ok(rows)
}
pub fn cpu(before: &Path, after: &Path) -> Result<Value> {
    let b = csv_rows(before)?;
    let a = csv_rows(after)?;
    ensure!(a.keys().eq(b.keys()), "CPU matrix mismatch");
    let mut failures = Vec::new();
    let mut investigations = Vec::new();
    let mut cases = Vec::new();
    for (k, row) in &a {
        let old = &b[k];
        for field in ["repetitions", "warmup_ticks", "measured_ticks"] {
            ensure!(
                number(row, field)? == number(old, field)?,
                "CPU protocol mismatch"
            );
        }
        let p95 = number(row, "p95_autopilot_ms")?;
        let guard = if k.1 { 75.0 } else { 50.0 };
        if p95 >= guard {
            failures.push(format!("{k:?}: p95 {p95} exceeds {guard}"));
        }
        if !k.1 && p95 > number(old, "p95_autopilot_ms")? * 1.05 {
            investigations.push(format!("{k:?}: hold regression above five percent"));
        }
        ensure!(
            number(row, "min_enabled")? == 100.0,
            "Missing enabled controllers"
        );
        ensure!(
            number(row, "max_controller_expansions")? <= 256.0,
            "Expansion bound exceeded"
        );
        ensure!(
            number(row, "peak_search_records")? <= 1_000_000.0,
            "Search memory exceeded"
        );
        ensure!(
            number(row, "max_service_delay_ticks")? <= 1.0,
            "Controller starvation"
        );
        cases.push(json!({"scenario":k.0,"fire":k.1,"before":old,"after":row,"guard_ms":guard}));
    }
    // Investigations are actionable failures until separately measured and explained.
    failures.extend(investigations.clone());
    Ok(json!({"cases":cases,"failures":failures,"investigations":investigations}))
}

/// A pair of equally truncated traces must never count as a complete replay.
pub fn validate_trace(path: &Path, summary: &Value, kind: &str) -> Result<()> {
    let mut expected = BTreeMap::new();
    for row in summary.as_array().context("Summary array")? {
        let k = key(
            row,
            if kind == "pursuit" {
                "pursuit"
            } else {
                "movement"
            },
        )?;
        let ticks = row["ticks"].as_u64().context("Summary ticks")?;
        if let Some(old) = expected.insert(k, ticks) {
            ensure!(old == ticks, "Participant duration mismatch");
        }
    }
    let mut seen = BTreeMap::new();
    for line in BufReader::new(File::open(path)?).lines() {
        let row: Value = serde_json::from_str(&line?)?;
        let k = key(
            &row,
            if kind == "pursuit" {
                "pursuit"
            } else {
                "movement"
            },
        )?;
        ensure!(expected.contains_key(&k), "Unexpected trace case");
        let tick = row["tick"].as_u64().context("Trace tick")?;
        let prior = seen.entry(k).or_insert(0);
        ensure!(tick == *prior + 1, "Missing/duplicate trace tick");
        ensure!(
            row["digest"]
                .as_str()
                .is_some_and(|s| s.len() == 16 && s.bytes().all(|c| c.is_ascii_hexdigit())),
            "Invalid state/dice digest"
        );
        *prior = tick;
    }
    ensure!(seen == expected, "Incomplete trace matrix");
    Ok(())
}
