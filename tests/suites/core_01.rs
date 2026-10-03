//! Integration suite for core 01 scenarios.

/// Every top-level scenario must be compiled by exactly one explicit suite.
#[test]
fn every_scenario_is_in_one_suite() {
    use std::fs;

    let tests = support::repository_root().join("tests");
    let mut scenarios: Vec<_> = fs::read_dir(&tests)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .map(|path| path.file_name().unwrap().to_str().unwrap().to_owned())
        .collect();
    scenarios.sort();

    let mut assigned = Vec::new();
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
                assigned.push(scenario.to_owned());
            }
        }
    }
    assigned.sort();
    assert_eq!(assigned, scenarios);
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
