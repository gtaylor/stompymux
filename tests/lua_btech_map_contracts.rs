//! Source-backed C contract coverage for the canonical `btech.map` package.

use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

#[tokio::test]
async fn terrain_zones_cargo_links_and_strict_errors_match_c_shapes() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Map".into(), Kind::Room);
    let parent = world.create(&config, "Parent".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "map",
        BattleMapAsset::parse("3 2\n.0~1^2\n@3#4+5\n").unwrap(),
    )
    .unwrap();
    create_battle_map(
        &mut world,
        parent,
        "parent",
        BattleMapAsset::parse("2 2\n.0.0\n.0.0\n").unwrap(),
    )
    .unwrap();
    set_battle_landing_exclusion(
        &mut world,
        map,
        0,
        Some(BattleLandingExclusion {
            coordinate: BattleHexCoordinate { x: 1, y: 0 },
            radius: 2,
            exempt_team: 0,
            owner: ObjectId(1),
            data_short: 2,
        }),
    )
    .unwrap();
    let root = config.path(&config.database.map_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("contract.map"), "2 2\n.0.0\n.0.0\n").unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("map_id", map.0)
        .unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("parent_id", parent.0)
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
            local assertion=assert; local step=0; function assert(value,message) step=step+1; return assertion(value,message or ('step '..step)) end
            assert(btech.map.elevation(map_id,{x=1,y=0})==1)
            assert(btech.map.elevation(tostring(map_id)..'.9',{x=1,y=0})==1)
            for _,number in ipairs({0/0,1/0,-1/0,2147483648}) do
                local numeric_ok,numeric_err=mux.error.pcall(btech.map.elevation,number,{x=0,y=0})
                assert(not numeric_ok and numeric_err.code=='mux.object.invalid' and numeric_err.message=='bad argument #1 to \'?\' (object is invalid)' and numeric_err.detail.argument==1)
            end
            assert(btech.map.terrain(map_id,{x=1,y=0})=='water')
            assert(btech.map.range(map_id,{x=0,y=0},{x=1,y=0})==1.0198038816452026)
            assert(btech.map.range(map_id,{x=0,y=0,z=0},{x=0,y=0,z=5})==1.0)
            assert(btech.map.range(map_id,{x=0,y=0,z=2},{x=2,y=1,z=7})==2.2360680103302)
            assert(btech.map.elevation(map_id,{x=1,y=0},'extra')==1)
            local zones=btech.map.blast_zones(map_id)
            assert(#zones==1 and zones[1].x==1 and zones[1].y==0 and zones[1].radius==2)
            assert(btech.map.in_blast_zone(map_id,{x=0,y=0}))
            assert(select('#',btech.map.set_cargo_transfer_point(map_id,{x=2,y=1,reveal_hint=true}))==0)
            local cargo=btech.map.cargo_transfer_point(map_id)
            assert(cargo.x==2 and cargo.y==1 and cargo.reveal_hint)
            assert(select('#',btech.map.set_link(map_id,{parent=mux.world.object(parent_id),x=1,y=1,entrances={north={mode='offset',offset=0},east={mode='exact',x=2,y=1}}}))==0)
            local link=btech.map.link(map_id)
            assert(link.parent==mux.world.object(parent_id) and link.x==1 and link.y==1)
            assert(link.entrances.north.mode=='offset' and link.entrances.north.offset==0)
            assert(link.entrances.east.mode=='exact' and link.entrances.east.x==2 and link.entrances.east.y==1)
            assert(type(btech.map.authored_link)=='function' and type(btech.map.set_authored_link)=='function')
            assert(type(btech.map.load_as)=='function' and type(btech.map.emit_as)=='function' and type(btech.map.update_links_as)=='function')
            local ok,err=mux.error.pcall(function() btech.map.elevation(map_id,{x=3,y=0}) end)
            assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==2)
            ok,err=mux.error.pcall(function() btech.map.set_cargo_transfer_point(map_id,{x=0,y=0,reveal_hint=1}) end)
            assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==2)
            ok,err=mux.error.pcall(function() btech.map.set_link(map_id,{parent=map_id,x=0,y=0}) end)
            assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==2)
            assert(select('#',btech.map.set_link(map_id,nil))==0 and btech.map.link(map_id)==nil)
            assert(select('#',btech.map.set_cargo_transfer_point(map_id,nil))==0 and btech.map.cargo_transfer_point(map_id)==nil)
            for _,case in ipairs({
                function() btech.map.elevation() end,
                function() btech.map.terrain(map_id,false) end,
                function() btech.map.units(map_id,false) end,
                function() btech.map.range(map_id,{x=0,y=0}) end,
                function() btech.map.place_unit(nil,map_id,{x=0,y=0}) end,
                function() btech.map.emit(map_id,'x',false) end,
                function() btech.map.set_link(map_id) end,
            }) do
                local accepted,failure=mux.error.pcall(case)
                assert(not accepted and (failure.code=='mux.arg.invalid' or failure.code=='mux.object.invalid'))
            end
        "#,
        )
        .unwrap();
    scripts
        .eval_callback::<()>(&format!(
            "assert(select('#',btech.map.update_links({}))==0)",
            map.0
        ))
        .unwrap();
    assert!(scripts.drain_outbox().is_empty());
    scripts.eval_callback::<()>(&format!("assert(select('#',btech.map.load({},'contract.map'))==0);assert(btech.map.elevation({},{{x=1,y=1}})==0)", map.0, map.0)).unwrap();
    let load_output = format!("{:?}", scripts.drain_outbox());
    assert!(load_output.contains("Map Cleared"), "{load_output}");
    scripts.eval_callback::<()>(&format!("local ok,err=mux.error.pcall(btech.map.load,{},'missing.map');assert(not ok and err.code=='btech.operation.failed' and err.detail.reason=='map_file_invalid')", map.0)).unwrap();
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech.maps()[&map].name, "contract.map");
    assert_eq!(
        (
            restored.btech.maps()[&map].width,
            restored.btech.maps()[&map].height
        ),
        (2, 2)
    );
}

#[tokio::test]
async fn mixed_membership_range_lookup_los_and_exact_placement_are_canonical() {
    let (_dir, config, mut world, vehicle, mech, _) = firing::fixture_with_target(
        include_str!("../game/mechs/Demolisher"),
        None,
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    let map = world.btech.vehicles()[&vehicle].position().unwrap().map;
    let second_pilot = world.create(&config, "Second pilot".into(), Kind::Player);
    world.objects.get_mut(&second_pilot).unwrap().location = Some(mech);
    assign_battle_pilot(&mut world, mech, second_pilot).unwrap();
    let map_root = config.path(&config.database.map_database);
    std::fs::create_dir_all(&map_root).unwrap();
    std::fs::write(
        map_root.join("occupied.map"),
        format!("1 12\n{}", ".0\n".repeat(12)),
    )
    .unwrap();
    let vehicle_label = world.btech.vehicles()[&vehicle].battlefield_id().unwrap();
    let mech_label = world.btech.constructed_units()[&mech]
        .battlefield_id()
        .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (name, value) in [
        ("map_id", map.0),
        ("vehicle_id", vehicle.0),
        ("mech_id", mech.0),
    ] {
        scripts.inspect_lua().globals().set(name, value).unwrap();
    }
    scripts
        .inspect_lua()
        .globals()
        .set("vehicle_label", vehicle_label)
        .unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("mech_label", mech_label)
        .unwrap();
    scripts
        .eval_callback::<()>(
            r#"
            local members=btech.map.units(map_id)
            assert(#members==2 and members[1]==mux.world.object(vehicle_id) and members[2]==mux.world.object(mech_id))
            assert(btech.map.unit_by_id(map_id,vehicle_label)==members[1])
            assert(btech.map.unit_by_id(members[1],mech_label)==members[2])
            assert(btech.map.unit_by_id(map_id,'ZZ')==nil)
            local nearby=btech.map.units(map_id,{origin={x=0,y=11},range=0})
            assert(#nearby==1 and nearby[1]==members[1])
            assert(math.abs(btech.map.range(map_id,members[1],members[2])-1)<1e-6)
            assert(btech.map.line_of_sight(members[1],members[2])=='clear')
            assert(btech.map.line_of_sight(members[1],{x=0,y=10})=='clear')
            assert(select('#',btech.map.place_unit(members[2],map_id,{x=0,y=9,z=3}))==0)
            local moved=btech.map.units(map_id,{origin={x=0,y=9},range=0})
            assert(#moved==1 and moved[1]==members[2])
            local ok,err=mux.error.pcall(function() btech.map.units(map_id,{origin={x=0,y=0}}) end)
            assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==2)
            ok,err=mux.error.pcall(function() btech.map.place_unit(members[2],map_id,{x=0,y=9,z=3.5}) end)
            assert(not ok and err.code=='mux.arg.invalid' and err.detail.argument==3)
        "#,
        )
        .unwrap();
    set_battle_map_hex_action(
        &scripts,
        &config,
        ObjectId(1),
        map,
        BattleHexCoordinate { x: 0, y: 10 },
        Terrain::Mountains,
        9,
    )
    .unwrap();
    scripts.drain_outbox();
    scripts
        .eval_callback::<()>(&format!(
            "assert(btech.map.line_of_sight(mux.world.object({}),{{x=0,y=9}})=='blocked')",
            vehicle.0
        ))
        .unwrap();
    set_battle_visibility(
        &mut scripts.world_mut(),
        mech,
        BattleVisibility {
            invisible: true,
            clairvoyant: false,
        },
    )
    .unwrap();
    scripts
        .eval_callback::<()>(&format!(
            "assert(btech.map.line_of_sight(mux.world.object({}),mux.world.object({}))=='none')",
            vehicle.0, mech.0
        ))
        .unwrap();
    scripts
        .eval_callback::<()>(&format!(
            "assert(select('#',btech.map.emit({},'ALL_MESSAGE'))==0)",
            map.0
        ))
        .unwrap();
    let all_output = format!("{:?}", scripts.drain_outbox());
    assert_eq!(all_output.matches("ALL_MESSAGE").count(), 2, "{all_output}");
    assert!(!all_output.contains("Message sent!"));
    scripts.eval_callback::<()>(&format!(
        "assert(select('#',btech.map.emit({},'RANGE_MESSAGE',{{audience='range',origin={{x=0,y=11}},range=0}}))==0)", map.0
    )).unwrap();
    let range_output = format!("{:?}", scripts.drain_outbox());
    assert_eq!(
        range_output.matches("RANGE_MESSAGE").count(),
        1,
        "{range_output}"
    );
    scripts.eval_callback::<()>(&format!(
        "assert(select('#',btech.map.emit({},'Danger at $H',{{audience='line_of_sight',origin={{x=0,y=11}}}}))==0)", map.0
    )).unwrap();
    let output = format!("{:?}", scripts.drain_outbox());
    assert!(output.contains("YOUR HEX"), "{output}");
    scripts
        .eval_callback::<()>(&format!(
            "assert(select('#',btech.map.load({},'occupied.map'))==0)",
            map.0
        ))
        .unwrap();
    assert!(
        battle_map_unit_order(&scripts.world(), map)
            .unwrap()
            .is_empty()
    );
    let load_output = format!("{:?}", scripts.drain_outbox());
    assert!(load_output.contains("Map Cleared"), "{load_output}");
}

#[tokio::test(flavor = "current_thread")]
async fn going_handles_and_checking_mode_preserve_object_and_mutation_boundaries() {
    use std::sync::Arc;
    use stompymux_rs::lua::{RuntimeMode, sources::Sources};

    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "map",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set(
            "saved_map",
            scripts
                .inspect_lua()
                .load(format!("return mux.world.object({})", map.0))
                .eval::<mlua::Value>()
                .unwrap(),
        )
        .unwrap();
    scripts
        .world_mut()
        .objects
        .get_mut(&map)
        .unwrap()
        .flags
        .insert(Flag::Going);
    scripts
        .eval_callback::<()>(
            r#"
        local ok,err=mux.error.pcall(btech.map.elevation,saved_map,{x=0,y=0})
        assert(not ok and err.code=='mux.object.unavailable' and err.detail.argument==1)
    "#,
        )
        .unwrap();
    scripts
        .world_mut()
        .objects
        .get_mut(&map)
        .unwrap()
        .flags
        .remove(Flag::Going);
    scripts
        .world_mut()
        .objects
        .get_mut(&map)
        .unwrap()
        .generation = Default::default();
    scripts.eval_callback::<()>(r#"
        local ok,err=mux.error.pcall(btech.map.elevation,saved_map,{x=0,y=0})
        assert(not ok and err.code=='mux.object.invalid' and err.message=='bad argument #1 to \'?\' (object no longer exists)' and err.detail.argument==1)
    "#).unwrap();

    let sources = Arc::new(Sources::read(&config).unwrap());
    let checking = Scripts::from_sources(
        &config,
        Rc::new(RefCell::new(scripts.world().clone())),
        scripts.help().clone(),
        sources,
        RuntimeMode::Checking,
    )
    .unwrap();
    checking
        .inspect_lua()
        .globals()
        .set("map_id", map.0)
        .unwrap();
    checking
        .eval_callback::<()>(
            r#"
        local ok,err=mux.error.pcall(btech.map.elevation,map_id,{x=0,y=0})
        assert(not ok and err.code=='mux.unavailable.checking')
        ok,err=mux.error.pcall(btech.map.set_cargo_transfer_point,map_id,nil)
        assert(not ok and err.code=='mux.unavailable.checking')
    "#,
        )
        .unwrap();
}
