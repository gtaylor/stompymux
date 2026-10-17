//! Artillery timing, bounded effect geometry, scatter and saved random-stream replay.
use crate::support;
use stompymux_rs::*;

/// Advance an isolated shot to arrival without applying its effects to a live world.
fn arrive(
    flight: &mut ArtilleryFlight,
    dimensions: (u16, u16),
    dice: &mut Dice,
) -> ArtilleryImpactPattern {
    loop {
        if let Some(pattern) = flight.advance(dimensions, dice).unwrap() {
            return pattern;
        }
    }
}

/// Flight delay truncates fractional seconds, never falls below ten, and consumes no in-flight dice.
#[test]
fn artillery_flight_timing_and_saved_cursor_validation() {
    let origin = HexCoordinate { x: 0, y: 0 };
    for (distance, delay) in [(0, 10), (49, 10), (50, 10), (54, 10), (55, 11), (999, 199)] {
        let target = HexCoordinate { x: 0, y: distance };
        let mut flight = ArtilleryFlight::new(
            origin,
            target,
            Weapon::LongTom,
            ArtilleryMode::Standard,
            ArtilleryOutcome::Hit,
        )
        .unwrap();
        assert_eq!(flight.remaining(), delay);
        let mut dice = Dice::seeded([1; 32]);
        let untouched = dice.clone();
        for _ in 1..delay {
            assert!(flight.advance((1000, 1000), &mut dice).unwrap().is_none());
            assert_eq!(dice, untouched);
            let restored: ArtilleryFlight =
                serde_json::from_value(serde_json::to_value(&flight).unwrap()).unwrap();
            assert_eq!(restored, flight);
        }
        assert!(flight.advance((1000, 1000), &mut dice).unwrap().is_some());
        assert_eq!(flight.remaining(), 0);
        assert_eq!(dice, untouched);
        let done = flight.clone();
        assert!(flight.advance((1000, 1000), &mut dice).is_err());
        assert_eq!(flight, done);
        assert_eq!(dice, untouched);
    }
    let flight = ArtilleryFlight::new(
        origin,
        origin,
        Weapon::LongTom,
        ArtilleryMode::Mine,
        ArtilleryOutcome::Scatter { hexes: 3 },
    )
    .unwrap();
    let data = serde_json::to_value(&flight).unwrap();
    for (field, invalid) in [("remaining", 11), ("weapon", 0)] {
        let mut corrupt = data.clone();
        corrupt[field] = invalid.into();
        assert!(serde_json::from_value::<ArtilleryFlight>(corrupt).is_err());
    }
    let mut corrupt = data.clone();
    corrupt["outcome"] = serde_json::json!({ "scatter": { "hexes": 2001 } });
    assert!(serde_json::from_value::<ArtilleryFlight>(corrupt).is_err());
    for weapon in [Weapon::MediumLaser, Weapon::Srm4] {
        assert!(
            ArtilleryFlight::new(
                origin,
                origin,
                weapon,
                ArtilleryMode::Standard,
                ArtilleryOutcome::Hit
            )
            .is_err()
        );
        let mut corrupt = data.clone();
        corrupt["weapon"] = serde_json::to_value(weapon).unwrap();
        assert!(serde_json::from_value::<ArtilleryFlight>(corrupt).is_err());
    }
    let mut corrupt = data;
    corrupt["target"]["x"] = (-1).into();
    assert!(serde_json::from_value::<ArtilleryFlight>(corrupt).is_err());
}

/// High-explosive, cluster and smoke rounds cover the center and its six neighbors, cluster
/// rounds with their own Damage Values in 5-point groups, and FASCAM stays in the target hex
/// (Tactical Operations: Advanced Rules p. 152; Advanced Units & Equipment pp. 166-169). No
/// dice are drawn for a round that hits.
#[test]
fn artillery_payload_patterns_follow_the_ordnance_table() {
    let center = HexCoordinate { x: 5, y: 5 };
    for (weapon, mode, damage) in [
        (Weapon::Thumper, ArtilleryMode::Standard, Some((15, 5))),
        (Weapon::Thumper, ArtilleryMode::Cluster, Some((10, 1))),
        (Weapon::Sniper, ArtilleryMode::Cluster, Some((15, 5))),
        (Weapon::LongTom, ArtilleryMode::Cluster, Some((20, 10))),
        (Weapon::LongTom, ArtilleryMode::Smoke, None),
        (Weapon::Sniper, ArtilleryMode::Mine, None),
    ] {
        let mut flight =
            ArtilleryFlight::new(center, center, weapon, mode, ArtilleryOutcome::Hit).unwrap();
        let mut dice = Dice::seeded([7; 32]);
        let untouched = dice.clone();
        let pattern = arrive(&mut flight, (12, 12), &mut dice);
        assert_eq!(dice, untouched);
        assert_eq!(pattern.impact, center);
        assert!(!pattern.missed);
        assert!(pattern.cells[0].direct);
        assert_eq!(pattern.cells[0].position, center);
        if mode == ArtilleryMode::Mine {
            assert_eq!(pattern.cells.len(), 1);
            assert_eq!(
                pattern.cells[0].effect,
                ArtilleryEffect::Mine { strength: 20 }
            );
            continue;
        }
        // Smoke reaches only one ring, even from a Long Tom.
        assert_eq!(pattern.cells.len(), 7, "{weapon:?} {mode:?}");
        assert_eq!(
            pattern.cells[1..]
                .iter()
                .map(|cell| cell.position)
                .collect::<Vec<_>>(),
            center.neighbors().unwrap()
        );
        for (index, cell) in pattern.cells.iter().enumerate() {
            assert_eq!(cell.direct, index == 0);
            let expected = match damage {
                Some((center, ring)) => ArtilleryEffect::Damage {
                    total: if index == 0 { center } else { ring },
                    packet_size: 5,
                    table: HitTable::Weapon,
                },
                None => ArtilleryEffect::Smoke,
            };
            assert_eq!(cell.effect, expected, "{weapon:?} {mode:?}");
        }
    }
    // Artillery cannons fire no special rounds, and the Inner Sphere Thumper no FASCAM
    // (Advanced Units & Equipment pp. 97, 165; Advanced Rules p. 152).
    for (weapon, mode) in [
        (Weapon::LongTomCannon, ArtilleryMode::Smoke),
        (Weapon::SniperCannon, ArtilleryMode::Cluster),
        (Weapon::Thumper, ArtilleryMode::Mine),
    ] {
        assert!(ArtilleryFlight::new(center, center, weapon, mode, ArtilleryOutcome::Hit).is_err());
    }
}

/// A miss scatters in a direction rolled on the Scatter Diagram, clockwise from north: an
/// indirect round as many hexes as it missed by, a direct-fire round 1D6 hexes (Tactical
/// Operations: Advanced Rules pp. 150, 153). Invalid arrivals change nothing, and a round that
/// scatters off the battlefield lands without effect.
#[test]
fn artillery_scatter_and_atomic_invalid_arrival() {
    let center = HexCoordinate { x: 10, y: 10 };
    let mut directions = std::collections::BTreeSet::new();
    let mut distances = std::collections::BTreeSet::new();
    for seed in 0..32 {
        let mut flight = ArtilleryFlight::new(
            center,
            center,
            Weapon::LongTom,
            ArtilleryMode::Standard,
            ArtilleryOutcome::Scatter { hexes: 4 },
        )
        .unwrap();
        let mut dice = Dice::seeded([seed; 32]);
        for _ in 0..9 {
            assert!(flight.advance((40, 40), &mut dice).unwrap().is_none());
        }
        for dimensions in [(0, 20), (1001, 20), (1, 1)] {
            let before = flight.clone();
            let before_dice = dice.clone();
            assert!(flight.advance(dimensions, &mut dice).is_err());
            assert_eq!(flight, before);
            assert_eq!(dice, before_dice);
        }
        let mut expected = dice.clone();
        let hexside = usize::from(expected.die(6).unwrap() - 1);
        let mut landing = center;
        for _ in 0..4 {
            landing = landing.neighbors().unwrap()[hexside];
        }
        let pattern = arrive(&mut flight, (40, 40), &mut dice);
        assert!(pattern.missed);
        assert_eq!(pattern.target, center);
        assert_eq!(pattern.impact, landing);
        assert_eq!(pattern.impact.distance(center), 4);
        directions.insert(hexside);

        let mut direct = ArtilleryFlight::new(
            center,
            center,
            Weapon::Sniper,
            ArtilleryMode::Standard,
            ArtilleryOutcome::DirectMiss,
        )
        .unwrap();
        let mut dice = Dice::seeded([seed; 32]);
        let mut expected = dice.clone();
        expected.die(6).unwrap();
        let hexes = expected.die(6).unwrap();
        let pattern = arrive(&mut direct, (40, 40), &mut dice);
        assert_eq!(pattern.impact.distance(center), u64::from(hexes));
        distances.insert(hexes);
    }
    assert_eq!(directions.len(), 6);
    assert_eq!(distances.len(), 6);
    // Off the map edge the round is lost.
    let corner = HexCoordinate { x: 0, y: 0 };
    let mut flight = ArtilleryFlight::new(
        corner,
        corner,
        Weapon::Thumper,
        ArtilleryMode::Standard,
        ArtilleryOutcome::Scatter { hexes: 30 },
    )
    .unwrap();
    let pattern = arrive(&mut flight, (20, 20), &mut Dice::seeded([3; 32]));
    assert!(pattern.missed);
    assert_eq!(pattern.impact.distance(corner), 30);
    assert!(pattern.cells.is_empty());
}

/// A launch roll decides where the round lands: indirect fire scatters its margin of failure,
/// direct fire 1D6 on arrival, and a cannon half its margin, rounded up, but no farther than
/// its target (Tactical Operations: Advanced Units & Equipment p. 97).
#[test]
fn artillery_launch_outcomes_follow_the_fire_type() {
    use ArtilleryOutcome as O;
    assert_eq!(
        O::of_launch(ArtilleryFire::Indirect, true, 9, 9, 30),
        O::Hit
    );
    assert_eq!(
        O::of_launch(ArtilleryFire::Indirect, false, 12, 7, 30),
        O::Scatter { hexes: 5 }
    );
    assert_eq!(
        O::of_launch(ArtilleryFire::Direct, false, 12, 7, 10),
        O::DirectMiss
    );
    assert_eq!(
        O::of_launch(ArtilleryFire::Cannon, false, 12, 7, 10),
        O::Scatter { hexes: 3 }
    );
    assert_eq!(
        O::of_launch(ArtilleryFire::Cannon, false, 12, 2, 4),
        O::Scatter { hexes: 4 }
    );
}

/// Artillery uses map-sheet range and the Tactical Operations: Advanced Rules arithmetic:
/// indirect fire is Gunnery +7 with the spotter's term (truncated toward zero) less one per
/// adjusted shot, and no extra term for an unspotted shot (pp. 148-150); a visible hex within
/// seventeen hexes is fired at directly at Gunnery +4 (p. 153).
#[test]
fn artillery_aim_ranges_observers_and_corrections() {
    // Indirect pieces reach the Tactical Operations p. 149 mapsheet counts (8, 9, 30, 18 and 21
    // sheets of seventeen hexes); artillery cannons keep their direct-fire hex ranges.
    let weapons = [
        (Weapon::ArrowIv, 8 * ARTILLERY_MAPSHEET_HEXES),
        (Weapon::ClanArrowIv, 9 * ARTILLERY_MAPSHEET_HEXES),
        (Weapon::LongTom, 30 * ARTILLERY_MAPSHEET_HEXES),
        (Weapon::Sniper, 18 * ARTILLERY_MAPSHEET_HEXES),
        (Weapon::Thumper, 21 * ARTILLERY_MAPSHEET_HEXES),
        (Weapon::LongTomCannon, 20),
        (Weapon::SniperCannon, 12),
        (Weapon::ThumperCannon, 14),
    ];
    assert_eq!(ARTILLERY_MAPSHEET_HEXES, 17);
    assert_eq!(Weapon::LongTom.effective_range(), 510);
    for (weapon, maximum) in weapons {
        let cannon = !weapon.is_indirect_artillery();
        let base = ArtilleryAimInput {
            distance: f64::from(maximum),
            submerged: false,
            visible: false,
            gunnery: 4,
            observer: ArtilleryObserver::Unassisted,
            adjustment: 0,
            interference: false,
            attacker_movement: 0,
            attacker_condition: 0,
            intervening: 0,
            registered: false,
        };
        assert_eq!(
            weapon.artillery_aim(base).unwrap(),
            ArtilleryAim {
                target_number: 11,
                maximum_range: maximum,
                range: ArtilleryRange::InRange,
                fire: if cannon {
                    ArtilleryFire::Cannon
                } else {
                    ArtilleryFire::Indirect
                },
                automatic: false,
            }
        );
        for (observer, indirect) in [
            (ArtilleryObserver::Unassisted, 11),
            (ArtilleryObserver::Unavailable, 11),
            (ArtilleryObserver::Spotting(0), 9),
            (ArtilleryObserver::Spotting(1), 10),
            (ArtilleryObserver::Spotting(3), 11),
            (ArtilleryObserver::Spotting(5), 11),
            (ArtilleryObserver::Spotting(7), 12),
            (ArtilleryObserver::Spotting(8), 13),
        ] {
            for adjustment in [0, 1, 255] {
                let result = weapon
                    .artillery_aim(ArtilleryAimInput {
                        observer,
                        adjustment,
                        ..base
                    })
                    .unwrap();
                assert_eq!(result.target_number, indirect - i32::from(adjustment));
                // Electromagnetic interference adds two to direct and indirect fire alike
                // (Tactical Operations: Advanced Rules p. 53).
                for visible in [false, true] {
                    let aim = |interference| {
                        weapon
                            .artillery_aim(ArtilleryAimInput {
                                observer,
                                adjustment,
                                visible,
                                interference,
                                distance: 10.0,
                                ..base
                            })
                            .unwrap()
                            .target_number
                    };
                    assert_eq!(aim(true), aim(false) + 2);
                }
            }
        }
        if !cannon {
            let direct = weapon
                .artillery_aim(ArtilleryAimInput {
                    distance: 12.0,
                    visible: true,
                    adjustment: 3,
                    observer: ArtilleryObserver::Spotting(0),
                    ..base
                })
                .unwrap();
            assert_eq!(direct.target_number, 8);
            assert_eq!(direct.fire, ArtilleryFire::Direct);
        }
        let out = weapon
            .artillery_aim(ArtilleryAimInput {
                distance: f64::from(maximum) + 0.001,
                adjustment: 255,
                ..base
            })
            .unwrap();
        assert_eq!(out.range, ArtilleryRange::OutOfRange);
        let submerged = weapon
            .artillery_aim(ArtilleryAimInput {
                submerged: true,
                distance: 2000.0,
                ..base
            })
            .unwrap();
        assert_eq!(submerged.range, ArtilleryRange::Underwater);
        for distance in [f64::NAN, f64::INFINITY, -1.0] {
            assert!(
                weapon
                    .artillery_aim(ArtilleryAimInput { distance, ..base })
                    .is_err()
            );
        }
        assert!(Weapon::MediumLaser.artillery_aim(base).is_err());
        // Only artillery cannons have ordinary range brackets (Tactical Operations: Advanced
        // Units & Equipment p. 97).
        assert_eq!(weapon.range_modifier(5.0).is_ok(), cannon);
        assert!(weapon.damage_groups(None).is_err());
        assert_eq!(weapon.gunnery_skill(false), "Gunnery-Artillery");
    }
}

/// Artillery construction uses the same structural validation as other launchers.
#[test]
fn artillery_catalogue_admits_delayed_launchers() {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    let section = template.sections.get_mut(&MechSection::LeftTorso).unwrap();
    section.criticals.clear();
    for slot in 0..12 {
        section.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: Weapon::ClanArrowIv.name().into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    }
    let loadout = MechLoadout::resolve(&template).unwrap();
    assert!(
        loadout
            .weapons
            .iter()
            .any(|mount| mount.weapon == Weapon::ClanArrowIv)
    );
    for section in template.sections.values_mut() {
        section
            .criticals
            .retain(|_, part| part.equipment != "JumpJet");
    }
    template.jump_speed = 0.0;
    assert!(Mech::from_template(crate::support::templates::legal(template.clone())).is_ok());
}

/// Every occupant of a shelled hex takes its own attack direction from the line to the gun
/// (Tactical Operations: Advanced Rules p. 150). A Mech facing the gun keeps its rear armor
/// even after a Mech earlier in the same hex was struck from behind.
#[tokio::test]
async fn artillery_occupants_resolve_their_own_attack_direction() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Artillery field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        MapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    // The first Mech faces away from the gun to the north, the second faces it.
    for heading in [180.0, 0.0] {
        let id = world.create(&config, "Target".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(
            &mut world,
            id,
            MechTemplate::parse("AS7-D", include_str!("../game/units/AS7-D.toml")).unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 1, 2).unwrap();
        world
            .btech
            .edit_unit_motion(id, |motion| {
                motion.heading = heading;
                motion.desired_heading = heading;
            })
            .unwrap();
        ids.push(id);
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    let before = world.clone();
    let mut flight = ArtilleryFlight::new(
        HexCoordinate { x: 1, y: 0 },
        HexCoordinate { x: 1, y: 2 },
        Weapon::LongTom,
        ArtilleryMode::Standard,
        ArtilleryOutcome::Hit,
    )
    .unwrap();
    let report = loop {
        if let Some(report) =
            advance_artillery_flight(&mut world, map, &mut flight, MovementRules::STANDARD.fall)
                .unwrap()
        {
            break report;
        }
    };
    assert_eq!(
        report
            .hits
            .iter()
            .map(|hit| (hit.unit, hit.arc))
            .collect::<Vec<_>>(),
        [(ids[0], HitArc::Rear), (ids[1], HitArc::Front)]
    );
    let torsos = [
        MechSection::LeftTorso,
        MechSection::CenterTorso,
        MechSection::RightTorso,
    ];
    let armor = |world: &World, id: ObjectId| {
        torsos.map(|section| {
            let state = &world.btech.constructed_units()[&id].sections()[&section];
            (state.armor, state.rear)
        })
    };
    let (rear_hit, facing) = (armor(&world, ids[0]), armor(&world, ids[1]));
    let (rear_before, facing_before) = (armor(&before, ids[0]), armor(&before, ids[1]));
    // The Mech struck from behind loses rear torso armor and keeps its front torso armor.
    assert!(
        rear_hit
            .iter()
            .zip(rear_before)
            .any(|(now, was)| now.1 < was.1)
    );
    assert!(
        rear_hit
            .iter()
            .zip(rear_before)
            .all(|(now, was)| now.0 == was.0)
    );
    // The Mech facing the gun takes its torso hits on the front and keeps its rear armor.
    assert!(
        facing
            .iter()
            .zip(facing_before)
            .any(|(now, was)| now.0 < was.0)
    );
    assert!(
        facing
            .iter()
            .zip(facing_before)
            .all(|(now, was)| now.1 == was.1)
    );
}
