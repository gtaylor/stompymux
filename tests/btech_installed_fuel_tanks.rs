//! Installed tanks and loose stock contribute to one derived VTOL fuel capacity and cargo load.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Two distinct tank slots share the left hull face without replacing weapon equipment.
fn aircraft(cargo_tech: bool) -> BattleVehicleTemplate {
    let mut template = BattleVehicleTemplate::parse("Kestrel",include_str!("../game/mechs/Kestrel.toml")).unwrap();
    if !cargo_tech {
        template
            .attributes
            .insert("specials".into(), "ICEEngine_Tech".into());
    }
    for slot in [0, 1] {
        template
            .sections
            .get_mut(&BattleVehicleSection::Left)
            .unwrap()
            .criticals
            .insert(
                slot,
                CriticalDefinition {
                    equipment: "Fuel_Tank".into(),
                    data: "-".into(),
                    modes: vec![],
                    brand: None,
                },
            );
    }
    template
}

/// Installed tanks add cargo mass once, share stock discounts and retain original starting fuel.
#[tokio::test]
async fn installed_and_carried_tanks_share_capacity_load_and_restart() {
    for cargo_tech in [false, true] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let template = aircraft(cargo_tech);
        let mut empty = template.clone();
        empty
            .sections
            .get_mut(&BattleVehicleSection::Left)
            .unwrap()
            .criticals
            .clear();
        assert_eq!(template.mass().unwrap(), empty.mass().unwrap());
        let id = world.create(&config, "Tanker".into(), Kind::Thing);
        BattleUnitTemplate::Vehicle(template)
            .create(&mut world, id)
            .unwrap();
        let fuel = battle_vtol_fuel_status(&world, id).unwrap();
        assert_eq!(
            (
                fuel.capacity,
                fuel.remaining,
                fuel.installed_tanks,
                fuel.auxiliary_tanks
            ),
            (8000, 4000, 2, 0)
        );
        let divisor = if cargo_tech { 2 } else { 1 };
        assert_eq!(
            battle_unit_load(&world, id, true).unwrap().carried_mass,
            204 / divisor
        );
        assert_eq!(battle_inventory_mass(&world, id).unwrap(), 0);
        set_battle_inventory_quantity(&mut world, ObjectId(1), id, 422, 5, 3).unwrap();
        let fuel = set_battle_vtol_fuel(&mut world, &config, ObjectId(1), id, 14000).unwrap();
        assert_eq!(
            (fuel.capacity, fuel.installed_tanks, fuel.auxiliary_tanks),
            (14000, 2, 3)
        );
        assert_eq!(
            battle_unit_load(&world, id, true).unwrap().carried_mass,
            (204 + 306 + 10000) / divisor
        );
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let output = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech fuel #{}", id.0),
        );
        assert!(output.contains("installed tanks 2"), "{output}");
        let installed: u64 = scripts
            .eval_callback(&format!("return btech.unit.fuel({}).installed_tanks", id.0))
            .unwrap();
        assert_eq!(installed, 2);
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, saved.btech);
        assert_eq!(battle_vtol_fuel_status(&loaded, id).unwrap(), fuel);
    }
}

/// Damage does not reinterpret saved tank slots as removed cargo or erase their capacity.
#[tokio::test]
async fn damaged_installed_tanks_retain_capacity_and_cargo_contribution() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Tanker".into(), Kind::Thing);
    BattleUnitTemplate::Vehicle(aircraft(true))
        .create(&mut world, id)
        .unwrap();
    let before = battle_vtol_fuel_status(&world, id).unwrap();
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    let unit = &mut encoded["vehicles"][id.0.to_string()];
    unit["lost_criticals"] = serde_json::json!([{"section":"left","slot":0}]);
    unit["sections"]["left"]["internal"] = 0.into();
    unit["sections"]["left"]["armor"] = 0.into();
    world.btech = serde_json::from_value(encoded).unwrap();
    assert_eq!(battle_vtol_fuel_status(&world, id).unwrap(), before);
    assert_eq!(
        battle_unit_load(&world, id, true).unwrap().carried_mass,
        102
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
}

/// Ground vehicles may describe tank equipment without acquiring VTOL capacity or duplicated stock weight.
#[test]
fn tank_equipment_uses_shared_identity_without_creating_ground_fuel_state() {
    assert_eq!(
        BattleSystem::parse("fuel_tank").unwrap(),
        BattleSystem::FuelTank
    );
    let mut template =
        BattleVehicleTemplate::parse("Demolisher",include_str!("../game/mechs/Demolisher.toml")).unwrap();
    let before = template.mass().unwrap();
    template
        .sections
        .get_mut(&BattleVehicleSection::Left)
        .unwrap()
        .criticals
        .insert(
            0,
            CriticalDefinition {
                equipment: "Fuel_Tank".into(),
                data: "-".into(),
                modes: vec![],
                brand: None,
            },
        );
    let vehicle = BattleVehicle::new(template).unwrap();
    assert_eq!(vehicle.mass().unwrap(), before);
    assert_eq!(vehicle.installed_fuel_tanks(), 0);
    assert!(vehicle.vtol_fuel().is_none());
}
