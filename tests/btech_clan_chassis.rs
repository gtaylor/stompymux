//! Clan chassis technology derives sink groups, containment and construction mass from owned facts.
use crate::support;
use stompymux_rs::*;

/// A complete Clan biped with two external double sinks and ordinary fusion construction.
fn definition() -> BattleTemplate {
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    template
        .attributes
        .insert("specials".into(), "Clan FlipArms".into());
    template.heat_sinks = 24;
    for section in template.sections.values_mut() {
        section.criticals.retain(|_, p| p.equipment != "HeatSink");
    }
    for slot in 2..6 {
        template
            .sections
            .get_mut(&BattleSection::LeftTorso)
            .unwrap()
            .criticals
            .insert(
                slot,
                CriticalDefinition {
                    equipment: "HeatSink".into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
    }
    template
}

/// Two-slot loss removes two units of cooling; seven material slots activate Clan construction.
#[test]
fn clan_sink_groups_containment_and_material_mass() {
    let template = definition();
    assert!(template.has_double_heat_sinks());
    assert_eq!(template.heat_sink_slots(), 2);
    let original = BattleUnit::from_template(template.clone()).unwrap();
    for section in original.sections().keys() {
        assert!(original.has_case(*section));
    }
    for slot in 2..6 {
        let mut unit = original.clone();
        unit.destroy_critical(CriticalLocation {
            section: BattleSection::LeftTorso,
            slot,
        })
        .unwrap();
        assert_eq!(unit.system_hits(BattleSystem::HeatSink), 2);
        assert_eq!(unit.heat_rates(&World::default()).dissipation, 22.0);
        assert_eq!(
            unit.mass().unwrap().equipment,
            original.mass().unwrap().equipment - 1024
        );
        let pair = if slot < 4 { [2, 3] } else { [4, 5] };
        for slot in pair {
            assert!(unit.lost_criticals().contains(&CriticalLocation {
                section: BattleSection::LeftTorso,
                slot
            }));
        }
    }
    let mut incomplete = template.clone();
    incomplete
        .sections
        .get_mut(&BattleSection::LeftTorso)
        .unwrap()
        .criticals
        .remove(&3);
    assert!(BattleUnit::from_template(incomplete).is_err());
    for count in [6, 7] {
        let mut advanced = template.clone();
        for (section, name, start) in [
            (BattleSection::RightTorso, "EndoSteel", 3),
            (BattleSection::LeftArm, "FerroFibrous", 4),
        ] {
            for slot in start..start + count {
                advanced
                    .sections
                    .get_mut(&section)
                    .unwrap()
                    .criticals
                    .insert(
                        slot,
                        CriticalDefinition {
                            equipment: name.into(),
                            data: "-".into(),
                            modes: vec![],
                        },
                    );
            }
        }
        let unit = BattleUnit::from_template(advanced).unwrap();
        assert_eq!(
            unit.mass().unwrap().structure,
            if count == 7 { 2048 } else { 3584 }
        );
        assert_eq!(
            unit.mass().unwrap().armor,
            if count == 7 { 3584 } else { 4096 }
        );
    }
}

/// Clan built-in CASE contains a selected torso magazine explosion and replays after restart.
#[tokio::test]
async fn clan_ammunition_containment_and_sink_losses_survive_restart() {
    let (_dir, config, mut base) = support::isolated_world().await;
    let id = base.create(&config, "Clan target".into(), Kind::Thing);
    base.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(&mut base, id, definition()).unwrap();
    support::seed_object_dice(&mut base, id, support::FIXTURE_DICE_SEED);
    let mut corrupt = base.clone();
    corrupt
        .btech
        .rewrite_unit_record(id, |record| {
            record["lost_criticals"] = serde_json::json!([{"section":"LeftTorso","slot":2}]);
        })
        .unwrap();
    assert!(corrupt.validate(&config).is_err());
    destroy_battle_critical(
        &mut base,
        id,
        CriticalLocation {
            section: BattleSection::LeftTorso,
            slot: 3,
        },
    )
    .unwrap();
    let mut found = false;
    for seed in 0..=u8::MAX {
        base.btech
            .rewrite_unit_record(id, |record| {
                record["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
            })
            .unwrap();
        let hit = BattleHit {
            section: BattleSection::RightTorso,
            rear_armor: false,
            through_armor_critical: true,
            crew_stun: false,
        };
        let mut damaged = base.clone();
        let report = resolve_battle_impact(&mut damaged, id, hit, 1).unwrap();
        if !report.criticals.iter().any(|(_, loss)| {
            matches!(
                loss,
                BattleCriticalLoss::Ammunition {
                    explosion_damage: 200,
                    ..
                }
            )
        }) {
            continue;
        }
        let before = &base.btech.constructed_units()[&id];
        let after = &damaged.btech.constructed_units()[&id];
        assert_eq!(after.sections()[&BattleSection::RightTorso].internal, 0);
        assert_eq!(
            after.sections()[&BattleSection::CenterTorso],
            before.sections()[&BattleSection::CenterTorso]
        );
        assert!(!after.is_destroyed());
        persistence::save(&config.database(), &base).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            resolve_battle_impact(&mut restored, id, hit, 1).unwrap(),
            report
        );
        assert_eq!(restored.btech, damaged.btech);
        found = true;
        break;
    }
    assert!(found);
}

/// Representative unchanged game assets exercise Clan sinks, engines, materials and weapon families together.
#[test]
fn clan_game_assets_construct_without_rewriting_templates() {
    for (source, cooling) in [
        (include_str!("../game/mechs/MadCat-A.toml"), 40.0),
        (include_str!("../game/mechs/Vulture-C.toml"), 24.0),
        (include_str!("../game/mechs/Vixen-1.toml"), 20.0),
    ] {
        let unit =
            BattleUnit::from_template(BattleTemplate::parse("test", source).unwrap()).unwrap();
        assert_eq!(unit.engine().unwrap(), BattleEngine::Xl);
        assert_eq!(unit.heat_rates(&World::default()).dissipation, cooling);
        assert!(
            unit.sections()
                .keys()
                .all(|&section| unit.has_case(section))
        );
        assert!(unit.mass().unwrap().total > 0);
        let restored: BattleUnit =
            serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored, unit);
    }
}

/// Laser heat sinks are Clan technology; on a Clan chassis the designation keeps the double
/// sink rules.
#[test]
fn laser_sink_designation_preserves_double_sink_behavior() {
    let mut inner_sphere =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let specials = inner_sphere
        .attributes
        .entry("specials".into())
        .or_default();
    if specials == "-" {
        specials.clear();
    }
    specials.push_str(" LaserHS_Tech");
    assert!(
        BattleUnit::from_template(inner_sphere)
            .unwrap_err()
            .to_string()
            .contains("Laser heat sinks are Clan technology")
    );
    {
        let mut template = definition();
        let baseline = BattleUnit::from_template(template.clone()).unwrap();
        let specials = template.attributes.entry("specials".into()).or_default();
        specials.push_str(" LaserHS_Tech");
        let mut unit = BattleUnit::from_template(template).unwrap();
        assert_eq!(unit.mass().unwrap(), baseline.mass().unwrap());
        assert_eq!(
            unit.heat_rates(&World::default()),
            baseline.heat_rates(&World::default())
        );
        assert_eq!(
            unit.definition().heat_sink_slots(),
            baseline.definition().heat_sink_slots()
        );
        let sink = unit
            .loadout()
            .unwrap()
            .systems
            .iter()
            .find(|part| part.system == BattleSystem::HeatSink)
            .unwrap()
            .location;
        let mut ordinary = baseline;
        unit.destroy_critical(sink).unwrap();
        ordinary.destroy_critical(sink).unwrap();
        assert_eq!(unit.lost_criticals(), ordinary.lost_criticals());
        assert_eq!(unit.mass().unwrap(), ordinary.mass().unwrap());
        assert_eq!(
            unit.heat_rates(&World::default()),
            ordinary.heat_rates(&World::default())
        );
        let restored: BattleUnit =
            serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored, unit);
        assert!(restored.definition().attributes["specials"].contains("LaserHS_Tech"));
    }
}

/// Unchanged Night Gyr assets exercise the laser-sink designation with actual Clan equipment.
#[tokio::test]
async fn night_gyr_laser_sink_assets_construct_and_replay_damage() {
    let (_dir, config, mut world) = support::isolated_world().await;
    for source in [
        include_str!("../game/mechs/NightGyr-Prime.toml"),
        include_str!("../game/mechs/NightGyr-A.toml"),
        include_str!("../game/mechs/NightGyr-C.toml"),
        include_str!("../game/mechs/NightGyr-D.toml"),
    ] {
        let template = BattleTemplate::parse("test", source).unwrap();
        let cooling = f64::from(template.heat_sinks);
        let id = world.create(&config, template.name.clone(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(&mut world, id, template).unwrap();
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.heat_rates(&world).dissipation, cooling);
        // Some variants fit all cooling inside the engine and have no external sink.
        if let Some(sink) = unit
            .loadout()
            .unwrap()
            .systems
            .iter()
            .find(|part| part.system == BattleSystem::HeatSink)
            .map(|part| part.location)
        {
            destroy_battle_critical(&mut world, id, sink).unwrap();
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(unit.system_hits(BattleSystem::HeatSink), 2);
            assert_eq!(unit.heat_rates(&world).dissipation, cooling - 2.0);
        }
    }
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    restored.validate(&config).unwrap();
}

/// Recognizing a designation does not bypass incomplete equipment checks.
#[test]
fn laser_sink_designation_does_not_accept_incomplete_clan_sinks() {
    let mut template =
        BattleTemplate::parse("NightGyr-B", include_str!("../game/mechs/NightGyr-B.toml")).unwrap();
    assert!(BattleUnit::from_template(template.clone()).is_ok());
    template
        .sections
        .get_mut(&BattleSection::LeftArm)
        .unwrap()
        .criticals
        .remove(&8);
    assert!(
        BattleUnit::from_template(template)
            .unwrap_err()
            .to_string()
            .contains("Incomplete heat sink installation")
    );
}

/// Declared cooling below twenty is preserved for Clan sinks instead of inventing additional dissipation.
#[test]
fn low_capacity_clan_cooling_constructs_and_replays() {
    for source in [
        include_str!("../game/mechs/SnowFox-1.toml"),
        include_str!("../game/mechs/SnowFox-2.toml"),
    ] {
        let template = BattleTemplate::parse("test", source).unwrap();
        for capacity in [10, 12, 14, 16, 18, 20] {
            let mut adjusted = template.clone();
            adjusted.heat_sinks = capacity;
            let unit = BattleUnit::from_template(adjusted).unwrap();
            assert!(unit.definition().has_double_heat_sinks());
            assert_eq!(
                unit.heat_rates(&World::default()).dissipation,
                f64::from(capacity)
            );
            let restored: BattleUnit =
                serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
            assert_eq!(restored.mass().unwrap(), unit.mass().unwrap());
            assert_eq!(
                restored.heat_rates(&World::default()),
                unit.heat_rates(&World::default())
            );
        }
        let mut invalid = template.clone();
        invalid.heat_sinks = 11;
        assert!(BattleUnit::from_template(invalid).is_err());
        let mut invalid = template;
        invalid.heat_sinks = 8;
        assert!(BattleUnit::from_template(invalid).is_err());
    }
}
