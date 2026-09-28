//! Source-backed contracts for the trusted btech.unit administration setters.
//!
//! Reference: btmux-khi src/mux/lua/packages/btech/unit/btech_unit_admin_bindings.c
//! (BTECH_UNIT_ADMIN_ENTRIES), btech_unit_operations.c (set_armor/set_max_speed/
//! set_tonnage), and btech_unit_bindings.c (set_preferred_id/set_markings/
//! set_display_name/set_assigned_pilot) at the pinned revision.

use crate::support;
use sqlx::{Connection, SqliteConnection};
use stompymux_rs::Scripts;
use support::isolated_scripts;

/// Register two raw units and load the branded PARITY template onto the first
/// through the native loader, mirroring the pinned C construction oracle.
async fn branded_scripts() -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    Scripts,
    stompymux_rs::ObjectId,
) {
    let (_d, config, mut world) = support::isolated_world().await;
    let unit = world.create(&config, "Admin unit".into(), stompymux_rs::Kind::Thing);
    let deferred = world.create(&config, "Deferred unit".into(), stompymux_rs::Kind::Thing);
    let mut btech = serde_json::to_value(&world.btech).unwrap();
    btech["registrations"][unit.0.to_string()] = serde_json::json!("MECH");
    btech["registrations"][deferred.0.to_string()] = serde_json::json!("MECH");
    world.btech = serde_json::from_value(btech).unwrap();
    let root = config.path(&config.database.mech_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("PARITY"),
        include_str!("fixtures/btech/mechs/PARITY"),
    )
    .unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", unit.0)
        .unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("deferred_id", deferred.0)
        .unwrap();
    scripts
        .eval_callback::<()>(
            "assert(select('#',btech.unit.load_template(mux.world.object(unit_id),'PARITY'))==0)",
        )
        .unwrap();
    (_d, config, scripts, unit)
}

#[tokio::test(flavor = "current_thread")]
async fn supported_unit_admin_setters_match_zero_return_validation_and_effects() {
    let (_d, config, s, unit) = branded_scripts().await;
    s.eval_callback::<()>(r#"
      local u=mux.world.object(unit_id);local unit=btech.unit
      local function zero(name,f,...)
        local count=select('#',f(...)); assert(count==0,name..': expected 0 returns, got '..count)
      end
      local function has(code,group)
        for _,row in ipairs(unit.technologies(u)) do
          if row.code==code then return row.group==group and row.source=='configured' end
        end
        return false
      end
      -- Scalar administrative family: exact zero returns (C handlers return 0 values).
      zero('tonnage',unit.set_tonnage,u,70000)
      zero('max_speed',unit.set_max_speed,u,5)
      zero('jump_speed',unit.set_jump_speed,u,3)
      zero('heat_sinks',unit.set_heat_sinks,u,12)
      zero('lrs',unit.set_long_range_sensor_range,u,20)
      zero('tactical',unit.set_tactical_range,u,10)
      zero('scan',unit.set_scan_range,u,5)
      zero('radio_quality',unit.set_radio_quality,u,4)
      zero('radio_range',unit.set_radio_range,u,0)
      zero('cargo',unit.set_cargo_capacity,u,4,20)
      -- Identity family stores, re-reads, and clears with nil.
      zero('display',unit.set_display_name,u,'Scout'); assert(unit.display_name(u)=='Scout','display')
      zero('markings',unit.set_markings,u,'blue'); assert(unit.markings(u)=='blue','markings')
      zero('preferred',unit.set_preferred_id,u,'dz'); assert(unit.preferred_id(u)=='DZ','preferred')
      zero('pilot',unit.set_assigned_pilot,u,mux.world.object(1)); assert(unit.assigned_pilot(u):dbref()==1,'pilot')
      zero('display-clear',unit.set_display_name,u,nil); assert(unit.display_name(u)==nil,'display-clear')
      zero('markings-clear',unit.set_markings,u,nil); assert(unit.markings(u)==nil,'markings-clear')
      zero('preferred-clear',unit.set_preferred_id,u,nil); assert(unit.preferred_id(u)==nil,'preferred-clear')
      zero('pilot-clear',unit.set_assigned_pilot,u,nil); assert(unit.assigned_pilot(u)==nil,'pilot-clear')
      -- Technologies: add/remove re-reads across primary, secondary, and infantry groups.
      zero('primary-add',unit.add_technology,u,unit.technology.CRITICAL_PROOF)
      assert(has(unit.technology.CRITICAL_PROOF,'primary'),'primary-add')
      zero('primary-remove',unit.remove_technology,u,unit.technology.CRITICAL_PROOF)
      assert(not has(unit.technology.CRITICAL_PROOF),'primary-remove')
      zero('secondary-add',unit.add_technology,u,unit.technology.STEALTH_ARMOR)
      assert(has(unit.technology.STEALTH_ARMOR,'secondary'),'secondary-add')
      zero('group-clear',unit.clear_technologies,u,unit.technology_groups.UNIT)
      assert(not has(unit.technology.STEALTH_ARMOR),'group-clear')
      -- Armor patch reads inherited fields through __index like the native lua_getfield.
      local meta={armor=7};local patch=setmetatable({internal=3},{__index=meta})
      zero('armor',unit.set_armor,u,unit.sections.HEAD,patch)
      local armor=unit.armor(u,unit.sections.HEAD)
      assert(armor.armor.current==7 and armor.internal.current==3,'armor-reread')
      zero('armor-255',unit.set_armor,u,unit.sections.HEAD,{armor=255,rear_armor=0})
      armor=unit.armor(u,unit.sections.HEAD)
      assert(armor.armor.current==255 and armor.rear_armor.current==0,'armor-255-reread')
      -- Class and movement retyping; Naval has no HEAD section.
      zero('unit_type',unit.set_unit_type,u,unit.types.NAVAL)
      zero('movement_type',unit.set_movement_type,u,unit.movement_types.HULL)
      local classok,classe=mux.error.pcall(function() unit.set_armor(u,unit.sections.HEAD,{armor=1}) end)
      assert(not classok,'naval-head-raise:'..tostring(classe and classe.code)..':'..tostring(classe and classe.message))
      assert(classe.message:sub(-#'section is not valid for this unit)')=='section is not valid for this unit)','naval-head-invalid')
      -- Infantry technology requires a battlesuit; battlesuits force Biped movement.
      local infantry=unit.technology.SWARM_ATTACK
      local infantry_ok,infantry_error=mux.error.pcall(function() unit.add_technology(u,infantry) end)
      assert(not infantry_ok and infantry_error.code=='mux.object.invalid'
        and infantry_error.detail.argument==1,'infantry-gate')
      zero('battlesuit',unit.set_unit_type,u,unit.types.BATTLESUIT)
      zero('infantry-add',unit.add_technology,u,infantry)
      assert(has(infantry,'infantry'),'infantry-add')
      zero('infantry-clear',unit.clear_technologies,u,unit.technology_groups.INFANTRY)
      assert(not has(infantry),'infantry-clear')
    "#).unwrap();

    let report = stompymux_rs::view_battle_unit_fields_action(
        &s,
        &config,
        stompymux_rs::ObjectId(1),
        unit,
        "",
    )
    .unwrap();
    let field = |name: &str| {
        report
            .fields
            .iter()
            .find(|field| field.name == name)
            .and_then(|field| field.value.as_deref())
    };
    assert_eq!(field("tons"), Some("70000"));
    assert_eq!(field("maxspeed"), Some("53.75"));
    assert_eq!(field("maxjumpspeed"), Some("32.25"));
    assert_eq!(field("mechtype"), Some("Battlesuit"));
    assert_eq!(field("mechmovetype"), Some("Biped"));
    assert_eq!(field("radiotype"), Some("24"));
    assert_eq!(field("radiorange"), Some("0"));
    assert_eq!(field("cargospace"), Some("200"));
}

#[tokio::test(flavor = "current_thread")]
async fn unit_admin_argument_edges_match_native_messages() {
    let (_d, _config, s, _deferred) = branded_scripts().await;
    s.eval_callback::<()>(r#"
      local u=mux.world.object(unit_id);local unit=btech.unit
      local function fails(name,number,callable,detail,code,argument)
        local ok,e=mux.error.pcall(callable)
        assert(not ok,name..': expected failure')
        local expected_code=code or 'mux.arg.invalid'
        assert(e.code==expected_code,name..': code '..tostring(e.code)..' ~= '..expected_code)
        local suffix="bad argument #"..number.." to '"..detail.name.."' ("..detail.text..")"
        assert(e.message:sub(-#suffix)==suffix,name..': message '..e.message)
        if argument then assert(e.detail.argument==argument,name..': argument detail') end
        return e
      end
      -- Number coercion and boundaries (tons: [1, INT_MAX/1024]).
      fails('tons-type',2,function() unit.set_tonnage(u,false) end,{name='set_tonnage',text='tons must be a number'})
      fails('tons-fraction',2,function() unit.set_tonnage(u,1.5) end,{name='set_tonnage',text='tons must be an integer'})
      fails('tons-low',2,function() unit.set_tonnage(u,0.5) end,{name='set_tonnage',text='tons is outside its valid range'})
      fails('tons-empty',2,function() unit.set_tonnage(u) end,{name='set_tonnage',text='tons must be a number'})
      assert(select('#',unit.set_tonnage(u,2097151))==0)
      fails('tons-high',2,function() unit.set_tonnage(u,2097152) end,{name='set_tonnage',text='tons is outside its valid range'})
      fails('tons-nan',2,function() unit.set_tonnage(u,0/0) end,{name='set_tonnage',text='tons is outside its valid range'})
      fails('tons-huge',2,function() unit.set_tonnage(u,math.huge) end,{name='set_tonnage',text='tons is outside its valid range'})
      -- Speeds: [0, 10000] number.
      fails('speed-string',2,function() unit.set_max_speed(u,'5') end,{name='set_max_speed',text='speed must be a number'})
      fails('speed-negative',2,function() unit.set_max_speed(u,-0.5) end,{name='set_max_speed',text='speed is outside its valid range'})
      fails('speed-high',2,function() unit.set_jump_speed(u,10000.5) end,{name='set_jump_speed',text='speed is outside its valid range'})
      fails('speed-nan',2,function() unit.set_jump_speed(u,0/0) end,{name='set_jump_speed',text='speed is outside its valid range'})
      assert(select('#',unit.set_max_speed(u,0))==0 and select('#',unit.set_jump_speed(u,10000))==0)
      -- Integer ranges: heat sinks and sensors [0,127], radio quality [1,5], radio range [0,32767].
      fails('heat-type',2,function() unit.set_heat_sinks(u,false) end,{name='set_heat_sinks',text='count must be an integer'})
      fails('heat-fraction',2,function() unit.set_heat_sinks(u,12.5) end,{name='set_heat_sinks',text='count is outside its valid range'})
      fails('heat-high',2,function() unit.set_heat_sinks(u,128) end,{name='set_heat_sinks',text='count is outside its valid range'})
      assert(select('#',unit.set_heat_sinks(u,0))==0 and select('#',unit.set_heat_sinks(u,127))==0)
      fails('lrs-high',2,function() unit.set_long_range_sensor_range(u,128) end,{name='set_long_range_sensor_range',text='range is outside its valid range'})
      fails('tac-low',2,function() unit.set_tactical_range(u,-1) end,{name='set_tactical_range',text='range is outside its valid range'})
      fails('scan-string',2,function() unit.set_scan_range(u,'5') end,{name='set_scan_range',text='range must be an integer'})
      fails('quality-low',2,function() unit.set_radio_quality(u,0) end,{name='set_radio_quality',text='quality is outside its valid range'})
      fails('quality-high',2,function() unit.set_radio_quality(u,6) end,{name='set_radio_quality',text='quality is outside its valid range'})
      assert(select('#',unit.set_radio_quality(u,2.0))==0)
      fails('radio-range-high',2,function() unit.set_radio_range(u,32768) end,{name='set_radio_range',text='range is outside its valid range'})
      assert(select('#',unit.set_radio_range(u,32767))==0)
      -- Cargo: space [0,5000], maximum_tons [1,100]; extra arguments are ignored.
      fails('space-high',2,function() unit.set_cargo_capacity(u,5001,10) end,{name='set_cargo_capacity',text='space is outside its valid range'})
      fails('space-type',2,function() unit.set_cargo_capacity(u,false,1) end,{name='set_cargo_capacity',text='space must be an integer'})
      fails('tons-min-low',3,function() unit.set_cargo_capacity(u,10,0) end,{name='set_cargo_capacity',text='maximum_tons is outside its valid range'})
      fails('tons-min-high',3,function() unit.set_cargo_capacity(u,10,101) end,{name='set_cargo_capacity',text='maximum_tons is outside its valid range'})
      assert(select('#',unit.set_cargo_capacity(u,1,2,'ignored'))==0)
      -- Typed-constant identity enforcement per catalog.
      fails('movement-typed',2,function() unit.set_movement_type(u,unit.types.MECH) end,
        {name='set_movement_type',text='movement_type must be a btech.unit.movement_types constant from this runtime'})
      fails('unit-type-typed',2,function() unit.set_unit_type(u,unit.movement_types.TRACK) end,
        {name='set_unit_type',text='unit_type must be a btech.unit.types constant from this runtime'})
      fails('unit-type-number',2,function() unit.set_unit_type(u,5) end,
        {name='set_unit_type',text='unit_type must be a btech.unit.types constant from this runtime'})
      fails('technology-typed',2,function() unit.add_technology(u,unit.sections.HEAD) end,
        {name='add_technology',text='technology must be a btech.unit.technology constant from this runtime'})
      fails('remove-typed',2,function() unit.remove_technology(u,'ECM') end,
        {name='remove_technology',text='technology must be a btech.unit.technology constant from this runtime'})
      fails('group-typed',2,function() unit.clear_technologies(u,unit.types.MECH) end,
        {name='clear_technologies',text='group must be a btech.unit.technology_groups constant from this runtime'})
      -- Armor section and patch edges.
      fails('armor-section-missing',2,function() unit.set_armor(u) end,
        {name='set_armor',text='section is required and must exist on the unit'})
      fails('armor-section-nil',2,function() unit.set_armor(u,nil,{armor=1}) end,
        {name='set_armor',text='section is required and must exist on the unit'})
      fails('armor-section-typed',2,function() unit.set_armor(u,unit.technology.ECM,{armor=1}) end,
        {name='set_armor',text='section must be a btech.unit.sections constant from this runtime'})
      fails('armor-patch-type',3,function() unit.set_armor(u,unit.sections.HEAD,false) end,
        {name='set_armor',text='value must be a table'})
      fails('armor-patch-field',3,function() unit.set_armor(u,unit.sections.HEAD,{armor=1,zzz=0}) end,
        {name='set_armor',text="unknown field 'zzz'"})
      fails('armor-patch-empty',3,function() unit.set_armor(u,unit.sections.HEAD,{}) end,
        {name='set_armor',text='patch must contain at least one field'})
      fails('armor-value-type',3,function() unit.set_armor(u,unit.sections.HEAD,{armor=false}) end,
        {name='set_armor',text='armor must be an integer'})
      fails('armor-value-fraction',3,function() unit.set_armor(u,unit.sections.HEAD,{armor=1.5}) end,
        {name='set_armor',text='armor must be an integer from 0 through 255'})
      fails('armor-value-low',3,function() unit.set_armor(u,unit.sections.HEAD,{internal=-1}) end,
        {name='set_armor',text='internal must be an integer from 0 through 255'})
      fails('armor-value-nan',3,function() unit.set_armor(u,unit.sections.HEAD,{rear_armor=0/0}) end,
        {name='set_armor',text='rear_armor must be an integer from 0 through 255'})
      -- Identity strings: nil clears, everything else is a bounded string.
      fails('display-type',2,function() unit.set_display_name(u,false) end,
        {name='set_display_name',text='name must be a string or nil'})
      fails('display-empty',2,function() unit.set_display_name(u,'') end,
        {name='set_display_name',text='name must contain 1 to 16383 bytes'})
      fails('markings-type',2,function() unit.set_markings(u,42) end,
        {name='set_markings',text='markings must be a string or nil'})
      fails('preferred-empty',2,function() unit.set_preferred_id(u,'') end,
        {name='set_preferred_id',text='preferred_id must contain 1 to 2 bytes'})
      fails('preferred-long',2,function() unit.set_preferred_id(u,'abc') end,
        {name='set_preferred_id',text='preferred_id must contain 1 to 2 bytes'})
      fails('preferred-letters',2,function() unit.set_preferred_id(u,'1a') end,
        {name='set_preferred_id',text='preferred_id must contain exactly two ASCII letters'})
      -- Display names beyond the native 120-byte store cap fail as an operation error.
      local ok,e=mux.error.pcall(function() unit.set_display_name(u,string.rep('n',121)) end)
      assert(not ok and e.code=='btech.operation.failed' and e.detail.reason=='display_name_store_failed')
      assert(select('#',unit.set_display_name(u,string.rep('n',120)))==0)
      assert(unit.display_name(u)==string.rep('n',120))
      assert(select('#',unit.set_markings(u,string.rep('m',16383)))==0)
      assert(unit.markings(u)==string.rep('m',16383))
      local ok,e=mux.error.pcall(function() unit.set_markings(u,string.rep('m',16384)) end)
      assert(not ok and e.code=='mux.arg.invalid'
        and e.message:sub(-#'markings must contain 1 to 16383 bytes)')=='markings must contain 1 to 16383 bytes)','markings-long')
      -- Handle edges: unregistered object, missing unit, non-player pilot.
      fails('handle-player',1,function() unit.set_max_speed(mux.world.object(1),5) end,
        {name='set_max_speed',text='object is not a registered BTech unit'},'mux.object.invalid')
      local nil_ok,nil_e=mux.error.pcall(function() unit.set_tonnage(nil,false) end)
      assert(not nil_ok and nil_e.code=='mux.object.invalid' and nil_e.detail.argument==1)
      fails('pilot-not-player',2,function() unit.set_assigned_pilot(u,u) end,
        {name='set_assigned_pilot',text='assigned pilot must be a player'},'mux.object.invalid')
      -- Arity floors for the identity family (minimum-only, like lua_btech_check_arity).
      local arity_ok,arity_e=mux.error.pcall(function() unit.set_display_name() end)
      assert(not arity_ok and arity_e.code=='mux.arg.invalid' and arity_e.message=='expected at least 2 arguments')
      arity_ok,arity_e=mux.error.pcall(function() unit.set_assigned_pilot(u) end)
      assert(not arity_ok and arity_e.message=='expected at least 2 arguments')
      -- The deferred raw-registered unit accepts identity writes with extra arguments.
      local d=mux.world.object(deferred_id)
      assert(select('#',unit.set_display_name(d,'Stored',false,'extra'))==0)
      assert(select('#',unit.set_markings(d,'red','extra'))==0)
      assert(select('#',unit.set_preferred_id(d,'dz','extra'))==0)
      assert(select('#',unit.set_assigned_pilot(d,mux.world.object(1),'extra'))==0)
      assert(unit.display_name(d)=='Stored' and unit.markings(d)=='red' and unit.preferred_id(d)=='DZ')
      assert(unit.assigned_pilot(d):dbref()==1)
      assert(select('#',unit.set_max_speed(d,5))==0)
    "#).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn unit_admin_rejects_checking_mode_and_rolls_back_with_transaction() {
    use std::{cell::RefCell, rc::Rc};
    let (_d, config, s, unit) = branded_scripts().await;
    // A failing callback rolls the whole transaction back, including admin writes.
    let failed = s.eval_callback::<()>(
        r#"
      btech.unit.set_tonnage(mux.world.object(unit_id),70000)
      btech.unit.set_display_name(mux.world.object(unit_id),'Doomed')
      error('parity rollback')
    "#,
    );
    assert!(failed.is_err());
    assert_eq!(
        stompymux_rs::btech::administrative_unit_tonnage(&s.world(), unit),
        Some(75)
    );
    assert_eq!(
        stompymux_rs::btech::unit_configuration(&s.world(), unit).display_name,
        None
    );
    // GOING units are rejected before any mutation.
    s.world_mut()
        .objects
        .get_mut(&unit)
        .unwrap()
        .flags
        .insert(stompymux_rs::Flag::Going);
    s.eval_callback::<()>(r#"
      local ok,e=mux.error.pcall(function() btech.unit.set_max_speed(mux.world.object(unit_id),5) end)
      assert(not ok and e.code=='mux.object.unavailable'
        and e.message:sub(-#'object is going away)')=='object is going away)' and e.detail.argument==1,'going')
      ok,e=mux.error.pcall(btech.unit.set_tonnage,unit_id,80)
      assert(not ok and e.code=='mux.object.unavailable' and e.detail.argument==1)
    "#).unwrap();
    s.world_mut()
        .objects
        .get_mut(&unit)
        .unwrap()
        .flags
        .remove(stompymux_rs::Flag::Going);
    let saved = s
        .inspect_lua()
        .load("return mux.world.object(unit_id)")
        .eval::<mlua::Value>()
        .unwrap();
    s.inspect_lua().globals().set("saved_unit", saved).unwrap();
    s.world_mut().objects.get_mut(&unit).unwrap().generation = Default::default();
    s.eval_callback::<()>(
        r#"
      local ok,e=mux.error.pcall(btech.unit.set_max_speed,saved_unit,5)
      assert(not ok and e.code=='mux.object.invalid' and e.detail.argument==1
        and e.message:find("bad argument #1 to '?' (object no longer exists)",1,true))
    "#,
    )
    .unwrap();
    // Every admin callable is unavailable while @lua/check validates sources.
    let sources = std::sync::Arc::new(stompymux_rs::lua::sources::Sources::read(&config).unwrap());
    let checking = stompymux_rs::Scripts::from_sources(
        &config,
        Rc::new(RefCell::new(s.world().clone())),
        s.help().clone(),
        sources,
        stompymux_rs::RuntimeMode::Checking,
    )
    .unwrap();
    checking
        .eval_callback::<()>(
            r#"
      local unit=btech.unit
      for _,call in ipairs({
        function() unit.set_max_speed(unit_id,5) end,
        function() unit.set_display_name(unit_id,'x') end,
        function() unit.set_armor(unit_id,unit.sections.HEAD,{armor=1}) end,
        function() unit.add_technology(unit_id,unit.technology.ECM) end,
        function() unit.clear_technologies(unit_id,unit.technology_groups.ALL) end,
        function() unit.set_movement_type(unit_id,unit.movement_types.TRACK) end,
      }) do
        local ok,e=mux.error.pcall(call)
        assert(not ok and e.code=='mux.unavailable.checking')
      end
    "#,
        )
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn registered_default_admin_materializes_only_after_validated_update_and_persists() {
    let (_d, config, s) = isolated_scripts().await;
    let id = s
        .world_mut()
        .create(&config, "Fresh unit".into(), stompymux_rs::Kind::Thing);
    stompymux_rs::register_empty_battle_unit(&mut s.world_mut(), id).unwrap();
    s.inspect_lua().globals().set("fresh_id", id.0).unwrap();
    let before = serde_json::to_value(&s.world().btech).unwrap();
    s.eval_callback::<()>(
        r"
      local u=mux.world.object(fresh_id)
      local ok,e=mux.error.pcall(function() btech.unit.set_max_speed(u,false) end)
      assert(not ok and e.code=='mux.arg.invalid' and e.detail.argument==2)
    ",
    )
    .unwrap();
    assert_eq!(serde_json::to_value(&s.world().btech).unwrap(), before);
    assert!(s.world().btech.constructed_units().get(&id).is_none());

    s.eval_callback::<()>(
        r"
      local u=mux.world.object(fresh_id)
      assert(select('#',btech.unit.set_max_speed(u,5,'ignored'))==0)
    ",
    )
    .unwrap();
    assert!(s.world().btech.constructed_units().get(&id).is_some());
    assert_eq!(
        stompymux_rs::btech::administrative_unit_tonnage(&s.world(), id),
        Some(0)
    );
    let world_snapshot = s.world().clone();
    stompymux_rs::persistence::save(&config.database(), &world_snapshot)
        .await
        .unwrap();
    let restored = stompymux_rs::persistence::load(&config.database())
        .await
        .unwrap();
    assert!(restored.btech.constructed_units().get(&id).is_some());
}

/// btech.unit.unregister is a documented Rust extension with no C Lua callable:
/// the reference exposes teardown only through the native @btech/unregister
/// command, and the binding calls that command's shared teardown helper. It
/// rejects wrong-kind, stale, and going handles with the structured object
/// errors, succeeds silently for plain or already-torn-down objects, joins the
/// callback transaction, and persists the same row deletions as the command
/// (B7: btech_registration.rs covers the native dialogue).
#[tokio::test(flavor = "current_thread")]
async fn unit_unregister_extension_matches_native_teardown_contract() {
    use std::{cell::RefCell, rc::Rc};
    let (_d, config, s, unit) = branded_scripts().await;
    // Argument validation rejects wrong-kind and garbage targets before any mutation.
    s.eval_callback::<()>(r#"
      local unit=btech.unit
      for _,wrong in ipairs({mux.world.object(0),mux.world.object(2),mux.world.object(8)}) do
        local ok,e=mux.error.pcall(unit.unregister,wrong)
        assert(not ok and e.code=='mux.object.invalid' and e.detail.argument==1
          and e.message:sub(-#'object must be a live thing)')=='object must be a live thing)','wrong-kind')
      end
      for _,garbage in ipairs({nil,false,{},'text'}) do
        local ok,e=mux.error.pcall(unit.unregister,garbage)
        assert(not ok and e.code=='mux.object.invalid' and e.detail.argument==1,'garbage')
      end
    "#).unwrap();
    assert!(s.world().btech.registrations().contains_key(&unit));
    // A stale handle keeps the shared structured stale error.
    let saved = s
        .inspect_lua()
        .load("return mux.world.object(unit_id)")
        .eval::<mlua::Value>()
        .unwrap();
    s.inspect_lua().globals().set("saved_unit", saved).unwrap();
    let generation = s.world().objects[&unit].generation;
    s.world_mut().objects.get_mut(&unit).unwrap().generation = Default::default();
    s.eval_callback::<()>(
        r#"
      local ok,e=mux.error.pcall(btech.unit.unregister,saved_unit)
      assert(not ok and e.code=='mux.object.invalid' and e.detail.argument==1
        and e.message:find("bad argument #1 to '?' (object no longer exists)",1,true),'stale')
    "#,
    )
    .unwrap();
    s.world_mut().objects.get_mut(&unit).unwrap().generation = generation;
    // A going-away thing is rejected as unavailable before any mutation.
    s.world_mut()
        .objects
        .get_mut(&unit)
        .unwrap()
        .flags
        .insert(stompymux_rs::Flag::Going);
    s.eval_callback::<()>(r#"
      local ok,e=mux.error.pcall(btech.unit.unregister,mux.world.object(unit_id))
      assert(not ok and e.code=='mux.object.unavailable'
        and e.message:sub(-#'object is going away)')=='object is going away)' and e.detail.argument==1,'going')
    "#).unwrap();
    s.world_mut()
        .objects
        .get_mut(&unit)
        .unwrap()
        .flags
        .remove(stompymux_rs::Flag::Going);
    assert!(s.world().btech.registrations().contains_key(&unit));
    // The teardown joins the surrounding callback transaction: a later failure
    // restores the registration, the unit state, and the configuration.
    btech_set_identity(&s, unit);
    let before = s.world().btech.registrations().get(&unit).cloned();
    let failed = s.eval_callback::<()>(
        r#"
      assert(btech.unit.unregister(mux.world.object(unit_id))==true)
      error('parity rollback')
    "#,
    );
    assert!(failed.is_err());
    assert_eq!(s.world().btech.registrations().get(&unit), before.as_ref());
    assert!(s.world().btech.constructed_units().contains_key(&unit));
    assert_eq!(
        stompymux_rs::btech::unit_configuration(&s.world(), unit).display_name,
        Some("Doomed".into())
    );
    // MECH teardown removes registration and unit state, forgets configuration,
    // keeps the container thing, and stays idempotent for a second call.
    s.eval_callback::<()>(
        r#"
      assert(select('#',btech.unit.unregister(mux.world.object(unit_id)))==1)
      assert(btech.unit.unregister(mux.world.object(unit_id))==true)
      -- An unregistered plain thing and a freshly created plain thing both succeed.
      assert(btech.unit.unregister(mux.world.object(deferred_id))==true)
      local plain=mux.world.create_object({
        type=mux.world.types.THING, name='Plain thing', location=mux.world.object(1)})
      assert(btech.unit.unregister(plain,'ignored')==true)
    "#,
    )
    .unwrap();
    let after = s.world().clone();
    assert!(!after.btech.registrations().contains_key(&unit));
    assert!(!after.btech.units().contains_key(&unit));
    assert!(!after.btech.constructed_units().contains_key(&unit));
    assert!(!after.btech.vehicles().contains_key(&unit));
    assert_eq!(
        stompymux_rs::btech::unit_configuration(&after, unit),
        stompymux_rs::BattleUnitConfiguration::default()
    );
    assert!(!after.btech.registrations().contains_key(&deferred_unit(&s)));
    assert_eq!(after.objects[&unit].kind, stompymux_rs::Kind::Thing);
    // The removed roles persist as deleted rows and reload to the same state.
    stompymux_rs::persistence::save(&config.database(), &after)
        .await
        .unwrap();
    let restored = stompymux_rs::persistence::load(&config.database())
        .await
        .unwrap();
    assert!(!restored.btech.registrations().contains_key(&unit));
    assert!(!restored.btech.units().contains_key(&unit));
    assert!(!restored.btech.constructed_units().contains_key(&unit));
    assert_eq!(
        stompymux_rs::btech::unit_configuration(&restored, unit),
        stompymux_rs::BattleUnitConfiguration::default()
    );
    assert_eq!(restored.objects[&unit].kind, stompymux_rs::Kind::Thing);
    // The deleted roles agree at the row level with the native command's saves
    // (this fixture keeps unit state in the legacy btech_mechs table).
    let mut db = SqliteConnection::connect(&format!("sqlite://{}", config.database().display()))
        .await
        .unwrap();
    for sql in [
        "SELECT COUNT(*) FROM btech_special_registrations WHERE dbref IN (?,?)",
        "SELECT COUNT(*) FROM btech_mechs WHERE dbref IN (?,?)",
    ] {
        let count: i64 = sqlx::query_scalar(sql)
            .bind(unit.0)
            .bind(deferred_unit(&s).0)
            .fetch_one(&mut db)
            .await
            .unwrap();
        assert_eq!(count, 0, "{sql} rows survive the teardown");
    }
    db.close().await.unwrap();
    // Checking mode raises the per-entry btech unavailability message.
    let sources = std::sync::Arc::new(stompymux_rs::lua::sources::Sources::read(&config).unwrap());
    let checking = stompymux_rs::Scripts::from_sources(
        &config,
        Rc::new(RefCell::new(s.world().clone())),
        s.help().clone(),
        sources,
        stompymux_rs::RuntimeMode::Checking,
    )
    .unwrap();
    checking
        .eval_callback::<()>(
            r#"
      local ok,e=mux.error.pcall(btech.unit.unregister,unit_id)
      assert(not ok and e.code=='mux.unavailable.checking'
        and e.message=='btech.unit.unregister is unavailable during @lua/check',tostring(e))
    "#,
        )
        .unwrap();
}

/// Store identity configuration on a unit so teardown can prove it is forgotten.
fn btech_set_identity(s: &Scripts, unit: stompymux_rs::ObjectId) {
    stompymux_rs::btech::set_unit_identity_configuration(
        &mut s.world_mut(),
        unit,
        "display_name",
        Some("Doomed".into()),
    );
}

/// Read the deferred unit's dbref back out of the Lua globals.
fn deferred_unit(s: &Scripts) -> stompymux_rs::ObjectId {
    let id: i64 = s.inspect_lua().globals().get("deferred_id").unwrap();
    stompymux_rs::ObjectId(id)
}

#[tokio::test(flavor = "current_thread")]
async fn c_schema_unit_configuration_normalizes_empty_invalid_and_stale_rows() {
    let (_d, config, s) = isolated_scripts().await;
    let template =
        stompymux_rs::BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
    stompymux_rs::create_battle_unit(&mut s.world_mut(), stompymux_rs::ObjectId(14), template)
        .unwrap();
    stompymux_rs::btech::set_administrative_scalar(
        &mut s.world_mut(),
        stompymux_rs::ObjectId(14),
        "tons",
        70000.0,
    )
    .unwrap();
    stompymux_rs::btech::set_administrative_assigned_pilot(
        &mut s.world_mut(),
        stompymux_rs::ObjectId(14),
        Some(stompymux_rs::ObjectId(1)),
    )
    .unwrap();
    stompymux_rs::btech::set_administrative_unit_type(
        &mut s.world_mut(),
        stompymux_rs::ObjectId(14),
        3,
    )
    .unwrap();
    stompymux_rs::btech::set_administrative_movement_type(
        &mut s.world_mut(),
        stompymux_rs::ObjectId(14),
        5,
    )
    .unwrap();
    stompymux_rs::btech::set_administrative_radio_quality(
        &mut s.world_mut(),
        stompymux_rs::ObjectId(14),
        4,
    )
    .unwrap();
    stompymux_rs::btech::set_administrative_scalar(
        &mut s.world_mut(),
        stompymux_rs::ObjectId(14),
        "radiorange",
        0.0,
    )
    .unwrap();
    stompymux_rs::btech::set_administrative_cargo(
        &mut s.world_mut(),
        stompymux_rs::ObjectId(14),
        4,
        20,
    )
    .unwrap();
    stompymux_rs::btech::set_unit_identity_configuration(
        &mut s.world_mut(),
        stompymux_rs::ObjectId(14),
        "display_name",
        Some("Persisted".into()),
    );
    let world_snapshot = s.world().clone();
    stompymux_rs::persistence::save(&config.database(), &world_snapshot)
        .await
        .unwrap();
    let valid = stompymux_rs::persistence::load(&config.database())
        .await
        .unwrap();
    assert_eq!(
        stompymux_rs::btech::administrative_unit_tonnage(&valid, stompymux_rs::ObjectId(14)),
        Some(70000)
    );
    assert_eq!(
        stompymux_rs::btech::administrative_assigned_pilot(&valid, stompymux_rs::ObjectId(14)),
        Some(stompymux_rs::ObjectId(1))
    );
    assert_eq!(
        stompymux_rs::btech::administrative_unit_class(&valid, stompymux_rs::ObjectId(14))
            .as_deref(),
        Some("Naval")
    );
    assert_eq!(
        stompymux_rs::btech::administrative_unit_movement(&valid, stompymux_rs::ObjectId(14))
            .as_deref(),
        Some("Hull")
    );
    assert_eq!(
        stompymux_rs::btech::unit_configuration(&valid, stompymux_rs::ObjectId(14))
            .display_name
            .as_deref(),
        Some("Persisted")
    );
    let mut db = SqliteConnection::connect(&format!("sqlite://{}", config.database().display()))
        .await
        .unwrap();
    sqlx::query("UPDATE btech_unit_configuration SET preferred_id='BAD',display_name='name',markings='marks',assigned_pilot=1 WHERE object_dbref=14")
        .execute(&mut db).await.unwrap();
    let loaded = stompymux_rs::persistence::load(&config.database())
        .await
        .unwrap();
    assert_eq!(
        stompymux_rs::btech::unit_configuration(&loaded, stompymux_rs::ObjectId(14)),
        stompymux_rs::BattleUnitConfiguration::default()
    );
    sqlx::query("UPDATE btech_unit_configuration SET preferred_id='xy',display_name='',markings='',assigned_pilot=999 WHERE object_dbref=14")
        .execute(&mut db).await.unwrap();
    let loaded = stompymux_rs::persistence::load(&config.database())
        .await
        .unwrap();
    let values = stompymux_rs::btech::unit_configuration(&loaded, stompymux_rs::ObjectId(14));
    assert_eq!(values.preferred_id.as_deref(), Some("xy"));
    assert_eq!(values.display_name, None);
    assert_eq!(values.markings, None);
    assert_eq!(values.assigned_pilot, None);
    db.close().await.unwrap();
}
