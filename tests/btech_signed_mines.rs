//! Signed mine strengths preserve shared blast geometry, heat checks and durable burn timers.
use crate::support;
use stompymux_rs::*;

/// Ordinary blast rules, with both mobile-vehicle fire policies exercised below.
fn rules(advanced_fire: bool) -> BattleFallRules {
    BattleFallRules {
        vehicle_impact: BattleVehicleImpactRules {
            advanced_fire,
            ..BattleVehicleImpactRules::STANDARD
        },
        stacking: BattleStackingRules::STANDARD,
        stagger: BattleStaggerMode::Retain,
        hit: BattleHitRules {
            inferno_penalty: false,
            exile_stun_mode: 0,
        },
        extended_piloting: true,
        toughness: false,
    }
}

#[tokio::test]
async fn signed_mines_share_burn_adjustments_neighbor_effects_and_restart() {
    for chassis in [
        "biped",
        "quad",
        "tracked",
        "wheeled",
        "hover",
        "stationary",
        "vtol",
    ] {
        let (_dir, config, mut base) = support::isolated_world().await;
        let map = base.create(&config, "Signed mine field".into(), Kind::Room);
        create_battle_map(
            &mut base,
            map,
            "signed",
            BattleMapAsset::from_cells("3 3\n.0.0.0\n\"0.0.0\n.0.0.0\n").unwrap(),
        )
        .unwrap();
        let id = base.create(&config, "Blast target".into(), Kind::Thing);
        base.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        let mech = matches!(chassis, "biped" | "quad");
        if mech {
            create_battle_unit(
                &mut base,
                id,
                BattleTemplate::parse(
                    "test",
                    if chassis == "quad" {
                        include_str!("../game/mechs/SCP-1N.toml")
                    } else {
                        include_str!("../game/mechs/JR7-D.toml")
                    },
                )
                .unwrap(),
            )
            .unwrap();
        } else {
            let text = match chassis {
                "vtol" => include_str!("../game/mechs/Kestrel.toml").to_owned(),
                "stationary" => include_str!("../game/mechs/Demolisher.toml")
                    .replace("movement = \"track\"", "movement = \"none\"")
                    .replace("walk_mp = 5", "walk_mp = 0"),
                "wheeled" => include_str!("../game/mechs/Demolisher.toml")
                    .replace("movement = \"track\"", "movement = \"wheel\""),
                "hover" => include_str!("../game/mechs/Demolisher.toml")
                    .replace("movement = \"track\"", "movement = \"hover\""),
                _ => include_str!("../game/mechs/Demolisher.toml").to_owned(),
            };
            create_battle_vehicle(
                &mut base,
                id,
                BattleVehicleTemplate::parse("test", &text).unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut base, id, map, 1, 1).unwrap();
        for kind in [
            BattleMineKind::Standard,
            BattleMineKind::Inferno,
            BattleMineKind::Command,
            BattleMineKind::Vibra,
        ] {
            for strength in [i16::MIN, -2, -1, 0] {
                for initial in [0_u32, 60] {
                    for advanced in [false, true] {
                        let mut world = base.clone();
                        let mut state = serde_json::to_value(&world.btech).unwrap();
                        let key = if mech { "constructed" } else { "vehicles" };
                        state[key][id.0.to_string()]["inferno_remaining"] = initial.into();
                        world.btech = serde_json::from_value(state).unwrap();
                        let mine = BattleMinefield {
                            coordinate: BattleHexCoordinate { x: 1, y: 1 },
                            kind,
                            strength,
                            extra: 42,
                            owner: ObjectId(1),
                        };
                        set_minefield(&mut world, map, 0, Some(mine)).unwrap();
                        let before = world.clone();
                        let report =
                            resolve_mine_blast(&mut world, map, 0, rules(advanced)).unwrap();
                        assert_eq!(report.hits.len(), 1, "{chassis} {kind:?} {strength}");
                        let hit = &report.hits[0];
                        assert_eq!(hit.damage, 0);
                        assert!(hit.impacts.is_empty());
                        assert_eq!(report.removed, [0]);
                        let neighbors =
                            matches!(kind, BattleMineKind::Command | BattleMineKind::Vibra)
                                && strength <= -2;
                        assert_eq!(
                            report.ignited,
                            if neighbors {
                                vec![BattleHexCoordinate { x: 0, y: 1 }]
                            } else {
                                vec![]
                            }
                        );
                        if mech || chassis == "stationary" {
                            let adjustment = if kind == BattleMineKind::Inferno {
                                i64::from(strength) * 6
                            } else {
                                0
                            };
                            let expected = if adjustment == 0 {
                                initial
                            } else {
                                (i64::from(initial) + adjustment).max(1) as u32
                            };
                            assert_eq!(hit.burn_seconds, adjustment);
                            let mut expected_state = serde_json::to_value(&before.btech).unwrap()
                                [key][id.0.to_string()]
                            .clone();
                            expected_state["inferno_remaining"] = expected.into();
                            assert_eq!(
                                serde_json::to_value(&world.btech).unwrap()[key][id.0.to_string()],
                                expected_state
                            );
                        } else {
                            // Mobile fire checks depend on policy and dice, not the signed heat amount.
                            let mut zero = before.clone();
                            let exposure = resolve_battle_vehicle_heat_exposure(
                                &mut zero,
                                id,
                                0,
                                rules(advanced).vehicle_impact,
                            )
                            .unwrap();
                            assert_eq!(hit.vehicle_heat.as_ref(), Some(&exposure));
                            assert_eq!(world.btech.vehicles()[&id], zero.btech.vehicles()[&id]);
                        }
                        let mut replay = before;
                        assert_eq!(
                            resolve_mine_blast(&mut replay, map, 0, rules(advanced)).unwrap(),
                            report
                        );
                        assert_eq!(replay.btech, world.btech);
                        world.validate(&config).unwrap();
                        if kind == BattleMineKind::Inferno && strength == -2 && !advanced {
                            persistence::save(&config.database(), &world).await.unwrap();
                            let restored = persistence::load(&config.database()).await.unwrap();
                            assert_eq!(restored.btech, world.btech);
                        }
                    }
                }
            }
        }
    }
}
