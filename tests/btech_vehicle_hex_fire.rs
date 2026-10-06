//! Vehicle coordinate shots reuse launch, packet, terrain and host publication rules.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// A running turreted gun platform faces an empty forest cell.
async fn fixture(
    mode: HexTargetMode,
    terrain: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    fixture_with_template(mode, terrain, include_str!("../game/units/Demolisher.toml")).await
}

/// Supply a weapon variant while retaining the same coordinate setup.
async fn fixture_with_template(
    mode: HexTargetMode,
    terrain: &str,
    template: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Terrain field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        MapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let shooter = world.create(&config, "Demolisher".into(), Kind::Thing);
    world.objects.get_mut(&shooter).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        shooter,
        VehicleTemplate::parse("test", template).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, shooter, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, shooter, map, 1, 1).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
    assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, shooter, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    let index = world.btech.vehicles()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|m| matches!(m.weapon, Weapon::Ac20 | Weapon::Ac2 | Weapon::MachineGun))
        .unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    crate::support::set_hex_terrain(
        &mut saved["maps"][map.0.to_string()]["terrain"][1],
        Terrain::from_name(terrain).unwrap(),
    );
    saved["vehicles"][shooter.0.to_string()]["dice"] =
        serde_json::to_value(Dice::seeded([42; 32])).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
    select_battle_hex_target(
        &mut world,
        shooter,
        ObjectId(1),
        HexCoordinate { x: 1, y: 0 },
        mode,
    )
    .unwrap();
    (dir, config, world, shooter, map, index)
}

#[tokio::test]
async fn vehicle_coordinate_modes_share_native_lua_expenditure_and_restart() {
    for mode in [
        HexTargetMode::UnitAtHex,
        HexTargetMode::Hex,
        HexTargetMode::Clear,
        HexTargetMode::Ignite,
        HexTargetMode::Building,
    ] {
        let (_dir, config, world, shooter, _map, index) = fixture(mode, "heavy_woods").await;
        persistence::save(&config.database(), &world).await.unwrap();
        let original = world.btech.vehicles()[&shooter].clone();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let replay = Scripts::new(
            &config,
            Rc::new(RefCell::new(
                persistence::load(&config.database()).await.unwrap(),
            )),
        )
        .unwrap();
        let code = format!(
            "local r=btech.unit.fire({},1,{index}); return r.launched,r.coordinate.x,r.coordinate.y,#r.terrain,r.recoil==nil",
            shooter.0
        );
        let report: (bool, i32, i32, usize, bool) = lua.eval_callback(&code).unwrap();
        assert!(report.0);
        assert_eq!((report.1, report.2, report.4), (1, 0, true));
        if mode == HexTargetMode::UnitAtHex {
            assert_eq!(report.3, 0);
        } else if mode != HexTargetMode::Building {
            assert!(report.3 > 0);
        }
        let again: (bool, i32, i32, usize, bool) = replay.eval_callback(&code).unwrap();
        assert_eq!(report, again);
        let text = support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
        assert!(text.contains("You fire AC/20 at (1,0)"), "{text}");
        assert_eq!(lua.world().btech, native.world().btech);
        assert_eq!(lua.world().btech, replay.world().btech);
        let fired = lua.world().clone();
        let vehicle = &fired.btech.vehicles()[&shooter];
        assert_eq!(
            original.ammunition().iter().sum::<u16>() - vehicle.ammunition().iter().sum::<u16>(),
            1
        );
        assert_eq!(
            vehicle.weapon_heat(),
            f64::from(Weapon::Ac20.profile().heat)
        );
        assert!(vehicle.weapon_recycle()[&index] > 0);
        assert!(
            lua.eval_callback::<()>(&format!("btech.unit.fire({},1,{index})", shooter.0))
                .is_err()
        );
        assert_eq!(lua.world().btech, fired.btech);
        fired.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn vehicle_hex_fire_rollback_restores_inventory_terrain_and_dice() {
    let (_dir, config, world, shooter, _map, index) =
        fixture(HexTargetMode::Clear, "heavy_woods").await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.fire({},1,{index}); error('abort')",
                shooter.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.fire({},2,{index})", shooter.0))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
}

#[tokio::test]
async fn vehicle_surface_shots_use_shooter_dice_and_shared_fracture() {
    let (_dir, config, world, shooter, map, index) = fixture(HexTargetMode::Hex, "ice").await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report:(bool,usize,bool)=scripts.eval_callback(&format!("local r=btech.unit.fire({},1,{index}); return r.hit,#r.surfaces,r.surfaces[1]~=nil and r.surfaces[1].fracture~=nil",shooter.0)).unwrap();
    assert_eq!(report, (true, 1, true));
    assert_eq!(
        scripts.world().btech.maps()[&map]
            .base_hex(1, 0)
            .unwrap()
            .terrain(),
        Terrain::Water
    );
    scripts.world().validate(&config).unwrap();
}

#[tokio::test]
async fn vehicle_coordinate_misload_is_tagged_and_rolls_back_with_terrain_action() {
    let template = include_str!("../game/units/Demolisher.toml")
        .replace("IS.AC/20", "IS.AC/2")
        .replace("\"IS.AC/2\" }", "\"IS.AC/2\", modes = [\"RapidFire\"] }");
    let (_dir, config, mut world, shooter, _map, index) =
        fixture_with_template(HexTargetMode::Hex, "heavy_woods", &template).await;
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    world
        .btech
        .set_unit_dice(shooter, Dice::seeded([seed; 32]))
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let replay = Scripts::new(
        &config,
        Rc::new(RefCell::new(
            persistence::load(&config.database()).await.unwrap(),
        )),
    )
    .unwrap();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.fire({},1,{index}); error('abort')",
            shooter.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    let code = format!(
        "local r=btech.unit.fire({},1,{index}); return r.loader_destroyed,r.misload.kind,#r.terrain",
        shooter.0
    );
    let report: (bool, String, usize) = lua.eval_callback(&code).unwrap();
    assert_eq!(report, (true, "vehicle".into(), 0));
    assert_eq!(
        report,
        replay
            .eval_callback::<(bool, String, usize)>(&code)
            .unwrap()
    );
    assert_eq!(lua.world().btech, replay.world().btech);
}

/// Replace the fixture's wizard with an ordinary character crew.
fn character_crew(world: &mut World, shooter: ObjectId) {
    release_battle_pilot(world, shooter, ObjectId(1)).unwrap();
    world
        .objects
        .get_mut(&shooter)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let pilot = world.objects.get_mut(&ObjectId(2)).unwrap();
    pilot.flags.remove(Flag::Wizard);
    pilot.location = Some(shooter);
    set_battle_character(
        &mut *world,
        ObjectId(2),
        Character {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    assign_battle_pilot(world, shooter, ObjectId(2)).unwrap();
    support::seed_object_dice(world, ObjectId(2), support::FIXTURE_DICE_SEED);
}

#[tokio::test]
async fn character_vehicle_terrain_fire_shares_commands_replay_and_rollback() {
    for template in [
        include_str!("../game/units/Demolisher.toml"),
        include_str!("../game/units/Kestrel.toml"),
    ] {
        for mode in [
            HexTargetMode::UnitAtHex,
            HexTargetMode::Hex,
            HexTargetMode::Clear,
            HexTargetMode::Ignite,
            HexTargetMode::Building,
        ] {
            let (_dir, config, mut world, shooter, _, index) =
                fixture_with_template(mode, "heavy_woods", template).await;
            character_crew(&mut world, shooter);
            if world.btech.vehicles()[&shooter].definition().is_vtol() {
                world
                    .btech
                    .rewrite_unit_record(shooter, |record| {
                        record["vtol_flight"]["phase"] =
                            serde_json::to_value(VtolFlightPhase::Airborne).unwrap();
                        record["vtol_flight"]["altitude"] = serde_json::json!(1.5);
                    })
                    .unwrap();
            }
            persistence::save(&config.database(), &world).await.unwrap();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let replay = Scripts::new(
                &config,
                Rc::new(RefCell::new(
                    persistence::load(&config.database()).await.unwrap(),
                )),
            )
            .unwrap();
            let call = format!("btech.unit.fire({},2,{index})", shooter.0);
            assert!(
                scripts
                    .eval_callback::<()>(&format!("{call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, world.btech);
            assert!(scripts.drain_outbox().is_empty());
            let code = format!(
                "local r={call}; return r.launched,r.coordinate.x,r.coordinate.y,#r.terrain"
            );
            let report: (bool, i32, i32, usize) = scripts.eval_callback(&code).unwrap();
            assert!(report.0);
            assert_eq!((report.1, report.2), (1, 0));
            assert_eq!(
                report,
                replay
                    .eval_callback::<(bool, i32, i32, usize)>(&code)
                    .unwrap()
            );
            let text =
                support::run_text(&native, &config, ObjectId(2), 1, &format!("fire {index}"));
            assert!(
                text.contains("You fire ") && text.contains(" at (1,0)"),
                "{text}"
            );
            assert_eq!(scripts.world().btech, native.world().btech);
            assert_eq!(scripts.world().btech, replay.world().btech);
            assert_eq!(scripts.world().btech.characters(), world.btech.characters());
            assert_eq!(
                scripts.world().objects[&ObjectId(2)].location,
                Some(shooter)
            );
            scripts.world().validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn character_coordinate_misload_publishes_injuries_and_rolls_back_failed_evacuation() {
    let template = include_str!("../game/units/Demolisher.toml")
        .replace("IS.AC/20", "IS.AC/2")
        .replace("\"IS.AC/2\" }", "\"IS.AC/2\", modes = [\"RapidFire\"] }");
    for fatal in [false, true] {
        let (dir, _config, mut world, shooter, _, index) =
            fixture_with_template(HexTargetMode::Hex, "heavy_woods", &template).await;
        let path = dir.path().join("stompymux.toml");
        let mut settings: toml::Value =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for (key, value) in [("vcrit", 1), ("fasaadvvhlcrit", 0)] {
            settings["battletech"]
                .as_table_mut()
                .unwrap()
                .insert(key.into(), value.into());
        }
        std::fs::write(path, toml::to_string(&settings).unwrap()).unwrap();
        let config = Config::load(dir.path()).unwrap();
        character_crew(&mut world, shooter);
        let seed = (0_u32..100_000)
            .find_map(|number| {
                let mut seed = [0; 32];
                seed[..4].copy_from_slice(&number.to_le_bytes());
                let mut dice = Dice::seeded(seed);
                let attack = dice.two_d6();
                dice.two_d6();
                let count = dice.two_d6();
                let preliminary = dice.die(3).unwrap();
                let effect = dice.d6();
                (preliminary != 2
                    && attack == 2
                    && matches!(count, 8 | 9)
                    && effect == if fatal { 4 } else { 1 })
                .then_some(seed)
            })
            .unwrap();
        world
            .btech
            .set_unit_dice(shooter, Dice::seeded(seed))
            .unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let replay = Scripts::new(
            &config,
            Rc::new(RefCell::new(
                persistence::load(&config.database()).await.unwrap(),
            )),
        )
        .unwrap();
        let code = format!(
            "local r=btech.unit.fire({},2,{index}); return r.loader_destroyed,r.misload.kind,#r.terrain",
            shooter.0
        );
        if fatal {
            let afterlife = ObjectId(config.battletech.afterlife_dbref);
            let room = scripts.world_mut().objects.remove(&afterlife).unwrap();
            let before = scripts.world().clone();
            assert!(
                scripts
                    .eval_callback::<(bool, String, usize)>(&code)
                    .is_err()
            );
            assert_eq!(scripts.world().btech, before.btech);
            assert_eq!(
                scripts.world().objects[&ObjectId(2)].location,
                Some(shooter)
            );
            assert!(scripts.drain_outbox().is_empty());
            scripts
                .world_mut()
                .objects
                .insert(afterlife, std::sync::Arc::unwrap_or_clone(room));
        }
        let report: (bool, String, usize) = scripts.eval_callback(&code).unwrap();
        assert_eq!(report, (true, "vehicle".into(), 0));
        assert_eq!(
            report,
            replay
                .eval_callback::<(bool, String, usize)>(&code)
                .unwrap()
        );
        assert_eq!(scripts.world().btech, replay.world().btech);
        assert_eq!(
            scripts.world().btech.vehicles()[&shooter].crew_killed(),
            fatal
        );
        assert_eq!(
            scripts.world().objects[&ObjectId(2)].location,
            Some(if fatal {
                ObjectId(config.battletech.afterlife_dbref)
            } else {
                shooter
            })
        );
        if !fatal {
            assert_ne!(
                scripts.world().btech.characters()[&ObjectId(2)],
                world.btech.characters()[&ObjectId(2)]
            );
        }
        scripts.world().validate(&config).unwrap();
    }
}
