//! Vehicle Artemis controller geometry, shared cockpit admission and durable selections.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Ground movement classes and rotorcraft, including stationary rotorcraft anatomy.
fn templates() -> Vec<String> {
    let ground = include_str!("../game/mechs/Demolisher.toml");
    let vtol = include_str!("../game/mechs/Kestrel.toml");
    vec![
        ground.into(),
        ground.replace("movement = \"track\"", "movement = \"wheel\""),
        ground.replace("movement = \"track\"", "movement = \"hover\""),
        ground
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 5", "walk_mp = 0"),
        vtol.into(),
        vtol.replace("movement = \"vtol\"", "movement = \"none\"")
            .replace("walk_mp = 18", "walk_mp = 0"),
    ]
}

/// Install one turret launcher and a controller without changing chassis identity.
fn definition(source: &str, section: BattleVehicleSection, link: &str) -> BattleVehicleTemplate {
    let mut template = BattleVehicleTemplate::parse("test", source).unwrap();
    for section in template.sections.values_mut() {
        section.criticals.clear();
    }
    let mut turret = template.sections[&BattleVehicleSection::Front].clone();
    turret.criticals.insert(
        0,
        CriticalDefinition {
            equipment: "IS.LRM-5".into(),
            data: "-".into(),
            modes: vec![],
        },
    );
    template
        .sections
        .insert(BattleVehicleSection::Turret, turret);
    template
        .sections
        .get_mut(&section)
        .unwrap()
        .criticals
        .insert(
            1,
            CriticalDefinition {
                equipment: "ArtemisIV".into(),
                data: link.into(),
                modes: vec![],
            },
        );
    for (slot, modes) in [(2, vec![]), (3, vec!["Artemis/Mine".into()])] {
        template
            .sections
            .get_mut(&BattleVehicleSection::Left)
            .unwrap()
            .criticals
            .insert(
                slot,
                CriticalDefinition {
                    equipment: "Ammo_IS.LRM-5".into(),
                    data: "24".into(),
                    modes,
                },
            );
    }
    template
}

/// Local links work on all classes; rear-to-turret routing belongs only to ground vehicles.
#[test]
fn vehicle_artemis_links_loss_and_roundtrip() {
    for source in templates() {
        for section in [
            BattleVehicleSection::Turret,
            BattleVehicleSection::Rear,
            BattleVehicleSection::Front,
        ] {
            let unit = BattleVehicle::new(definition(&source, section, "1")).unwrap();
            let linked = section == BattleVehicleSection::Turret
                || (section == BattleVehicleSection::Rear && !unit.definition().is_vtol());
            assert_eq!(
                unit.artemis_operational(0).unwrap(),
                linked,
                "{section:?} {:?}",
                unit.definition().movement
            );
            assert!(unit.artemis_operational(1).is_err());
            let controller = unit.artemis_controllers().unwrap().remove(0);
            assert_eq!(
                controller.weapon_indices,
                if linked { vec![0] } else { vec![] }
            );
            let mut state = serde_json::to_value(&unit).unwrap();
            let section_name = serde_json::to_value(section).unwrap();
            state["sections"][section_name.as_str().unwrap()] =
                serde_json::json!({"armor":0,"internal":0,"rear":0});
            let section_lost: BattleVehicle = serde_json::from_value(state).unwrap();
            assert!(!section_lost.artemis_operational(0).unwrap());
            let mut damaged = unit.clone();
            assert!(damaged.destroy_critical(controller.location).unwrap());
            assert!(!damaged.artemis_operational(0).unwrap());
            assert!(!damaged.destroy_critical(controller.location).unwrap());
            assert_eq!(damaged.mass().unwrap(), unit.mass().unwrap());
            let restored: BattleVehicle =
                serde_json::from_value(serde_json::to_value(&damaged).unwrap()).unwrap();
            assert_eq!(
                restored.artemis_controllers().unwrap(),
                damaged.artemis_controllers().unwrap()
            );
        }
        for link in ["-", "0", "2", "255"] {
            let unit = BattleVehicle::new(definition(&source, BattleVehicleSection::Turret, link))
                .unwrap();
            assert!(!unit.artemis_operational(0).unwrap());
            assert!(unit.artemis_controllers().unwrap()[0].operational);
        }
        for link in ["bad", "-1", "256"] {
            assert!(
                BattleVehicle::new(definition(&source, BattleVehicleSection::Turret, link))
                    .is_err()
            );
        }
    }
}

/// A running, piloted vehicle with local Artemis hardware and both ammunition supplies.
async fn fixture(source: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Artemis vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        definition(source, BattleVehicleSection::Turret, "1"),
    )
    .unwrap();
    let map = world.create(&config, "Field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "field",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    world
        .btech
        .set_unit_power(id, BattlePower::Running)
        .unwrap();
    (dir, config, world, id)
}

/// Native and Lua selections agree, feed the matching bins, roll back and survive restart.
#[tokio::test]
async fn vehicle_artemis_controls_feed_rollback_and_restart() {
    for source in templates() {
        let (_dir, config, world, id) = fixture(&source).await;
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let command = format!("btech.unit.artemis({},1,0)", id.0);
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!("{command}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(lua.drain_outbox().is_empty());
        support::run_text(&native, &config, ObjectId(1), 1, "artemis 0");
        lua.eval_callback::<mlua::Value>(&command).unwrap();
        assert_eq!(lua.world().btech, native.world().btech);
        assert_eq!(
            lua.world().btech.vehicles()[&id]
                .ammunition_mode(0)
                .unwrap(),
            BattleAmmunitionMode::Artemis
        );
        assert!(
            lua.eval_callback::<bool>(&format!(
                "return btech.unit.state({}).artemis[1].operational",
                id.0
            ))
            .unwrap()
        );
        let selected = lua.world().clone();
        persistence::save(&config.database(), &selected)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, selected.btech);
        let bins = restored.btech.vehicles()[&id].ammunition().to_vec();
        let launch =
            reserve_battle_vehicle_weapon(&mut restored, id, ObjectId(1), 0, true).unwrap();
        assert!(launch.launched);
        assert_eq!(launch.ammunition_mode, BattleAmmunitionMode::Artemis);
        assert_eq!(
            restored.btech.vehicles()[&id].ammunition(),
            &[bins[0], bins[1] - 1]
        );
        let location = selected.btech.vehicles()[&id]
            .artemis_controllers()
            .unwrap()[0]
            .location;
        destroy_battle_vehicle_critical(&mut lua.world_mut(), id, location).unwrap();
        let damaged = lua.world().btech.clone();
        assert!(lua.eval_callback::<()>(&command).is_err());
        assert_eq!(lua.world().btech, damaged);
    }
}
