//! Check that generated LuaLS package functions exist in the Rust runtime.

use crate::support;

use std::collections::BTreeSet;

use mlua::{Table, Value};
use support::isolated_scripts;

/// Resolve each package function through its public Lua namespace.
#[tokio::test(flavor = "current_thread")]
async fn declared_package_functions_exist() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    let lua = scripts.inspect_lua();
    let mut missing = BTreeSet::new();
    for source in [
        include_str!("../game/lua/types/mux.d.lua"),
        include_str!("../game/lua/types/btech.d.lua"),
    ] {
        for line in source.lines() {
            let Some(declaration) = line.strip_prefix("function ") else {
                continue;
            };
            let Some((symbol, _)) = declaration.split_once('(') else {
                continue;
            };
            if symbol.contains(':') {
                continue;
            }
            let public = symbol.replace("mux_", "mux.").replace("btech_", "btech.");
            let mut value = Value::Table(lua.globals());
            for segment in public.split('.') {
                value = match value {
                    Value::Table(table) => table.get::<Value>(segment).unwrap_or(Value::Nil),
                    _ => Value::Nil,
                };
            }
            if !matches!(value, Value::Function(_)) {
                missing.insert(public);
            }
        }
    }
    assert!(
        missing.is_empty(),
        "LuaLS has unregistered functions: {missing:#?}"
    );
}

/// Walk public package tables and collect functions visible to a game callback.
fn package_functions(table: Table, prefix: &str, found: &mut BTreeSet<String>) {
    for pair in table.pairs::<String, Value>() {
        let (name, value) = pair.unwrap();
        let path = format!("{prefix}.{name}");
        match value {
            Value::Function(_) => {
                found.insert(path);
            }
            Value::Table(child) => package_functions(child, &path, found),
            _ => {}
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn all_public_package_functions_have_declarations() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    let lua = scripts.inspect_lua();
    let mut actual = BTreeSet::new();
    for module in ["mux", "btech"] {
        let package = lua.globals().get::<Table>(module).unwrap();
        package_functions(package, module, &mut actual);
    }
    let mut declared = BTreeSet::new();
    for source in [
        include_str!("../game/lua/types/mux.d.lua"),
        include_str!("../game/lua/types/btech.d.lua"),
    ] {
        for line in source.lines() {
            let Some(declaration) = line.strip_prefix("function ") else {
                continue;
            };
            let Some((symbol, _)) = declaration.split_once('(') else {
                continue;
            };
            if !symbol.contains(':') {
                declared.insert(symbol.replace("mux_", "mux.").replace("btech_", "btech."));
            }
        }
    }
    let missing = actual.difference(&declared).collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "Rust package functions lack LuaLS declarations: {missing:#?}"
    );
}
