//! Source-backed contracts for the MUX world, Object, State, Flags, and Powers APIs.
use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::{RuntimeMode, Scripts, lua::sources::Sources};

use crate::support;
use support::isolated_scripts;

fn callback(scripts: &Scripts, source: &str) {
    scripts.eval_callback::<()>(source).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn pemit_validates_message_before_checking_and_object_identity() {
    let (_directory, config, live) = isolated_scripts().await;
    let checking = Scripts::from_sources(
        &config,
        Rc::new(RefCell::new(live.world().clone())),
        live.help().clone(),
        Arc::new(Sources::read(&config).unwrap()),
        RuntimeMode::Checking,
    )
    .unwrap();
    callback(
        &checking,
        r#"
        local ok,e=mux.error.pcall(function() mux.world.pemit(false,false) end)
        assert(not ok and type(e)=='table' and e.code=='mux.runtime'
          and e.message=="bad argument #2 to '?' (string expected, got boolean)", tostring(e))
        ok,e=mux.error.pcall(function() mux.world.pemit(false,'valid') end)
        assert(not ok and e.code=='mux.unavailable.checking', tostring(e))
        ok,e=mux.error.pcall(function() mux.world.pemit(false,42) end)
        assert(not ok and e.code=='mux.unavailable.checking', tostring(e))
        ok,e=mux.error.pcall(function() mux.world.pemit(false,'bad\0message') end)
        assert(not ok and e.code=='mux.connection.invalid' and e.detail.argument==2, tostring(e))
        ok,e=mux.error.pcall(function() mux.world.pemit(false,string.char(255)) end)
        assert(not ok and e.code=='mux.connection.invalid' and e.detail.argument==2, tostring(e))
    "#,
    );
}

#[tokio::test(flavor = "current_thread")]
async fn world_and_object_contracts_cover_creation_identity_and_relationships() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    callback(
        &scripts,
        r#"
        local function count(...) return select('#', ...), ... end
        local function is_error(call, code, detail)
          local ok, error_value = pcall(call)
          assert(not ok and mux.error.is(error_value, code), tostring(error_value))
          assert(error_value.message:find(detail, 1, true), error_value.message)
        end

        local god, room = mux.world.object(1), mux.world.object(0)
        assert(type(god) == 'userdata' and god == mux.world.object('1'), 'identity')
        assert(god ~= room and god:dbref() == 1 and tostring(god) == 'object(#1)', 'basic object')
        assert(god.__tostring(god) == 'object(#1)', 'direct tostring')
        assert(god.__eq(god, mux.world.object(1)) == true, 'direct eq')
        assert(god:name() == 'GOD' and tostring(god:type()) == 'PLAYER', 'name/type')

        local created_room = mux.world.create_object{
          type=mux.world.types.ROOM, name='Contract Room', zone=0,
        }
        local thing = mux.world.create_object{
          type=mux.world.types.THING, name='Contract Thing', location=created_room,
        }
        local exit = mux.world.create_object{
          type=mux.world.types.EXIT, name='Contract Exit', location=created_room,
          destination=room,
        }
        assert(thing:location() == created_room and thing:home() == created_room, 'thing references')
        assert(exit:destination() == room, 'exit destination')
        assert(created_room:zone() == room, 'room zone')
        local inherited = mux.world.create_object(setmetatable({}, {__index={
          type=mux.world.types.THING, name='Inherited Thing', location=created_room,
        }}))
        assert(inherited:location() == created_room and inherited:home() == created_room,
          'create inherited fields')

        thing:set_name('Renamed', 'ignored')
        thing:set_description('public', 'ignored')
        thing:set_internal_description(42, 'ignored')
        assert(thing:name() == 'Renamed')
        assert(thing:description() == 'public')
        assert(thing:internal_description() == '42')
        assert(count(thing:set_description(nil)) == 0 and thing:description() == nil)
        assert(count(thing:set_internal_description(nil)) == 0)

        thing:set_zone(room, 'ignored')
        thing:set_affiliation(god, 'ignored')
        thing:set_home(room, 'ignored')
        assert(thing:zone() == room and thing:affiliation() == god and thing:home() == room)
        thing:set_zone(nil)
        thing:set_affiliation(nil)
        assert(thing:zone() == nil and thing:affiliation() == nil)
        exit:set_destination(nil, 'ignored')
        assert(exit:destination() == nil)

        local contents = created_room:contents()
        assert(#contents == 3)
        local things = created_room:contents{types={mux.world.types.THING}}
        assert(#things == 2 and
          ((things[1] == thing and things[2] == inherited) or
           (things[1] == inherited and things[2] == thing)))
        local inherited_filter = setmetatable({}, {__index={types={mux.world.types.EXIT}}})
        assert(#created_room:contents(inherited_filter) == 1)
        assert(#created_room:contents{types={}} == 0)
        local listed = mux.world.list_objects{types={mux.world.types.THING}}
        assert(#listed > 0)
        for _, object in ipairs(listed) do assert(object:type() == mux.world.types.THING) end
        assert(#mux.world.list_objects(setmetatable({}, {__index={types={mux.world.types.EXIT}}})) > 0)

        assert(count(mux.world.teleport_object{object=thing,destination=room}) == 0)
        assert(thing:location() == room)
        assert(count(mux.world.pemit(god, 'world contract')) == 0)
        assert(type(mux.world.lock_passes{object=room,enactor=god,lock=mux.world.locks.TAKE}) == 'boolean')

        is_error(function() mux.world.create_object(false) end, mux.error.codes.arg.invalid,
          'options must be a table')
        is_error(function() mux.world.create_object{type=mux.world.types.THING,name='x'} end,
          mux.error.codes.arg.invalid, 'options.location is required')
        is_error(function() thing:set_home() end, mux.error.codes.arg.invalid, 'home is required')
        is_error(function() exit:set_destination() end, mux.error.codes.arg.invalid,
          'destination is required; pass nil to unlink')
        is_error(function() thing:set_name() end, mux.error.codes.arg.invalid, 'name is required')
        is_error(function() mux.world.pemit(god, 'a\0b') end,
          mux.error.codes.connection.invalid, 'message contains an embedded NUL byte')

        local held = thing
        local held_flags, held_state = held:flags(), held:state('held')
        assert(count(mux.world.destroy_object(thing)) == 0)
        assert(held_flags:has(mux.world.flags.GOING))
        assert(held_state:get('absent') == nil)
        local doomed = mux.world.create_object{
          type=mux.world.types.THING,name='Inherited destroy',location=room,
        }
        assert(count(mux.world.destroy_object(doomed,
          setmetatable({}, {__index={override=false}}))) == 0)
        mux.check_db()
        is_error(function() return held:name() end, mux.error.codes.object.invalid, 'stale')
        is_error(function() return tostring(held_flags) end, mux.error.codes.object.invalid, 'stale')
        is_error(function() return held_state:get('absent') end, mux.error.codes.object.invalid,
          'invalid state object')
        "#,
    );
}

#[tokio::test(flavor = "current_thread")]
async fn state_flags_and_powers_preserve_types_arity_order_and_atomicity() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    callback(
        &scripts,
        r#"
        local function count(...) return select('#', ...), ... end
        local function is_error(call, code, detail)
          local ok, error_value = pcall(call)
          assert(not ok and mux.error.is(error_value, code), tostring(error_value))
          assert(error_value.message:find(detail, 1, true), error_value.message)
        end
        local object = mux.world.object(0)
        local state = object:state('world_contracts')
        assert(tostring(state) == 'state(#0, world_contracts)')
        assert(state.__tostring(state) == 'state(#0, world_contracts)')
        assert(count(state:get('absent')) == 1 and state:get('absent') == nil)
        local marker = {}
        assert(state:get('absent', marker, 'ignored') == marker)

        assert(count(state:set('z', 'bytes\0\255', 'ignored')) == 0)
        state:set('a', true)
        state:set('two', 9.5)
        state:set_many{middle=7, toggle=false}
        assert(state:has('z') and state:has('two'))
        assert(state:get('z') == 'bytes\0\255' and state:get('two') == 9.5)
        local keys = state:keys()
        assert(table.concat(keys, ',') == 'a,middle,toggle,two,z')
        local entries = state:entries()
        assert(#entries == 5 and entries[1].key == 'a' and entries[5].key == 'z')
        local many = state:get_many{'z', 'two', 'missing'}
        assert(many.z == 'bytes\0\255' and many.two == 9.5 and many.missing == nil)
        assert(state:delete('middle') and not state:delete('middle'))
        assert(count(state:set('a', nil)) == 0 and not state:has('a'))

        is_error(function() state:set('omitted') end, mux.error.codes.state.invalid,
          'state values must be strings, booleans, or finite numbers')
        is_error(function() state:set('bad', {}) end, mux.error.codes.state.invalid,
          'state values must be strings, booleans, or finite numbers')
        is_error(function() state:set('bad', 0/0) end, mux.error.codes.state.invalid,
          'state values must be strings, booleans, or finite numbers')
        is_error(function() state:get('bad\0key') end, mux.error.codes.state.invalid,
          'invalid state key')
        is_error(function() state:get(2) end, mux.error.codes.state.invalid,
          'invalid state key')
        is_error(function() object:state('bad\0namespace') end, mux.error.codes.state.invalid,
          'invalid state namespace')
        is_error(function() state:set_many{[1]='bad'} end, mux.error.codes.state.invalid,
          'state update keys must be strings')
        assert(not state:has('bad'))

        local flags, powers = object:flags(), object:powers()
        assert(tostring(flags) == 'flags(#0)' and flags.__tostring(flags) == 'flags(#0)')
        assert(tostring(powers) == 'powers(#0)' and powers.__tostring(powers) == 'powers(#0)')
        local before = flags:has(mux.world.flags.DARK)
        local changed = flags:add(mux.world.flags.DARK)
        assert(changed == (not before) and flags:has(mux.world.flags.DARK))
        assert(flags:add(mux.world.flags.DARK) == false)
        assert(flags:remove(mux.world.flags.DARK) == true)
        assert(flags:remove(mux.world.flags.DARK) == false)
        assert(type(flags:list()) == 'table' and type(powers:list()) == 'table')
        is_error(function() flags:has(mux.world.powers.IDLE) end,
          mux.error.codes.flag.invalid, 'expected a mux.world.flags constant')
        is_error(function() powers:has(mux.world.flags.DARK) end,
          mux.error.codes.power.invalid, 'expected a mux.world.powers constant')

        assert(mux.world.types['ROOM'] == mux.world.types.ROOM)
        assert(getmetatable(mux.world.types) == 'protected object type namespace metatable')
        assert(getmetatable(mux.world.types.ROOM) == 'protected object type constant metatable')
        assert(getmetatable(object:type()) == 'protected object type constant metatable')
        assert(getmetatable(mux.world.flags) == 'protected flag or power namespace metatable')
        assert(getmetatable(mux.world.flags.DARK) == 'protected flag or power constant metatable')
        assert(getmetatable(mux.world.powers) == 'protected flag or power namespace metatable')
        assert(getmetatable(mux.world.powers.IDLE) == 'protected flag or power constant metatable')
        assert(getmetatable(mux.world.locks) == 'protected lock namespace metatable')
        assert(getmetatable(mux.world.locks.TAKE) == 'protected lock constant metatable')
        is_error(function() return mux.world.types[12] end, mux.error.codes.arg.invalid,
          'object type name must be a string')
        is_error(function() return mux.world.flags[123] end, mux.error.codes.flag.invalid,
          "unknown flag constant '123'")
        is_error(function() return mux.world.powers[123] end, mux.error.codes.power.invalid,
          "unknown power constant '123'")
        is_error(function() return mux.world.locks[123] end, mux.error.codes.arg.invalid,
          "unknown lock '123'")
        is_error(function() return mux.world.flags.NO_SUCH_FLAG end, mux.error.codes.flag.invalid,
          "unknown flag constant 'NO_SUCH_FLAG'")
        is_error(function() return mux.world.powers.NO_SUCH_POWER end, mux.error.codes.power.invalid,
          "unknown power constant 'NO_SUCH_POWER'")
        is_error(function() return mux.world.locks.NO_SUCH_LOCK end, mux.error.codes.arg.invalid,
          "unknown lock 'NO_SUCH_LOCK'")
        "#,
    );
}

#[tokio::test(flavor = "current_thread")]
async fn world_state_mutations_survive_the_public_persistence_path() {
    let (_directory, config, scripts) = isolated_scripts().await;
    let object = scripts
        .eval_callback::<i64>(
            r#"
            local object=mux.world.create_object{
              type=mux.world.types.THING,name='Persisted world contract',location=0,
            }
            object:set_description('persisted description')
            object:set_zone(0)
            object:state('contracts'):set_many{
              text='bytes\0\255', boolean=true, integer=9007199254740991,
              number=1.25,
            }
            object:flags():add(mux.world.flags.DARK)
            return object:dbref()
            "#,
        )
        .unwrap();
    let persisted = scripts.world().clone();
    stompymux_rs::persistence::save(&config.database(), &persisted)
        .await
        .unwrap();
    let loaded = stompymux_rs::persistence::load(&config.database())
        .await
        .unwrap();
    let reloaded = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
    reloaded
        .inspect_lua()
        .globals()
        .set("persisted_object", object)
        .unwrap();
    callback(
        &reloaded,
        r#"
        local object=mux.world.object(persisted_object)
        assert(object:name()=='Persisted world contract')
        assert(object:description()=='persisted description' and object:zone()==mux.world.object(0))
        assert(object:flags():has(mux.world.flags.DARK))
        local state=object:state('contracts')
        assert(state:get('text')=='bytes\0\255' and state:get('boolean')==true)
        assert(state:get('integer')==9007199254740991 and state:get('number')==1.25)
        "#,
    );
}
