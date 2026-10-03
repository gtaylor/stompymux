//! Source-backed contracts for trusted live-unit and construction operations.

use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

#[tokio::test]
async fn damage_piloting_and_template_lifecycle_match_zero_return_contracts() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Unit".into(), Kind::Thing);
    let mut btech = serde_json::to_value(&world.btech).unwrap();
    btech["registrations"][id.0.to_string()] = serde_json::json!("MECH");
    world.btech = serde_json::from_value(btech).unwrap();
    let pilot = world.create(&config, "Pilot".into(), Kind::Player);
    world.objects.get_mut(&pilot).unwrap().location = Some(id);
    let root = config.path(&config.database.mech_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("PARITY.toml"),
        include_str!("fixtures/btech/mechs/PARITY.toml"),
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("pilot_id", pilot.0)
        .unwrap();
    scripts.eval_callback::<()>(r#"
        local unit=mux.world.object(unit_id)
        assert(select('#',btech.unit.load_template(unit,'PARITY'))==0)
        assert(select('#',btech.unit.apply_damage(unit,{amount=2,cluster_size=1,direction_code=0,force_critical=false,unit_message='contract damage'}))==0)
        assert(btech.unit.armor(unit,btech.unit.sections.LEFT_ARM).armor.current==8)
        assert(btech.unit.section_condition(unit,btech.unit.sections.LEFT_ARM)=='operational')
        assert(select('#',btech.unit.set_tonnage(unit,95))==0)
        assert(select('#',btech.unit.set_unit_type(unit,btech.unit.types.VEHICLE))==0)
        assert(select('#',btech.unit.set_movement_type(unit,btech.unit.movement_types.WHEEL))==0)
        assert(select('#',btech.unit.save_template(unit,'saved-contract'))==0)
    "#).unwrap();
    assign_battle_pilot(&mut scripts.world_mut(), id, pilot).unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        btech.unit.radio_frequency(unit_id,pilot_id,0,123)
        assert(select('#',btech.unit.load_template(unit,'PARITY'))==0)
        assert(btech.unit.radio_channels(unit)[1].frequency==123)
        assert(select('#',btech.unit.load_template(unit,'parity'))==0)
        assert(btech.unit.armor(unit,btech.unit.sections.LEFT_ARM).armor.current==10)
        assert(btech.unit.radio_channels(unit)[1].frequency==0)
        -- Saved templates carry the C brand-zero part spellings and are not reloadable.
        local ok,err=mux.error.pcall(btech.template.exists,'saved-contract')
        assert(not ok and err.code=='btech.template.invalid' and err.detail.argument==1)
    "#,
        )
        .unwrap();
    assign_battle_pilot(&mut scripts.world_mut(), id, pilot).unwrap();
    scripts.eval_callback::<()>(r#"
        local unit=mux.world.object(unit_id)
        btech.unit.radio_frequency(unit_id,pilot_id,0,456)
        assert(select('#',btech.unit.restore(unit))==0)
        assert(btech.unit.radio_channels(unit)[1].frequency==456)
        local ok,err=mux.error.pcall(btech.unit.load_template,unit,'missing')
        assert(not ok and err.code=='btech.template.not_found' and err.message:find("bad argument #2 to '?' (template was not found)",1,true) and err.detail.argument==2)
        ok,err=mux.error.pcall(btech.unit.load_template,unit,'saved-contract')
        assert(not ok and err.code=='btech.template.invalid' and err.detail.argument==2)
        ok,err=mux.error.pcall(btech.unit.save_template,unit,'../bad')
        assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==2)
    "#).unwrap();
    scripts.drain_outbox();
    assert!(root.join("saved-contract.toml").is_file());
    let saved = std::fs::read_to_string(root.join("saved-contract.toml")).unwrap();
    assert!(saved.contains("class = \"vehicle\""));
    assert!(saved.contains("movement = \"wheel\""));
    assert!(saved.contains("tons = 95"));
    assert!(!saved.contains("administrative_"));
}

#[tokio::test]
async fn critical_weapon_ammunition_modes_and_special_edits_are_strict() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Unit".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("JR7-D", include_str!("../game/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    let root = config.path(&config.database.mech_database);
    std::fs::create_dir_all(&root).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    scripts.eval_callback::<()>(r#"
        local unit=mux.world.object(unit_id)
        local ok,err=mux.error.pcall(btech.unit.apply_damage,unit,false)
        assert(not ok and err.code=='mux.arg.invalid' and err.message:find("bad argument #2 to '?' (value must be a table)",1,true) and err.detail.argument==2)
        ok,err=mux.error.pcall(btech.unit.apply_damage,unit,{cluster_size=1,direction_code=0})
        assert(not ok and err.code=='mux.arg.invalid' and err.message:find("bad argument #2 to '?' (amount must be an integer)",1,true) and err.detail.argument==2)
        ok,err=mux.error.pcall(btech.unit.piloting_check,unit,{roll_modifier=0,damage_modifier=0,extra=true})
        assert(not ok and err.code=='mux.arg.invalid' and err.message:find("bad argument #2 to '?' (unknown field 'extra')",1,true) and err.detail.argument==2)
        assert(select('#',btech.unit.reset_critical_slots(unit))==0)
        assert(#btech.unit.weapons(unit)==0)
        assert(select('#',btech.unit.install_weapon(unit,{part='IS.MediumLaser',section=btech.unit.sections.LEFT_TORSO,slots={1},rear_facing=true}))==0)
        local weapons=btech.unit.weapons(unit); assert(#weapons==1 and weapons[1].first_slot==1)
        local fire={
          btech.unit.fire_modes.DESTROYED,btech.unit.fire_modes.DISABLED,btech.unit.fire_modes.BROKEN,
          btech.unit.fire_modes.DAMAGED,btech.unit.fire_modes.TARGETING_COMPUTER,btech.unit.fire_modes.REAR_MOUNT,
          btech.unit.fire_modes.HOTLOAD,btech.unit.fire_modes.HALF_TON,btech.unit.fire_modes.ONE_SHOT,
          btech.unit.fire_modes.ONE_SHOT_USED,btech.unit.fire_modes.ULTRA,btech.unit.fire_modes.RAPID_FIRE,
          btech.unit.fire_modes.GATLING,btech.unit.fire_modes.ROTARY_TWO_SHOT,btech.unit.fire_modes.ROTARY_FOUR_SHOT,
          btech.unit.fire_modes.ROTARY_SIX_SHOT,btech.unit.fire_modes.HEAT,btech.unit.fire_modes.BACKPACK,
          btech.unit.fire_modes.JETTISONED,btech.unit.fire_modes.OMNI_BASE,btech.unit.fire_modes.ROCKET_FIRED}
        local ammo={
          btech.unit.ammunition_modes.LBX_CLUSTER,btech.unit.ammunition_modes.ARTEMIS_MINE,
          btech.unit.ammunition_modes.NARC_SMOKE,btech.unit.ammunition_modes.CLUSTER,btech.unit.ammunition_modes.MINE,
          btech.unit.ammunition_modes.SMOKE,btech.unit.ammunition_modes.INFERNO,btech.unit.ammunition_modes.SWARM,
          btech.unit.ammunition_modes.SWARM_1,btech.unit.ammunition_modes.INARC_EXPLOSIVE,
          btech.unit.ammunition_modes.INARC_HAYWIRE,btech.unit.ammunition_modes.INARC_ECM,
          btech.unit.ammunition_modes.INARC_NEMESIS,btech.unit.ammunition_modes.ARMOR_PIERCING,
          btech.unit.ammunition_modes.FLECHETTE,btech.unit.ammunition_modes.INCENDIARY,
          btech.unit.ammunition_modes.PRECISION,btech.unit.ammunition_modes.STINGER,
          btech.unit.ammunition_modes.CASELESS,btech.unit.ammunition_modes.SEMI_GUIDED,
          btech.unit.ammunition_modes.EXTENDED_RANGE,btech.unit.ammunition_modes.HIGH_EXPLOSIVE,
          btech.unit.ammunition_modes.MML_LRM}
        assert(select('#',btech.unit.set_weapon_modes(unit,0,{fire_modes=fire,ammunition_modes=ammo}))==0)
        local first=btech.unit.critical_slots(unit,btech.unit.sections.LEFT_TORSO)[1]
        assert(#first.fire_modes==21 and #first.ammunition_modes==23 and not first.operational)
        assert(select('#',btech.unit.configure_ammunition(unit,{weapon='IS.SRM-4',section=btech.unit.sections.RIGHT_TORSO,slot=1,half_ton=true,ammunition_modes={btech.unit.ammunition_modes.INFERNO}}))==0)
        assert(select('#',btech.unit.restock_ammunition(unit,btech.unit.sections.RIGHT_TORSO,1))==0)
        assert(select('#',btech.unit.install_special(unit,{section=btech.unit.sections.HEAD,slot=4}))==0)
        first=btech.unit.critical_slots(unit,btech.unit.sections.LEFT_TORSO)[1]
        assert(#first.fire_modes==21 and #first.ammunition_modes==23)
        assert(select('#',btech.unit.save_template(unit,'mode-contract'))==0)
        ok,err=mux.error.pcall(btech.unit.install_weapon,unit,{part={id=2047},section=btech.unit.sections.LEFT_TORSO,slots={1}})
        assert(not ok and err.code=='btech.part.not_found' and err.message:find("bad argument #2 to '?' (part was not found)",1,true) and err.detail.argument==2)
        ok,err=mux.error.pcall(btech.unit.install_weapon,unit,{part='IS.MediumLaser',section=btech.unit.sections.LEFT_TORSO,slots={1,1}})
        assert(not ok and err.code=='mux.arg.invalid' and err.message:find("bad argument #2 to '?' (slots must contain the required critical slots)",1,true))
        ok,err=mux.error.pcall(btech.unit.set_weapon_modes,unit,9,{})
        assert(not ok and err.code=='mux.arg.invalid' and err.message:find("bad argument #2 to '?' (weapon number is not mounted)",1,true) and err.detail.argument==2)
        ok,err=mux.error.pcall(btech.unit.configure_ammunition,unit,{weapon='IS.MediumLaser',section=btech.unit.sections.RIGHT_TORSO,slot=1})
        assert(not ok and err.code=='mux.arg.invalid' and err.message:find("bad argument #2 to '?' (weapon does not use ammunition)",1,true))
        ok,err=mux.error.pcall(btech.unit.install_special,unit,{part='IS.MediumLaser',section=btech.unit.sections.HEAD,slot=4})
        assert(not ok and err.code=='btech.part.wrong_kind' and err.message:find("bad argument #2 to '?' (part must be special equipment)",1,true))
    "#).unwrap();
    let saved = BattleUnitTemplate::parse(
        "mode-contract",
        &std::fs::read_to_string(root.join("mode-contract.toml")).unwrap(),
    )
    .unwrap();
    let BattleUnitTemplate::Mech(saved) = saved else {
        panic!("expected Mech template")
    };
    let modes = &saved.sections[&BattleSection::LeftTorso].criticals[&0].modes;
    assert_eq!(modes.len(), 44);
}

#[tokio::test(flavor = "current_thread")]
async fn unit_operation_boundaries_reject_going_objects() {
    use std::sync::Arc;
    use stompymux_rs::lua::{RuntimeMode, sources::Sources};
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Unit".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("JR7-D", include_str!("../game/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let saved = scripts
        .inspect_lua()
        .load(format!("return mux.world.object({})", id.0))
        .eval::<mlua::Value>()
        .unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("saved_unit", saved)
        .unwrap();
    scripts
        .world_mut()
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    scripts.eval_callback::<()>(r#"
        local ok,err=mux.error.pcall(btech.unit.restore,unit_id)
        assert(not ok and err.code=='mux.object.unavailable' and err.message:find("bad argument #1 to '?' (object is going away)",1,true) and err.detail.argument==1)
    "#).unwrap();
    scripts
        .world_mut()
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .remove(Flag::Going);
    scripts.world_mut().objects.get_mut(&id).unwrap().generation = Default::default();
    scripts.eval_callback::<()>(r#"
        local ok,err=mux.error.pcall(btech.unit.restore,saved_unit)
        assert(not ok and err.code=='mux.object.invalid' and err.message:find("bad argument #1 to '?' (object no longer exists)",1,true) and err.detail.argument==1)
    "#).unwrap();
    let checking = Scripts::from_sources(
        &config,
        Rc::new(RefCell::new(scripts.world().clone())),
        scripts.help().clone(),
        Arc::new(Sources::read(&config).unwrap()),
        RuntimeMode::Checking,
    )
    .unwrap();
    checking
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    checking
        .eval_callback::<()>(
            r#"
        local ok,err=mux.error.pcall(btech.unit.reset_critical_slots,unit_id)
        assert(not ok and err.code=='mux.unavailable.checking')
    "#,
        )
        .unwrap();
}

#[tokio::test]
async fn vehicle_operations_cover_live_damage_falls_templates_and_equipment() {
    let (_dir, config, mut world, id, target, _) = firing::fixture_with_target(
        include_str!("../game/mechs/Demolisher.toml"),
        None,
        include_str!("../game/mechs/JR7-D.toml"),
    )
    .await;
    let observer = world.create(&config, "Observer".into(), Kind::Player);
    world.objects.get_mut(&observer).unwrap().location = Some(target);
    world
        .objects
        .get_mut(&observer)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(&mut world, target, observer).unwrap();
    refresh_battle_contacts(&mut world, &[target]).unwrap();
    let root = config.path(&config.database.mech_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("PARITY-GROUND.toml"),
        include_str!("fixtures/lua-probes/templates/PARITY-GROUND.toml"),
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        assert(btech.unit.armor(unit,btech.unit.sections.LEFT_SIDE).armor.current==30)
        assert(select('#',btech.unit.apply_damage(unit,{amount=1,cluster_size=1,direction_code=0}))==0)
        assert(btech.unit.armor(unit,btech.unit.sections.LEFT_SIDE).armor.current==29)
        assert(btech.unit.piloting_check(unit,{roll_modifier=2147483647,damage_modifier=0})==true)
        assert(btech.unit.piloting_check(unit,{roll_modifier=100,damage_modifier=0})==false)
        assert(select('#',btech.unit.save_template(unit,'vehicle-contract'))==0)
        assert(select('#',btech.unit.load_template(unit,'PARITY-GROUND'))==0)
        assert(btech.unit.armor(unit,btech.unit.sections.LEFT_SIDE).armor.current==24)
        assert(select('#',btech.unit.restore(unit))==0)
        assert(select('#',btech.unit.reset_critical_slots(unit))==0)
        assert(#btech.unit.weapons(unit)==0)
        assert(select('#',btech.unit.install_weapon(unit,{part='IS.MediumLaser',section=btech.unit.sections.FRONT_SIDE,slots={1}}))==0)
        assert(#btech.unit.weapons(unit)==1)
        assert(select('#',btech.unit.set_weapon_modes(unit,0,{fire_modes={btech.unit.fire_modes.HEAT}}))==0)
        assert(select('#',btech.unit.configure_ammunition(unit,{weapon='IS.SRM-4',section=btech.unit.sections.AFT_SIDE,slot=1,half_ton=true}))==0)
        assert(select('#',btech.unit.restock_ammunition(unit,btech.unit.sections.AFT_SIDE,1))==0)
        assert(select('#',btech.unit.install_special(unit,{section=btech.unit.sections.RIGHT_SIDE,slot=1}))==0)
    "#,
        )
        .unwrap();
    let messages = scripts.drain_outbox();
    assert!(
        messages.iter().any(|(_, message)| message
            .source()
            .contains("try to avoid taking personal damage")),
        "{messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|(_, message)| message.source().contains("take personal injury")),
        "{messages:?}"
    );
    assert!(root.join("vehicle-contract.toml").is_file());
}

#[tokio::test]
async fn off_map_vehicle_piloting_failure_still_applies_fall_damage() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Off-map vehicle".into(), Kind::Thing);
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
            .unwrap(),
    )
    .unwrap();
    let mut vehicle_state = serde_json::to_value(&world.btech).unwrap();
    vehicle_state["vehicles"][id.0.to_string()]["detached_heading"] = serde_json::json!(15.0);
    world.btech = serde_json::from_value(vehicle_state).unwrap();
    let pilot = world.create(&config, "Pilot".into(), Kind::Player);
    world.objects.get_mut(&pilot).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, pilot).unwrap();
    // Pin the fall dice: fresh streams are random, and occasionally route the
    // off-map fall through a critical or injury path, breaking the exact
    // eight-point full-damage contract asserted below.
    firing::edit(&mut world, id, |state| {
        state["dice"] = serde_json::to_value(BattleDice::seeded([42; 32])).unwrap();
    });
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        local armor=btech.unit.armor(unit)
        local before=armor.armor.current+armor.internal.current+armor.rear_armor.current
        assert(btech.unit.piloting_check(unit,{roll_modifier=100,damage_modifier=1})==false)
        armor=btech.unit.armor(unit)
        local after=armor.armor.current+armor.internal.current+armor.rear_armor.current
        assert(before-after==8,'off-map vehicle fall uses full damage path')
    "#,
        )
        .unwrap();
    assert!(scripts.world().btech.vehicles()[&id].pilot_injuries() <= 1);
    let heading = scripts.world().btech.vehicles()[&id].heading();
    assert_eq!(heading % 60.0, 15.0);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech.vehicles()[&id].heading(), heading);
}

#[tokio::test]
async fn editing_one_slot_preserves_unrelated_live_weapon_and_ammunition_state() {
    let (_dir, config, mut world, id, _, _) = firing::fixture_with_target(
        include_str!("../game/mechs/Demolisher.toml"),
        None,
        include_str!("../game/mechs/JR7-D.toml"),
    )
    .await;
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let vehicle = &mut state["vehicles"][id.0.to_string()];
    vehicle["ammunition"][0] = serde_json::json!(2);
    vehicle["weapon_recycle"] = serde_json::json!({"0":7});
    vehicle["jammed_weapons"] = serde_json::json!([0]);
    vehicle["weapon_failures"] = serde_json::json!({"0":"shorted"});
    world.btech = serde_json::from_value(state).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        assert(select('#',btech.unit.install_weapon(unit,{part='IS.MediumLaser',section=btech.unit.sections.LEFT_SIDE,slots={12}}))==0)
    "#,
        )
        .unwrap();
    let state = serde_json::to_value(&scripts.world().btech).unwrap();
    let vehicle = &state["vehicles"][id.0.to_string()];
    assert_eq!(vehicle["ammunition"][0], 2);
    assert_eq!(vehicle["weapon_recycle"]["1"], 7);
    assert_eq!(vehicle["jammed_weapons"], serde_json::json!([1]));
    assert_eq!(vehicle["weapon_failures"]["1"], "shorted");
}

#[tokio::test]
async fn mech_slot_edit_preserves_unrelated_live_weapon_and_ammunition_state() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Unit".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("JR7-D", include_str!("../game/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let unit = &mut state["constructed"][id.0.to_string()];
    unit["ammunition"][0] = serde_json::json!(2);
    unit["weapon_recycle"] = serde_json::json!({"0":7});
    unit["jammed_weapons"] = serde_json::json!([0]);
    unit["weapon_failures"] = serde_json::json!({"0":"shorted"});
    world.btech = serde_json::from_value(state).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        assert(select('#',btech.unit.install_weapon(unit,{part='IS.MediumLaser',section=btech.unit.sections.HEAD,slots={4}}))==0)
    "#,
        )
        .unwrap();
    let state = serde_json::to_value(&scripts.world().btech).unwrap();
    let unit = &state["constructed"][id.0.to_string()];
    assert_eq!(unit["ammunition"][0], 2);
    assert_eq!(unit["weapon_recycle"]["0"], 7);
    assert_eq!(unit["jammed_weapons"], serde_json::json!([0]));
    assert_eq!(unit["weapon_failures"]["0"], "shorted");
}

#[tokio::test]
async fn restock_allows_environmental_disable_but_rejects_destroyed_ammunition() {
    let (_dir, config, mut world, id, _, _) = firing::fixture_with_target(
        include_str!("../game/mechs/Demolisher.toml"),
        None,
        include_str!("../game/mechs/JR7-D.toml"),
    )
    .await;
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let vehicle = &mut state["vehicles"][id.0.to_string()];
    vehicle["ammunition"][0] = serde_json::json!(2);
    vehicle["breached_sections"] = serde_json::json!(["turret"]);
    world.btech = serde_json::from_value(state).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        assert(select('#',btech.unit.restock_ammunition(unit,btech.unit.sections.TURRET,3))==0)
    "#,
        )
        .unwrap();
    let mut state = serde_json::to_value(&scripts.world().btech).unwrap();
    let vehicle = &mut state["vehicles"][id.0.to_string()];
    vehicle["ammunition"][0] = serde_json::json!(2);
    vehicle["contract_loadout"] = serde_json::json!(true);
    vehicle["definition"]["sections"]["turret"]["criticals"]["2"]["modes"] =
        serde_json::json!(["Destroyed"]);
    scripts.world_mut().btech = serde_json::from_value(state).unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        local ok,err=mux.error.pcall(btech.unit.restock_ammunition,unit,btech.unit.sections.TURRET,3)
        assert(not ok and err.code=='btech.operation.failed' and err.message=='destroyed ammunition cannot be restocked')
        assert(err.detail.reason=='ammunition_destroyed')
    "#,
        )
        .unwrap();
}

#[tokio::test]
async fn signed_integer_boundaries_and_array_holes_follow_c_contracts() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Unit".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("JR7-D", include_str!("../game/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    // Pin the fall dice: fresh streams are random, and about one run in thirty
    // routes a five-point fall group into a through-armor critical (hit roll 2
    // with a d12 confirmation), breaking the exact tonnage contract below.
    firing::edit(&mut world, id, |state| {
        state["dice"] = serde_json::to_value(BattleDice::seeded([42; 32])).unwrap();
    });
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    scripts
        .eval_callback::<()>(
        r#"
        local unit=mux.world.object(unit_id)
        local armor=btech.unit.armor(unit)
        local before=armor.armor.current+armor.internal.current+armor.rear_armor.current
        btech.unit.set_tonnage(unit,95)
        assert(btech.unit.piloting_check(unit,{roll_modifier=2147483647,damage_modifier=1})==false,'max roll')
        armor=btech.unit.armor(unit)
        local after=armor.armor.current+armor.internal.current+armor.rear_armor.current
        assert(before-after==10,'off-map fall uses administrative tonnage')
        assert(btech.unit.piloting_check(unit,{roll_modifier=-2147483648,damage_modifier=0})==true,'min roll')
        local slots={[1]=1,[3]=3}
        local ok,err=mux.error.pcall(btech.unit.install_weapon,unit,{part='IS.MediumLaser',section=btech.unit.sections.LEFT_TORSO,slots=slots})
        assert(ok and err==nil,'slot raw length')
        ok,err=mux.error.pcall(btech.unit.set_weapon_modes,unit,0,{fire_modes={[1]=btech.unit.fire_modes.HEAT,[3]=btech.unit.fire_modes.ULTRA}})
        assert(ok and err==nil,'mode raw length')
    "#,
        )
        .unwrap();
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].posture(),
        BattlePosture::Prone
    );
}

#[tokio::test]
async fn weapon_install_accepts_native_slot_layouts() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Unit".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("JR7-D", include_str!("../game/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    let paired = stompymux_rs::btech::part_catalogue()
        .into_iter()
        .find(|part| {
            BattleWeapon::from_part_id(part.part_id)
                .is_some_and(|weapon| weapon.profile().critical_slots == 2)
        })
        .unwrap();
    let paired_slots = BattleWeapon::from_part_id(paired.part_id)
        .unwrap()
        .profile()
        .critical_slots;
    let partial = stompymux_rs::btech::part_catalogue()
        .into_iter()
        .find(|part| {
            BattleWeapon::from_part_id(part.part_id)
                .is_some_and(|weapon| weapon.profile().critical_slots >= 9)
        })
        .unwrap();
    let partial_slots = BattleWeapon::from_part_id(partial.part_id)
        .unwrap()
        .profile()
        .critical_slots;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("paired_id", paired.part_id)
        .unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("paired_slot_count", paired_slots)
        .unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("partial_id", partial.part_id)
        .unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("partial_slots", partial_slots)
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        btech.unit.reset_critical_slots(unit)
        btech.unit.install_weapon(unit,{part={id=paired_id},section=btech.unit.sections.LEFT_TORSO,slots={2,8}})
        local weapons=btech.unit.weapons(unit)
        assert(#weapons==1 and weapons[1].slot_count==2 and weapons[1].first_slot==2)
        btech.unit.reset_critical_slots(unit)
        local paired_slots={}
        for i=1,paired_slot_count do paired_slots[i]=i end
        btech.unit.install_weapon(unit,{part={id=paired_id},section=btech.unit.sections.LEFT_TORSO,slots=paired_slots})
        local slots=btech.unit.critical_slots(unit,btech.unit.sections.LEFT_TORSO)
        assert(slots[1].part.id==paired_id)
        btech.unit.reset_critical_slots(unit)
        local partial={id=partial_id}
        btech.unit.install_weapon(unit,{part=partial,section=btech.unit.sections.LEFT_TORSO,slots={3,10}})
        weapons=btech.unit.weapons(unit)
        assert(#weapons==1 and weapons[1].slot_count==partial_slots and weapons[1].first_slot==3)
        slots=btech.unit.critical_slots(unit,btech.unit.sections.LEFT_TORSO)
        assert(slots[3].part.id==partial_id and slots[10].part.id==partial_id)
        local ok,err=mux.error.pcall(btech.unit.install_weapon,unit,{part=115,section=btech.unit.sections.LEFT_TORSO,slots={1}})
        assert(not ok and err.code=='btech.part.not_found')
        ok,err=mux.error.pcall(btech.unit.install_weapon,unit,{part={id=115},section=btech.unit.sections.LEFT_TORSO,slots={1}})
        assert(not ok and err.code=='btech.part.not_found')
    "#,
        )
        .unwrap();
}

#[tokio::test]
async fn raw_registered_criticals_survive_restart() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Raw unit".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("JR7-D", include_str!("../game/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        btech.unit.reset_critical_slots(unit)
        local rawpart; for _,p in ipairs(btech.parts.list('weapon')) do if p.id==171 then rawpart=p break end end
        btech.unit.install_weapon(unit,{part=rawpart,section=btech.unit.sections.LEFT_TORSO,slots={1}})
        btech.unit.configure_ammunition(unit,{weapon='IS.SRM-4',section=btech.unit.sections.RIGHT_TORSO,slot=1})
        local raw=btech.unit.critical_slots(unit,btech.unit.sections.LEFT_TORSO)[1]
        assert(raw.kind=='weapon' and raw.part.id==171)
    "#,
        )
        .unwrap();

    persistence::save(&config.database(), &scripts.world())
        .await
        .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    let restarted = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
    restarted
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    restarted
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        local weapon=btech.unit.critical_slots(unit,btech.unit.sections.LEFT_TORSO)[1]
        local ammo=btech.unit.critical_slots(unit,btech.unit.sections.RIGHT_TORSO)[1]
        local special=btech.unit.critical_slots(unit,btech.unit.sections.HEAD)[1]
        assert(weapon.kind=='weapon' and weapon.part.id==171 and not weapon.temporary_failure)
        assert(ammo.kind=='ammunition' and not ammo.temporary_failure)
        assert(special.kind=='special' and not special.temporary_failure)
    "#,
        )
        .unwrap();
}

#[tokio::test]
async fn load_template_initializes_a_registered_unit_without_runtime_construction() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Deferred unit".into(), Kind::Thing);
    let mut btech = serde_json::to_value(&world.btech).unwrap();
    btech["registrations"][id.0.to_string()] = serde_json::json!("MECH");
    world.btech = serde_json::from_value(btech).unwrap();
    let root = config.path(&config.database.mech_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("PARITY.toml"),
        include_str!("fixtures/btech/mechs/PARITY.toml"),
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        local ok,err=mux.error.pcall(btech.unit.install_weapon,unit,false)
        assert(not ok and err.code=='mux.arg.invalid')
    "#,
        )
        .unwrap();
    assert!(!scripts.world().btech.constructed_units().contains_key(&id));
    scripts
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        assert(select('#',btech.unit.reset_critical_slots(unit))==0)
        assert(#btech.unit.weapons(unit)==0)
        assert(select('#',btech.unit.install_weapon(unit,{part='IS.MediumLaser',section=btech.unit.sections.LEFT_TORSO,slots={1}}))==0)
        assert(#btech.unit.weapons(unit)==1)
        assert(select('#',btech.unit.load_template(unit,'PARITY'))==0)
        assert(btech.unit.armor(unit,btech.unit.sections.LEFT_ARM).armor.current==10)
    "#,
        )
        .unwrap();
}

/// Loading and restoring accept stock templates, while a part no catalogue knows still
/// rejects the whole template.
#[tokio::test]
async fn load_template_accepts_stock_parts() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Stock unit".into(), Kind::Thing);
    let mut btech = serde_json::to_value(&world.btech).unwrap();
    btech["registrations"][id.0.to_string()] = serde_json::json!("MECH");
    world.btech = serde_json::from_value(btech).unwrap();
    let root = config.path(&config.database.mech_database);
    std::fs::create_dir_all(&root).unwrap();
    let stock = include_str!("fixtures/btech/mechs/JR7-D.toml");
    std::fs::write(root.join("JR7-D.toml"), stock).unwrap();
    let unknown = stock.replacen("IS.MediumLaser", "IS.NoSuchLaser", 1);
    assert_ne!(unknown, stock);
    std::fs::write(root.join("UNKNOWN.toml"), unknown).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        assert(select('#',btech.unit.load_template(unit,'JR7-D'))==0)
        assert(#btech.unit.weapons(unit)>0)
        assert(select('#',btech.unit.restore(unit))==0)
        local ok,err=mux.error.pcall(btech.unit.load_template,unit,'UNKNOWN')
        assert(not ok and err.code=='btech.template.invalid' and err.detail.argument==2)
    "#,
        )
        .unwrap();
    assert!(scripts.world().btech.constructed_units().contains_key(&id));
}

/// Registered raw unit on a parity template exercising argument coercion,
/// option-field edges, extra-argument tolerance, and rollback of the lazy
/// runtime materialization when a mutation rejects.
#[tokio::test]
async fn option_field_edges_extra_arguments_and_rollback_match_c_shapes() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Unit".into(), Kind::Thing);
    let mut btech = serde_json::to_value(&world.btech).unwrap();
    btech["registrations"][id.0.to_string()] = serde_json::json!("MECH");
    world.btech = serde_json::from_value(btech).unwrap();
    let root = config.path(&config.database.mech_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("PARITY.toml"),
        include_str!("fixtures/btech/mechs/PARITY.toml"),
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        local god=mux.world.object(1)
        local room=mux.world.create_object({type=mux.world.types.ROOM,name='Parity Room'})
        btech.unit.load_template(unit,'PARITY')
        local counter=0
        local function fails(fn,...)
          counter=counter+1
          local ok,e=mux.error.pcall(fn,...)
          assert(not ok,'expected failure at call '..counter)
          return e
        end
        local function check(e,code,needle,argument)
          assert(e.code==code,'call '..counter..' code '..tostring(e.code))
          if needle then assert(e.message:find(needle,1,true),'call '..counter..' message '..tostring(e.message)) end
          if argument then assert(e.detail.argument==argument,'call '..counter..' argument') end
        end
        -- Boundary values and range messages for integer option fields.
        assert(select('#',btech.unit.apply_damage(unit,{amount=1,cluster_size=1,direction_code=0,force_critical=false,unit_message=nil,map_message=nil}))==0)
        check(fails(btech.unit.apply_damage,unit,{amount=0,cluster_size=1,direction_code=0}),'mux.arg.invalid','amount must be an integer from 1 to 1000',2)
        check(fails(btech.unit.apply_damage,unit,{amount=1001,cluster_size=1,direction_code=0}),'mux.arg.invalid','amount must be an integer from 1 to 1000')
        check(fails(btech.unit.apply_damage,unit,{amount=1,cluster_size=0,direction_code=0}),'mux.arg.invalid','cluster_size must be an integer from 1 to 1000')
        check(fails(btech.unit.apply_damage,unit,{amount=1,cluster_size=1,direction_code=22}),'mux.arg.invalid','direction_code must be an integer from 0 to 21')
        check(fails(btech.unit.apply_damage,unit,{amount=-1,cluster_size=1,direction_code=0}),'mux.arg.invalid','amount must be an integer from 1 to 1000')
        check(fails(btech.unit.apply_damage,unit,{amount=2.5,cluster_size=1,direction_code=0}),'mux.arg.invalid','amount must be an integer from 1 to 1000')
        check(fails(btech.unit.apply_damage,unit,{amount=1/0,cluster_size=1,direction_code=0}),'mux.arg.invalid','amount must be an integer from 1 to 1000')
        check(fails(btech.unit.apply_damage,unit,{amount=2.0,cluster_size=1,direction_code=0,extra=false}),'mux.arg.invalid',"unknown field 'extra'")
        -- Optional fields: omitted, explicit nil, and false behave identically.
        assert(select('#',btech.unit.apply_damage(unit,{amount=1,cluster_size=1,direction_code=0,force_critical=false}))==0)
        -- Extra positional arguments are ignored: the handlers read fixed slots.
        assert(select('#',btech.unit.load_template(unit,'PARITY','extra','more'))==0,'load extras')
        assert(select('#',btech.unit.configure_ammunition(unit,{weapon='IS.SRM-4',section=btech.unit.sections.RIGHT_TORSO,slot=1},'extra'))==0,'configure extras')
        assert(select('#',btech.unit.restock_ammunition(unit,btech.unit.sections.RIGHT_TORSO,1,'extra'))==0,'restock extras')
        assert(select('#',btech.unit.set_weapon_modes(unit,0,{},'extra'))==0)
        -- Wrong-kind handles fail on argument one before request validation.
        check(fails(btech.unit.apply_damage,god,{amount=1,cluster_size=1,direction_code=0}),'mux.object.invalid','object is not a registered BTech unit',1)
        check(fails(btech.unit.restore,room),'mux.object.invalid',nil,1)
        check(fails(btech.unit.install_weapon,42,false),'mux.object.invalid')
        -- Slot-value type and range shapes follow require_integer_at.
        check(fails(btech.unit.install_weapon,unit,{part='IS.PPC',section=btech.unit.sections.LEFT_TORSO,slots={'2',3,4}}),'mux.arg.invalid','slot must be an integer')
        check(fails(btech.unit.install_weapon,unit,{part='IS.PPC',section=btech.unit.sections.LEFT_TORSO,slots={1.5,3,4}}),'mux.arg.invalid','slot is outside its valid range')
        check(fails(btech.unit.restock_ammunition,unit,btech.unit.sections.HEAD,0),'mux.arg.invalid','slot is outside its valid range',3)
        check(fails(btech.unit.set_weapon_modes,unit,-1,{}),'mux.arg.invalid','weapon_number is outside its valid range',2)
        check(fails(btech.unit.set_weapon_modes,unit,1.5,{}),'mux.arg.invalid','weapon_number is outside its valid range')
        check(fails(btech.unit.install_special,unit,{section=btech.unit.sections.HEAD,slot=1,auxiliary_data='x'}),'mux.arg.invalid','auxiliary_data must be an integer')
        check(fails(btech.unit.install_special,unit,{section=btech.unit.sections.HEAD,slot=1,auxiliary_data=1.5}),'mux.arg.invalid','auxiliary_data is outside its valid range')
        -- Piloting modifiers accept the full signed range and reject fractions.
        check(fails(btech.unit.piloting_check,unit,{roll_modifier=0.5,damage_modifier=0}),'mux.arg.invalid','roll_modifier must be an integer from -2147483648 to 2147483647')
        check(fails(btech.unit.piloting_check,unit,{roll_modifier=1/0,damage_modifier=0}),'mux.arg.invalid','roll_modifier must be an integer from -2147483648 to 2147483647')
        check(fails(btech.unit.piloting_check,unit,{roll_modifier=0,damage_modifier='x'}),'mux.arg.invalid','damage_modifier must be an integer')
    "#,
        )
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        assert(select('#',btech.unit.load_template(unit,'PARITY'))==0)
    "#,
        )
        .unwrap();
    assert!(scripts.world().btech.constructed_units().contains_key(&id));
}

/// An uncaught rejection rolls the whole callback transaction back, so the
/// lazy raw-unit materialization a mutation triggered does not survive it.
#[tokio::test]
async fn uncaught_rejection_rolls_back_lazy_materialization() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Raw unit".into(), Kind::Thing);
    let mut btech = serde_json::to_value(&world.btech).unwrap();
    btech["registrations"][id.0.to_string()] = serde_json::json!("MECH");
    world.btech = serde_json::from_value(btech).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    let rejected = scripts.eval_callback::<()>(
        r#"
        local unit=mux.world.object(unit_id)
        btech.unit.set_weapon_modes(unit,9,{fire_modes={btech.unit.fire_modes.HEAT}})
    "#,
    );
    assert!(rejected.is_err(), "unmounted weapon rejection");
    assert!(
        !scripts.world().btech.constructed_units().contains_key(&id),
        "rejected mutation rolls back lazy materialization"
    );
}

/// Every btech.unit operation rejects checking mode before any validation.
#[tokio::test(flavor = "current_thread")]
async fn every_unit_operation_is_unavailable_while_checking() {
    use std::sync::Arc;
    use stompymux_rs::lua::{RuntimeMode, sources::Sources};
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Unit".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let checking = Scripts::from_sources(
        &config,
        Rc::new(RefCell::new(scripts.world().clone())),
        scripts.help().clone(),
        Arc::new(Sources::read(&config).unwrap()),
        RuntimeMode::Checking,
    )
    .unwrap();
    checking
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    checking
        .eval_callback::<()>(
            r#"
        local section=btech.unit.sections.HEAD
        local operations={
          {btech.unit.load_template,unit,'PARITY'},
          {btech.unit.save_template,unit,'x'},
          {btech.unit.restore,unit},
          {btech.unit.reset_critical_slots,unit},
          {btech.unit.apply_damage,unit,{amount=1,cluster_size=1,direction_code=0}},
          {btech.unit.piloting_check,unit,{roll_modifier=0,damage_modifier=0}},
          {btech.unit.install_weapon,unit,{part='IS.MediumLaser',section=section,slots={1}}},
          {btech.unit.configure_ammunition,unit,{weapon='IS.SRM-4',section=section,slot=1}},
          {btech.unit.restock_ammunition,unit,section,1},
          {btech.unit.set_weapon_modes,unit,0,{}},
          {btech.unit.install_special,unit,{section=section,slot=1}},
        }
        for index,call in ipairs(operations) do
          local ok,err=mux.error.pcall(call[1],call[2],call[3],call[4])
          assert(not ok,index)
          assert(err.code=='mux.unavailable.checking',index)
        end
    "#,
        )
        .unwrap();
}

/// The deterministic apply_damage direction codes pin exact sections and the
/// reverse arc damages rear armor before the front.
#[tokio::test]
async fn deterministic_direction_codes_pin_sections_and_rear_arc() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Unit".into(), Kind::Thing);
    let mut btech = serde_json::to_value(&world.btech).unwrap();
    btech["registrations"][id.0.to_string()] = serde_json::json!("MECH");
    world.btech = serde_json::from_value(btech).unwrap();
    let root = config.path(&config.database.mech_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("PARITY.toml"),
        include_str!("fixtures/btech/mechs/PARITY.toml"),
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("unit_id", id.0)
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
        local unit=mux.world.object(unit_id)
        local sections=btech.unit.sections
        local function armor_of(section)
          local armor=btech.unit.armor(unit,section)
          return armor.armor.current,armor.rear_armor.current
        end
        btech.unit.load_template(unit,'PARITY')
        -- Direction codes below eight select the section by storage order.
        btech.unit.apply_damage(unit,{amount=3,cluster_size=3,direction_code=0})
        local front=armor_of(sections.LEFT_ARM)
        assert(front==7,'LA front '..tostring(front))
        btech.unit.apply_damage(unit,{amount=2,cluster_size=1,direction_code=4})
        front=armor_of(sections.CENTER_TORSO)
        assert(front==18,'CT front '..tostring(front))
        -- Codes eight through fifteen repeat the arc with rear armor first.
        btech.unit.apply_damage(unit,{amount=2,cluster_size=1,direction_code=12})
        local front,rear=armor_of(sections.CENTER_TORSO)
        assert(front==18 and rear==3,'CT rear pass 1: '..tostring(front)..','..tostring(rear))
        btech.unit.apply_damage(unit,{amount=4,cluster_size=1,direction_code=12})
        front,rear=armor_of(sections.CENTER_TORSO)
        assert(front==18 and rear==0,'CT rear pass 2: '..tostring(front)..','..tostring(rear))
    "#,
        )
        .unwrap();
}
