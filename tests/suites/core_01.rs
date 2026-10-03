//! Integration suite for core 01 scenarios.

/// Every top-level scenario must be compiled by exactly one explicit suite. A shared
/// helper module, which holds no tests of its own, may be included by several suites but
/// must still be included by at least one.
#[test]
fn every_scenario_is_in_one_suite() {
    use std::collections::BTreeMap;
    use std::fs;

    let tests = support::repository_root().join("tests");
    let mut expected = BTreeMap::new();
    for entry in fs::read_dir(&tests).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        let has_tests = source.lines().any(|line| {
            let line = line.trim_start();
            line.starts_with("#[test]") || line.starts_with("#[tokio::test")
        });
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        expected.insert(name, has_tests);
    }

    let mut included: BTreeMap<String, usize> = BTreeMap::new();
    for suite in fs::read_dir(tests.join("suites")).unwrap() {
        let path = suite.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }

        for line in fs::read_to_string(path).unwrap().lines() {
            let Some(scenario) = line
                .strip_prefix("#[path = \"../")
                .and_then(|line| line.strip_suffix("\"]"))
            else {
                continue;
            };
            if !scenario.contains('/') {
                *included.entry(scenario.to_owned()).or_default() += 1;
            }
        }
    }

    let mut problems = Vec::new();
    for (name, has_tests) in &expected {
        match (included.get(name).copied().unwrap_or(0), has_tests) {
            (1, _) => {}
            (0, _) => problems.push(format!("{name} is not included by any suite")),
            (count, true) => problems.push(format!("{name} is included by {count} suites")),
            (_, false) => {}
        }
    }
    for name in included.keys() {
        if !expected.contains_key(name) {
            problems.push(format!("{name} is included but does not exist"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

use stompymux_test_support as support;

#[path = "../access.rs"]
mod access;

#[path = "../failure_event_parity.rs"]
mod failure_event_parity;

#[path = "../foundation.rs"]
mod foundation;

#[path = "../help.rs"]
mod help;

#[path = "../lua_autopilot.rs"]
mod lua_autopilot;
#[path = "../lua_btech_constants.rs"]
mod lua_btech_constants;

#[path = "../lua_btech_repair_contracts.rs"]
mod lua_btech_repair_contracts;

#[path = "../lua_command_access_contracts.rs"]
mod lua_command_access_contracts;

#[path = "../lua_comsys_flags_contracts.rs"]
mod lua_comsys_flags_contracts;

#[path = "../lua_contract_inventory.rs"]
mod lua_contract_inventory;

#[path = "../lua_error_catalog_contracts.rs"]
mod lua_error_catalog_contracts;

#[path = "../lua_shipped_package_contracts.rs"]
mod lua_shipped_package_contracts;

#[path = "../message_cache.rs"]
mod message_cache;

#[path = "../persistence.rs"]
mod persistence;

#[path = "../policies.rs"]
mod policies;

#[path = "../sqlite.rs"]
mod sqlite;

#[path = "../text.rs"]
mod text;

#[path = "../lua_tactical.rs"]
mod lua_tactical;
#[path = "../lua_tactical_director.rs"]
mod lua_tactical_director;
