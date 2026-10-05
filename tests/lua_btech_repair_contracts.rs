use crate::support;
use std::{cell::RefCell, rc::Rc};
use support::isolated_scripts;

#[tokio::test(flavor = "current_thread")]
async fn repair_queries_use_section_anatomy_and_persisted_technician_time() {
    let (_d, _c, s) = isolated_scripts().await;
    let template =
        stompymux_rs::MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
            .unwrap();
    stompymux_rs::create_battle_unit(&mut s.world_mut(), stompymux_rs::ObjectId(14), template)
        .unwrap();
    stompymux_rs::register_empty_battle_unit(&mut s.world_mut(), stompymux_rs::ObjectId(15))
        .unwrap();
    // C mech_section_is_destroyed (mech_equipment_state.c:282-291) reads a
    // section as destroyed only when armor and internal are both exhausted.
    let snapshot = serde_json::to_value(&s.world().btech).unwrap();
    let set_section = |s: &stompymux_rs::Scripts, name: &str, armor: i32, internal: i32| {
        let mut world = s.world_mut();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"]["14"]["sections"][name]["armor"] = armor.into();
        state["constructed"]["14"]["sections"][name]["internal"] = internal.into();
        world.btech = serde_json::from_value(state).unwrap();
    };
    let section_number = |name: &str, field: &str| {
        snapshot["constructed"]["14"]["sections"][name][field]
            .as_i64()
            .unwrap()
    };
    {
        let mut world = s.world_mut();
        let configuration = stompymux_rs::PlayerConfiguration {
            technician_available_at: stompymux_rs::clock::wall_time() + 20,
            ..Default::default()
        };
        stompymux_rs::btech::set_player_configuration(
            &mut world,
            stompymux_rs::ObjectId(1),
            configuration,
        )
        .unwrap();
    }
    s.eval_callback::<()>(
        r#"
      local unit=mux.world.object(14)
      local function argument_error(error_value,number,name,detail)
        local suffix="bad argument #"..number.." to '"..name.."' ("..detail..")"
        return error_value.message:sub(-#suffix)==suffix
      end
      assert(btech.repair.is_fixable(unit))
      local wait=btech.repair.technician_available_in(mux.world.object(1),'ignored')
      assert(wait >= 19 and wait <= 20)
      assert(btech.repair.is_fixable(mux.world.object(15)))
    "#,
    )
    .unwrap();

    // A destroyed head leaves a Mech fixable (C unit_is_fixable forbids only CTORSO).
    set_section(&s, "Head", 0, 0);
    s.eval_callback::<()>("assert(btech.repair.is_fixable(mux.world.object(14)))")
        .unwrap();
    set_section(&s, "CenterTorso", 0, 0);
    s.eval_callback::<()>("assert(not btech.repair.is_fixable(mux.world.object(14)))")
        .unwrap();
    // Relabeled classes drop the Mech center-torso restriction: BSUIT always
    // stays fixable and the ground-vehicle rule allows loss at the turret index
    // but not at the still-destroyed head.
    s.eval_callback::<()>(
        r#"
        local u=mux.world.object(14)
        btech.unit.set_unit_type(u,btech.unit.types.BATTLESUIT)
        assert(btech.repair.is_fixable(u))
        btech.unit.set_unit_type(u,btech.unit.types.VEHICLE)
        assert(not btech.repair.is_fixable(u))
    "#,
    )
    .unwrap();
    // With only the center torso destroyed, the ground-vehicle turret index
    // keeps the relabeled unit fixable, while arm loss is rejected.
    set_section(
        &s,
        "Head",
        section_number("Head", "armor") as i32,
        section_number("Head", "internal") as i32,
    );
    s.eval_callback::<()>("assert(btech.repair.is_fixable(mux.world.object(14)))")
        .unwrap();
    set_section(&s, "LeftArm", 0, 0);
    s.eval_callback::<()>("assert(not btech.repair.is_fixable(mux.world.object(14)))")
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn repair_fixability_allows_only_vehicle_turret_or_vtol_rotor_loss() {
    for (template, allowed, forbidden) in [
        (
            include_str!("../game/units/Demolisher.toml"),
            "Turret",
            "Front_Side",
        ),
        (
            include_str!("../game/units/Kestrel.toml"),
            "Rotor",
            "Front_Side",
        ),
    ] {
        let (_d, _c, s) = isolated_scripts().await;
        stompymux_rs::create_battle_vehicle(
            &mut s.world_mut(),
            stompymux_rs::ObjectId(14),
            stompymux_rs::VehicleTemplate::parse("test", template).unwrap(),
        )
        .unwrap();
        support::seed_object_dice(
            &mut s.world_mut(),
            stompymux_rs::ObjectId(14),
            support::FIXTURE_DICE_SEED,
        );
        s.eval_callback::<()>("assert(btech.repair.is_fixable(mux.world.object(14)))")
            .unwrap();
        stompymux_rs::damage_battle_vehicle_phase(
            &mut s.world_mut(),
            stompymux_rs::ObjectId(14),
            stompymux_rs::VehicleSection::parse(allowed).unwrap(),
            u16::MAX,
            stompymux_rs::DamagePhase::Internal,
        )
        .unwrap();
        s.eval_callback::<()>("assert(btech.repair.is_fixable(mux.world.object(14)))")
            .unwrap();
        stompymux_rs::damage_battle_vehicle_phase(
            &mut s.world_mut(),
            stompymux_rs::ObjectId(14),
            stompymux_rs::VehicleSection::parse(forbidden).unwrap(),
            u16::MAX,
            stompymux_rs::DamagePhase::Internal,
        )
        .unwrap();
        s.eval_callback::<()>("assert(not btech.repair.is_fixable(mux.world.object(14)))")
            .unwrap();
    }
}

#[tokio::test(flavor = "current_thread")]
async fn repair_apply_admits_only_five_immediate_operations_with_c_field_shapes() {
    let (_d, _c, s) = isolated_scripts().await;
    let template =
        stompymux_rs::MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
            .unwrap();
    stompymux_rs::create_battle_unit(&mut s.world_mut(), stompymux_rs::ObjectId(14), template)
        .unwrap();
    s.eval_callback::<()>(r#"
      local u=mux.world.object(14);local r=btech.repair;local o=r.operations;local sec=btech.unit.sections
      local function zero(request)
        local ok,e=mux.error.pcall(function() return r.apply(u,request) end)
        assert(ok,tostring(e))
      end
      local function rejected(request,detail)
        local ok,e=mux.error.pcall(function() return r.apply(u,request) end)
        local suffix='('..detail..')'
        assert(not ok and e.code=='mux.arg.invalid' and e.detail.argument==2
          and e.message:sub(-#suffix)==suffix,tostring(e))
      end
      -- C btech_repair_bindings.c:95-125 installs exactly these 21 operations and
      -- lua_btech_repair_apply accepts only the five immediate kinds.
      local names={'REATTACH','REPAIR_PART','REPAIR_WEAPON_TEMPORARY','REPAIR_ENHANCEMENT',
        'REPAIR_FOCUS','REPAIR_CRYSTAL','REPAIR_BARREL','REPAIR_AMMO_FEED','REPAIR_RANGING',
        'REPAIR_AMMO_MOUNT','REPLACE_WEAPON','RELOAD','REPAIR_ARMOR','REPAIR_REAR_ARMOR',
        'REPAIR_INTERNAL','DETACH','SCRAP_PART','SCRAP_WEAPON','UNLOAD','RESEAL','REPLACE_SUIT'}
      assert(#names==21,'catalog size')
      local immediate={REPAIR_ARMOR=true,REPAIR_INTERNAL=true,REPAIR_REAR_ARMOR=true,
        REPAIR_PART=true,REATTACH=true}
      for _,name in ipairs(names) do
        assert(o[name]~=nil,name)
        if not immediate[name] then
          rejected({operation=o[name],section=sec.HEAD,value=1,slot=1},
            "operation is not an immediate repair operation")
        end
      end
      -- Immediate option shapes (C VALUE_FIELDS/SLOT_FIELDS/SECTION_FIELDS).
      zero{operation=o.REPAIR_ARMOR,section=sec.HEAD,value=0}
      zero{operation=o.REPAIR_ARMOR,section=sec.HEAD,value=255}
      zero{operation=o.REPAIR_INTERNAL,section=sec.HEAD,value=1}
      zero{operation=o.REPAIR_REAR_ARMOR,section=sec.LEFT_TORSO,value=3}
      zero{operation=o.REPAIR_REAR_ARMOR,section=sec.RIGHT_TORSO,value=3}
      zero{operation=o.REPAIR_REAR_ARMOR,section=sec.CENTER_TORSO,value=3}
      zero{operation=o.REPAIR_PART,section=sec.HEAD,slot=1}
      zero{operation=o.REATTACH,section=sec.HEAD}
      rejected({operation=o.REPAIR_ARMOR,section=sec.HEAD,value=1,slot=1},"unknown field 'slot'")
      rejected({operation=o.REPAIR_INTERNAL,section=sec.HEAD,value=1,extra=0},"unknown field 'extra'")
      rejected({operation=o.REPAIR_PART,section=sec.HEAD,slot=1,value=1},"unknown field 'value'")
      rejected({operation=o.REATTACH,section=sec.HEAD,value=1},"unknown field 'value'")
      rejected({operation=o.REPAIR_REAR_ARMOR,section=sec.HEAD,value=1},
        'rear armor is only valid on Mech torso sections')
      -- C bounds: value 0..UCHAR_MAX, slot 1..crits_in_loc (mech heads and legs six rows).
      rejected({operation=o.REPAIR_ARMOR,section=sec.HEAD,value=256},'value must be an integer from 0 to 255')
      rejected({operation=o.REPAIR_ARMOR,section=sec.HEAD,value=-1},'value must be an integer from 0 to 255')
      rejected({operation=o.REPAIR_ARMOR,section=sec.HEAD,value=1.5},'value must be an integer from 0 to 255')
      rejected({operation=o.REPAIR_ARMOR,section=sec.HEAD},'value must be an integer')
      rejected({operation=o.REPAIR_PART,section=sec.HEAD,slot=0},'slot must be an integer from 1 to 6')
      rejected({operation=o.REPAIR_PART,section=sec.HEAD,slot=7},'slot must be an integer from 1 to 6')
      zero{operation=o.REPAIR_PART,section=sec.HEAD,slot=6}
      rejected({operation=o.REPAIR_PART,section=sec.CENTER_TORSO,slot=13},'slot must be an integer from 1 to 12')
      zero{operation=o.REPAIR_PART,section=sec.CENTER_TORSO,slot=12}
      -- A section the unit does not carry is rejected before any mutation.
      rejected({operation=o.REPAIR_ARMOR,section=sec.TURRET,value=1},
        'section is not valid for this unit')
      -- C btech_repair_bindings.c:272-280 installs the repair package with its
      -- operations child namespace; needs/is_under_repair stay prerequisite-blocked.
      assert(type(btech.repair)=='table' and rawequal(btech.repair,require('btech').repair))
      assert(rawequal(btech.repair.operations,o))
      for _,name in ipairs({'apply','is_fixable','technician_available_in'}) do
        assert(type(btech.repair[name])=='function',name)
      end
    "#).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn repair_apply_reattach_restores_only_destroyed_sections_and_rolls_back_failures() {
    let (_d, _c, s) = isolated_scripts().await;
    let template =
        stompymux_rs::MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
            .unwrap();
    stompymux_rs::create_battle_unit(&mut s.world_mut(), stompymux_rs::ObjectId(14), template)
        .unwrap();
    support::seed_object_dice(
        &mut s.world_mut(),
        stompymux_rs::ObjectId(14),
        support::FIXTURE_DICE_SEED,
    );
    stompymux_rs::register_empty_battle_unit(&mut s.world_mut(), stompymux_rs::ObjectId(15))
        .unwrap();
    support::seed_object_dice(
        &mut s.world_mut(),
        stompymux_rs::ObjectId(15),
        support::FIXTURE_DICE_SEED,
    );
    // C mech_re_attach (mech_maintenance.c:501-514) only restores sections that
    // read as destroyed: armor and internal both exhausted for ground hulls.
    let snapshot = serde_json::to_value(&s.world().btech).unwrap();
    {
        let mut world = s.world_mut();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"]["14"]["sections"]["LeftLeg"]["armor"] = 0.into();
        state["constructed"]["14"]["sections"]["LeftLeg"]["internal"] = 0.into();
        world.btech = serde_json::from_value(state).unwrap();
    }
    s.eval_callback::<()>(
        r#"
      local u=mux.world.object(14);local r=btech.repair;local o=r.operations
      local sec=btech.unit.sections
      local leg=btech.unit.armor(u,sec.LEFT_LEG)
      assert(leg.internal.current==0 and leg.internal.original>0,'destroyed leg')
      assert(select('#',r.apply(u,{operation=o.REATTACH,section=sec.LEFT_LEG}))==0)
      leg=btech.unit.armor(u,sec.LEFT_LEG)
      assert(leg.internal.current==leg.internal.original,'reattached leg')
      assert(leg.armor.current==0,'reattach leaves armor')
      -- Internal exhaustion alone is not destruction; reattach keeps it at zero.
      assert(select('#',r.apply(u,{operation=o.REPAIR_INTERNAL,section=sec.HEAD,value=0}))==0)
      assert(select('#',r.apply(u,{operation=o.REATTACH,section=sec.HEAD}))==0)
      assert(btech.unit.armor(u,sec.HEAD).internal.current==0,'reattach no-op')
      -- C mech_repair_part succeeds silently on empty slots; head slot three is
      -- empty in the registered default layout.
      local raw=mux.world.object(15)
      assert(select('#',r.apply(raw,{operation=o.REPAIR_PART,section=sec.HEAD,slot=3}))==0)
      -- A failed validation leaves every live value untouched.
      local before=btech.unit.armor(u,sec.HEAD)
      local ok,e=mux.error.pcall(function()
        return r.apply(u,{operation=o.REPAIR_ARMOR,section=sec.HEAD,value=999})
      end)
      assert(not ok and e.code=='mux.arg.invalid')
      local after=btech.unit.armor(u,sec.HEAD)
      assert(before.armor.current==after.armor.current and
             before.internal.current==after.internal.current,'rollback')
    "#,
    )
    .unwrap();
    // Only the head (internal zeroed) and left leg (armor exhausted, internal
    // reattached) may differ; every unrelated section keeps its snapshot value.
    let final_state = serde_json::to_value(&s.world().btech).unwrap();
    for section in [
        "RightArm",
        "LeftTorso",
        "RightTorso",
        "CenterTorso",
        "RightLeg",
    ] {
        assert_eq!(
            snapshot["constructed"]["14"]["sections"][section],
            final_state["constructed"]["14"]["sections"][section],
            "failed repair mutated {section}"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn immediate_repairs_validate_exact_shapes_and_change_only_live_material() {
    let (_d, _c, s) = isolated_scripts().await;
    let template =
        stompymux_rs::MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
            .unwrap();
    stompymux_rs::create_battle_unit(&mut s.world_mut(), stompymux_rs::ObjectId(14), template)
        .unwrap();
    support::seed_object_dice(
        &mut s.world_mut(),
        stompymux_rs::ObjectId(14),
        support::FIXTURE_DICE_SEED,
    );
    s.eval_callback::<()>(r#"
      local u=mux.world.object(14);local r=btech.repair;local o=r.operations;local sec=btech.unit.sections
      local function zero(request) assert(select('#',r.apply(u,request))==0) end
      local function argument_error(error_value,number,name,detail)
        local suffix="bad argument #"..number.." to '"..name.."' ("..detail..")"
        return error_value.message:sub(-#suffix)==suffix
      end
      local indexed=0
      local synthetic=setmetatable({}, {__index=function(_,key)
        indexed=indexed+1
        if key=='operation' then return o.REPAIR_ARMOR end
        if key=='section' then return sec.HEAD end
        if key=='value' then return 2 end
      end})
      local synthetic_ok,synthetic_error=mux.error.pcall(function() r.apply(u,synthetic) end)
      assert(not synthetic_ok and synthetic_error.code=='mux.arg.invalid' and indexed==0)
      local before=btech.unit.armor(u,sec.HEAD);local original=before.armor.original
      zero{operation=o.REPAIR_ARMOR,section=sec.HEAD,value=2}
      local after=btech.unit.armor(u,sec.HEAD);assert(after.armor.current==2 and after.armor.original==original,'armor')
      zero{operation=o.REPAIR_INTERNAL,section=sec.HEAD,value=1}
      zero{operation=o.REPAIR_REAR_ARMOR,section=sec.CENTER_TORSO,value=3}
      zero{operation=o.REPAIR_PART,section=sec.HEAD,slot=1}
      local srm
      for _,weapon in ipairs(btech.unit.weapons(u)) do
        if weapon.section==sec.CENTER_TORSO then srm=weapon.number end
      end
      assert(srm~=nil,'SRM weapon')
      local fm=btech.unit.fire_modes
      assert(select('#',btech.unit.set_weapon_modes(u,srm,{fire_modes={
        fm.DESTROYED,fm.DISABLED,fm.BROKEN,fm.DAMAGED,fm.ONE_SHOT_USED,
        fm.JETTISONED,fm.ROCKET_FIRED
      }}))==0)
      local function slot(section,number)
        for _,critical in ipairs(btech.unit.critical_slots(u,section)) do
          if critical.slot==number then return critical end
        end
      end
      local weapon=slot(sec.CENTER_TORSO,11)
      assert(not weapon.operational and #weapon.fire_modes>=7,'damaged weapon')
      zero{operation=o.REPAIR_PART,section=sec.CENTER_TORSO,slot=11}
      weapon=slot(sec.CENTER_TORSO,11)
      assert(weapon.operational and not weapon.temporary_failure and #weapon.fire_modes==0,'repaired weapon')
      local bin=slot(sec.RIGHT_TORSO,1)
      assert(bin.ammunition.rounds>0,'loaded ammunition')
      zero{operation=o.REPAIR_PART,section=sec.RIGHT_TORSO,slot=1}
      bin=slot(sec.RIGHT_TORSO,1)
      assert(bin.ammunition.rounds==0 and bin.auxiliary_data==0,'repaired ammunition data')
      zero{operation=o.REATTACH,section=sec.HEAD}
      assert(btech.unit.armor(u,sec.HEAD).internal.current==1,'reattach')
      local ok,e=mux.error.pcall(function() r.apply(u) end)
      assert(not ok and e.code=='mux.arg.invalid' and argument_error(e,2,'apply','repair request is required') and e.detail.argument==2,'missing '..tostring(e))
      ok,e=mux.error.pcall(function() r.apply(u,nil) end)
      assert(not ok and e.code=='mux.arg.invalid' and argument_error(e,2,'apply','repair request must be a table') and e.detail.argument==2,'nil '..tostring(e))
      ok,e=mux.error.pcall(function() r.apply(u,{operation=o.RELOAD,section=sec.HEAD}) end)
      assert(not ok and argument_error(e,2,'apply','operation is not an immediate repair operation'),'op '..tostring(e))
      ok,e=mux.error.pcall(function() r.apply(u,{operation=o.REPAIR_ARMOR,section=sec.HEAD,value=1,extra=true}) end)
      assert(not ok and argument_error(e,2,'apply',"unknown field 'extra'"),'extra '..tostring(e))
      ok,e=mux.error.pcall(function() r.apply(u,{operation=o.REPAIR_REAR_ARMOR,section=sec.HEAD,value=1}) end)
      assert(not ok and argument_error(e,2,'apply','rear armor is only valid on Mech torso sections'),'rear '..tostring(e))
    "#).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn registered_default_repairs_materialize_only_on_success_and_survive_restart() {
    let (_directory, config, scripts) = isolated_scripts().await;
    stompymux_rs::register_empty_battle_unit(&mut scripts.world_mut(), stompymux_rs::ObjectId(15))
        .unwrap();
    let before = serde_json::to_value(&scripts.world().btech).unwrap();
    scripts
        .eval_callback::<()>(
            r#"
            local u=mux.world.object(15)
            local ok,e=mux.error.pcall(function() btech.repair.apply(u,nil) end)
            assert(not ok and e.code=='mux.arg.invalid')
            "#,
        )
        .unwrap();
    assert_eq!(
        before,
        serde_json::to_value(&scripts.world().btech).unwrap(),
        "caught malformed repair materialized state"
    );

    scripts
        .eval_callback::<()>(
            r#"
            local u=mux.world.object(15); local r=btech.repair
            local o=r.operations; local s=btech.unit.sections
            assert(select('#',r.apply(u,{operation=o.REPAIR_ARMOR,section=s.HEAD,value=2}))==0)
            assert(select('#',r.apply(u,{operation=o.REPAIR_INTERNAL,section=s.HEAD,value=1}))==0)
            assert(select('#',r.apply(u,{operation=o.REPAIR_PART,section=s.HEAD,slot=1}))==0)
            local armor=btech.unit.armor(u,s.HEAD)
            assert(armor.armor.current==2 and armor.armor.original==0)
            assert(armor.internal.current==1 and armor.internal.original==0)
            "#,
        )
        .unwrap();
    let saved = scripts.world().clone();
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
            local armor=btech.unit.armor(mux.world.object(15),btech.unit.sections.HEAD)
            assert(armor.armor.current==2 and armor.armor.original==0)
            assert(armor.internal.current==1 and armor.internal.original==0)
            "#,
        )
        .unwrap();
}
