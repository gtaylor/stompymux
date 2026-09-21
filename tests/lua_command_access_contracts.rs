//! C command-access constants and declaration gating.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::{Flag, Kind, Scripts};

use crate::support;
use support::{isolated_world, run_text};

#[tokio::test(flavor = "current_thread")]
async fn access_namespace_has_typed_identity_protection_and_c_string_lookup() {
    let (_directory, _config, scripts) = support::isolated_scripts().await;
    scripts
        .eval_callback::<()>(
            r#"
            local access=mux.world.access
            assert(type(access)=='userdata')
            assert(tostring(access.PUBLIC)=='PUBLIC')
            assert(tostring(access.WIZARD)=='WIZARD')
            assert(tostring(access.GOD)=='GOD')
            assert(access.PUBLIC==access.PUBLIC and access.PUBLIC~=access.WIZARD)
            assert(access['PUBLIC\0ignored']==access.PUBLIC)
            assert(getmetatable(access)=='protected command access namespace metatable')
            assert(getmetatable(access.PUBLIC)=='protected command access constant metatable')
            local ok,err=pcall(function() return access[false] end)
            assert(not ok and type(err)=='table' and err.code=='mux.access.invalid')
            assert(err.detail.argument==2 and err.message:find('constant name must be a string',1,true))
            ok,err=pcall(function() return access.MISSING end)
            assert(not ok and type(err)=='table' and err.code=='mux.access.invalid')
            assert(err.detail.argument==2 and err.message:find("unknown command access constant 'MISSING'",1,true))
            local prefix="unknown command access constant '"
            local split=string.rep('x',2047-#prefix-1)..'é'
            ok,err=pcall(function() return access[split] end)
            assert(not ok and err.code=='mux.access.invalid')
            assert(err.message:byte(#err.message-1)==0xc3 and err.message:byte(#err.message)==string.byte(')'))
            ok,err=pcall(function() access.PUBLIC=false end)
            assert(not ok and type(err)=='table' and err.code=='mux.access.invalid')
            assert(err.message=='mux.world.access constants are immutable')
            ok,err=pcall(function() access.PUBLIC.changed=true end)
            assert(not ok and type(err)=='table' and err.code=='mux.access.invalid')
            assert(err.message=='command access constants are immutable')
            "#,
        )
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn declarations_default_public_and_gate_wizard_god_and_unnamed_commands() {
    let (directory, config, mut world) = isolated_world().await;
    let ordinary = world.create(&config, "Ordinary".into(), Kind::Player);
    let wizard = world.create(&config, "Wizard2".into(), Kind::Player);
    world
        .objects
        .get_mut(&wizard)
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    let path = directory
        .path()
        .join("lua/global_logic/access_contract.lua");
    std::fs::write(
        &path,
        r#"return {commands={
          {pattern='^access%-public$',handler=function(ctx) mux.world.pemit(ctx.enactor,'PUBLIC'); return true end},
          {name='access-wizard',access=mux.world.access.WIZARD,pattern='^access%-wizard$',handler=function(ctx) mux.world.pemit(ctx.enactor,'WIZARD'); return true end},
          {name='access-god',access=mux.world.access.GOD,pattern='^access%-god$',handler=function(ctx) mux.world.pemit(ctx.enactor,'GOD'); return true end},
          {name='access-nil',access=nil,pattern='^access%-nil$',handler=function(ctx) mux.world.pemit(ctx.enactor,'NIL'); return true end},
        }}"#,
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();

    assert!(scripts.dispatch(ordinary, None, "access-public").unwrap());
    assert!(scripts.dispatch(ordinary, None, "access-nil").unwrap());
    assert!(!scripts.dispatch(ordinary, None, "access-wizard").unwrap());
    assert!(!scripts.dispatch(ordinary, None, "access-god").unwrap());
    assert!(scripts.dispatch(wizard, None, "access-wizard").unwrap());
    assert!(!scripts.dispatch(wizard, None, "access-god").unwrap());
    assert!(
        scripts
            .dispatch(stompymux_rs::ObjectId(1), None, "access-god")
            .unwrap()
    );
    let wizard_list = run_text(&scripts, &config, wizard, 7, "@list commands");
    assert!(wizard_list.contains("access-wizard"), "{wizard_list}");
    assert!(!wizard_list.contains("access-god"), "{wizard_list}");
    let god_list = run_text(
        &scripts,
        &config,
        stompymux_rs::ObjectId(1),
        8,
        "@list commands",
    );
    assert!(god_list.contains("access-wizard"), "{god_list}");
    assert!(god_list.contains("access-god"), "{god_list}");
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_access_rejects_check_and_failed_reload_keeps_old_commands() {
    let (directory, config, world) = isolated_world().await;
    let path = directory
        .path()
        .join("lua/global_logic/access_contract.lua");
    std::fs::write(
        &path,
        "return {commands={{pattern='^old%-access$',handler=function() return true end}}}",
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(
        scripts
            .dispatch(stompymux_rs::ObjectId(1), None, "old-access")
            .unwrap()
    );

    for value in ["false", "{}", "mux.world.types.PLAYER"] {
        std::fs::write(
            &path,
            format!(
                "return {{commands={{{{access={value},pattern='^new%-access$',handler=function() return true end}}}}}}"
            ),
        )
        .unwrap();
        let error = match scripts.rebuild_for_inspection(&config) {
            Ok(_) => panic!("invalid command access unexpectedly loaded"),
            Err(error) => format!("{error:#}"),
        };
        assert!(
            error.contains(
                "command access in global_logic/access_contract.lua must be a mux.world.access constant"
            ),
            "{error}"
        );
        assert!(
            scripts
                .dispatch(stompymux_rs::ObjectId(1), None, "old-access")
                .unwrap()
        );
        assert!(
            !scripts
                .dispatch(stompymux_rs::ObjectId(1), None, "new-access")
                .unwrap()
        );
    }
}
