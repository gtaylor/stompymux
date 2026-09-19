//! Sandbox removal contract: the eight runtime-control globals stay absent while
//! the permitted library tables and the module loader remain installed.
mod support;
use support::isolated_scripts;

/// C lua_install_sandbox (lua_runtime.c:270-313) nils sixteen runtime-control
/// globals and reinstalls only the module loader; scripts can never recover the
/// coroutine/GC/environment-control entry points.
#[tokio::test(flavor = "current_thread")]
async fn blocked_runtime_globals_stay_absent_after_sandbox_install() {
    let (_d, _c, s) = isolated_scripts().await;
    s.eval_callback::<()>(
        r#"
        -- The eight actionable removals plus the other eight blocked names.
        local blocked = {
          'collectgarbage','coroutine','getfenv','load','loadstring',
          'module','package','setfenv',
          'io','os','debug','jit','ffi','dofile','loadfile',
        }
        for _, name in ipairs(blocked) do
          assert(_G[name] == nil, name)
          assert(rawget(_G, name) == nil, name)
          assert(type(_G[name]) == 'nil', name)
        end
        -- Adjacent edges: a long _G key list and lookups from library code
        -- cannot smuggle the removed entry points back.
        assert(string.rep('ab', 2) == 'abab')
        assert(type(string) == 'table' and type(table) == 'table')
        assert(type(math) == 'table' and type(bit) == 'table')
        assert(type(bit.tobit) == 'function' and type(bit.band) == 'function')
        assert(type(require) == 'function')
        local seen = {}
        for key in pairs(_G) do
          seen[key] = true
          assert(key ~= 'coroutine' and key ~= 'collectgarbage', key)
        end
        assert(seen.string and seen.table and seen.math and seen.bit)
        assert(not seen.coroutine and not seen.getfenv and not seen.setfenv)
        assert(not seen.load and not seen.loadstring and not seen.module)
        assert(not seen.package)
        -- Assigning a shadow value cannot revive the loader tables the sandbox
        -- dropped; require still resolves only editable game modules.
        collectgarbage = nil
        local ok, err = pcall(function() return require('no_such_module') end)
        assert(not ok and mux.error.is(err, mux.error.codes.module.unavailable))
        "#,
    )
    .unwrap();
}
