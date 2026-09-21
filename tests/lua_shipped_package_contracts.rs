//! Host-integration contracts for the shipped Lua packages: testing, access_policy
//! and object_appearances. The pinned Lua sources are the behavioral oracle; these
//! tests audit module loading, cache identity, and the callable contracts over
//! fixed inputs against the Rust host.
use crate::support;
use support::isolated_scripts;

/// require loads each shipped module once, caches it, and keeps its environment
/// isolated from the globals while every expect matcher runs both ways.
#[tokio::test(flavor = "current_thread")]
async fn testing_module_exports_cached_suite_and_expect_contracts() {
    let (_d, _c, s) = isolated_scripts().await;
    s.eval_callback::<()>(
        r#"
                local first=require('testing')
                assert(type(first)=='table' and rawequal(first,require('testing')))
                assert(not rawequal(first,require('access_policy')))
                -- The module body only touches its private environment; no globals leak.
                assert(pipeline_leak==nil)
        
                -- Error catalog nodes are plain checked code-tree tables.
                local codes=first.error.codes
                assert(type(codes)=='table' and rawequal(codes,mux.error.code_tree('testing')))
                assert(type(codes.assertion)=='table' and codes.assertion.code=='testing.assertion')
                assert(type(codes.runtime)=='table' and codes.runtime.code=='testing.runtime')
                assert(tostring(codes.runtime)=='testing.runtime')
                local ok,err=mux.error.pcall(function() codes.assertion.extra=true end)
                assert(not ok and err.code=='mux.arg.invalid'
                  and err.message=='Lua error code nodes are immutable')
                ok,err=mux.error.pcall(function() return codes.unknown end)
                assert(not ok and err.message=="unknown Lua error code segment 'unknown'")
        
                -- Suite/test factory contracts.
                local run=function() end
                local case=first.test('case',run)
                assert(type(case)=='table' and case.name=='case' and case.run==run)
                -- testing.lua uses plain assert(), so failures carry the module position.
                local plain=function(call,detail)
                  local success,failure=pcall(call)
                  assert(not success and failure:sub(-#detail)==detail,tostring(failure))
                end
                plain(function() return first.test('',run) end,
                  'test name must be a non-empty string')
                plain(function() return first.test('x',7) end,
                  'test callback must be a function')
                local definition=first.suite('parity',{tests={}})
                assert(definition.name=='parity' and type(definition.tests)=='table')
                assert(type(definition.expect)=='table' and type(definition.expect.equal)=='function')
                assert(rawequal(definition.expect,first.suite('other',{tests={}}).expect))
                plain(function() return first.suite('',{tests={}}) end,
                  'suite name must be a non-empty string')
                plain(function() return first.suite('x',7) end,
                  'suite definition must be a table')
                plain(function() return first.suite('x',{tests=7}) end,
                  'suite tests must be a table')
        
                -- Every matcher in its passing form.
                local expect=definition.expect
                expect.equal(1,1)
                expect.not_equal(1,2)
                expect.truthy('yes')
                expect.falsy(nil)
                expect.falsy(false)
                expect.is_nil(nil)
                expect.contains('hello world','world')
                expect.contains({10,20,30},20)
                expect.near(0.5,0.5)
                expect.near(1,1.5,0.6)
                expect.error_matches(function() error('boom') end,'boom')
                expect.error_matches(function() error('kaboom') end,'oo')
                local structured=mux.error.new{code='mux.runtime',message='structured'}
                local raised=expect.raises(function() error(structured) end)
                assert(raised==structured)
                local wrapped=expect.raises(function() error('plain') end)
                assert(wrapped.code=='testing.runtime'
                  and wrapped.message:find('plain',1,true))
                local coded=expect.raises_code(function() error(structured) end,'mux.runtime')
                assert(rawequal(coded,structured))
                expect.no_error(function() return 7 end)
                assert(expect.is_error(structured,'mux.runtime')==structured)
        
                -- Every matcher in its failing form raises testing.assertion with the
                -- expected/actual detail pair.
                local function assertion_failure(call,message)
                  local success,failure=pcall(call)
                  assert(not success and type(failure)=='table'
                    and mux.error.is(failure,codes.assertion),tostring(failure))
                  assert(failure.message:sub(-#message)==message,tostring(failure.message))
                  return failure
                end
                local failure
                failure=assertion_failure(function() expect.equal(1,2) end,'expected 2, got 1')
                assert(failure.detail.expected==2 and failure.detail.actual==1)
                failure=assertion_failure(function() expect.not_equal(1,1) end,'did not expect 1')
                assert(failure.detail.expected=='not 1' and failure.detail.actual==1)
                failure=assertion_failure(function() expect.truthy(false) end,
                  'expected a truthy value, got false')
                assert(failure.detail.expected==true and failure.detail.actual==false)
                failure=assertion_failure(function() expect.falsy('x') end,
                  'expected a falsy value, got x')
                failure=assertion_failure(function() expect.is_nil(0) end,'expected nil, got 0')
                failure=assertion_failure(function() expect.contains('abc','z') end,
                  'expected abc to contain z')
                -- Table display text embeds the table address; compare the stable tail.
                local success,failure=pcall(function() expect.contains({1,2},3) end)
                assert(not success and failure.message:sub(-#' to contain 3')==' to contain 3','contains-table: '..tostring(failure.message))
                failure=assertion_failure(function() expect.near('a',1) end,
                  'expected a to be within 1e-09 of 1')
                assert(failure.detail.expected==1 and failure.detail.actual=='a','near detail')
                failure=assertion_failure(function() expect.near(2,1) end,
                  'expected 2 to be within 1e-09 of 1')
                failure=assertion_failure(function() expect.error_matches(function() end,'x') end,
                  'expected function to raise an error matching x')
                -- String errors carry their raise position; compare stably.
                local success,failure=pcall(function()
                  expect.error_matches(function() error('boom') end,'silent')
                end)
                assert(not success and mux.error.is(failure,codes.assertion),'em identity')
                assert(failure.message:sub(1,26)=='error did not match silent','prefix '..failure.message)
                assert(failure.message:sub(-#'boom')=='boom',failure.message)
                assert(failure.detail.expected=='silent','em detail '..tostring(failure.detail.expected))
                failure=assertion_failure(function() expect.raises(function() end) end,
                  'expected function to raise an error')
                failure=assertion_failure(function()
                  expect.raises_code(function() error(structured) end,'mux.arg.invalid')
                end,'expected error code mux.arg.invalid, got mux.runtime')
                local success,failure=pcall(function()
                  expect.no_error(function() error('bad') end)
                end)
                assert(not success and mux.error.is(failure,codes.assertion),'no_error identity')
                assert(failure.message:find('expected function not to raise an error: ',1,true)
                  and failure.message:sub(-#'bad')=='bad','no_error text '..failure.message)
                -- A non-convertible code surfaces mux.error.is's own type error;
                -- a mismatched convertible code produces the assertion failure.
                local success,failure=pcall(function() expect.is_error(5,nil) end)
                assert(not success and failure:find("bad argument #2 to 'is' (string expected, got nil)",1,true))
                failure=assertion_failure(function()
                  expect.is_error(structured,'mux.arg.invalid') end,
                  'expected an error matching mux.arg.invalid')
                assert(failure.detail.actual==structured,'ie detail')
                "#,
    )
    .unwrap();
}

/// access_policy.evaluate combines flag, affiliation and state requirements with
/// AND semantics, validates every entry, and returns structured denials.
#[tokio::test(flavor = "current_thread")]
async fn access_policy_evaluate_combines_requirements_and_fails_closed() {
    let (_d, _c, s) = isolated_scripts().await;
    s.eval_callback::<()>(
        r#"
        local policy=require('access_policy')
        assert(type(policy)=='table' and rawequal(policy,require('access_policy')))
        assert(type(policy.evaluate)=='function')

        local god=mux.world.object(1)
        local home=mux.world.object(0)
        local holder=mux.world.create_object{type=mux.world.types.THING,
          name='Policy Holder',location=home}
        local subject=mux.world.create_object{type=mux.world.types.THING,
          name='Subject',location=home}
        subject:set_affiliation(god)
        subject:state('identity'):set('rank','officer')
        subject:state('identity'):set('clearance',7)
        local function entry(key,value) holder:state('lock'):set(key,value) end
        local function evaluate(options) return policy.evaluate(
          {object=holder:dbref(),subject=subject:dbref()}, options) end

        -- Empty and message-only policies pass; message entries are metadata.
        assert(evaluate{namespace='lock'}==true)
        entry('message/enactor','denied'); entry('message/others','blocked')
        assert(evaluate{namespace='lock'}==true)

        -- Flag requirement: subject carries no WIZARD flag.
        entry('flag/WIZARD',false)
        assert(evaluate{namespace='lock'}==true)
        entry('flag/WIZARD',true)
        local denial=evaluate{namespace='lock'}
        assert(type(denial)=='table' and denial.passes==false)
        assert(denial.enactor_message=='denied' and denial.other_message=='blocked')

        -- Options supply default messages when entries set none.
        holder:state('lock'):set('message/enactor',nil)
        holder:state('lock'):set('message/others',nil)
        denial=evaluate{namespace='lock',enactor_message='default no',
          other_message='default blocked'}
        assert(denial.passes==false and denial.enactor_message=='default no'
          and denial.other_message=='default blocked')

        -- Affiliation requirement: subject's affiliation must be the live dbref.
        holder:state('lock'):set('flag/WIZARD',nil)
        entry('affiliation',god:dbref())
        assert(evaluate{namespace='lock'}==true)
        entry('affiliation',subject:dbref())
        assert(evaluate{namespace='lock'}.passes==false)

        -- State requirement: exact typed comparison keeps 7 distinct from '7'.
        holder:state('lock'):set('affiliation',nil)
        entry('state/identity/rank','officer')
        entry('state/identity/clearance',7)
        assert(evaluate{namespace='lock'}==true)
        holder:state('lock'):set('state/identity/clearance','7')
        assert(evaluate{namespace='lock'}.passes==false)
        holder:state('lock'):set('state/identity/clearance',8)
        assert(evaluate{namespace='lock'}.passes==false)

        -- Malformed entries raise plain errors and fail closed in locks;
        -- each probe clears its key again so the next one starts clean.
        local function policy_error(key, expected)
          local success, failure = pcall(evaluate, {namespace='lock'})
          holder:state('lock'):set(key,nil)
          assert(not success, key)
          assert(failure:sub(-#expected)==expected, key..' '..tostring(failure))
        end
        holder:state('lock'):set('state/identity/rank',nil)
        holder:state('lock'):set('state/identity/clearance',nil)
        entry('bogus','x')
        policy_error('bogus',
          'invalid access policy "lock" key "bogus": unknown or malformed policy entry')
        entry('flag/WIZARD',1)
        policy_error('flag/WIZARD',
          'invalid access policy "lock" key "flag/WIZARD": flag requirement must be a boolean')
        entry('flag/NOT_A_FLAG',true)
        policy_error('flag/NOT_A_FLAG',
          'invalid access policy "lock" key "flag/NOT_A_FLAG": unsupported flag "NOT_A_FLAG"')
        entry('affiliation',-3)
        policy_error('affiliation',
          'invalid access policy "lock" key "affiliation": affiliation must be a non-negative integer dbref')
        entry('affiliation',99999)
        policy_error('affiliation',
          'invalid access policy "lock" key "affiliation": affiliation dbref does not identify a live object')
        entry('message/enactor',5)
        policy_error('message/enactor',
          'invalid access policy "lock" key "message/enactor": enactor message must be a string')
        "#,
    )
    .unwrap();
}

/// object_appearances renders fixed room inventories through mux.text byte parity.
#[tokio::test(flavor = "current_thread")]
async fn object_appearances_render_contents_exits_and_internal_appearance() {
    let (_d, _c, s) = isolated_scripts().await;
    s.eval_callback::<()>(
        r#"
        local appearance=require('object_appearances')
        assert(type(appearance)=='table' and rawequal(appearance,require('object_appearances')))
        assert(type(appearance.render_contents)=='function')
        assert(type(appearance.render_exits)=='function')
        assert(type(appearance.render_internal_appearance)=='function')

        local god=mux.world.object(1)
        local room=mux.world.create_object{type=mux.world.types.ROOM,name='Parity Room',zone=0}
        local crate=mux.world.create_object{type=mux.world.types.THING,name='Crate',
          location=room}
        local hidden=mux.world.create_object{type=mux.world.types.THING,name='Hidden',
          location=room}
        local north=mux.world.create_object{type=mux.world.types.EXIT,
          name='north;n;leave',location=room,destination=room}
        local ctx={object=room:dbref(),enactor=god:dbref()}

        local contents=appearance.render_contents(ctx)
        assert(type(contents)=='table' and #contents==2)
        -- C walks the head-inserted contents chain (mux_object_bindings.c
        -- lua_mux_contents), so members render newest-first.
        assert(#contents==2)
        assert((contents[1]:find('Crate',1,true) or contents[2]:find('Crate',1,true))
          and (contents[1]:find('Hidden',1,true) or contents[2]:find('Hidden',1,true)))
        local exits=appearance.render_exits(ctx)
        assert(#exits==1 and exits[1]:find('[send=',1,true)
          and exits[1]:find('(n) north',1,true))

        local rendered=appearance.render_internal_appearance(ctx)
        assert(type(rendered)=='string')
        assert(rendered:find('Parity Room',1,true))
        assert(rendered:find('Obvious Exits:',1,true))
        assert(rendered:find('Contents:',1,true))
        assert(rendered:find('Crate',1,true) and rendered:find('north',1,true))
        -- The enactor is a player but is not in the room, so no player column.
        assert(not rendered:find(' Players:',1,true))
        "#,
    )
    .unwrap();
}
