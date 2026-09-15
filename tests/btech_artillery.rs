//! Artillery timing, bounded effect geometry, scatter and saved random-stream replay.
use stompymux_rs::*;

/// Advance an isolated shot to arrival without applying its effects to a live world.
fn arrive(
    flight: &mut BattleArtilleryFlight,
    dimensions: (u16, u16),
    wind: u16,
    dice: &mut BattleDice,
) -> BattleArtilleryImpactPattern {
    loop {
        if let Some(pattern) = flight.advance(dimensions, wind, dice).unwrap() {
            return pattern;
        }
    }
}

/// Flight delay truncates fractional seconds, never falls below ten, and consumes no in-flight dice.
#[test]
fn artillery_flight_timing_and_saved_cursor_validation() {
    let origin = BattleHexCoordinate { x: 0, y: 0 };
    for (distance, delay) in [(0, 10), (49, 10), (50, 10), (54, 10), (55, 11), (999, 199)] {
        let target = BattleHexCoordinate { x: 0, y: distance };
        let mut flight = BattleArtilleryFlight::new(
            origin,
            target,
            BattleWeapon::LongTom,
            BattleArtilleryMode::Standard,
            true,
        )
        .unwrap();
        assert_eq!(flight.remaining(), delay);
        let mut dice = BattleDice::seeded([1; 32]);
        let untouched = dice.clone();
        for _ in 1..delay {
            assert!(
                flight
                    .advance((1000, 1000), 0, &mut dice)
                    .unwrap()
                    .is_none()
            );
            assert_eq!(dice, untouched);
            let restored: BattleArtilleryFlight =
                serde_json::from_value(serde_json::to_value(&flight).unwrap()).unwrap();
            assert_eq!(restored, flight);
        }
        assert!(
            flight
                .advance((1000, 1000), 0, &mut dice)
                .unwrap()
                .is_some()
        );
        assert_eq!(flight.remaining(), 0);
        assert_eq!(dice, untouched);
        let done = flight.clone();
        assert!(flight.advance((1000, 1000), 0, &mut dice).is_err());
        assert_eq!(flight, done);
        assert_eq!(dice, untouched);
    }
    let flight = BattleArtilleryFlight::new(
        origin,
        origin,
        BattleWeapon::LongTom,
        BattleArtilleryMode::Mine,
        false,
    )
    .unwrap();
    let data = serde_json::to_value(&flight).unwrap();
    for (field, invalid) in [("remaining", 11), ("weapon", 0)] {
        let mut corrupt = data.clone();
        corrupt[field] = invalid.into();
        assert!(serde_json::from_value::<BattleArtilleryFlight>(corrupt).is_err());
    }
    for weapon in [BattleWeapon::MediumLaser, BattleWeapon::Srm4] {
        assert!(
            BattleArtilleryFlight::new(origin, origin, weapon, BattleArtilleryMode::Standard, true)
                .is_err()
        );
        let mut corrupt = data.clone();
        corrupt["weapon"] = serde_json::to_value(weapon).unwrap();
        assert!(serde_json::from_value::<BattleArtilleryFlight>(corrupt).is_err());
    }
    let mut corrupt = data;
    corrupt["target"]["x"] = (-1).into();
    assert!(serde_json::from_value::<BattleArtilleryFlight>(corrupt).is_err());
}

/// Ordinary blasts cover the center and six neighbors; smoke draws one duration per cell and mines stay local.
#[test]
fn artillery_standard_smoke_and_mine_patterns() {
    let center = BattleHexCoordinate { x: 5, y: 5 };
    for mode in [
        BattleArtilleryMode::Standard,
        BattleArtilleryMode::Smoke,
        BattleArtilleryMode::Mine,
    ] {
        let mut flight =
            BattleArtilleryFlight::new(center, center, BattleWeapon::Thumper, mode, true).unwrap();
        let mut dice = BattleDice::seeded([7; 32]);
        let mut expected_dice = dice.clone();
        let pattern = arrive(&mut flight, (12, 12), 32767, &mut dice);
        assert_eq!(pattern.impact, center);
        assert!(!pattern.missed);
        assert!(pattern.cells[0].direct);
        assert_eq!(pattern.cells[0].position, center);
        if mode == BattleArtilleryMode::Mine {
            assert_eq!(pattern.cells.len(), 1);
            assert_eq!(
                pattern.cells[0].effect,
                BattleArtilleryEffect::Mine { strength: 5 }
            );
        } else {
            assert_eq!(pattern.cells.len(), 7);
            assert_eq!(
                pattern.cells[1..]
                    .iter()
                    .map(|cell| cell.position)
                    .collect::<Vec<_>>(),
                center.neighbors().unwrap()
            );
            for (index, cell) in pattern.cells.iter().enumerate() {
                assert_eq!(cell.direct, index == 0);
                if mode == BattleArtilleryMode::Smoke {
                    assert_eq!(
                        cell.effect,
                        BattleArtilleryEffect::Smoke {
                            seconds: 89 + expected_dice.die(61).unwrap()
                        }
                    );
                } else {
                    assert_eq!(
                        cell.effect,
                        BattleArtilleryEffect::Damage {
                            total: if index == 0 { 5 } else { 2 },
                            packet_size: 5,
                            table: BattleHitTable::Weapon
                        }
                    );
                }
            }
        }
        assert_eq!(dice, expected_dice);
    }
}

/// Cluster rounds retain every two-point bomblet on the map, including one-cell and edge maps.
#[test]
fn artillery_cluster_conservation_bounds_and_replay() {
    for dimensions in [(1, 1), (2, 3), (20, 20)] {
        for center in [
            BattleHexCoordinate { x: 0, y: 0 },
            BattleHexCoordinate {
                x: i32::from(dimensions.0) - 1,
                y: i32::from(dimensions.1) - 1,
            },
        ] {
            for seed in 0..32 {
                let mut flight = BattleArtilleryFlight::new(
                    center,
                    center,
                    BattleWeapon::LongTom,
                    BattleArtilleryMode::Cluster,
                    true,
                )
                .unwrap();
                let mut dice = BattleDice::seeded([seed; 32]);
                for _ in 0..5 {
                    assert!(flight.advance(dimensions, 0, &mut dice).unwrap().is_none());
                }
                let mut restored =
                    serde_json::from_value(serde_json::to_value(&flight).unwrap()).unwrap();
                let mut restored_dice =
                    serde_json::from_value(serde_json::to_value(&dice).unwrap()).unwrap();
                let pattern = arrive(&mut flight, dimensions, 0, &mut dice);
                assert_eq!(
                    arrive(&mut restored, dimensions, 0, &mut restored_dice),
                    pattern
                );
                assert_eq!(restored_dice, dice);
                assert!(pattern.cells.len() <= 20);
                let mut total_damage = 0;
                for cell in pattern.cells {
                    assert!((0..i32::from(dimensions.0)).contains(&cell.position.x));
                    assert!((0..i32::from(dimensions.1)).contains(&cell.position.y));
                    assert!((cell.position.x - center.x).abs() <= 2);
                    assert!((cell.position.y - center.y).abs() <= 2);
                    let BattleArtilleryEffect::Damage {
                        total,
                        packet_size: 2,
                        table: BattleHitTable::Punch,
                    } = cell.effect
                    else {
                        panic!("Unexpected cluster effect")
                    };
                    assert!(total.is_multiple_of(2));
                    total_damage += total;
                }
                assert_eq!(total_damage, 40);
            }
        }
    }
}

/// Misses scatter at arrival with current wind, clamp to map bounds and retain the original aim point.
#[test]
fn artillery_scatter_and_atomic_invalid_arrival() {
    let center = BattleHexCoordinate { x: 10, y: 10 };
    for seed in 0..32 {
        let mut flight = BattleArtilleryFlight::new(
            center,
            center,
            BattleWeapon::LongTom,
            BattleArtilleryMode::Standard,
            false,
        )
        .unwrap();
        let mut dice = BattleDice::seeded([seed; 32]);
        for _ in 0..9 {
            assert!(flight.advance((20, 20), 0, &mut dice).unwrap().is_none());
        }
        for (dimensions, wind) in [
            ((0, 20), 0),
            ((1001, 20), 0),
            ((1, 1), 0),
            ((20, 20), 32768),
        ] {
            let before = flight.clone();
            let before_dice = dice.clone();
            assert!(flight.advance(dimensions, wind, &mut dice).is_err());
            assert_eq!(flight, before);
            assert_eq!(dice, before_dice);
        }
        let mut windy = flight.clone();
        let mut windy_dice = dice.clone();
        let calm = arrive(&mut flight, (20, 20), 0, &mut dice);
        let gust = arrive(&mut windy, (20, 20), 32767, &mut windy_dice);
        assert!(calm.missed && gust.missed);
        assert_eq!(calm.target, center);
        assert!((calm.impact.x - center.x).abs() <= 7 && (calm.impact.y - center.y).abs() <= 7);
        assert!(
            gust.impact.x == 0 || gust.impact.x == 19 || gust.impact.y == 0 || gust.impact.y == 19
        );
        assert_eq!(dice, windy_dice);
    }
    let center = BattleHexCoordinate { x: 0, y: 0 };
    let mut flight = BattleArtilleryFlight::new(
        center,
        center,
        BattleWeapon::Thumper,
        BattleArtilleryMode::Standard,
        false,
    )
    .unwrap();
    let pattern = arrive(&mut flight, (1, 1), 0, &mut BattleDice::seeded([3; 32]));
    assert!(pattern.missed);
    assert_eq!(pattern.impact, pattern.target);
    assert_eq!(pattern.cells.len(), 1);
}

/// Artillery uses map-sheet range, direct/indirect skill arithmetic and truncation toward zero.
#[test]
fn artillery_aim_ranges_observers_and_corrections() {
    let weapons = [
        (BattleWeapon::ArrowIv, 100),
        (BattleWeapon::ClanArrowIv, 120),
        (BattleWeapon::LongTom, 400),
        (BattleWeapon::Sniper, 240),
        (BattleWeapon::Thumper, 280),
        (BattleWeapon::LongTomCannon, 400),
        (BattleWeapon::SniperCannon, 240),
        (BattleWeapon::ThumperCannon, 280),
    ];
    for (weapon, maximum) in weapons {
        for extended_range in [false, true] {
            let base = BattleArtilleryAimInput {
                distance: f64::from(maximum),
                extended_range,
                submerged: false,
                visible: false,
                gunnery: 4,
                observer: BattleArtilleryObserver::Unassisted,
                adjustment: 0,
            };
            assert_eq!(
                weapon.artillery_aim(base).unwrap(),
                BattleArtilleryAim {
                    target_number: 12,
                    maximum_range: maximum,
                    range: BattleArtilleryRange::InRange
                }
            );
            for (observer, indirect) in [
                (BattleArtilleryObserver::Unassisted, 12),
                (BattleArtilleryObserver::Unavailable, 11),
                (BattleArtilleryObserver::Spotting(0), 9),
                (BattleArtilleryObserver::Spotting(1), 10),
                (BattleArtilleryObserver::Spotting(3), 11),
                (BattleArtilleryObserver::Spotting(5), 11),
                (BattleArtilleryObserver::Spotting(7), 12),
                (BattleArtilleryObserver::Spotting(8), 13),
            ] {
                for adjustment in [0, 1, 255] {
                    for visible in [false, true] {
                        let result = weapon
                            .artillery_aim(BattleArtilleryAimInput {
                                observer,
                                adjustment,
                                visible,
                                ..base
                            })
                            .unwrap();
                        assert_eq!(
                            result.target_number,
                            if visible { 9 } else { indirect } - i32::from(adjustment)
                        );
                    }
                }
            }
            let out = weapon
                .artillery_aim(BattleArtilleryAimInput {
                    distance: f64::from(maximum) + 0.001,
                    adjustment: 255,
                    ..base
                })
                .unwrap();
            assert_eq!(out.range, BattleArtilleryRange::OutOfRange);
            assert_eq!(out.target_number, 1000);
            let submerged = weapon
                .artillery_aim(BattleArtilleryAimInput {
                    submerged: true,
                    distance: 2000.0,
                    ..base
                })
                .unwrap();
            assert_eq!(submerged.range, BattleArtilleryRange::Underwater);
            assert_eq!(submerged.target_number, 5000);
            for distance in [f64::NAN, f64::INFINITY, -1.0] {
                assert!(
                    weapon
                        .artillery_aim(BattleArtilleryAimInput { distance, ..base })
                        .is_err()
                );
            }
            assert!(BattleWeapon::MediumLaser.artillery_aim(base).is_err());
        }
        assert!(weapon.range_modifier(1.0, false).is_err());
        assert!(weapon.damage_groups(None).is_err());
        assert_eq!(weapon.gunnery_skill(false), "Gunnery-Artillery");
    }
}

/// Artillery construction uses the same structural validation as other launchers.
#[test]
fn artillery_catalogue_admits_delayed_launchers() {
    let mut template = BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
    let section = template
        .sections
        .get_mut(&BattleSection::LeftTorso)
        .unwrap();
    section.criticals.clear();
    for slot in 0..12 {
        section.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: BattleWeapon::ClanArrowIv.name().into(),
                data: "-".into(),
                modes: vec![],
                brand: None,
            },
        );
    }
    let loadout = BattleLoadout::resolve(&template).unwrap();
    assert!(
        loadout
            .weapons
            .iter()
            .any(|mount| mount.weapon == BattleWeapon::ClanArrowIv)
    );
    for section in template.sections.values_mut() {
        section
            .criticals
            .retain(|_, part| part.equipment != "JumpJet");
    }
    template.jump_speed = 0.0;
    assert!(BattleUnit::from_template(template.clone()).is_ok());
}
