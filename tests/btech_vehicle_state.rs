//! Vehicle material phases and validated snapshot replay are independent of Mech anatomy.
use stompymux_rs::*;

/// An intact tracked fixture with two turret weapons and four ammunition bins.
fn vehicle() -> BattleVehicle {
    BattleVehicle::new(
        BattleVehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn vehicle_turret_loss_clears_bins_without_destroying_hull() {
    let mut vehicle = vehicle();
    vehicle.expend_ammunition(0, 2).unwrap();
    assert_eq!(vehicle.ammunition(), &[3, 5, 5, 5]);
    let before = vehicle.clone();
    assert!(vehicle.expend_ammunition(0, 4).is_err());
    assert!(vehicle.expend_ammunition(4, 1).is_err());
    assert_eq!(vehicle, before);
    let armor = vehicle
        .damage_phase(
            BattleVehicleSection::Turret,
            45,
            BattleDamagePhase::Armor { rear: true },
        )
        .unwrap();
    assert_eq!((armor.absorbed, armor.remaining), (40, 5));
    assert!(!armor.unit_destroyed);
    let internal = vehicle
        .damage_phase(
            BattleVehicleSection::Turret,
            armor.remaining,
            BattleDamagePhase::Internal,
        )
        .unwrap();
    assert_eq!((internal.absorbed, internal.remaining), (5, 0));
    assert!(internal.destroyed_sections.is_empty());
    let snapshot = serde_json::to_value(&vehicle).unwrap();
    let mut restored: BattleVehicle = serde_json::from_value(snapshot).unwrap();
    let result = vehicle
        .damage_phase(
            BattleVehicleSection::Turret,
            10,
            BattleDamagePhase::Internal,
        )
        .unwrap();
    assert_eq!(
        restored
            .damage_phase(
                BattleVehicleSection::Turret,
                10,
                BattleDamagePhase::Internal
            )
            .unwrap(),
        result
    );
    assert_eq!((result.absorbed, result.remaining), (3, 7));
    assert_eq!(
        result.destroyed_sections,
        vec![BattleVehicleSection::Turret]
    );
    assert!(!result.unit_destroyed);
    assert_eq!(vehicle.ammunition(), &[0, 0, 0, 0]);
    assert_eq!(restored, vehicle);
    let again = vehicle
        .damage_phase(BattleVehicleSection::Turret, 3, BattleDamagePhase::Internal)
        .unwrap();
    assert_eq!((again.absorbed, again.remaining), (0, 3));
    assert!(again.destroyed_sections.is_empty());
    let result = vehicle
        .damage_phase(BattleVehicleSection::Front, 8, BattleDamagePhase::Internal)
        .unwrap();
    assert!(result.unit_destroyed);
    assert_eq!(vehicle.sections()[&BattleVehicleSection::Front].armor, 0);
    assert!(vehicle.expend_ammunition(0, 1).is_err());
    let restored: BattleVehicle =
        serde_json::from_value(serde_json::to_value(&vehicle).unwrap()).unwrap();
    assert!(restored.is_destroyed());
}

#[test]
fn vehicle_snapshot_rejects_inconsistent_material_state() {
    let original = serde_json::to_value(vehicle()).unwrap();
    for (pointer, value) in [
        ("/sections/front/armor", serde_json::json!(41)),
        ("/sections/front/internal", serde_json::json!(9)),
        ("/sections/front/internal", serde_json::json!(0)),
        ("/ammunition/0", serde_json::json!(6)),
        ("/ammunition", serde_json::json!([])),
        ("/definition/tons", serde_json::json!(0)),
    ] {
        let mut snapshot = original.clone();
        *snapshot.pointer_mut(pointer).unwrap() = value;
        assert!(
            serde_json::from_value::<BattleVehicle>(snapshot).is_err(),
            "{pointer}"
        );
    }
    let mut snapshot = original.clone();
    snapshot["sections"]
        .as_object_mut()
        .unwrap()
        .remove("front");
    assert!(serde_json::from_value::<BattleVehicle>(snapshot).is_err());
    let mut snapshot = original;
    snapshot["sections"]["turret"] = serde_json::json!({"armor":0,"internal":0,"rear":0});
    assert!(serde_json::from_value::<BattleVehicle>(snapshot.clone()).is_err());
    snapshot["ammunition"] = serde_json::json!([0, 0, 0, 0]);
    assert!(
        !serde_json::from_value::<BattleVehicle>(snapshot)
            .unwrap()
            .is_destroyed()
    );
    let mut truck = BattleVehicle::new(
        BattleVehicleTemplate::parse(
            "Flatbed_Truck",
            include_str!("../game/mechs/Flatbed_Truck.toml"),
        )
        .unwrap(),
    )
    .unwrap();
    let before = truck.clone();
    assert!(
        truck
            .damage_phase(
                BattleVehicleSection::Turret,
                10,
                BattleDamagePhase::Internal
            )
            .is_err()
    );
    assert_eq!(truck, before);
    for section in [
        BattleVehicleSection::Left,
        BattleVehicleSection::Right,
        BattleVehicleSection::Front,
        BattleVehicleSection::Rear,
    ] {
        let mut vehicle = vehicle();
        assert!(
            vehicle
                .damage_phase(section, 8, BattleDamagePhase::Internal)
                .unwrap()
                .unit_destroyed
        );
    }
}
