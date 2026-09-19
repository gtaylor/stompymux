//! Source-backed table identity and mutable Error behavior from lua_error.c.

mod support;
use support::isolated_scripts;

#[tokio::test(flavor = "current_thread")]
async fn native_and_custom_error_code_nodes_are_plain_tables() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    scripts
        .eval_callback::<()>(
            r#"
        local root=mux.error.code_tree('mux')
        assert(type(root)=='table' and root==mux.error.codes and root==mux.error.code_tree('mux'))
        assert(type(root.object)=='table' and rawget(root.object,'code')=='mux.object')
        local seen=false
        for key,value in pairs(root.object) do if key=='invalid' then seen=type(value)=='table' end end
        assert(seen and tostring(root.object.invalid)=='mux.object.invalid')
        assert(not rawequal(root.object.invalid,mux.error.namespace('author',{'object.invalid'}).object.invalid))

        local custom=mux.error.namespace('author',{'object.invalid','runtime'})
        assert(type(custom)=='table' and custom.code=='author' and custom.object.code=='author.object')
        custom.code='changed'
        assert(tostring(custom)=='changed')
        local ok,err=mux.error.pcall(function() custom.missing=true end)
        assert(not ok and err.code=='mux.arg.invalid' and err.message=='Lua error code nodes are immutable')
        ok,err=mux.error.pcall(function() return custom.unknown end)
        assert(not ok and err.message=="unknown Lua error code segment 'unknown'")
        ok,err=mux.error.pcall(function() return custom[{}] end)
        assert(not ok and err.message=="unknown Lua error code segment '<non-string>'")
        ok,err=mux.error.pcall(function() return mux.error.code_tree('\255') end)
        assert(not ok and err.code=='mux.arg.invalid')
        assert(err.message:sub(1,29)=="unknown Lua error code root '")
        assert(err.message:byte(30)==255 and err.message:sub(31)=="'")
    "#,
        )
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn mutable_errors_use_c_string_coercion_fallbacks_and_fixed_root_depth() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    scripts
        .eval_callback::<()>(
            r#"
        local err=mux.error.new({code=mux.error.codes.object.invalid,message=42})
        assert(err.code=='mux.object.invalid' and err.message=='42')
        assert(mux.error.is({code=123},{code=123}))
        assert(mux.error.is({code='prefix.child\0ignored'},'prefix'))
        local ok,bad=mux.error.pcall(function() return mux.error.new({code={},message={}}) end)
        assert(not ok and bad.code=='mux.runtime')
        local raw_ok,raw_bad=pcall(mux.error.new,{code={},message={}})
        assert(not raw_ok and raw_bad=="bad argument #4 to '?' (string expected, got nil)")
        raw_ok,raw_bad=pcall(mux.error.new,{code='author.bad',message={}})
        assert(not raw_ok and raw_bad=="bad argument #4 to '?' (string expected, got table)")
        raw_ok,raw_bad=pcall(mux.error.pcall,false)
        assert(not raw_ok and raw_bad=="bad argument #1 to '?' (function expected, got boolean)")
        local touched=0
        local cause=setmetatable({}, {__tostring=function() touched=touched+1 return 'cause' end})
        raw_ok,raw_bad=pcall(mux.error.wrap,cause,{}, {})
        assert(not raw_ok and raw_bad=="bad argument #5 to '?' (string expected, got nil)")
        assert(touched==0)
        local truncated=mux.error.new({code='author.bad\0suffix',message='short\0suffix'})
        assert(truncated.code=='author.bad' and truncated.message=='short')
        err.code={}; err.message=nil
        assert(tostring(err)=='lua.error: unknown Lua error','mutable tostring')
        assert(err.root(12)==12,'root non-table self')

        local described=setmetatable({}, {__tostring=function() return 'metamethod' end})
        ok,bad=mux.error.pcall(function() error(described) end)
        assert(not ok and bad.message=='table','table normalize')
        ok,bad=mux.error.pcall(function() error(function() end) end)
        assert(not ok and bad.message=='function','function normalize')
        ok,bad=mux.error.pcall(function() error('trace-source') end)
        assert(not ok and bad.traceback:find('trace-source',1,true),'string traceback')
        local numeric={code=123,message='retained'}
        ok,bad=mux.error.pcall(function() error(numeric) end)
        assert(not ok and bad==numeric,'numeric code identity')
        for _,value in ipairs({false,function() end,io and io.stdout}) do
            if value~=nil then
                ok,bad=mux.error.pcall(function() error(value) end)
                assert(not ok and bad.traceback==value,'trace value '..type(value)..' '..tostring(bad.traceback))
            end
        end
        ok,bad=mux.error.pcall(function() error(42) end)
        assert(not ok and type(bad.traceback)=='string' and bad.traceback:find('42',1,true),'numeric traceback')
        local nil_error
        ok,bad=mux.error.pcall(function() error(nil_error) end)
        assert(not ok and bad.message=='nil' and bad.traceback==nil,'nil traceback')

        local chain={}
        local cursor=chain
        for i=1,70 do cursor.cause={index=i}; cursor=cursor.cause end
        local chain_error=mux.error.new({code='author.chain',message='chain',cause=chain})
        assert(chain_error:root().index==63,'64-depth chain')
        local a,b,c={name='a'},{name='b'},{name='c'}
        a.cause=b; b.cause=c; c.cause=a
        local cycle_error=mux.error.new({code='author.cycle',message='cycle',cause=a})
        -- The Error itself is the first node; 64 subsequent links land on a here.
        assert(cycle_error:root()==a,'64-depth cycle')
    "#,
        )
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn argument_failures_capture_public_call_names_methods_and_c_buffer_limits() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    let messages = scripts
        .eval_callback::<(String, String, String, String, usize, i64)>(
            r#"
        local function failure(fn)
            local ok,err=pcall(fn)
            assert(not ok and type(err)=='table' and err.detail.argument~=nil)
            return err.message,err.detail.argument
        end
        local direct=select(1,failure(function() btech.parts.list(false) end))
        local list=btech.parts.list
        local alias_ok,alias_error=pcall(list,false)
        assert(not alias_ok)
        local alias=alias_error.message
        local method,argument=failure(function() btech.parts:list(false) end)
        local function tail() return btech.parts.list(false) end
        local tail_message=select(1,failure(tail))
        local long=string.rep('x',3000)
        local long_message=select(1,failure(function() btech.parts.resolve({brand=long}) end))
        return direct,alias,method,tail_message,#long_message,argument
        "#,
        )
        .unwrap();
    assert!(
        messages
            .0
            .contains("bad argument #1 to 'list' (category must be a string)"),
        "{}",
        messages.0
    );
    assert!(
        messages
            .1
            .contains("bad argument #1 to '?' (category must be a string)"),
        "{}",
        messages.1
    );
    assert!(
        messages
            .2
            .contains("calling 'list' on bad self (category must be a string)"),
        "{}",
        messages.2
    );
    assert!(
        messages
            .3
            .contains("bad argument #1 to '?' (category must be a string)"),
        "{}",
        messages.3
    );
    assert!(messages.4 <= 2303, "{}", messages.4);
    assert_eq!(messages.5, 0);
}

#[tokio::test(flavor = "current_thread")]
async fn ordinary_error_api_type_checks_keep_dynamic_names_types_and_stack_arity() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    let messages = scripts
        .eval_callback::<(String, String, String, String, String)>(
            r#"
        local function capture(fn)
            local ok,err=pcall(fn)
            assert(not ok and type(err)=='string')
            return err
        end
        local new=mux.error.new
        return capture(function() mux.error.new(false) end),
            capture(function() new(false) end),
            capture(function() mux.error.new() end),
            capture(function() mux.error.new({code={},message={}},1,2) end),
            capture(function() mux.error:raise({}, {}) end)
        "#,
        )
        .unwrap();
    for (message, suffix) in [
        (
            &messages.0,
            "bad argument #1 to 'new' (table expected, got boolean)",
        ),
        (
            &messages.1,
            "bad argument #1 to 'new' (table expected, got boolean)",
        ),
        (
            &messages.2,
            "bad argument #1 to 'new' (table expected, got no value)",
        ),
        (
            &messages.3,
            "bad argument #6 to 'new' (string expected, got nil)",
        ),
        (
            &messages.4,
            "bad argument #4 to 'raise' (string expected, got nil)",
        ),
    ] {
        assert!(message.ends_with(suffix), "{message:?}");
    }
}

/// C lua_error_check_code pushes a table's code field to stack index 3, where
/// raise's detail slot and wrap's message slot read it back (probe-verified).
#[tokio::test(flavor = "current_thread")]
async fn raise_and_wrap_inherit_the_pushed_code_field() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    scripts
        .eval_callback::<()>(
            r#"
        local ok, err = pcall(mux.error.raise, mux.error.codes.access.invalid, 'parity')
        assert(not ok and err.code == 'mux.access.invalid' and err.message == 'parity')
        assert(err.detail == 'mux.access.invalid', 'table codes carry the field as detail')
        ok, err = pcall(mux.error.raise, 'mux.runtime', 'parity')
        -- C lua_error_push lands the raised table at the detail slot's absolute
        -- index 3, so a plain string code aliases the error itself as detail
        -- (probed differentially in error_api.lua raise_string_detail).
        assert(not ok and err.detail == err, 'plain string codes alias the raised table as detail')
        local wrapped = mux.error.wrap('cause', mux.error.codes.object.invalid)
        assert(wrapped.code == 'mux.object.invalid')
        assert(wrapped.message == 'mux.object.invalid', 'omitted message reads the pushed field')
        assert(wrapped.cause.code == 'mux.runtime' and wrapped.cause.message == 'cause')
        local ok, failure = pcall(function()
          return mux.error.code_tree(false)
        end)
        assert(not ok and failure:find("bad argument #1 to '?' (string expected, got boolean)", 1, true), failure)
        ok, failure = pcall(function()
          return mux.error.code_tree('author')
        end)
        assert(not ok and mux.error.is(failure, mux.error.codes.arg.invalid))
        -- A removed code field makes tostring re-enter __index exactly as C does.
        local node = mux.error.namespace('author', { 'leaf' })
        node.code = nil
        ok, failure = pcall(function() return tostring(node) end)
        assert(not ok and mux.error.is(failure, mux.error.codes.arg.invalid))
        assert(failure.message:find("unknown Lua error code segment 'code'", 1, true))
    "#,
        )
        .unwrap();
}
