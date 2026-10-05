//! Vehicle material phases and validated snapshot replay are independent of Mech anatomy.
use stompymux_rs::*;

/// An intact tracked fixture with two turret weapons and four ammunition bins.
fn vehicle() -> Vehicle {
    Vehicle::new(
        VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
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
            VehicleSection::Turret,
            45,
            DamagePhase::Armor { rear: true },
        )
        .unwrap();
    assert_eq!((armor.absorbed, armor.remaining), (40, 5));
    assert!(!armor.unit_destroyed);
    let internal = vehicle
        .damage_phase(
            VehicleSection::Turret,
            armor.remaining,
            DamagePhase::Internal,
        )
        .unwrap();
    assert_eq!((internal.absorbed, internal.remaining), (5, 0));
    assert!(internal.destroyed_sections.is_empty());
    let snapshot = serde_json::to_value(&vehicle).unwrap();
    let mut restored: Vehicle = serde_json::from_value(snapshot).unwrap();
    let result = vehicle
        .damage_phase(VehicleSection::Turret, 10, DamagePhase::Internal)
        .unwrap();
    assert_eq!(
        restored
            .damage_phase(VehicleSection::Turret, 10, DamagePhase::Internal)
            .unwrap(),
        result
    );
    assert_eq!((result.absorbed, result.remaining), (3, 7));
    assert_eq!(result.destroyed_sections, vec![VehicleSection::Turret]);
    assert!(!result.unit_destroyed);
    assert_eq!(vehicle.ammunition(), &[0, 0, 0, 0]);
    assert_eq!(restored, vehicle);
    let again = vehicle
        .damage_phase(VehicleSection::Turret, 3, DamagePhase::Internal)
        .unwrap();
    assert_eq!((again.absorbed, again.remaining), (0, 3));
    assert!(again.destroyed_sections.is_empty());
    let result = vehicle
        .damage_phase(VehicleSection::Front, 8, DamagePhase::Internal)
        .unwrap();
    assert!(result.unit_destroyed);
    assert_eq!(vehicle.sections()[&VehicleSection::Front].armor, 0);
    assert!(vehicle.expend_ammunition(0, 1).is_err());
    let restored: Vehicle =
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
            serde_json::from_value::<Vehicle>(snapshot).is_err(),
            "{pointer}"
        );
    }
    let mut snapshot = original.clone();
    snapshot["sections"]
        .as_object_mut()
        .unwrap()
        .remove("front");
    assert!(serde_json::from_value::<Vehicle>(snapshot).is_err());
    let mut snapshot = original;
    snapshot["sections"]["turret"] = serde_json::json!({"armor":0,"internal":0,"rear":0});
    assert!(serde_json::from_value::<Vehicle>(snapshot.clone()).is_err());
    snapshot["ammunition"] = serde_json::json!([0, 0, 0, 0]);
    assert!(
        !serde_json::from_value::<Vehicle>(snapshot)
            .unwrap()
            .is_destroyed()
    );
    let mut truck = Vehicle::new(
        VehicleTemplate::parse(
            "Flatbed_Truck",
            include_str!("../game/units/Flatbed_Truck.toml"),
        )
        .unwrap(),
    )
    .unwrap();
    let before = truck.clone();
    assert!(
        truck
            .damage_phase(VehicleSection::Turret, 10, DamagePhase::Internal)
            .is_err()
    );
    assert_eq!(truck, before);
    for section in [
        VehicleSection::Left,
        VehicleSection::Right,
        VehicleSection::Front,
        VehicleSection::Rear,
    ] {
        let mut vehicle = vehicle();
        assert!(
            vehicle
                .damage_phase(section, 8, DamagePhase::Internal)
                .unwrap()
                .unit_destroyed
        );
    }
}
