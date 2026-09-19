//! Non-BattleTech Lua userdata, sandbox and shipped-package compatibility contracts.
use stompymux_rs::Scripts;

mod support;
use support::isolated_scripts;

/// The sandbox retains safe language helpers while hiding runtime control and loader state.
#[tokio::test(flavor = "current_thread")]
async fn sandbox_and_private_require_match_the_supported_mux_surface() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    scripts
        .eval_callback::<()>(
            r#"
            for _, name in ipairs({
              'io','os','debug','package','coroutine','ffi','jit','dofile','loadfile',
              'loadstring','load','collectgarbage','module','getfenv','setfenv'
            }) do
              assert(_G[name] == nil, name)
            end
            for _, name in ipairs({'string','table','math'}) do
              assert(type(_G[name]) == 'table')
              local ok, err = pcall(require, name)
              assert(not ok and mux.error.is(err, mux.error.codes.module.unavailable))
            end
            assert(require('mux') == mux and require('btech') == btech)
            local ok, err = pcall(require, '')
            assert(not ok and mux.error.is(err, mux.error.codes.module.invalid))
            ok, err = pcall(require, '.testing')
            assert(not ok and mux.error.is(err, mux.error.codes.module.invalid))
            ok, err = pcall(require, 'testing.')
            assert(not ok and mux.error.is(err, mux.error.codes.module.invalid))
            ok, err = pcall(require, 'missing.module')
            assert(not ok and mux.error.is(err, mux.error.codes.module.unavailable))
            ok, err = pcall(require, {})
            assert(not ok and type(err) ~= 'table')
            "#,
        )
        .unwrap();
}

/// Public userdata retain C string forms, identities, catalog separation and immutability.
#[tokio::test(flavor = "current_thread")]
async fn mux_userdata_contracts_cover_every_non_btech_handle_family() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    scripts
        .eval_callback::<()>(
            r#"
            local object = mux.world.object(1)
            local flags, powers = object:flags(), object:powers()
            local state = object:state('contracts')
            local channel = mux.comsys.create_channel('Contracts')
            local channel_flags = channel:flags()
            assert(tostring(object) == 'object(#1)')
            assert(tostring(flags) == 'flags(#1)')
            assert(tostring(powers) == 'powers(#1)')
            assert(tostring(state) == 'state(#1, contracts)')
            assert(tostring(channel) == 'channel(Contracts)')
            assert(tostring(channel_flags) == 'channel_flags(Contracts)')
            assert(tostring(mux.world.types.ROOM) == 'ROOM')
            assert(tostring(mux.world.flags.WIZARD) == 'WIZARD')
            assert(tostring(mux.world.powers.IDLE) == 'IDLE')
            assert(tostring(mux.world.locks.TAKE) == 'TAKE')
            assert(tostring(mux.comsys.flags.PUBLIC) == 'PUBLIC')
            assert(mux.world.object(1) == object)
            assert(mux.world.types.ROOM == mux.world.types.ROOM)
            assert(mux.world.flags.WIZARD == mux.world.flags.WIZARD)
            assert(mux.world.powers.IDLE == mux.world.powers.IDLE)
            assert(mux.world.locks.TAKE == mux.world.locks.TAKE)
            assert(mux.comsys.flags.PUBLIC == mux.comsys.flags.PUBLIC)
            assert(mux.world.flags.WIZARD ~= mux.world.powers.IDLE)
            for _, value in ipairs({
              object, flags, powers, state, channel, channel_flags,
              mux.world.types.ROOM, mux.world.types, mux.world.flags.WIZARD,
              mux.world.flags, mux.world.powers.IDLE, mux.world.powers,
              mux.world.locks.TAKE, mux.world.locks, mux.comsys.flags.PUBLIC,
              mux.comsys.flags, mux.error.codes.object
            }) do
              assert(not pcall(function() value.forged = true end))
            end
            "#,
        )
        .unwrap();
}

/// The three packages shipped with the game remain loadable and preserve their core behavior.
#[tokio::test(flavor = "current_thread")]
async fn shipped_packages_load_once_and_expose_their_c_contracts() {
    let (_directory, _config, scripts): (_, _, Scripts) = isolated_scripts().await;
    scripts
        .eval_callback::<()>(
            r#"
            local testing = require('testing')
            assert(require('testing') == testing)
            local suite = testing.suite('contracts', {tests={testing.test('ok', function() end)}})
            assert(suite.name == 'contracts' and suite.expect ~= nil and #suite.tests == 1)
            local ok, err = pcall(function() suite.expect.equal(1, 2) end)
            assert(not ok and mux.error.is(err, 'testing.assertion'))
            assert(err.detail.expected == 2 and err.detail.actual == 1)

            local policy = require('access_policy')
            assert(policy.evaluate({object=0, subject=1}, {namespace='contracts'}) == true)

            local appearances = require('object_appearances')
            assert(type(appearances.render_contents) == 'function')
            assert(type(appearances.render_exits) == 'function')
            assert(type(appearances.render_internal_appearance) == 'function')
            assert(type(appearances.render_contents({object=0, enactor=1})) == 'table')
            "#,
        )
        .unwrap();
}
