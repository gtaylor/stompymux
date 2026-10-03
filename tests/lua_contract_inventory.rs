//! Completeness and executable presence checks for the pinned native Lua contract ledger.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use stompymux_rs::Scripts;

use crate::support;

const REFERENCE_REVISION: &str = "2bbbe6fcdbabe69e229d73089f44bf38f91c0591";
const RUST_BASELINE_REVISION: &str = "7917a95642e2fe690bf0ac03a018a0e2b7342b21";

fn ledger() -> Value {
    serde_json::from_str(include_str!("fixtures/lua-api-contracts.json")).unwrap()
}

fn string<'a>(entry: &'a Value, field: &str) -> &'a str {
    entry[field]
        .as_str()
        .unwrap_or_else(|| panic!("{} has no string {field}", entry["symbol"]))
}

#[test]
fn ledger_is_complete_well_formed_and_keeps_the_legacy_fixture_compatible() {
    let ledger = ledger();
    assert_eq!(ledger["schema_version"], 1);
    assert_eq!(ledger["reference_revision"], REFERENCE_REVISION);
    assert_eq!(ledger["rust_baseline_revision"], RUST_BASELINE_REVISION);
    let namespace_classes = ledger["constant_namespace_classes"].as_object().unwrap();
    for class in [
        "ChannelFlagNamespace",
        "AccessNamespace",
        "FlagNamespace",
        "LockNamespace",
        "PowerNamespace",
        "ObjectTypeNamespace",
        "BtechAutopilotOrderNamespace",
        "BtechAutopilotDirectionNamespace",
        "BtechAutopilotRoamModeNamespace",
        "BtechAutopilotAutogunModeNamespace",
        "BtechRepairOperationNamespace",
        "BtechUnitTypeNamespace",
        "BtechMovementTypeNamespace",
        "BtechSectionNamespace",
        "BtechTechnologyCodeNamespace",
        "BtechTechnologyGroupNamespace",
        "BtechFireModeNamespace",
        "BtechAmmunitionModeNamespace",
    ] {
        assert!(
            namespace_classes.contains_key(class),
            "unmapped C constant namespace {class}"
        );
    }
    assert_eq!(namespace_classes.len(), 18);

    let accepted = ledger["accepted_differences"].as_array().unwrap();
    assert_eq!(
        accepted.len(),
        8,
        "new exceptions require an explicit review"
    );
    let accepted_ids = accepted
        .iter()
        .map(|row| string(row, "id"))
        .collect::<BTreeSet<_>>();

    let entries = ledger["entries"].as_array().unwrap();
    let registered = ledger["registered_public_symbols"].as_array().unwrap();
    assert_eq!(
        registered.len(),
        201,
        "native registration inventory changed"
    );
    assert_eq!(
        ledger["registered_public_symbols_by_family"]["btech_native_arrays"]
            .as_array()
            .unwrap()
            .len(),
        112
    );
    assert_eq!(
        ledger["registered_public_symbols_by_family"]["mux_native_installers"]
            .as_array()
            .unwrap()
            .len(),
        89
    );
    for symbol in registered {
        assert!(
            entries
                .iter()
                .any(|entry| entry["kind"] == "callable" && entry["symbol"] == *symbol),
            "registered callable {} omitted from inventory",
            symbol.as_str().unwrap()
        );
    }
    assert_eq!(
        entries.len(),
        673,
        "reference inventory changed; regenerate and audit it"
    );
    let mut symbols = BTreeSet::new();
    let mut counts = BTreeMap::new();
    for entry in entries {
        let symbol = string(entry, "symbol");
        let kind = string(entry, "kind");
        let status = string(entry, "status");
        assert!(symbols.insert((kind, symbol)), "duplicate {kind} {symbol}");
        assert!(!string(entry, "package").is_empty(), "{symbol}: package");
        assert!(
            !string(entry, "reference").is_empty(),
            "{symbol}: reference"
        );
        assert!(!string(entry, "contract").is_empty(), "{symbol}: contract");
        assert!(
            !string(entry, "rust_evidence").is_empty(),
            "{symbol}: Rust evidence"
        );
        assert!(
            !string(entry, "acceptance").is_empty(),
            "{symbol}: acceptance"
        );
        // Superseded entries describe pinned C symbols deliberately replaced
        // by the new unit-attached autopilot contract without legacy aliases.
        assert!(matches!(
            status,
            "verified" | "actionable" | "prerequisite-blocked" | "superseded"
        ));
        if status == "prerequisite-blocked" {
            assert!(
                !string(entry, "prerequisite").is_empty(),
                "{symbol}: concrete prerequisite"
            );
        } else {
            assert!(
                string(entry, "prerequisite").is_empty(),
                "{symbol}: spurious prerequisite"
            );
        }
        for difference in entry["accepted_difference_ids"].as_array().unwrap() {
            assert!(
                accepted_ids.contains(difference.as_str().unwrap()),
                "{symbol}: unknown difference"
            );
        }
        *counts.entry((kind, status)).or_insert(0usize) += 1;
    }
    assert_eq!(counts.values().sum::<usize>(), 673);

    let legacy: Value = serde_json::from_str(include_str!("fixtures/lua-api.json")).unwrap();
    let legacy = legacy.as_array().unwrap();
    assert_eq!(legacy.len(), 89);
    for entry in legacy {
        let symbol = string(entry, "symbol");
        assert!(
            entries.iter().any(|candidate| {
                candidate["kind"] == "callable" && candidate["symbol"] == symbol
            }),
            "legacy callable {symbol} missing from full ledger"
        );
    }
}

async fn scripts() -> (tempfile::TempDir, Scripts) {
    let (directory, _, scripts) = support::isolated_scripts().await;
    (directory, scripts)
}

#[tokio::test(flavor = "current_thread")]
async fn every_verified_entry_resolves_in_an_initialized_rust_vm() {
    let (_directory, scripts) = scripts().await;
    let inventory = ledger();
    let lua = scripts.inspect_lua();
    lua.globals()
        .set(
            "contract_inventory",
            mlua::LuaSerdeExt::to_value(&lua, &inventory).unwrap(),
        )
        .unwrap();
    scripts.eval_callback::<()>(r#"
      local representatives = {
        Object = mux.world.object(1),
      }
      local channel = mux.comsys.create_channel("contract_inventory")
      representatives.Channel = channel
      representatives.ChannelFlags = channel:flags()
      representatives.Flags = representatives.Object:flags()
      representatives.Powers = representatives.Object:powers()
      representatives.State = representatives.Object:state("contract-inventory")
      representatives.Error = mux.error.new({code="testing.runtime", message="inventory"})

      local function resolve(path)
        local value = _G
        for part in path:gmatch("[^.]+") do
          assert(value ~= nil, path .. " missing before " .. part)
          value = value[part]
        end
        return value
      end
      local function error_code(code)
        local root, tail = code:match("^([^.]+)%.(.+)$")
        local value = mux.error.code_tree(root)
        for part in tail:gmatch("[^.]+") do value = value[part] end
        return value
      end

      for _, entry in ipairs(contract_inventory.entries) do
        if entry.status == "verified" then
          if entry.kind == "callable" then
            local class, method = entry.symbol:match("^([^:]+):(.+)$")
            local value = class and representatives[class][method] or resolve(entry.symbol)
            assert(type(value) == "function", entry.symbol)
          elseif entry.kind == "constant" then
            assert(resolve(entry.symbol) ~= nil, entry.symbol)
          elseif entry.kind == "error" then
            assert(tostring(error_code(entry.symbol)) == entry.symbol, entry.symbol)
          elseif entry.kind == "module-callable" then
            local module, tail = entry.symbol:match("^([^.]+)%.(.+)$")
            local value = require(module)
            if module == "testing" and tail:match("^expect%.") then
              value = value.suite("contract inventory", {tests={}})
            end
            for part in tail:gmatch("[^.]+") do
              assert(value ~= nil, entry.symbol .. " missing before " .. part)
              value = value[part]
            end
            assert(type(value) == "function", entry.symbol)
          elseif entry.kind == "receiver-callable" then
            local module = require(entry.package)
            local value = module.suite("contract inventory", {tests={}})
            for part in entry.receiver_path:gmatch("[^.]+") do value = value[part] end
            local method = entry.symbol:match("%.([^.]+)$")
            assert(type(value[method]) == "function", entry.symbol)
          elseif entry.kind == "sandbox-global" then
            local present = _G[entry.symbol] ~= nil
            assert(present == entry.expected_presence, entry.symbol)
          elseif entry.kind == "sandbox-function" or entry.kind == "sandbox-constant" then
            assert(type(resolve(entry.symbol)) == entry.expected_type, entry.symbol)
          elseif entry.kind == "package" then
            if entry.symbol == "access_policy" or entry.symbol == "object_appearances" or entry.symbol == "testing" then
              assert(type(require(entry.symbol)) == "table", entry.symbol)
            else
              -- Typed constant namespaces are userdata with protected metatables in C and Rust.
              local resolved = type(resolve(entry.symbol))
              assert(resolved == "table" or resolved == "userdata", entry.symbol)
            end
          end
        end
      end
    "#).unwrap();
}
