//! The VTOL Combat Vehicle Hit Location Table: every arc and roll, the turret row, rotor
//! footnotes and the critical-proof exemption.
use stompymux_rs::*;

/// A Kestrel, optionally critical-proof or fitted with a turret.
fn kestrel(proof: bool, turret: bool) -> Vehicle {
    let mut template =
        VehicleTemplate::parse("Kestrel", include_str!("../game/units/Kestrel.toml")).unwrap();
    if proof {
        template
            .attributes
            .insert("specials".into(), "CritProof_Tech".into());
    }
    if turret {
        // The turret's copied front weapons take the mass of the troop compartment.
        template.attributes.remove("transport_bays");
        template.sections.insert(
            VehicleSection::Turret,
            template.sections[&VehicleSection::Front].clone(),
        );
    }
    Vehicle::new(template).unwrap()
}

#[test]
fn vtol_locations_follow_total_warfare_and_tactical_operations() {
    use VehicleSection as S;
    for proof in [false, true] {
        for turret in [false, true] {
            let unit = kestrel(proof, turret);
            // Rows 2–12: face, rotors, turret (or rotors), 5/9 side mappings, rotors (TW p. 196,
            // TO:AR p. 193). A chin turret takes row 4 alone (TO:AUE p. 160).
            for (arc, row) in [
                (HitArc::Front, "FOTRFFFLOOO"),
                (HitArc::Rear, "BOTLBBBROOO"),
                (HitArc::Left, "LOTFLLLBOOO"),
                (HitArc::Right, "ROTFRRRBOOO"),
            ] {
                for (index, location) in row.chars().enumerate() {
                    let roll = index as u8 + 2;
                    let before = unit.clone();
                    let report = unit.vtol_hit(arc, roll).unwrap();
                    let expected = match location {
                        'F' => S::Front,
                        'B' => S::Rear,
                        'L' => S::Left,
                        'R' => S::Right,
                        'T' if turret => S::Turret,
                        _ => S::Rotor,
                    };
                    assert_eq!(
                        report.hit.section, expected,
                        "{arc:?}/{roll}/{proof}/{turret}"
                    );
                    assert_eq!(unit.vtol_hit_location(arc, roll).unwrap(), expected);
                    let side = matches!(arc, HitArc::Left | HitArc::Right);
                    assert_eq!(
                        report.hit.through_armor_critical,
                        !proof && (matches!(roll, 2 | 12) || (side && roll == 8)),
                        "{arc:?}/{roll}/{proof}"
                    );
                    assert_eq!(report.rotor, !proof && expected == S::Rotor);
                    assert_eq!(report.hit.motive, None);
                    assert_eq!(unit, before);
                }
            }
        }
    }
}

#[test]
fn invalid_rolls_and_ground_vehicles_are_rejected() {
    let unit = kestrel(false, false);
    for roll in [0, 1, 13, 255] {
        assert!(unit.vtol_hit(HitArc::Front, roll).is_err());
        assert!(unit.vtol_hit_location(HitArc::Front, roll).is_err());
    }
    let ground = Vehicle::new(
        VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
            .unwrap(),
    )
    .unwrap();
    assert!(ground.vtol_hit(HitArc::Front, 7).is_err());
    assert!(ground.vtol_hit_location(HitArc::Front, 7).is_err());
}

/// Destroying all of a chin turret's internal structure destroys the VTOL as normal, and the
/// turret's criticals use the vehicle turret column (Tactical Operations: Advanced Units &
/// Equipment p. 160).
#[test]
fn a_destroyed_chin_turret_destroys_the_vtol() {
    let mut unit = kestrel(false, true);
    let internal = unit.sections()[&VehicleSection::Turret].internal;
    unit.damage_phase(VehicleSection::Turret, internal - 1, DamagePhase::Internal)
        .unwrap();
    assert!(!unit.is_destroyed());
    unit.damage_phase(VehicleSection::Turret, 1, DamagePhase::Internal)
        .unwrap();
    assert!(unit.is_destroyed());
}

#[test]
fn a_lost_turret_sends_turret_hits_to_the_rotors() {
    let mut unit = kestrel(false, true);
    assert_eq!(
        unit.vtol_hit(HitArc::Left, 4).unwrap().hit.section,
        VehicleSection::Turret
    );
    unit.damage_phase(VehicleSection::Turret, u16::MAX, DamagePhase::Internal)
        .unwrap();
    assert_eq!(
        unit.vtol_hit(HitArc::Left, 4).unwrap().hit.section,
        VehicleSection::Rotor
    );
}
