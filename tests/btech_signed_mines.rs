//! Signed mine strengths preserve shared blast geometry, heat checks and durable burn timers.
use crate::support;
use stompymux_rs::*;

/// Ordinary blast rules.
fn rules() -> FallRules {
    FallRules {
        vehicle_impact: VehicleImpactRules::STANDARD,
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
            MapAsset::from_cells("3 3\n.0.0.0\n\"0.0.0\n.0.0.0\n").unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut base, map, support::FIXTURE_DICE_SEED);
        let id = base.create(&config, "Blast target".into(), Kind::Thing);
        base.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        let mech = matches!(chassis, "biped" | "quad");
        if mech {
            create_battle_unit(
                &mut base,
                id,
                MechTemplate::parse(
                    "test",
                    if chassis == "quad" {
                        include_str!("../game/units/SCP-1N.toml")
                    } else {
                        include_str!("../game/units/JR7-D.toml")
                    },
                )
                .unwrap(),
            )
            .unwrap();
            support::seed_object_dice(&mut base, id, support::FIXTURE_DICE_SEED);
        } else {
            let text = match chassis {
                "vtol" => include_str!("../game/units/Kestrel.toml").to_owned(),
                "stationary" => include_str!("../game/units/Demolisher.toml")
                    .replace("movement = \"track\"", "movement = \"none\"")
                    .replace("engine_rating = 240\n", "")
                    .replace("walk_mp = 3", "walk_mp = 0"),
                "wheeled" => include_str!("../game/units/Demolisher.toml")
                    .replace("movement = \"track\"", "movement = \"wheel\"")
                    .replace("engine_rating = 240", "engine_rating = 220"),
                "hover" => crate::support::templates::hovercraft(include_str!(
                    "../game/units/Demolisher.toml"
                )),
                _ => include_str!("../game/units/Demolisher.toml").to_owned(),
            };
            create_battle_vehicle(
                &mut base,
                id,
                VehicleTemplate::parse("test", &text).unwrap(),
            )
            .unwrap();
            support::seed_object_dice(&mut base, id, support::FIXTURE_DICE_SEED);
        }
        place_battle_unit(&mut base, id, map, 1, 1).unwrap();
        for kind in [
            MineKind::Standard,
            MineKind::Inferno,
            MineKind::Command,
            MineKind::Vibra,
        ] {
            for strength in [i16::MIN, -2, -1, 0] {
                // Only a stationary unit carries a jelly duration; Mechs take infernos as heat.
                for initial in if mech { &[0_u32][..] } else { &[0, 60] } {
                    let initial = *initial;
                    for advanced in [false, true] {
                        let mut world = base.clone();
                        let key = if mech { "constructed" } else { "vehicles" };
                        if !mech {
                            let mut state = serde_json::to_value(&world.btech).unwrap();
                            state[key][id.0.to_string()]["inferno_remaining"] = initial.into();
                            world.btech = serde_json::from_value(state).unwrap();
                        }
                        let mine = Minefield {
                            coordinate: HexCoordinate { x: 1, y: 1 },
                            kind,
                            strength,
                            extra: 42,
                            owner: ObjectId(1),
                        };
                        set_minefield(&mut world, map, 0, Some(mine)).unwrap();
                        let before = world.clone();
                        let report = resolve_mine_blast(&mut world, map, 0, rules()).unwrap();
                        assert_eq!(report.hits.len(), 1, "{chassis} {kind:?} {strength}");
                        let hit = &report.hits[0];
                        assert_eq!(hit.damage, 0);
                        assert!(hit.impacts.is_empty());
                        assert_eq!(report.removed, [0]);
                        // Only an inferno mine lights its hex, and this field is clear ground.
                        assert!(report.ignited.is_empty());
                        if mech || chassis == "stationary" {
                            // A spent inferno field has no missiles left for a 'Mech.
                            let adjustment = if kind == MineKind::Inferno && !mech {
                                i64::from(strength) * 6
                            } else {
                                0
                            };
                            assert_eq!(hit.heat, 0);
                            let expected = if adjustment == 0 {
                                initial
                            } else {
                                (i64::from(initial) + adjustment).max(1) as u32
                            };
                            assert_eq!(hit.burn_seconds, adjustment);
                            let mut expected_state = serde_json::to_value(&before.btech).unwrap()
                                [key][id.0.to_string()]
                            .clone();
                            if !mech {
                                expected_state["inferno_remaining"] = expected.into();
                            }
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
                                rules().vehicle_impact,
                            )
                            .unwrap();
                            assert_eq!(hit.vehicle_heat.as_ref(), Some(&exposure));
                            assert_eq!(world.btech.vehicles()[&id], zero.btech.vehicles()[&id]);
                        }
                        let mut replay = before;
                        assert_eq!(
                            resolve_mine_blast(&mut replay, map, 0, rules()).unwrap(),
                            report
                        );
                        assert_eq!(replay.btech, world.btech);
                        world.validate(&config).unwrap();
                        if kind == MineKind::Inferno && strength == -2 && !advanced {
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
