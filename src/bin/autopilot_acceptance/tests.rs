//! Small deterministic acceptance-tool regression tests.
use super::{compare, evidence, runner};
use serde_json::json;
#[test]
fn reference_capture_requires_explicit_supported_policy() {
    let dir = tempfile::tempdir().unwrap();
    assert!(runner::validate_capture_policy(dir.path()).is_err());
    for policy in [
        json!({"encounters":true,"benchmark":true}),
        json!({"encounters":true,"benchmark":true,"pursuit_policy":"unknown"}),
        json!({"encounters":false,"benchmark":true,"pursuit_policy":"adaptive"}),
    ] {
        std::fs::write(
            dir.path().join("policy-capabilities.json"),
            policy.to_string(),
        )
        .unwrap();
        assert!(runner::validate_capture_policy(dir.path()).is_err());
    }
    std::fs::write(
        dir.path().join("policy-capabilities.json"),
        json!({"encounters":true,"benchmark":true,"pursuit_policy":"adaptive"}).to_string(),
    )
    .unwrap();
    runner::validate_capture_policy(dir.path()).unwrap();
}

#[test]
fn policy_metadata_rejects_defaults_mismatches_and_unverified_runs() {
    assert!(
        runner::validate_policy_metadata(
            &json!({"requested":"adaptive","resolved":"adaptive","verified_each_tick":true}),
            "adaptive"
        )
        .is_ok()
    );
    for value in [
        json!({}),
        json!({"requested":"adaptive","resolved":"unknown","verified_each_tick":true}),
        json!({"requested":"adaptive","resolved":"adaptive","verified_each_tick":false}),
    ] {
        assert!(runner::validate_policy_metadata(&value, "adaptive").is_err());
    }
}
#[test]
fn rejects_empty_and_duplicate_matrices() {
    assert!(compare::indexed(&json!([]), "pursuit").is_err());
    let r = json!({"schema":1,"scenario":"x","chassis":"mech","seed":1,"fire":false});
    assert!(compare::indexed(&json!([r, r]), "pursuit").is_err());
}
#[test]
fn movement_thresholds() {
    let a = json!([{"scenario":"approach","chassis":"mech","seed":1,"ticks":240,"time_to_engage":10,"arc_fraction":0.5,"state":"executing","settled_distance":0.25,"settled_ticks":20}]);
    let mut b = a.clone();
    assert!(
        compare::compare("movement", &a, &b).unwrap()["failures"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    b[0]["time_to_engage"] = json!(12);
    assert_eq!(
        compare::compare("movement", &a, &b).unwrap()["failures"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn traces_reject_truncation_and_report_first_difference() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("a");
    let b = d.path().join("b");
    std::fs::write(&a, "{}\n{}\n").unwrap();
    std::fs::write(&b, "{}\n{\"x\":1}\n").unwrap();
    assert_eq!(evidence::traces(&a, &b).unwrap()["first_difference"], 2);
    std::fs::write(&b, "{}").unwrap();
    assert!(evidence::traces(&a, &b).is_err());
}
fn pursuit() -> serde_json::Value {
    json!({"schema":1,"scenario":"lateral","chassis":"mech","seed":1,"fire":false,"first_ready":100,"approach_distance":10.0,"ready_ticks":10,"visible_ticks":100,"outcome":"window_end","episodes":[]})
}
fn pair(kind: &str, b: serde_json::Value, a: serde_json::Value) -> serde_json::Value {
    compare::compare(kind, &json!([b]), &json!([a])).unwrap()
}
fn passes(v: &serde_json::Value) -> bool {
    v["failures"].as_array().unwrap().is_empty()
}
#[test]
fn pursuit_improvement_and_unchanged() {
    let b = pursuit();
    let mut a = b.clone();
    a["first_ready"] = json!(90);
    a["approach_distance"] = json!(9.0);
    assert!(passes(&pair("pursuit", b.clone(), a)));
    assert!(!passes(&pair("pursuit", b.clone(), b)));
}
#[test]
fn missing_opportunity_is_not_zero_cost() {
    let b = pursuit();
    let mut a = b.clone();
    a["first_ready"] = serde_json::Value::Null;
    a["approach_distance"] = serde_json::Value::Null;
    a["outcome"] = json!("destroyed");
    let v = pair("pursuit", b, a);
    assert!(!passes(&v));
    assert_eq!(v["unmatched"].as_array().unwrap().len(), 1);
    assert!(v["paired_excess_travel"].as_array().unwrap().is_empty());
}
#[test]
fn early_opportunity_cannot_hide_blocking() {
    let b = pursuit();
    let mut a = b.clone();
    a["first_ready"] = json!(90);
    a["approach_distance"] = json!(9.0);
    a["outcome"] = json!("Stuck");
    assert!(!passes(&pair("pursuit", b, a.clone())));
    a["scenario"] = json!("retreat");
    assert!(!passes(&pair("pursuit", a.clone(), a)));
}
#[test]
fn matrices_and_required_fields_are_checked() {
    let a = pursuit();
    let mut b = a.clone();
    b["seed"] = json!(2);
    assert!(compare::compare("pursuit", &json!([a.clone()]), &json!([b])).is_err());
    let mut b = a.clone();
    b.as_object_mut().unwrap().remove("first_ready");
    assert!(compare::compare("pursuit", &json!([a]), &json!([b])).is_err());
}
fn adversarial(s: &str) -> serde_json::Value {
    json!({"schema":1,"scenario":s,"chassis":"mech","seed":1,"role":"focal","ticks":240,"sampled_ticks":240,"first_shot":10,"arc_fraction":0.8,"outcome":"window_end","blocking_reason":null,"recovery_ticks":null})
}
#[test]
fn adversarial_shot_and_casualty() {
    let b = adversarial("crossing");
    let mut a = b.clone();
    a["first_shot"] = json!(12);
    assert!(!passes(&pair("adversarial", b, a)));
    let b = adversarial("duel");
    let mut a = b.clone();
    a["outcome"] = json!("destroyed");
    a["arc_fraction"] = json!(0.1);
    assert!(passes(&pair("adversarial", b, a)));
}
#[test]
fn bottleneck_requires_completion_and_recovery() {
    let b = adversarial("bottleneck");
    assert_eq!(
        pair("adversarial", b.clone(), b.clone())["failures"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let mut a = b.clone();
    a["outcome"] = json!("completed");
    a["recovery_ticks"] = json!(1);
    assert!(passes(&pair("adversarial", b, a)));
}
#[test]
fn limitations_remain_explicit() {
    let mut a = adversarial("no_passing_space");
    a["outcome"] = json!("blocked");
    a["blocking_reason"] = json!("Unreachable");
    let v = pair("adversarial", a.clone(), a);
    assert!(passes(&v));
    assert_eq!(v["limitations"].as_array().unwrap().len(), 1);
    let mut a = adversarial("occluded");
    a["outcome"] = json!("blocked");
    a["blocking_reason"] = json!("ContactLost");
    a["longest_contact_gap"] = json!(10);
    assert!(!passes(&pair("adversarial", a.clone(), a.clone())));
    a["longest_contact_gap"] = json!(31);
    assert!(passes(&pair("adversarial", a.clone(), a)));
}
#[test]
fn passage_opponent_must_complete() {
    let mut a = adversarial("passage");
    a["role"] = json!("opponent");
    assert!(!passes(&pair("adversarial", a.clone(), a.clone())));
    a["outcome"] = json!("completed");
    assert!(passes(&pair("adversarial", a.clone(), a)));
}
#[test]
fn clearance_gate_and_responsive_baseline() {
    let mut b = adversarial("late_clearance");
    b["outcome"] = json!("completed");
    b["congestion"] = json!({"clearance_to_search":20});
    let mut a = b.clone();
    a["congestion"]["clearance_to_search"] = json!(4);
    assert!(passes(&pair("adversarial", b.clone(), a.clone())));
    a["congestion"]["clearance_to_search"] = json!(6);
    assert!(!passes(&pair("adversarial", b, a.clone())));
    a["congestion"] = json!({"clearance_to_search":3,"early_starts":1});
    assert!(passes(&pair("adversarial", a.clone(), a)));
}
fn episode_fixture() -> (serde_json::Value, Vec<serde_json::Value>) {
    let mut row = pursuit();
    row["ticks"] = json!(3);
    row["episodes"] =
        json!([{"start":1,"end":1,"first_ready":null},{"start":3,"end":3,"first_ready":3}]);
    for c in [
        "reversals",
        "replans",
        "prediction_ticks",
        "prediction_fallbacks",
    ] {
        row[c] = json!(3);
    }
    let records=(1..=3).map(|tick|json!({"scenario":"lateral","chassis":"mech","seed":1,"fire":false,"tick":tick,"result":{"ready_ticks":tick,"reversals":tick,"replans":tick,"prediction_ticks":tick,"prediction_fallbacks":tick}})).collect();
    (row, records)
}
fn summarize(
    row: serde_json::Value,
    records: Vec<serde_json::Value>,
) -> anyhow::Result<serde_json::Value> {
    let d = tempfile::tempdir()?;
    let p = d.path().join("trace");
    std::fs::write(
        &p,
        records.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )?;
    evidence::episodes(&json!([row]), &[p])
}
#[test]
fn episode_gaps_and_reconciliation() {
    let (r, t) = episode_fixture();
    let v = summarize(r.clone(), t.clone()).unwrap();
    assert_eq!(v[0]["outside_contact_counters"]["replans"], 1);
    assert_eq!(v[0]["episodes"][1]["contact_gap_ticks"], 1);
    assert_eq!(v[0]["episodes"][1]["regain_opportunity_ticks"], 0);
    assert!(r["episodes"][0].get("replans").is_none());
    assert!(summarize(r.clone(), t[1..].to_vec()).is_err());
    let mut r = r;
    r["replans"] = json!(4);
    assert!(summarize(r, t).is_err());
}
#[test]
fn incomplete_identical_traces_are_not_replays() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("trace");
    std::fs::write(&p,"{\"scenario\":\"x\",\"chassis\":\"mech\",\"seed\":1,\"tick\":1,\"digest\":\"0123456789abcdef\"}\n").unwrap();
    let rows = json!([{"scenario":"x","chassis":"mech","seed":1,"ticks":2}]);
    assert!(evidence::validate_trace(&p, &rows, "movement").is_err());
}
#[test]
fn cpu_malformed_and_boundary_failures() {
    let d = tempfile::tempdir().unwrap();
    let b = d.path().join("b");
    let a = d.path().join("a");
    let header = "scenario,fire,repetitions,warmup_ticks,measured_ticks,p95_autopilot_ms,min_enabled,max_controller_expansions,peak_search_records,max_service_delay_ticks,p50_autopilot_ms,p50_heartbeat_ms,p95_heartbeat_ms\n";
    let row = "open,true,3,35,60,74,100,256,1000000,1,30,100,140\n";
    std::fs::write(&b, format!("{header}{row}")).unwrap();
    std::fs::write(&a, format!("{header}{}", row.replace(",74,", ",75,"))).unwrap();
    assert!(!passes(&evidence::cpu(&b, &a).unwrap()));
    std::fs::write(&a, format!("{header}{}", row.replace(",74,", ",NaN,"))).unwrap();
    assert!(evidence::cpu(&b, &a).is_err());
    std::fs::write(&a, format!("{header}{row}{row}")).unwrap();
    assert!(evidence::cpu(&b, &a).is_err());
}
#[test]
fn event_responses_are_measured_from_requested_controls() {
    let (mut row, traces) = episode_fixture();
    row["script_events"] = json!([[2, "stop_requested"], [4, "move_requested"]]);
    let v = summarize(row, traces).unwrap();
    assert_eq!(v[0]["event_responses"][0]["first_opportunity_delay"], 0);
    assert!(v[0]["event_responses"][1]["first_opportunity_delay"].is_null());
}

#[test]
fn timeline_rejects_gaps_and_counts_committed_decisions() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("trace.jsonl");
    let row = |tick, reason| {
        json!({"scenario":"x","chassis":"mech","seed":1,"fire":false,"tick":tick,
        "pursuit":{"evidence":{"reason":reason},"goal":[1,2]},"result":{}})
        .to_string()
    };
    std::fs::write(
        &path,
        format!("{}\n{}\n", row(1, "direct_score"), row(2, "predicted")),
    )
    .unwrap();
    let report = evidence::timeline(&path).unwrap();
    let case = report["cases"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap();
    assert_eq!(case["first_prediction"], 2);
    assert_eq!(case["decision_changes"], 1);
    std::fs::write(
        &path,
        format!("{}\n{}\n", row(1, "direct_score"), row(3, "predicted")),
    )
    .unwrap();
    assert!(evidence::timeline(&path).is_err());
}
