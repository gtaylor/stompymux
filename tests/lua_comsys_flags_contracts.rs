//! C-contract coverage for the typed `mux.comsys.flags` constants.
//!
//! Oracle: btmux-khi/src/mux/lua/packages/mux/comsys/mux_comsys_channel_flag_bindings.c
//! (CHANNEL_FLAGS at lines 61-65; namespace lookup, immutability and metatable
//! protection in the same file; ChannelFlags list order PUBLIC, LOUD, TRANSPARENT).
use std::{cell::RefCell, rc::Rc, sync::Arc};
use stompymux_rs::{
    Scripts,
    lua::{RuntimeMode, sources::Sources},
};

use crate::support;
use support::isolated_scripts;

/// The immutable namespace contract holds in live and checking runtimes.
fn verify(scripts: &Scripts) {
    scripts
        .eval_callback::<()>(
            r#"
            local flags = mux.comsys.flags
            assert(type(flags) == 'userdata')
            assert(getmetatable(flags) == 'protected channel flag namespace metatable')
            for _, name in ipairs({'PUBLIC', 'LOUD', 'TRANSPARENT'}) do
              local value = flags[name]
              assert(type(value) == 'userdata', name)
              assert(tostring(value) == name, name)
              assert(getmetatable(value) == 'protected channel flag constant metatable', name)
              assert(select('#', flags[name]) == 1, name)
              assert(value == flags[name], name)
            end
            assert(flags.PUBLIC ~= flags.LOUD and flags.LOUD ~= flags.TRANSPARENT)
            assert(flags.PUBLIC ~= flags.TRANSPARENT)
            -- Cross-namespace separation; the world-flag constant leads because
            -- its C __eq compares operands without userdata type checks.
            assert(mux.world.flags.CONNECTED ~= flags.PUBLIC)
            assert(btech.unit.types.MECH ~= flags.PUBLIC)
            local ok, err = mux.error.pcall(function() return flags.NO_SUCH end)
            assert(not ok and err.code == 'mux.channel_flag.invalid')
            assert(err.detail.argument == 2)
            assert(err.message:find("unknown channel flag constant 'NO_SUCH'", 1, true))
            ok, err = mux.error.pcall(function() return flags['no_such'] end)
            assert(not ok and err.code == 'mux.channel_flag.invalid')
            assert(err.message:find("unknown channel flag constant 'no_such'", 1, true))
            ok, err = mux.error.pcall(function() return flags[42] end)
            assert(not ok and err.code == 'mux.channel_flag.invalid')
            assert(err.detail.argument == 2)
            assert(err.message:find("unknown channel flag constant '42'", 1, true))
            ok, err = mux.error.pcall(function() return flags[false] end)
            assert(not ok and err.code == 'mux.channel_flag.invalid')
            assert(err.message:find('channel flag name must be a string', 1, true))
            ok, err = mux.error.pcall(function() flags.NEW = 1 end)
            assert(not ok and err.code == 'mux.channel_flag.invalid' and err.detail == nil)
            assert(err.message:find('channel flag values are immutable', 1, true))
            ok, err = mux.error.pcall(function() flags.PUBLIC.NEW = 1 end)
            assert(not ok and err.code == 'mux.channel_flag.invalid' and err.detail == nil)
            "#,
        )
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn channel_flag_constants_are_typed_immutable_and_available_while_checking() {
    let (_directory, config, live) = isolated_scripts().await;
    verify(&live);
    let sources = Arc::new(Sources::read(&config).unwrap());
    let checking = Scripts::from_sources(
        &config,
        Rc::new(RefCell::new(live.world().clone())),
        live.help().clone(),
        sources,
        RuntimeMode::Checking,
    )
    .unwrap();
    verify(&checking);
}

/// Channel flag collections list set flags in the C PUBLIC, LOUD, TRANSPARENT
/// order and reject untyped flag arguments with the C error code.
#[tokio::test(flavor = "current_thread")]
async fn channel_flag_collections_list_in_c_order_and_check_flag_types() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    scripts
        .eval_callback::<()>(
            r#"
            local channel = mux.comsys.create_channel('Flags')
            local set = channel:flags()
            assert(type(set) == 'userdata')
            assert(tostring(set) == 'channel_flags(Flags)')
            assert(set:add(mux.comsys.flags.TRANSPARENT))
            assert(set:add(mux.comsys.flags.PUBLIC))
            assert(set:add(mux.comsys.flags.LOUD))
            assert(not set:add(mux.comsys.flags.LOUD))
            local listed = set:list()
            assert(select('#', set:list()) == 1)
            assert(#listed == 3)
            assert(listed[1] == mux.comsys.flags.PUBLIC and tostring(listed[1]) == 'PUBLIC')
            assert(listed[2] == mux.comsys.flags.LOUD and tostring(listed[2]) == 'LOUD')
            assert(listed[3] == mux.comsys.flags.TRANSPARENT
              and tostring(listed[3]) == 'TRANSPARENT')
            assert(select('#', set:has(mux.comsys.flags.LOUD)) == 1)
            assert(set:has(mux.comsys.flags.LOUD)
              and set:has(mux.comsys.flags.PUBLIC)
              and set:has(mux.comsys.flags.TRANSPARENT))
            assert(set:remove(mux.comsys.flags.PUBLIC))
            assert(not set:remove(mux.comsys.flags.PUBLIC))
            assert(not set:has(mux.comsys.flags.PUBLIC) and #set:list() == 2)
            for _, bad in ipairs({mux.world.flags.CONNECTED, 'PUBLIC', false}) do
              local ok, err = mux.error.pcall(set.has, set, bad)
              assert(not ok and err.code == 'mux.channel_flag.invalid', tostring(bad))
              assert(err.message:find('expected a mux.comsys.flags constant', 1, true))
              local ok2, err2 = mux.error.pcall(set.add, set, bad)
              assert(not ok2 and err2.code == 'mux.channel_flag.invalid')
            end
            "#,
        )
        .unwrap();
}
