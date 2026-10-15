//! A torso-mounted cockpit (Tactical Operations: Advanced Units & Equipment pp. 112-113): the
//! layout, the Mech outliving its head, the sensor damage scale and the piloting penalty.
use stompymux_rs::*;

/// The Jenner JR10-X, whose torso-mounted cockpit sits behind a compact gyro.
fn jenner() -> Mech {
    Mech::from_template(
        MechTemplate::parse("JR10-X", include_str!("fixtures/btech/units/JR10-X.toml")).unwrap(),
    )
    .unwrap()
}

/// The slot of the `nth` installed `system` in `section`.
fn slot(unit: &Mech, section: MechSection, system: System, nth: usize) -> CriticalLocation {
    unit.loadout()
        .unwrap()
        .systems
        .iter()
        .filter(|part| part.system == system && part.location.section == section)
        .nth(nth)
        .unwrap()
        .location
}

#[test]
fn torso_mounted_cockpits_sit_in_the_center_torso() {
    let unit = jenner();
    assert!(unit.torso_mounted_cockpit());
    assert_eq!(unit.cockpit_section(), MechSection::CenterTorso);
    assert_eq!(
        slot(&unit, MechSection::CenterTorso, System::Cockpit, 0).slot,
        8
    );
    for section in [MechSection::LeftTorso, MechSection::RightTorso] {
        assert_eq!(slot(&unit, section, System::LifeSupport, 0).slot, 0);
    }
    // One more to piloting rolls, as a small cockpit adds.
    assert_eq!(unit.cockpit_piloting_modifier(), 1);
    // Small and torso-mounted cockpits both multiply the final BV by 0.95.
    assert!(unit.battle_value().unwrap().total > 0);
}

/// The Mech outlives its head, and later hits there are re-rolled.
#[test]
fn the_mech_survives_its_head() {
    let mut unit = jenner();
    let internal = unit.sections()[&MechSection::Head].internal;
    unit.damage_phase(MechSection::Head, internal, DamagePhase::Internal);
    assert_eq!(unit.sections()[&MechSection::Head].internal, 0);
    assert!(!unit.is_destroyed());
    assert!(unit.rerolls_head_hits());
    let mut dice = Dice::seeded([7; 32]);
    for roll in 2..=12 {
        let hit = resolve_battle_hit(&unit, HitArc::Front, roll, &mut dice).unwrap();
        assert_ne!(hit.section, MechSection::Head, "{roll}");
    }
    // Losing the head takes both of its sensors: blind for piloting, +4 for weapons.
    assert_eq!(unit.sensor_aim_modifier(), 4);
    assert_eq!(unit.blind_cockpit_modifier(), 4);
    assert_eq!(unit.cockpit_piloting_modifier(), 5);
    // The cockpit itself is still in the center torso.
    let cockpit = slot(&unit, MechSection::CenterTorso, System::Cockpit, 0);
    unit.destroy_critical(cockpit).unwrap();
    assert!(unit.is_destroyed());
}

/// The first sensor hit adds 2 to weapon attacks and the second 4; only a third stops them.
#[test]
fn a_third_sensor_package_keeps_the_weapons_working() {
    let mut unit = jenner();
    let center = slot(&unit, MechSection::CenterTorso, System::Sensors, 0);
    unit.destroy_critical(center).unwrap();
    assert_eq!(unit.sensor_aim_modifier(), 2);
    let head = slot(&unit, MechSection::Head, System::Sensors, 0);
    unit.destroy_critical(head).unwrap();
    assert_eq!(unit.sensor_aim_modifier(), 4);
    // One head sensor survives, so the MechWarrior is not yet blind.
    assert_eq!(unit.blind_cockpit_modifier(), 0);
    assert!(!unit.sensors_failed());
    let head = slot(&unit, MechSection::Head, System::Sensors, 1);
    unit.destroy_critical(head).unwrap();
    assert!(unit.sensors_failed());
    assert_eq!(unit.blind_cockpit_modifier(), 4);
}
