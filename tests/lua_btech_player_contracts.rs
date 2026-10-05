use crate::support;
use sqlx::{Connection, sqlite::SqliteConnectOptions};
use std::{cell::RefCell, rc::Rc};
use support::{copy, isolated_scripts};

#[tokio::test(flavor = "current_thread")]
async fn player_preferences_and_loadout_are_atomic_and_resettable() {
    let (directory, config, s) = isolated_scripts().await;
    copy(
        &support::repository_root().join("tests/fixtures/btech/units"),
        &directory.path().join("units"),
    );
    s.eval_callback::<()>(r#"
 local p=mux.world.object(1);local d=btech.player.ui_preferences(p);assert(not d.configured and d.tactical_width==21,'defaults')
 local v={tactical_height=12,tactical_width=30,lrs_height=20,include_dead=true,include_shutdown=false,include_enemies=true,include_allies=false,include_target=true,buildings='follow_brief'}
 assert(select('#',btech.player.set_ui_preferences(p,v))==0,'set ui arity');assert(btech.player.ui_preferences(p).configured,'ui configured')
 local l={armor={head=2,torso=8,hands=1,feet=2}}
 assert(btech.player.mechwarrior_template(p)==nil,'nil template')
 assert(select('#',btech.player.set_mechwarrior_template(p,'PARITY'))==0,'set template arity')
 assert(btech.player.mechwarrior_template(p)=='PARITY','template')
 local arity=select('#',btech.player.set_loadout(p,l));if arity~=0 then error('set loadout arity '..arity) end
 local got=btech.player.loadout(p);if not (got.armor.head==2 and got.armor.torso==8 and got.right==nil and got.left==nil) then error('loadout projection') end
 assert(select('#',btech.player.set_loadout(p,{armor=l.armor,right={weapon='PC.Blazer',ammunition=3}}))==0,'personal weapon')
 local armed=btech.player.loadout(p).right;assert(armed.weapon.id==6 and armed.ammunition==3,'personal weapon projection')
 assert(select('#',btech.player.set_loadout(p,l))==0 and btech.player.loadout(p).right==nil,'unarm')
 local ok2,e2=mux.error.pcall(function() btech.player.set_loadout(p,{armor=l.armor,right={weapon='IS.PPC'}}) end);assert(not ok2 and e2.code=='btech.part.wrong_kind' and btech.player.loadout(p).right==nil,'wrong kind: '..tostring(e2))
 local ok3,e3=mux.error.pcall(function() btech.player.set_mechwarrior_template(p,'missing') end);assert(not ok3 and e3.code=='btech.template.not_found' and btech.player.mechwarrior_template(p)=='PARITY','missing template: '..tostring(e3))
 -- Stock templates name parts without a manufacturer and are accepted.
 assert(select('#',btech.player.set_mechwarrior_template(p,'JR7-D'))==0 and btech.player.mechwarrior_template(p)=='JR7-D','stock template')
 assert(select('#',btech.player.set_mechwarrior_template(p,'PARITY'))==0,'restore template')
 assert(select('#',btech.player.set_ui_preferences(p,nil))==0,'clear ui arity');assert(not btech.player.ui_preferences(p).configured,'clear ui')
 "#).unwrap();

    let saved = s.world().clone();
    stompymux_rs::persistence::save(&config.database(), &saved)
        .await
        .unwrap();
    let loaded = stompymux_rs::persistence::load(&config.database())
        .await
        .unwrap();
    let reloaded = stompymux_rs::Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
    reloaded
        .eval_callback::<()>(
            r#"
      local p=mux.world.object(1)
      assert(not btech.player.ui_preferences(p).configured)
      assert(btech.player.mechwarrior_template(p)=='PARITY')
      local l=btech.player.loadout(p);assert(l.armor.head==2 and l.right==nil)
      assert(select('#',btech.player.set_mechwarrior_template(p,nil))==0)
      assert(select('#',btech.player.set_loadout(p,nil))==0)
    "#,
        )
        .unwrap();
    let cleared = reloaded.world().clone();
    stompymux_rs::persistence::save(&config.database(), &cleared)
        .await
        .unwrap();
    let cleared = stompymux_rs::persistence::load(&config.database())
        .await
        .unwrap();
    let cleared = stompymux_rs::Scripts::new(&config, Rc::new(RefCell::new(cleared))).unwrap();
    cleared.eval_callback::<()>(
      "local p=mux.world.object(1);assert(btech.player.mechwarrior_template(p)==nil);assert(btech.player.loadout(p)==nil);assert(not btech.player.ui_preferences(p).configured)"
    ).unwrap();
}

/// Every rejection raised by the C handlers keeps its exact code, argument
/// position, and `bad argument #N to 'f' (detail)` message shape, and extra
/// arguments stay ignored because the C arity checks are minimums only.
#[tokio::test(flavor = "current_thread")]
async fn player_configuration_errors_match_the_c_argument_shapes() {
    let (directory, _config, s) = isolated_scripts().await;
    copy(
        &support::repository_root().join("tests/fixtures/btech/units"),
        &directory.path().join("units"),
    );
    std::fs::write(
        directory.path().join("units/BROKEN.toml"),
        "not a template at all",
    )
    .unwrap();
    let failures = s
        .eval_callback::<Vec<String>>(
            r#"
      local p=mux.world.object(1)
      local out={}
      local function record(f,...)
        local ok,e=mux.error.pcall(f,...)
        out[#out+1]=ok and 'ok' or (e.code..' | '..e.message)
        return ok,e
      end
      -- Extra arguments are ignored everywhere, matching lua_btech_check_arity minimums.
      assert(type(btech.player.ui_preferences(p,'extra').tactical_height)=='number')
      assert(type(btech.player.mechwarrior_template(p,'extra'))=='nil')
      assert(type(btech.player.loadout(p,'extra'))=='nil')
      assert(select('#',btech.player.set_ui_preferences(p,nil,'extra'))==0)
      assert(select('#',btech.player.set_loadout(p,nil,'extra'))==0)
      assert(select('#',btech.player.set_mechwarrior_template(p,nil,'extra'))==0)
      -- Missing-argument arity rejections are raises without argument detail.
      record(btech.player.ui_preferences)
      record(btech.player.mechwarrior_template)
      record(btech.player.loadout)
      record(btech.player.set_ui_preferences,p)
      record(btech.player.set_mechwarrior_template,p)
      record(btech.player.set_loadout,p)
      -- Non-table and non-string second arguments.
      record(btech.player.set_ui_preferences,p,42)
      record(btech.player.set_loadout,p,42)
      record(btech.player.set_mechwarrior_template,p,42)
      -- Reference string validation on set_mechwarrior_template.
      record(btech.player.set_mechwarrior_template,p,'')
      record(btech.player.set_mechwarrior_template,p,string.rep('x',25))
      record(btech.player.set_mechwarrior_template,p,'a/b')
      record(btech.player.set_mechwarrior_template,p,'..\\JR7-D')
      record(btech.player.set_mechwarrior_template,p,'missing-template')
      record(btech.player.set_mechwarrior_template,p,'BROKEN')
      -- Strict option records and field validation for preferences.
      local good={tactical_height=12,tactical_width=30,lrs_height=20,include_dead=true,
        include_shutdown=false,include_enemies=true,include_allies=false,include_target=true,
        buildings='follow_brief'}
      local function with(overrides)
        local t={}
        for k,v in pairs(good) do t[k]=v end
        for k,v in pairs(overrides) do t[k]=v end
        return t
      end
      record(btech.player.set_ui_preferences,p,with{extra=1})
      record(btech.player.set_ui_preferences,p,with{tactical_height='x'})
      record(btech.player.set_ui_preferences,p,with{tactical_height=4})
      record(btech.player.set_ui_preferences,p,with{tactical_width=41})
      record(btech.player.set_ui_preferences,p,with{lrs_height=9})
      record(btech.player.set_ui_preferences,p,with{include_dead='x'})
      record(btech.player.set_ui_preferences,p,with{buildings=7})
      record(btech.player.set_ui_preferences,p,with{buildings='bogus-mode!'})
      record(btech.player.set_ui_preferences,p,with{buildings=string.rep('i',13)})
      -- C validates booleans before the buildings mode.
      record(btech.player.set_ui_preferences,p,with{include_dead='x',buildings='bogus-mode!'})
      -- Loadout validation.
      local armor={head=2,torso=8,hands=1,feet=2}
      record(btech.player.set_loadout,p,{armor=42})
      record(btech.player.set_loadout,p,{armor={head=9,torso=8,hands=1,feet=2}})
      record(btech.player.set_loadout,p,{armor={head='x',torso=8,hands=1,feet=2}})
      record(btech.player.set_loadout,p,{armor=armor,right=42})
      record(btech.player.set_loadout,p,{armor=armor,right={weapon='Missing.Weapon'}})
      record(btech.player.set_loadout,p,{armor=armor,right={weapon='Ammo_IS.SRM-4'}})
      record(btech.player.set_loadout,p,{armor=armor,right={weapon='IS.PPC'}})
      record(btech.player.set_loadout,p,{armor=armor,right={weapon='PC.Blazer',ammunition=256}})
      record(btech.player.set_loadout,p,{armor=armor,right={weapon='PC.Sword',ammunition=1}})
      record(btech.player.set_loadout,p,{armor=armor,right={ammo=1}})
      -- Non-player and non-object receivers.
      record(btech.player.ui_preferences,0)
      record(btech.player.ui_preferences,'#nope')
      -- Direct field-access calls resolve the native function name.
      record(function() return btech.player.set_ui_preferences(p,42) end)
      record(function() return btech.template.exists(42) end)
      return out
    "#,
        )
        .unwrap();
    let expected = [
        "mux.arg.invalid | expected at least 1 arguments",
        "mux.arg.invalid | expected at least 1 arguments",
        "mux.arg.invalid | expected at least 1 arguments",
        "mux.arg.invalid | expected at least 2 arguments",
        "mux.arg.invalid | expected at least 2 arguments",
        "mux.arg.invalid | expected at least 2 arguments",
        "mux.arg.invalid | bad argument #2 to '?' (value must be a table)",
        "mux.arg.invalid | bad argument #2 to '?' (value must be a table)",
        "mux.arg.invalid | bad argument #2 to '?' (reference must be a string or nil)",
        "mux.arg.invalid | bad argument #2 to '?' (reference must contain 1 to 24 bytes)",
        "mux.arg.invalid | bad argument #2 to '?' (reference must contain 1 to 24 bytes)",
        "mux.arg.invalid | bad argument #2 to '?' (reference must not contain path components)",
        "mux.arg.invalid | bad argument #2 to '?' (reference must not contain path components)",
        "btech.template.not_found | bad argument #2 to '?' (template was not found)",
        "btech.template.invalid | bad argument #2 to '?' (template is malformed)",
        "mux.arg.invalid | bad argument #2 to '?' (unknown field 'extra')",
        "mux.arg.invalid | bad argument #2 to '?' (tactical_height must be an integer)",
        "mux.arg.invalid | bad argument #2 to '?' (tactical_height must be an integer from 5 to 24)",
        "mux.arg.invalid | bad argument #2 to '?' (tactical_width must be an integer from 5 to 40)",
        "mux.arg.invalid | bad argument #2 to '?' (lrs_height must be an integer from 10 to 40)",
        "mux.arg.invalid | bad argument #2 to '?' (include_dead must be a boolean)",
        "mux.arg.invalid | bad argument #2 to '?' (buildings must be a string)",
        "mux.arg.invalid | bad argument #2 to '?' (buildings has an invalid mode)",
        "mux.arg.invalid | bad argument #2 to '?' (buildings must contain 1 to 12 bytes)",
        "mux.arg.invalid | bad argument #2 to '?' (include_dead must be a boolean)",
        "mux.arg.invalid | bad argument #2 to '?' (armor must be a table)",
        "mux.arg.invalid | bad argument #2 to '?' (head must be an integer from 0 to 2)",
        "mux.arg.invalid | bad argument #2 to '?' (head must be an integer)",
        "mux.arg.invalid | bad argument #2 to '?' (right must be a table or nil)",
        "btech.part.not_found | bad argument #2 to '?' (right.weapon was not found)",
        "btech.part.wrong_kind | bad argument #2 to '?' (right.weapon is not a weapon)",
        "btech.part.wrong_kind | bad argument #2 to '?' (right.weapon is not a personal-combat weapon)",
        "mux.arg.invalid | bad argument #2 to '?' (ammunition must be an integer from 0 to 255)",
        "mux.arg.invalid | bad argument #2 to '?' (right.ammunition is invalid for this weapon)",
        "mux.arg.invalid | bad argument #2 to '?' (unknown field 'ammo')",
        "mux.object.invalid | bad argument #1 to '?' (object is not a live player)",
        "mux.object.invalid | bad argument #1 to '?' (object must be a dbref or Object)",
        "mux.arg.invalid | bad argument #2 to '?' (value must be a table)",
        "mux.arg.invalid | bad argument #1 to '?' (reference must be a string)",
    ];
    assert_eq!(failures.len(), expected.len());
    for (index, (failure, expected)) in failures.iter().zip(&expected).enumerate() {
        assert_eq!(failure, expected, "player rejection {index}");
    }
    // Every rejected mutation left the player untouched.
    s.eval_callback::<()>(
        "local p=mux.world.object(1); assert(not btech.player.ui_preferences(p).configured); \
         assert(btech.player.loadout(p)==nil); assert(btech.player.mechwarrior_template(p)==nil)",
    )
    .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_stored_player_configuration_is_cleared_without_aborting_reload() {
    let (_directory, config, _scripts) = isolated_scripts().await;
    let mut database = sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    let statement = "INSERT OR REPLACE INTO btech_player_configuration \
        (player_dbref,has_ui,tactical_height,tactical_width,lrs_height,include_dead,\
         include_shutdown,include_enemies,include_allies,include_target,buildings,\
         mechwarrior_template,has_loadout,armor_head,armor_torso,armor_hands,armor_feet,\
         right_weapon,left_weapon,has_right_ammunition,has_left_ammunition,right_ammunition,\
         left_ammunition,technician_available_at) \
         VALUES (1,1,14,21,11,0,1,1,1,1,2,'',1,9,8,2,2,'PC.Blazer','',1,0,12,NULL,0)";
    sqlx::query(statement).execute(&mut database).await.unwrap();
    // A syntactically valid row owned by a non-player is ignored at the same restore boundary.
    sqlx::query(
        "INSERT INTO btech_player_configuration SELECT 14,has_ui,tactical_height,\
         tactical_width,lrs_height,include_dead,include_shutdown,include_enemies,include_allies,\
         include_target,buildings,mechwarrior_template,has_loadout,armor_head,armor_torso,\
         armor_hands,armor_feet,right_weapon,left_weapon,has_right_ammunition,\
         has_left_ammunition,right_ammunition,left_ammunition,technician_available_at \
         FROM btech_player_configuration WHERE player_dbref=1",
    )
    .execute(&mut database)
    .await
    .unwrap();
    drop(database);

    let loaded = stompymux_rs::persistence::load(&config.database())
        .await
        .unwrap();
    let scripts = stompymux_rs::Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
    scripts
        .eval_callback::<()>(
            "local p=mux.world.object(1); assert(btech.player.ui_preferences(p).configured); \
             assert(btech.player.mechwarrior_template(p)==nil); assert(btech.player.loadout(p)==nil)",
        )
        .unwrap();
}
