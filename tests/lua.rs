//! Built-in package initialization and sandbox ordering across editable game modules.
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{Config, Scripts, persistence};

use crate::support;
use support::copy;

/// Every built-in and sandbox restriction must exist before the earliest game module runs.
#[tokio::test(flavor = "current_thread")]
async fn builtins_and_sandbox_precede_lexical_game_loading() {
    let temp = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        temp.path(),
    );
    let config = Config::load(temp.path()).unwrap();
    let directory = config.lua_dir();
    let helper = directory.join("packages/probe");
    std::fs::create_dir_all(&helper).unwrap();
    std::fs::write(
        helper.join("nested.lua"),
        r#"
        local api = require('mux')
        assert(require==_G.require and __mux_module_root==2)
        assert(rawequal(api, mux))
        assert(type(api.world.object) == 'function')
        assert(type(api.world.pemit) == 'function')
        assert(type(api.session.connected_players) == 'function')
        assert(type(api.config.get) == 'function')
        assert(type(api.text.markdown) == 'function')
        assert(type(api.comsys.create_channel) == 'function')
        assert(api.world.object(2):flags():has(api.world.flags.WIZARD))
        assert(api.text.markup('[bold]ok[/]') == '[bold]ok[/]')
        assert(type(api.text.markdown('**ok**')) == 'userdata')
        assert(not pcall(api.session.flow_start))
        for _, name in ipairs({
            'io','os','debug','package','coroutine','ffi','jit','dofile','loadfile',
            'loadstring','load','collectgarbage','module','getfenv','setfenv'
        }) do
            assert(_G[name] == nil)
        end
        local ok, err = pcall(require, '../outside')
        assert(not ok and mux.error.is(err, mux.error.codes.module.invalid))
        ok, err = pcall(require, 'mux.world')
        assert(not ok and mux.error.is(err, mux.error.codes.module.unavailable))
        ok, err = pcall(require, 'string')
        assert(not ok and mux.error.is(err, mux.error.codes.module.unavailable))
        return api
        "#,
    )
    .unwrap();
    std::fs::write(
        directory.join("packages/same_root.lua"),
        "return {source='packages'}",
    )
    .unwrap();
    std::fs::write(
        directory.join("packages/probe_state.lua"),
        "return {order={},root_load_count=0}",
    )
    .unwrap();
    std::fs::write(
        directory.join("packages/cycle_a.lua"),
        "return require('cycle_b')",
    )
    .unwrap();
    std::fs::write(
        directory.join("packages/cycle_b.lua"),
        "return require('cycle_a')",
    )
    .unwrap();
    std::fs::write(directory.join("packages/not_a_table.lua"), "return 7").unwrap();
    for (path, source) in [
        (
            "object_logic/000_probe.lua",
            "local s=require('probe_state'); assert(require==_G.require and __mux_module_root==0); assert(require('probe.nested') == mux); table.insert(s.order,'object-first'); module_leak='object'; return {}",
        ),
        (
            "object_logic/010_root_loader.lua",
            "local s=require('probe_state'); local ok,value=pcall(require,'same_root'); assert(ok); s.pcall_required=value; s.root_required=require('same_root'); assert(s.root_required.source=='object_logic'); return {}",
        ),
        (
            "object_logic/same_root.lua",
            "local s=require('probe_state'); s.root_load_count=s.root_load_count+1; local module={source='object_logic'}; s.root_module=module; return module",
        ),
        (
            "object_logic/020_mutable_root.lua",
            "local s=require('probe_state'); __mux_module_root=2; s.mutable_required=require('same_root'); return {}",
        ),
        (
            "object_logic/zz_probe/nested.lua",
            "table.insert(require('probe_state').order,'object-last'); return {}",
        ),
        (
            "global_logic/000_probe.lua",
            "assert(require==_G.require and __mux_module_root==1); assert(require('probe.nested') == mux); table.insert(require('probe_state').order,'global-first'); return {}",
        ),
        (
            "global_logic/zz_probe/nested.lua",
            "table.insert(require('probe_state').order,'global-last'); return {}",
        ),
    ] {
        let path = directory.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }
    let world = persistence::load(&config.database()).await.unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let state: mlua::Table = scripts
        .inspect_lua()
        .load("return require('probe_state')")
        .eval()
        .unwrap();
    let order: Vec<String> = state.get("order").unwrap();
    assert_eq!(
        order,
        ["object-first", "object-last", "global-first", "global-last"]
    );
    scripts
        .inspect_lua()
        .load(
            r#"
            local state=require('probe_state')
            assert(state.root_load_count == 1 and rawequal(state.root_required, state.root_module))
            assert(rawequal(state.pcall_required,state.root_module))
            assert(state.mutable_required.source=='packages')
            assert(module_leak==nil)
            local ok, err = pcall(require, 'cycle_a')
            assert(not ok and mux.error.is(err, mux.error.codes.module.unavailable))
            ok, err = pcall(require, 'not_a_table')
            assert(not ok and mux.error.is(err, mux.error.codes.module.unavailable))
            assert(require('same_root' .. string.char(0) .. 'ignored').source == 'packages')
            ok, err = mux.error.pcall(require, string.rep('a', 4092))
            assert(not ok and err.code == 'mux.module.invalid' and err.message == 'module name is too long')
            "#,
        )
        .exec()
        .unwrap();
    assert_eq!(
        scripts
            .inspect_lua()
            .globals()
            .get::<mlua::Value>("package")
            .unwrap(),
        mlua::Value::Nil
    );
}
