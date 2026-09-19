//! Focused C-shape coverage for live-unit and template inspection namespaces.

mod support;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use stompymux_rs::{
    Scripts,
    lua::{RuntimeMode, sources::Sources},
};
use support::isolated_scripts;

#[tokio::test(flavor = "current_thread")]
async fn missing_template_root_is_a_lookup_miss() {
    let (_directory, config, scripts) = isolated_scripts().await;
    let root = config.path(&config.database.mech_database);
    assert!(!root.exists());
    scripts
        .eval_callback::<()>(
            "assert(not btech.template.exists('anything')); local ok,e=mux.error.pcall(function() btech.template.engine('anything') end); assert(not ok and e.code=='btech.template.not_found')",
        )
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn registered_unit_defaults_are_inspectable_without_constructed_runtime() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    stompymux_rs::btech::register_empty_battle_unit(
        &mut scripts.world_mut(),
        stompymux_rs::ObjectId(15),
    )
    .unwrap();
    let before = scripts.world().btech.clone();
    scripts
        .eval_callback::<()>(
            r#"
      local unit=mux.world.object(15)
      local armor=btech.unit.armor(unit)
      assert(armor.armor.current==0 and armor.armor.original==0,'armor')
      assert(armor.internal.current==0 and armor.internal.original==0,'internal')
      assert(armor.rear_armor.current==0 and armor.rear_armor.original==0,'rear')
      local head=btech.unit.critical_slots(unit,btech.unit.sections.HEAD)
      assert(#head==12 and head[1].kind=='special' and head[2].kind=='special','head12')
      assert(head[3].kind=='special' and head[4].kind=='empty','head34')
      assert(head[5].kind=='special' and head[6].kind=='special','head56')
      assert(head[1].operational and head[2].operational and head[3].operational,'operational')
      assert(head[1].fire_modes[1]==nil and not head[1].temporary_failure,'modes')
      assert(#btech.unit.weapons(unit)==0,'weapons')
      assert(#btech.unit.installed_parts(unit)==0 and #btech.unit.payload(unit)==0,'inventory')
      -- C projects no channels while the radio configuration is zeroed
      -- (btech_unit_bindings.c:300-317 with mech_radio_state.c:46-48).
      assert(#btech.unit.radio_channels(unit)==0,'radio')
      assert(btech.unit.engine(unit).rating==0,'engine')
      local bv=btech.unit.battle_value(unit)
      assert(bv.total==0 and bv.offensive==0 and bv.defensive==0,'bv '..bv.total..' '..bv.offensive..' '..bv.defensive)
      assert(btech.unit.effective_max_speed(unit)==0)
      assert(btech.unit.effective_max_speed_kph(unit)==0)
      assert(btech.unit.section_condition(unit,btech.unit.sections.HEAD)=='destroyed')
      assert(#btech.unit.tic_weapons(unit,0)==0)
      assert(#btech.unit.technologies(unit)==0)
    "#,
        )
        .unwrap();
    assert_eq!(scripts.world().btech, before);
}

#[tokio::test(flavor = "current_thread")]
async fn mech_inspection_projects_exact_record_shapes_and_keeps_the_old_report() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    let template =
        stompymux_rs::BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
    stompymux_rs::create_battle_unit(
        &mut scripts.world_mut(),
        stompymux_rs::ObjectId(14),
        template,
    )
    .unwrap();
    scripts.eval_callback::<()>(r#"
      local unit=mux.world.object(14)
      btech.unit.set_assigned_pilot(unit,mux.world.object(1))
      local armor=btech.unit.armor(unit)
      assert(armor.section==nil and armor.armor.current==53 and armor.armor.original==53)
      assert(armor.internal.current==58 and armor.internal.original==58)
      assert(armor.rear_armor.current==11 and armor.rear_armor.original==11)
      local head=btech.unit.armor(unit,btech.unit.sections.HEAD)
      assert(head.section==btech.unit.sections.HEAD and head.armor.current==7 and head.internal.current==3)
      local slots=btech.unit.critical_slots(unit,btech.unit.sections.HEAD)
      assert(#slots==12 and slots[1].slot==1 and slots[1].section==btech.unit.sections.HEAD)
      assert(slots[1].kind=='special' and slots[1].part.id>0 and slots[7].kind=='empty')
      assert(slots[1].part.short_name==nil and slots[1].part.long_name==nil and slots[1].part.very_long_name==nil)
      assert(type(slots[1].operational)=='boolean' and type(slots[1].temporary_failure)=='boolean')
      btech.unit.set_armor(unit,btech.unit.sections.HEAD,{internal=0})
      local zero_internal=btech.unit.critical_slots(unit,btech.unit.sections.HEAD)
      assert(zero_internal[1].operational and #zero_internal[1].fire_modes==0)
      btech.unit.set_armor(unit,btech.unit.sections.HEAD,{internal=3})
      local weapons=btech.unit.weapons(unit)
      assert(#weapons==5 and weapons[1].number==0 and weapons[1].part.id==79)
      assert(weapons[5].part.id==135 and weapons[5].slot_count==1 and weapons[5].recycle==0)
      assert(type(weapons[1].part)=='table' and type(weapons[1].operational)=='boolean')
      assert(type(btech.unit.weapon_states(14))=='table')
      local engine=btech.unit.engine(unit)
      assert(engine.rating==245 and engine.suspension_factor==0)
      local bv=btech.unit.battle_value(unit)
      assert(math.abs(bv.total-564.29998779297)<0.000001 and bv.offensive==215 and math.abs(bv.defensive-349.29998779297)<0.000001)
      local rr=btech.unit.radio_channels(unit); assert(#rr==4); for i,row in ipairs(rr) do assert(row.channel==i and row.frequency==0 and row.title=='' and #row.modes==0) end
      local installed=btech.unit.installed_parts(unit)
      local medium_lasers,srm,ammo=0,0,0
      for _,row in ipairs(installed) do
        if row.part.id==79 then medium_lasers=row.quantity end
        if row.part.id==135 then srm=row.quantity end
        if row.part.id==327 then ammo=row.quantity end
      end
      assert(medium_lasers==0 and srm==0 and ammo==0 and #installed==0)
      local payload=btech.unit.payload(unit)
      local payload_lasers=0
      for _,row in ipairs(payload) do if row.part.id==79 then payload_lasers=row.quantity end end
      assert(payload_lasers==0 and #payload==0)
      local technologies=btech.unit.technologies(unit)
      local flip=false; for _,row in ipairs(technologies) do if row.code==btech.unit.technology.FLIPPABLE_ARMS then flip=true end end; assert(flip)
      assert(btech.unit.section_condition(unit,btech.unit.sections.HEAD)=='operational')
      assert(type(btech.unit.tic_weapons(unit,0))=='table')
      assert(btech.unit.effective_max_speed(unit)==11)
      assert(btech.unit.effective_max_speed_kph(unit)==118.25)
      assert(btech.unit.assigned_pilot(unit)~=nil)
      assert(btech.unit.preferred_id(unit)==nil and btech.unit.markings(unit)==nil and btech.unit.display_name(unit)==nil)
      local ok,err=mux.error.pcall(function() btech.unit.critical_slots(unit) end)
      assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==2)
      ok,err=mux.error.pcall(function() btech.unit.armor(unit,btech.unit.sections.TURRET) end)
      assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==2)
      ok,err=mux.error.pcall(function() btech.unit.battle_value(unit,nil) end)
      assert(not ok and err.code=='mux.arg.invalid' and err.detail==nil)
      btech.unit.set_unit_type(unit,btech.unit.types.NAVAL)
      ok,err=mux.error.pcall(function() btech.unit.armor(unit,btech.unit.sections.HEAD) end)
      assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==2)
      btech.unit.set_unit_type(unit,btech.unit.types.MECH)
    "#).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn template_lookup_is_case_insensitive_bounded_and_registry_gated() {
    let (_directory, config, scripts) = isolated_scripts().await;
    let root = config.path(&config.database.mech_database);
    std::fs::create_dir_all(root.join("stock")).unwrap();
    std::fs::write(
        root.join("stock/JR7-D"),
        include_str!("fixtures/btech/mechs/JR7-D"),
    )
    .unwrap();
    let modes = include_str!("fixtures/btech/mechs/JR7-D")
        .replace("Reference        { JR7-D }", "Reference        { MODES }")
        .replace("LifeSupport - -", "LifeSupport - - 20")
        .replace(
            "Ammo_IS.SRM-4 25 -",
            "Ammo_IS.SRM-4 25 Destroyed Disabled Broken Damaged BackPack Jettisoned OmniBase RocketFired Inferno Precision",
        );
    std::fs::write(root.join("stock/MODES"), modes).unwrap();
    std::fs::write(
        root.join("PARITY"),
        include_str!("fixtures/btech/mechs/PARITY"),
    )
    .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
      local function fails(f,...)
        local ok,e=mux.error.pcall(f,...)
        return ok and 'ok' or (e.code..' | '..e.message)
      end
      -- The pinned C part registry has no unbranded rows, so templates naming
      -- legacy generic criticals load as malformed there and here.
      assert(fails(btech.template.exists,'jr7-d')=='btech.template.invalid | bad argument #1 to \'?\' (existing template is malformed)','jr7-d')
      assert(fails(btech.template.exists,'modes')=='btech.template.invalid | bad argument #1 to \'?\' (existing template is malformed)','modes')
      assert(fails(btech.template.engine,'jr7-d')=='btech.template.invalid | bad argument #1 to \'?\' (template is malformed)','engine')
      assert(fails(btech.template.armor,'jr7-d')=='btech.template.invalid | bad argument #1 to \'?\' (template is malformed)','armor')
      assert(fails(btech.template.critical_slots,'jr7-d',btech.unit.sections.HEAD)=='btech.template.invalid | bad argument #1 to \'?\' (template is malformed)','criticals')
      assert(fails(btech.template.weapons,'jr7-d')=='btech.template.invalid | bad argument #1 to \'?\' (template is malformed)','weapons')
      assert(fails(btech.template.base_cost,'jr7-d')=='btech.template.invalid | bad argument #1 to \'?\' (template is malformed)','cost')
      assert(fails(btech.template.battle_value,'jr7-d')=='btech.template.invalid | bad argument #1 to \'?\' (template is malformed)','bv')
      assert(fails(btech.template.payload,'jr7-d')=='btech.template.invalid | bad argument #1 to \'?\' (template is malformed)','payload')
      assert(fails(btech.template.installed_parts,'jr7-d')=='btech.template.invalid | bad argument #1 to \'?\' (template is malformed)','installed')
      assert(fails(btech.template.technologies,'jr7-d')=='btech.template.invalid | bad argument #1 to \'?\' (template is malformed)','technologies')
      assert(fails(btech.template.show_status,'jr7-d',mux.world.object(1))=='btech.template.invalid | bad argument #1 to \'?\' (template is malformed)','show')
      -- Branded templates stay loadable and case-insensitive.
      assert(btech.template.exists('parity'))
      assert(not btech.template.exists('definitely-missing-template'))
      assert(not btech.template.exists(string.char(255)))
      assert(btech.template.base_cost('parity')==8356250)
    "#,
        )
        .unwrap();
    std::fs::write(
        root.join("stock/LateTemplate"),
        include_str!("fixtures/btech/mechs/PARITY"),
    )
    .unwrap();
    scripts
        .eval_callback::<()>("assert(not btech.template.exists('LateTemplate'))")
        .unwrap();
    std::fs::remove_file(root.join("PARITY")).unwrap();
    scripts
        .eval_callback::<()>(
            "assert(not btech.template.exists('PARITY')); assert(btech.template.exists('LateTemplate'))",
        )
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn vehicle_inspection_uses_vehicle_sections_and_raw_slot_inventory() {
    let (_directory, config, scripts) = isolated_scripts().await;
    let source = include_str!("../game/mechs/Demolisher");
    let template = stompymux_rs::BattleVehicleTemplate::parse(source).unwrap();
    stompymux_rs::create_battle_vehicle(
        &mut scripts.world_mut(),
        stompymux_rs::ObjectId(14),
        template,
    )
    .unwrap();
    let root = config.path(&config.database.mech_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("Demolisher"), source).unwrap();
    // The shipped Demolisher names unbranded criticals, so the template
    // surface rejects it exactly like the pinned C registry does.
    scripts.eval_callback::<()>(r#"
      local function fails(f,...)
        local ok,e=mux.error.pcall(f,...)
        return ok and 'ok' or (e.code..' | '..e.message)
      end
      assert(fails(btech.template.exists,'demolisher')=='btech.template.invalid | bad argument #1 to \'?\' (existing template is malformed)')
      assert(fails(btech.template.armor,'demolisher')=='btech.template.invalid | bad argument #1 to \'?\' (template is malformed)')
      assert(fails(btech.template.weapons,'demolisher')=='btech.template.invalid | bad argument #1 to \'?\' (template is malformed)')
      assert(fails(btech.template.engine,'demolisher')=='btech.template.invalid | bad argument #1 to \'?\' (template is malformed)')
    "#).unwrap();
    scripts.eval_callback::<()>(r#"
      local unit=mux.world.object(14)
      local armor=btech.unit.armor(unit)
      assert(armor.armor.current==160 and armor.armor.original==160)
      assert(armor.internal.current==40 and armor.internal.original==40)
      local turret=btech.unit.armor(unit,btech.unit.sections.TURRET)
      assert(turret.armor.current==40 and turret.internal.current==8)
      local weapons=btech.unit.weapons(unit)
      assert(#weapons==2 and weapons[1].part.id==98 and weapons[1].section==btech.unit.sections.TURRET,'weapons')
      assert(weapons[1].first_slot==1 and weapons[1].slot_count==1 and weapons[1].recycle==0,'weapon fields '..weapons[1].slot_count)
      local installed=btech.unit.installed_parts(unit)
      local ac20,ammo=0,0
      for _,row in ipairs(installed) do
        if row.part.id==98 then ac20=row.quantity end
        if row.part.id==290 then ammo=row.quantity end
      end
      assert(ac20==0 and ammo==0 and #installed==0,'installed '..#installed..' '..ac20..' '..ammo)
      local slots=btech.unit.critical_slots(unit,btech.unit.sections.TURRET)
      assert(#slots==12 and slots[1].kind=='weapon' and slots[3].kind=='ammunition')
      assert(slots[3].ammunition.rounds==5 and slots[3].ammunition.capacity==5)
      assert(btech.unit.section_condition(unit,btech.unit.sections.TURRET)=='operational')
      assert(btech.unit.effective_max_speed_kph(unit)==53.75)
      -- 80-ton tracked vehicle: rating round((2*53.75/10.75)/3)*80 = 240,
      -- suspension 0 (mech_consistency.c susp_factor MOVE_TRACK).
      local vehicle_engine=btech.unit.engine(unit)
      assert(vehicle_engine.rating==240 and vehicle_engine.suspension_factor==0,'vehicle engine')
    "#).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn inspection_getters_follow_c_argument_contracts() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    let template =
        stompymux_rs::BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
    stompymux_rs::create_battle_unit(
        &mut scripts.world_mut(),
        stompymux_rs::ObjectId(14),
        template,
    )
    .unwrap();
    let before = scripts.world().btech.clone();
    scripts.eval_callback::<()>(r#"
      local unit=mux.world.object(14)
      local god=mux.world.object(1)
      -- Extra arguments are ignored (C lua_btech_check_arity is minimum-only and
      -- fixed-position handlers never read later stack slots).
      assert(btech.unit.armor(unit,btech.unit.sections.HEAD,'extra').armor.current==7,'armor extra')
      assert(#btech.unit.weapons(unit,nil,'extra')==5,'weapons extra')
      assert(#btech.unit.critical_slots(unit,btech.unit.sections.HEAD,'extra')==12,'criticals extra')
      assert(#btech.unit.radio_channels(unit,'extra')==4,'radio extra')
      assert(btech.unit.engine(unit,'extra').rating==245,'engine extra')
      assert(type(btech.unit.payload(unit,'extra'))=='table','payload extra')
      assert(type(btech.unit.installed_parts(unit,'extra'))=='table','installed extra')
      assert(type(btech.unit.technologies(unit,'extra'))=='table','technologies extra')
      assert(btech.unit.preferred_id(unit,'extra')==nil,'preferred extra')
      assert(btech.unit.markings(unit,'extra')==nil,'markings extra')
      assert(btech.unit.display_name(unit,'extra')==nil,'display extra')
      assert(btech.unit.assigned_pilot(unit,'extra')==nil,'pilot extra')
      assert(btech.unit.effective_max_speed(unit,'extra')==11,'speed extra')
      assert(btech.unit.effective_max_speed_kph(unit,'extra')==118.25,'kph extra')
      assert(btech.unit.section_condition(unit,btech.unit.sections.HEAD,'extra')=='operational','condition extra')
      assert(type(btech.unit.tic_weapons(unit,0,'extra'))=='table','tic extra')
      -- battle_value keeps C's exact-arity gate (btech_unit_bindings.c:340-347).
      local ok,err=mux.error.pcall(btech.unit.battle_value)
      assert(not ok and err.code=='mux.arg.invalid' and err.message=='expected exactly 1 argument' and err.detail==nil,'bv none')
      ok,err=mux.error.pcall(btech.unit.battle_value,unit,'extra')
      assert(not ok and err.code=='mux.arg.invalid' and err.message=='expected exactly 1 argument' and err.detail==nil,'bv extra')
      -- Minimum-arity gates (btech_package.c:76-80).
      for _,name in ipairs({'preferred_id','markings','assigned_pilot'}) do
        ok,err=mux.error.pcall(btech.unit[name])
        assert(not ok and err.code=='mux.arg.invalid' and err.message=='expected at least 1 arguments',name)
      end
      -- display_name has no arity gate; the object check fires first
      -- (btech_unit_bindings.c:604-613).
      ok,err=mux.error.pcall(btech.unit.display_name)
      assert(not ok and err.code=='mux.object.invalid' and err.detail.argument==1,'display arity')
      -- Every callable returns exactly one value.
      assert(select('#',btech.unit.armor(unit))==1,'armor count')
      assert(select('#',btech.unit.critical_slots(unit,btech.unit.sections.HEAD))==1,'criticals count')
      assert(select('#',btech.unit.weapons(unit))==1,'weapons count')
      assert(select('#',btech.unit.radio_channels(unit))==1,'radio count')
      assert(select('#',btech.unit.engine(unit))==1,'engine count')
      assert(select('#',btech.unit.battle_value(unit))==1,'bv count')
      assert(select('#',btech.unit.payload(unit))==1,'payload count')
      assert(select('#',btech.unit.installed_parts(unit))==1,'installed count')
      assert(select('#',btech.unit.technologies(unit))==1,'technologies count')
      assert(select('#',btech.unit.preferred_id(unit))==1,'preferred count')
      assert(select('#',btech.unit.markings(unit))==1,'markings count')
      assert(select('#',btech.unit.display_name(unit))==1,'display count')
      assert(select('#',btech.unit.assigned_pilot(unit))==1,'pilot count')
      assert(select('#',btech.unit.effective_max_speed(unit))==1,'speed count')
      assert(select('#',btech.unit.effective_max_speed_kph(unit))==1,'kph count')
      assert(select('#',btech.unit.section_condition(unit,btech.unit.sections.HEAD))==1,'condition count')
      assert(select('#',btech.unit.tic_weapons(unit,0))==1,'tic count')
      -- Required-section callables (btech_unit_bindings.c:205-215,
      -- btech_unit_operations.c:115-129).
      for _,call in ipairs({btech.unit.critical_slots,btech.unit.section_condition}) do
        ok,err=mux.error.pcall(call,unit)
        assert(not ok and err.code=='mux.arg.invalid' and err.message=="bad argument #2 to '?' (section is required)" and err.detail.argument==2,'section required')
        ok,err=mux.error.pcall(call,unit,nil)
        assert(not ok and err.code=='mux.arg.invalid' and err.message=="bad argument #2 to '?' (section is required)",'section explicit nil')
        ok,err=mux.error.pcall(call,unit,btech.unit.sections.TURRET)
        assert(not ok and err.code=='mux.arg.invalid' and err.message=="bad argument #2 to '?' (section is not valid for this unit)" and err.detail.argument==2,'section turret')
      end
      ok,err=mux.error.pcall(btech.unit.weapons,unit,btech.unit.sections.TURRET)
      assert(not ok and err.code=='mux.arg.invalid' and err.message=="bad argument #2 to '?' (section is not valid for this unit)",'weapons turret')
      -- Non-constant section values (btech_unit_constants.c:638-651).
      ok,err=mux.error.pcall(btech.unit.armor,unit,42)
      assert(not ok and err.code=='mux.arg.invalid' and err.message=="bad argument #2 to '?' (section must be a btech.unit.sections constant from this runtime)",'section number')
      ok,err=mux.error.pcall(btech.unit.armor,unit,'HEAD')
      assert(not ok and err.code=='mux.arg.invalid' and err.message=="bad argument #2 to '?' (section must be a btech.unit.sections constant from this runtime)",'section string')
      -- tic coercion edges in C order: type, finite range, integral
      -- (btech_unit_operations.c:46-57,248-267).
      local tic_errors={}
      for _,value in ipairs({'x',false,4,-1,1.5,0/0,math.huge}) do
        ok,err=mux.error.pcall(btech.unit.tic_weapons,unit,value)
        tic_errors[#tic_errors+1]=(ok and 'ok') or err.message
      end
      ok,err=mux.error.pcall(btech.unit.tic_weapons,unit,nil)
      tic_errors[#tic_errors+1]=(ok and 'ok') or err.message
      assert(tic_errors[1]=="bad argument #2 to '?' (tic must be a number)",tic_errors[1])
      assert(tic_errors[2]==tic_errors[1],tic_errors[2])
      assert(tic_errors[3]=="bad argument #2 to '?' (tic is outside its valid range)",tic_errors[3])
      assert(tic_errors[4]==tic_errors[3],tic_errors[4])
      assert(tic_errors[5]=="bad argument #2 to '?' (tic must be an integer)",tic_errors[5])
      assert(tic_errors[6]==tic_errors[3],tic_errors[6])
      assert(tic_errors[7]==tic_errors[3],tic_errors[7])
      assert(tic_errors[8]==tic_errors[1],tic_errors[8])
      assert(type(btech.unit.tic_weapons(unit,3))=='table','tic three')
      -- Handle edges shared with C's lua_mux_require_object_at.
      ok,err=mux.error.pcall(btech.unit.armor,nil)
      assert(not ok and err.code=='mux.object.invalid' and err.message=="bad argument #1 to '?' (object must be a dbref or Object)" and err.detail.argument==1,'nil handle')
      ok,err=mux.error.pcall(btech.unit.armor,true)
      assert(not ok and err.code=='mux.object.invalid' and err.message=="bad argument #1 to '?' (object must be a dbref or Object)",'boolean handle')
      ok,err=mux.error.pcall(btech.unit.armor,{})
      assert(not ok and err.code=='mux.object.invalid' and err.message=="bad argument #1 to '?' (object must be a dbref or Object)",'table handle')
      ok,err=mux.error.pcall(btech.unit.armor,999999)
      assert(not ok and err.code=='mux.object.invalid' and err.message=="bad argument #1 to '?' (object is invalid)",'missing dbref')
      -- Numeric strings coerce to dbrefs in C LuaJIT.
      assert(btech.unit.armor('14').armor.current==53,'numeric string handle')
      for _,handle in ipairs({god,mux.world.object(0)}) do
        ok,err=mux.error.pcall(btech.unit.armor,handle)
        assert(not ok and err.code=='mux.object.invalid' and err.message=="bad argument #1 to '?' (object is not a registered BTech unit)" and err.detail.argument==1,'unregistered handle')
      end
    "#).unwrap();
    assert_eq!(scripts.world().btech, before);
}

#[tokio::test(flavor = "current_thread")]
async fn engine_suspension_factor_matches_c_susp_factor() {
    let (_directory, _config, scripts) = isolated_scripts().await;
    let template =
        stompymux_rs::BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
    stompymux_rs::create_battle_unit(
        &mut scripts.world_mut(),
        stompymux_rs::ObjectId(14),
        template,
    )
    .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
      local unit=mux.world.object(14)
      local types=btech.unit.movement_types
      -- 35-ton chassis through C mech_consistency.c:170-218.
      local expected={
        BIPED=0,TRACK=0,WHEEL=20,HOVER=175,VTOL=140,HULL=30,FOIL=195,FLY=0,QUAD=0,SUB=30,
      }
      for name,factor in pairs(expected) do
        btech.unit.set_movement_type(unit,types[name])
        local engine=btech.unit.engine(unit)
        assert(engine.suspension_factor==factor,name..' '..engine.suspension_factor)
        assert(engine.rating==245,name..' rating')
      end
      btech.unit.set_movement_type(unit,types.BIPED)
      assert(btech.unit.engine(unit).suspension_factor==0,'restored')
    "#,
        )
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn inspection_getters_are_unavailable_while_checking() {
    let (_directory, config, scripts) = isolated_scripts().await;
    let sources = Arc::new(Sources::read(&config).unwrap());
    let world = stompymux_rs::persistence::load(&config.database())
        .await
        .unwrap();
    let checking = Scripts::from_sources(
        &config,
        Rc::new(RefCell::new(world)),
        scripts.help().clone(),
        sources,
        RuntimeMode::Checking,
    )
    .unwrap();
    checking
        .eval_callback::<()>(
            r#"
      local names={'armor','critical_slots','weapons','radio_channels','engine','battle_value',
        'payload','installed_parts','technologies','preferred_id','markings','display_name',
        'assigned_pilot','effective_max_speed','effective_max_speed_kph','section_condition',
        'tic_weapons'}
      for _,name in ipairs(names) do
        local ok,err=mux.error.pcall(function() btech.unit[name]() end)
        assert(not ok and err.code=='mux.unavailable.checking',name)
      end
    "#,
        )
        .unwrap();
}

#[test]
fn parity_probe_templates_parse_through_unit_construction() {
    let ground = include_str!("fixtures/lua-probes/templates/PARITY-GROUND");
    let vtol = include_str!("fixtures/lua-probes/templates/PARITY-VTOL");
    let naval = include_str!("fixtures/lua-probes/templates/PARITY-NAVAL");
    assert!(stompymux_rs::BattleUnitTemplate::parse(ground).is_ok());
    assert!(stompymux_rs::BattleUnitTemplate::parse(vtol).is_ok());
    // Naval unit construction stays blocked until the loader accepts the class
    // (C template_load.c loads every unit class).
    assert_eq!(
        stompymux_rs::BattleUnitTemplate::parse(naval)
            .unwrap_err()
            .to_string(),
        "Unsupported unit template type Naval"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn parity_probe_vehicle_templates_load_and_project() {
    let (_directory, config, scripts) = isolated_scripts().await;
    let root = config.path(&config.database.mech_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("PARITY-GROUND"),
        include_str!("fixtures/lua-probes/templates/PARITY-GROUND"),
    )
    .unwrap();
    std::fs::write(
        root.join("PARITY-VTOL"),
        include_str!("fixtures/lua-probes/templates/PARITY-VTOL"),
    )
    .unwrap();
    stompymux_rs::btech::register_empty_battle_unit(
        &mut scripts.world_mut(),
        stompymux_rs::ObjectId(14),
    )
    .unwrap();
    stompymux_rs::btech::register_empty_battle_unit(
        &mut scripts.world_mut(),
        stompymux_rs::ObjectId(15),
    )
    .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
      local ground=mux.world.object(14)
      local vtol=mux.world.object(15)
      btech.unit.load_template(ground,'PARITY-GROUND')
      btech.unit.load_template(vtol,'PARITY-VTOL')
      -- Manufacturer-qualified criticals (Agra.IS.PPC) load on both classes in C.
      local turret=btech.unit.armor(ground,btech.unit.sections.TURRET)
      assert(turret.armor.current==30 and turret.internal.current==6,'ground turret')
      assert(#btech.unit.weapons(ground)==1 and btech.unit.weapons(ground)[1].part.id==77,'ground weapon')
      assert(btech.unit.section_condition(vtol,btech.unit.sections.ROTOR)=='operational','vtol rotor')
      assert(#btech.unit.critical_slots(vtol,btech.unit.sections.TURRET)==12,'vtol turret slots')
    "#,
        )
        .unwrap();
}
