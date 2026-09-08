//! Built-in package initialization and sandbox ordering across editable game modules.
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{Config, Scripts, persistence};

mod support;
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
        for _, name in ipairs({'io','os','debug','ffi','jit'}) do
            assert(_G[name] == nil)
            assert(package.loaded[name] == nil)
            assert(package.preload[name] == nil)
            assert(not pcall(require, name))
        end
        assert(dofile == nil and loadfile == nil and package.loadlib == nil)
        assert(package.cpath == '')
        assert(not pcall(require, '../outside'))
        assert(not pcall(require, 'mux.world'))
        return api
        "#,
    )
    .unwrap();
    for (path, source) in [
        (
            "object_logic/000_probe.lua",
            "assert(require('probe.nested') == mux); _probe_order={'object-first'}; return {}",
        ),
        (
            "object_logic/zz_probe/nested.lua",
            "table.insert(_probe_order,'object-last'); return {}",
        ),
        (
            "global_logic/000_probe.lua",
            "assert(require('probe.nested') == mux); table.insert(_probe_order,'global-first'); return {}",
        ),
        (
            "global_logic/zz_probe/nested.lua",
            "table.insert(_probe_order,'global-last'); return {}",
        ),
    ] {
        let path = directory.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }
    let world = persistence::load(&config.database()).await.unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let order: Vec<String> = scripts.inspect_lua().globals().get("_probe_order").unwrap();
    assert_eq!(
        order,
        ["object-first", "object-last", "global-first", "global-last"]
    );
    let package: mlua::Table = scripts.inspect_lua().globals().get("package").unwrap();
    assert_eq!(package.get::<String>("path").unwrap(), "");
    assert_eq!(package.get::<mlua::Table>("loaders").unwrap().raw_len(), 1);
}
