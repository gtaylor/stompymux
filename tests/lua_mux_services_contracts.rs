//! Source-backed edge contracts for the non-world MUX service packages.

mod support;
use support::isolated_scripts;

#[tokio::test(flavor = "current_thread")]
async fn config_and_session_scalar_contracts_are_exact() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    scripts.eval_callback::<()>(r#"
        assert(type(mux.config.get('max_players'))=='number')
        local summary=mux.session.who_summary()
        assert(summary.hidden==0 and type(summary.record)=='number')
        local ok,err=mux.error.pcall(mux.config.get,false)
        assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==1)
        ok,err=mux.error.pcall(mux.config.get,'max_players\0ignored')
        assert(not ok and err.code=='mux.arg.invalid' and err.message:find('configuration name contains an embedded NUL byte',1,true))
        ok,err=mux.error.pcall(mux.config.get,'does_not_exist')
        assert(not ok and err.code=='mux.config.not_found' and err.message=="unknown configuration directive 'does_not_exist'")
        -- C has no 'mux' configuration directive; unknown names are not_found.
        ok,err=mux.error.pcall(mux.config.get,'mux')
        assert(not ok and err.code=='mux.config.not_found' and err.message=="unknown configuration directive 'mux'")
    "#).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn telnet_checks_descriptor_before_kind_and_name() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    scripts
        .eval_callback::<()>(
            r#"
        for _,name in ipairs({'environment_has','environment_get'}) do
            local fn=mux.telnet[name]
            local ok,err=mux.error.pcall(fn,'999999',false,false)
            assert(not ok and err.code=='mux.connection.invalid' and err.detail.argument==1)
            assert(err.message:find("bad argument #1 to '?' (no such descriptor)",1,true))
        end
    "#,
        )
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn text_style_nests_properties_and_rejects_c_string_edges() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    scripts.eval_callback::<()>(r#"
        local styled=mux.text.style('x',{foreground='red',bold=true})
        assert(styled=='[fg=red][bold]x[/][/]',styled)
        local ok,err=mux.error.pcall(mux.text.style,'x',{foreground='[red]'})
        assert(not ok and err.code=='mux.text.invalid' and err.message=='style fields have invalid types',tostring(err))
        ok,err=mux.error.pcall(mux.text.style,'x',{foreground=12})
        assert(not ok and err.code=='mux.text.invalid' and err.message~='style fields have invalid types',tostring(err))
        ok,err=mux.error.pcall(mux.text.style,'x\0y',{})
        assert(not ok and err.code=='mux.text.invalid' and err.detail.argument==1,tostring(err))
    "#).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn log_and_channel_argument_edges_keep_native_order() {
    let (directory, _config, scripts) = isolated_scripts().await;
    // C's log_to_file only writes to files that already exist under logs/.
    std::fs::create_dir_all(directory.path().join("logs")).unwrap();
    std::fs::write(directory.path().join("logs/binary"), b"").unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local ok,err=mux.error.pcall(mux.log,'bad\0name','ok')
        assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==1)
        ok,err=mux.error.pcall(mux.log,'name','bad\0message')
        assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==2)
        ok,err=mux.error.pcall(mux.comsys.channel,'missing\0suffix')
        assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==1)
        local channel=mux.comsys.create_channel('Parity')
        assert(select('#',channel:set_object(nil,'ignored'))==0)
        ok,err=mux.error.pcall(function() channel:set_object() end)
        assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==1)
        -- Empty messages succeed without writing; missing files are rejected.
        assert(mux.log('empty',''))
        assert(mux.log('binary',string.char(255)..'x'))
        assert(mux.log('definitely_absent.log','paired')==false)
        assert(mux.log('../escape','paired')==false)
    "#,
        )
        .unwrap();
    let requests = scripts.drain_logs_for_inspection();
    // C returns true for an empty message without staging any write.
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].filename(), "binary");
    assert_eq!(requests[0].message(), &[0xff, b'x', b'\n']);
}

/// C-verified comsys registry edges from mux_comsys_bindings.c via the
/// comsys_channels differential probe.
#[tokio::test(flavor = "current_thread")]
async fn comsys_channel_registry_matches_c_messages_and_codes() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    scripts
        .eval_callback::<()>(
            r#"
        local ok, err = mux.error.pcall(mux.comsys.channel, 'absent')
        assert(not ok and err.code == 'mux.channel.invalid' and err.detail.argument == 1)
        assert(err.message:find("unknown channel 'absent'", 1, true))
        local channel = mux.comsys.create_channel('Parity')
        ok, err = mux.error.pcall(mux.comsys.create_channel, 'PARITY')
        assert(not ok and err.code == 'mux.channel.invalid')
        assert(err.message:find("channel 'PARITY' already exists", 1, true))
        ok, err = mux.error.pcall(mux.comsys.create_channel, '')
        assert(not ok and err.code == 'mux.arg.invalid')
        assert(err.message:find('channel name must be printable ASCII without spaces and shorter than 50 bytes', 1, true))
        ok, err = mux.error.pcall(mux.comsys.create_channel, string.rep('x', 50))
        assert(not ok and err.code == 'mux.arg.invalid')
        -- Stale handles keep the C message through every method.
        mux.comsys.destroy_channel(channel)
        ok, err = mux.error.pcall(function() return channel:name() end)
        assert(not ok and err.code == 'mux.channel.invalid' and err.detail.argument == 1)
        assert(err.message:find('channel no longer exists', 1, true))
        -- Handles reject new fields with the C structured error.
        ok, err = mux.error.pcall(function() channel.forged = true end)
        assert(not ok and err.code == 'mux.arg.invalid' and err.message:find('channel values are immutable', 1, true))
        local plain_ok, plain = pcall(mux.comsys.destroy_channel, false)
        assert(not plain_ok and plain:find("bad argument #1 to '?' (btmux.channel expected, got boolean)", 1, true), plain)
        local listed = mux.comsys.list_channels()
        for index, item in ipairs(listed) do assert(tostring(item) == 'channel(' .. item:name() .. ')') end
    "#,
        )
        .unwrap();
}

/// C-verified membership, option and flag-method edges from the comsys
/// membership and channel-flag bindings.
#[tokio::test(flavor = "current_thread")]
async fn comsys_membership_and_options_match_c_codes() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    scripts
        .eval_callback::<()>(
            r#"
        local god = mux.world.object(1)
        local channel = mux.comsys.create_channel('Members')
        -- Called through a function value, LuaJIT leaves the method unnamed, so
        -- C's lua_error_arg keeps the stack argument numbers (probe-verified).
        local ok, err = mux.error.pcall(function() return channel:add_player(0, 'pp', false) end)
        assert(not ok and err.code == 'mux.object.invalid' and err.detail.argument == 2)
        assert(err.message:find('object must be a player', 1, true))
        ok, err = mux.error.pcall(function() return channel:add_player(god, 'pp', nil) end)
        assert(not ok and err.code == 'mux.arg.invalid' and err.detail.argument == 4)
        assert(err.message:find('quiet must be a boolean', 1, true))
        ok, err = mux.error.pcall(function() return channel:add_player(god, 'toolong', false) end)
        assert(not ok and err.code == 'mux.arg.invalid')
        assert(err.message:find('alias must be 1-5 printable ASCII characters without spaces', 1, true))
        channel:add_player(god, 'par', false)
        ok, err = mux.error.pcall(function() return channel:add_player(god, 'par', false) end)
        assert(not ok and err.code == 'mux.arg.invalid' and err.message:find('alias is already in use', 1, true))
        ok, err = mux.error.pcall(function() return channel:add_player(god, 'par', false) end)
        assert(not ok and err.code == 'mux.arg.invalid' and err.message:find('alias is already in use', 1, true))
        ok, err = mux.error.pcall(function() return channel:emit('x', { bogus = true }) end)
        assert(not ok and err.code == 'mux.arg.invalid' and err.detail.argument == 3)
        assert(err.message:find("unknown options field 'bogus'", 1, true))
        ok, err = mux.error.pcall(function() return channel:emit('x', { no_header = 1 }) end)
        assert(not ok and err.message:find('options.no_header must be a boolean', 1, true))
        ok, err = mux.error.pcall(function() return channel:who(false) end)
        assert(not ok and err.code == 'mux.arg.invalid' and err.message:find('options must be a table', 1, true))
        ok, err = mux.error.pcall(function() return channel:boot_player(0) end)
        assert(not ok and err.code == 'mux.channel.invalid')
        assert(err.message:find('object is not a member of this channel', 1, true))
        -- Flag methods validate the live channel before the flag argument.
        local flags = channel:flags()
        mux.comsys.destroy_channel(channel)
        ok, err = mux.error.pcall(function() return flags:has(mux.comsys.flags.PUBLIC) end)
        assert(not ok and err.code == 'mux.channel.invalid' and err.message:find('channel no longer exists', 1, true))
    "#,
        )
        .unwrap();
}

/// C-verified mux.text semantics: byte widths, raw-preserving truncation and
/// structured markup failures.
#[tokio::test(flavor = "current_thread")]
async fn text_service_edges_match_c_semantics() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    scripts
        .eval_callback::<()>(
            r#"
        assert(mux.text.width('abc') == 3)
        assert(mux.text.width('[bold]ab[/]c') == 3)
        -- Control bytes stay visible for C, which renders plain bytes.
        assert(mux.text.width('a\1b') == 3)
        assert(mux.text.strip_style('a\1b') == 'a\1b')
        assert(mux.text.truncate('[fg=red]color[/]', 1) == '[fg=red]c[/]')
        assert(mux.text.truncate('[bold]x[/]', 0) == '')
        local ok, err = mux.error.pcall(mux.text.truncate, 'abc', -1)
        assert(not ok and err.code == 'mux.text.invalid' and err.detail.argument == 2)
        assert(err.message:find('width must not be negative', 1, true))
        ok, err = mux.error.pcall(mux.text.markup, '[bold]x')
        assert(not ok and err.code == 'mux.text.invalid')
        assert(err.message:find('invalid styled-text markup: style tag is not closed', 1, true), tostring(err))
        ok, err = mux.error.pcall(mux.text.markup, '[fancy]x[/]')
        assert(not ok and err.message:find('invalid styled-text markup: unknown style tag', 1, true), tostring(err))
        ok, err = mux.error.pcall(mux.text.style, 'x', { foreground = 'no_such_color' })
        assert(not ok and err.message:find('invalid style: unknown foreground color', 1, true), tostring(err))
        local plain_ok, plain = pcall(mux.text.is_printable_ascii, 42)
        assert(not plain_ok and plain:find("bad argument #1 to '?' (string expected, got number)", 1, true), plain)
        plain_ok, plain = pcall(function() local f = mux.text.width return f(false) end)
        assert(not plain_ok and plain:find("bad argument #1 to '?' (string expected, got boolean)", 1, true), plain)
        assert(mux.text.width(42) == 2)
    "#,
        )
        .unwrap();
}

/// C-verified top-level and configuration edges from mux_package.c and
/// mux_config_bindings.c.
#[tokio::test(flavor = "current_thread")]
async fn top_level_and_config_edges_match_c() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    scripts
        .eval_callback::<()>(
            r#"
        assert(select('#', mux.check_db()) == 0)
        -- C knows only registry directive names; table-valued catalog entries
        -- stay readable-scalar rejections.
        local ok, err = mux.error.pcall(mux.config.get, 'bootstrap_object')
        assert(not ok and err.code == 'mux.config.unsupported')
        assert(err.message:find("configuration directive 'bootstrap_object' is not a readable scalar", 1, true))
        ok, err = mux.error.pcall(mux.config.get, 42)
        assert(not ok and err.code == 'mux.arg.invalid')
        assert(err.message:find('configuration name must be a string', 1, true))
        ok, err = mux.error.pcall(mux.log, false, 'ok')
        assert(not ok and err.code == 'mux.runtime')
        assert(err.message:find("bad argument #1 to '?' (string expected, got boolean)", 1, true), tostring(err))
    "#,
        )
        .unwrap();
}
