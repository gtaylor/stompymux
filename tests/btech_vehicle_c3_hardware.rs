//! Vehicle command computers use the shared inventory rules with single-slot installations.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Install independent computers in a turret and surviving hull face.
fn design() -> BattleVehicleTemplate {
    let mut template =
        BattleVehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
            .unwrap();
    for section in template.sections.values_mut() {
        section.criticals.clear();
    }
    for (section, slot, equipment) in [
        (BattleVehicleSection::Turret, 0, "C3Master"),
        (BattleVehicleSection::Turret, 4, "C3Master"),
        (BattleVehicleSection::Front, 0, "C3Slave"),
        (BattleVehicleSection::Front, 1, "C3i"),
    ] {
        template
            .sections
            .get_mut(&section)
            .unwrap()
            .criticals
            .insert(
                slot,
                CriticalDefinition {
                    equipment: equipment.into(),
                    data: "-".into(),
                    modes: vec![],
                    brand: None,
                },
            );
    }
    template
}

#[test]
fn vehicle_computers_use_single_slots_and_independent_master_damage() {
    let mut vehicle = BattleVehicle::new(design()).unwrap();
    let installed = vehicle.c3_hardware().unwrap();
    assert_eq!(
        installed,
        BattleC3Hardware {
            masters: 2,
            working_masters: 2,
            slave_installed: true,
            slave_operational: true,
            c3i_installed: true,
            c3i_operational: true
        }
    );
    assert_eq!(vehicle.power(), BattlePower::Off);
    assert!(vehicle.c3_operational().unwrap());
    vehicle
        .destroy_critical(VehicleCriticalLocation {
            section: BattleVehicleSection::Turret,
            slot: 0,
        })
        .unwrap();
    assert_eq!(vehicle.c3_hardware().unwrap().working_masters, 1);
    assert!(vehicle.c3_operational().unwrap());
    vehicle
        .destroy_critical(VehicleCriticalLocation {
            section: BattleVehicleSection::Turret,
            slot: 4,
        })
        .unwrap();
    let damaged = vehicle.c3_hardware().unwrap();
    assert_eq!(damaged.masters, 2);
    assert_eq!(damaged.working_masters, 0);
    assert!(damaged.slave_operational);
    assert!(!vehicle.c3_operational().unwrap());
    vehicle
        .destroy_critical(VehicleCriticalLocation {
            section: BattleVehicleSection::Front,
            slot: 1,
        })
        .unwrap();
    assert!(vehicle.c3_hardware().unwrap().c3i_installed);
    assert!(!vehicle.c3_hardware().unwrap().c3i_operational);
    let restored: BattleVehicle =
        serde_json::from_value(serde_json::to_value(&vehicle).unwrap()).unwrap();
    assert_eq!(
        restored.c3_hardware().unwrap(),
        vehicle.c3_hardware().unwrap()
    );
    assert_eq!(
        restored.c3_operational().unwrap(),
        vehicle.c3_operational().unwrap()
    );
}

#[test]
fn section_loss_and_wrecks_disable_computers_without_erasing_installation() {
    let mut vehicle = BattleVehicle::new(design()).unwrap();
    vehicle
        .damage_phase(
            BattleVehicleSection::Turret,
            u16::MAX,
            BattleDamagePhase::Internal,
        )
        .unwrap();
    assert!(!vehicle.is_destroyed());
    assert_eq!(vehicle.c3_hardware().unwrap().working_masters, 0);
    assert!(vehicle.c3_hardware().unwrap().c3i_operational);
    vehicle
        .damage_phase(
            BattleVehicleSection::Left,
            u16::MAX,
            BattleDamagePhase::Internal,
        )
        .unwrap();
    assert!(vehicle.is_destroyed());
    let hardware = vehicle.c3_hardware().unwrap();
    assert_eq!(hardware.masters, 2);
    assert!(hardware.slave_installed && hardware.c3i_installed);
    assert!(!hardware.slave_operational && !hardware.c3i_operational);
    assert!(!vehicle.c3_operational().unwrap());
}

#[tokio::test]
async fn vehicle_hardware_survives_storage_and_is_exposed_to_lua() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Command vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(&mut world, id, design()).unwrap();
    destroy_battle_vehicle_critical(
        &mut world,
        id,
        VehicleCriticalLocation {
            section: BattleVehicleSection::Turret,
            slot: 0,
        },
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech, loaded.btech);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
    let counts: (usize, usize, bool) = scripts.eval_callback(&format!("local h = btech.unit.state({}).c3_hardware; return h.masters,h.working_masters,h.c3i_operational",id.0)).unwrap();
    assert_eq!(counts, (2, 1, true));
}

#[test]
fn shipped_vehicle_master_and_slave_templates_have_live_hardware() {
    let master = BattleVehicle::new(
        BattleVehicleTemplate::parse("Schiltron", include_str!("../game/mechs/Schiltron.toml"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(master.c3_hardware().unwrap().working_masters, 1);
    assert!(master.c3_operational().unwrap());
    let slave = BattleVehicle::new(
        BattleVehicleTemplate::parse(
            "Demolisher-MRM",
            include_str!("../game/mechs/Demolisher-MRM.toml"),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(slave.c3_hardware().unwrap().slave_operational);
    assert!(slave.c3_operational().unwrap());
    let empty = BattleVehicle::new(
        BattleVehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(empty.c3_hardware().unwrap(), BattleC3Hardware::default());
    assert!(!empty.c3_operational().unwrap());
}
