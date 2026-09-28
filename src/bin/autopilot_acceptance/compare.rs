//! Deterministic acceptance gates, independent of process orchestration.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

pub fn read(path: &Path) -> Result<Value> {
    serde_json::from_slice(&fs::read(path)?).with_context(|| path.display().to_string())
}
pub fn number(row: &Value, field: &str) -> Result<f64> {
    row.get(field)
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite())
        .with_context(|| format!("Missing/invalid number {field}"))
}
pub fn text<'a>(row: &'a Value, field: &str) -> Result<&'a str> {
    row.get(field)
        .and_then(Value::as_str)
        .with_context(|| format!("Missing string {field}"))
}
pub fn optional(row: &Value, field: &str) -> Result<Option<f64>> {
    ensure!(row.get(field).is_some(), "Missing {field}");
    if row[field].is_null() {
        Ok(None)
    } else {
        number(row, field).map(Some)
    }
}
pub fn key(row: &Value, kind: &str) -> Result<String> {
    let mut fields = vec!["scenario", "chassis", "seed"];
    if kind == "adversarial" {
        fields.push("role");
    }
    if kind == "pursuit" {
        fields.push("fire");
    }
    let values = fields
        .iter()
        .map(|f| {
            row.get(*f)
                .cloned()
                .with_context(|| format!("Missing identity {f}"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(serde_json::to_string(&values)?)
}
pub fn indexed(rows: &Value, kind: &str) -> Result<BTreeMap<String, Value>> {
    let rows = rows.as_array().context("Expected summary array")?;
    ensure!(!rows.is_empty(), "Empty matrix");
    let mut out = BTreeMap::new();
    for row in rows {
        if kind != "movement" {
            ensure!(row["schema"] == 1, "Unsupported summary schema");
        }
        let k = key(row, kind)?;
        ensure!(
            out.insert(k.clone(), row.clone()).is_none(),
            "Duplicate case {k}"
        );
    }
    Ok(out)
}
fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    (v[(v.len() - 1) / 2] + v[v.len() / 2]) / 2.0
}
/// Preserve established movement, adversarial and pursuit thresholds.
pub fn compare(kind: &str, before: &Value, after: &Value) -> Result<Value> {
    ensure!(
        ["movement", "adversarial", "pursuit"].contains(&kind),
        "Unknown comparison"
    );
    let before = indexed(before, kind)?;
    let after = indexed(after, kind)?;
    ensure!(before.keys().eq(after.keys()), "Matrix mismatch");
    let mut failures = Vec::new();
    let mut limitations = Vec::new();
    let mut changes = Vec::new();
    let mut unmatched = Vec::new();
    let mut armed_outcomes = Vec::new();
    let mut excess = Vec::new();
    let mut lateral_b = Vec::new();
    let mut lateral_a = Vec::new();
    let mut ready = [0.0; 2];
    let mut visible = [0.0; 2];
    for (k, a) in &after {
        let b = &before[k];
        let scenario = text(a, "scenario")?;
        let mut fail = |reason: &str| {
            failures.push(json!({"case":serde_json::from_str::<Value>(k).unwrap(),"reason":reason}))
        };
        if kind == "movement" {
            ensure!(
                number(a, "ticks")? == number(b, "ticks")?,
                "Duration mismatch"
            );
            if let Some(first) = optional(b, "time_to_engage")?
                && optional(a, "time_to_engage")?.is_none_or(|x| x > first + 1.0)
            {
                fail("first-shot regression");
            }
            if number(a, "arc_fraction")? + 0.02 < number(b, "arc_fraction")? {
                fail("arc uptime regression");
            }
            if text(a, "state")? == "blocked" {
                fail("controller blocked");
            }
            if number(a, "settled_distance")? > 0.25 {
                fail("more than 0.25 hex traveled after settling");
            }
            if !["moving", "attack_move"].contains(&scenario) && number(a, "settled_ticks")? < 20.0
            {
                fail("fewer than twenty settled ticks");
            }
        } else if kind == "adversarial" {
            ensure!(
                number(a, "ticks")? == number(b, "ticks")?,
                "Duration mismatch"
            );
            let role = text(a, "role")?;
            let outcome = text(a, "outcome")?;
            if scenario == "no_passing_space" {
                if role == "focal" {
                    limitations
                        .push(json!({"case":k,"outcome":outcome,"reason":a["blocking_reason"]}));
                }
                continue;
            }
            let clearance = [
                "late_clearance",
                "alternating_clearance",
                "waiting_controllers",
            ]
            .contains(&scenario);
            let controlled = ((clearance || scenario == "bottleneck")
                && (role == "focal" || role.starts_with("waiting_")))
                || (scenario == "passage" && ["focal", "opponent"].contains(&role));
            if controlled && outcome != "completed" {
                fail("reachable movement did not complete");
            }
            if scenario == "bottleneck"
                && role == "focal"
                && optional(a, "recovery_ticks")?.is_none()
            {
                fail("no recovery after clearance");
            }
            if controlled && clearance {
                let delay = a["congestion"]["clearance_to_search"].as_f64();
                if delay.is_none_or(|d| d > 5.0) {
                    fail("clearance did not start search within five ticks");
                }
                let old = b["congestion"]["clearance_to_search"].as_f64();
                if scenario == "late_clearance"
                    && b["congestion"]["early_starts"].as_u64().unwrap_or(0) == 0
                    && old.is_some_and(|v| v > 5.0)
                    && delay.zip(old).is_some_and(|(n, o)| n >= o)
                {
                    fail("late-clearance delay did not improve");
                }
            }
            let expiry = scenario == "occluded"
                && a["blocking_reason"] == "ContactLost"
                && optional(a, "first_shot")?.is_some()
                && a["longest_contact_gap"].as_u64().unwrap_or(0) >= 30;
            if expiry {
                limitations.push(json!({"case":k,"outcome":"contact_expiry","longest_contact_gap":a["longest_contact_gap"],"reacquisitions":a.get("reacquisitions").cloned().unwrap_or(json!(0))}));
            } else if outcome == "blocked" && scenario == "duel" {
                limitations.push(
                    json!({"case":k,"outcome":"combat_blocked","reason":a["blocking_reason"]}),
                );
            }
            if outcome == "blocked" && scenario != "duel" && !expiry {
                fail("unexpected blocking");
            }
            let combatant = (role == "focal" && !controlled) || role.starts_with("pursuer");
            if combatant && optional(a, "first_shot")?.is_none() && outcome != "destroyed" {
                fail("did not engage");
            }
            if combatant
                && scenario != "duel"
                && outcome != "destroyed"
                && text(b, "outcome")? != "destroyed"
            {
                if let Some(old) = optional(b, "first_shot")?
                    && optional(a, "first_shot")?.is_none_or(|n| n > old + 1.0)
                {
                    fail("first-shot regression");
                }
                if number(a, "arc_fraction")? + 0.02 < number(b, "arc_fraction")? {
                    fail("arc uptime regression");
                }
            }
        } else {
            let fire = a["fire"].as_bool().context("Invalid fire mode")?;
            if fire {
                armed_outcomes.push(json!({"case":k,"before":{"outcome":b["outcome"],"ticks":b["ticks"],"first_shot":b["first_shot"],"target_destroyed_at":b["target_destroyed_at"]},"after":{"outcome":a["outcome"],"ticks":a["ticks"],"first_shot":a["first_shot"],"target_destroyed_at":a["target_destroyed_at"]}}));
                if scenario == "intercept_move" && text(a, "outcome")? != "completed" {
                    fail("armed attack-move did not complete");
                }
                continue;
            }
            for (i, row) in [b, a].iter().enumerate() {
                ready[i] += number(row, "ready_ticks")?;
                visible[i] += number(row, "visible_ticks")?;
            }
            if a["script_rejections"].as_u64().unwrap_or(0) > 0 {
                fail("scripted controls rejected");
            }
            let outcome = text(a, "outcome")?;
            if scenario != "expiry" && !["window_end", "completed"].contains(&outcome) {
                fail("sustained-pursuit failure");
            }
            if scenario == "expiry" {
                if !outcome.contains("ContactLost") {
                    fail("contact expiry missing");
                }
                continue;
            }
            if scenario == "intercept_move" {
                if outcome != "completed" {
                    fail("weapons-hold destination did not complete");
                }
                continue;
            }
            let first = optional(a, "first_ready")?;
            let old = optional(b, "first_ready")?;
            if first.is_none() {
                fail("no firing opportunity");
            }
            if let Some((new, old)) = first.zip(old) {
                if new > old + 5.0 {
                    fail("first opportunity regressed by more than five ticks");
                }
                excess.push(json!({"case":serde_json::from_str::<Value>(k)?,"additional_distance":number(a,"approach_distance")?-number(b,"approach_distance")?}));
                if scenario == "lateral" {
                    lateral_b.push(old);
                    lateral_a.push(new);
                }
            } else {
                unmatched.push(json!({"case":k,"before":b["outcome"],"after":a["outcome"]}));
            }
            if scenario == "distant"
                && (number(a, "settled_ticks")? < 20.0 || number(a, "settled_distance")? > 0.25)
            {
                fail("stationary settling regression");
            }
            if scenario == "short_occlusions" {
                let episodes = a["episodes"].as_array().context("Missing episodes")?;
                let mut gaps = 0;
                for pair in episodes.windows(2) {
                    let gap = number(&pair[1], "start")? - number(&pair[0], "end")? - 1.0;
                    if gap > 0.0 && gap < 30.0 {
                        gaps += 1;
                    }
                }
                if gaps < 3 {
                    fail("fewer than three short reacquisitions");
                }
            }
        }
        if a != b {
            changes.push(json!({"case":k,"before":b,"after":a}));
        }
    }
    let fractions = [
        ready[0] / visible[0].max(1.0),
        ready[1] / visible[1].max(1.0),
    ];
    let mut gain = None;
    if kind == "pursuit" {
        if fractions[1] < fractions[0] - 0.02 {
            failures.push(json!({"case":"aggregate","reason":"readiness fraction regressed"}));
        }
        let mut distances = excess
            .iter()
            .map(|v| v["additional_distance"].as_f64().unwrap())
            .collect::<Vec<_>>();
        if !distances.is_empty() && median(&mut distances) > 0.0 {
            failures
                .push(json!({"case":"aggregate","reason":"median approach distance increased"}));
        }
        if !lateral_b.is_empty() {
            let b = median(&mut lateral_b);
            let a = median(&mut lateral_a);
            gain = Some(1.0 - a / b);
            if a > b * 0.9 {
                failures.push(
                    json!({"case":"lateral","reason":"median improvement below ten percent"}),
                );
            }
        }
    }
    Ok(
        json!({"cases":after.len(),"failures":failures,"limitations":limitations,"changes":changes,"unmatched":unmatched,"armed_outcomes":armed_outcomes,"paired_excess_travel":excess,"lateral_improvement":gain,"readiness_before":fractions[0],"readiness_after":fractions[1]}),
    )
}
