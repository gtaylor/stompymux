//! Mech material entries preserve diagnostic dice across phases, transfers and durable replay.
use crate::support;
use stompymux_rs::*;

/// A chosen location lets these scenarios isolate material dice from hit-table dice.
fn hit(section: MechSection) -> Hit {
    Hit {
        section,
        rear_armor: false,
        through_armor_critical: false,
        crew_stun: false,
    }
}

/// Each entered section rolls once; armor/internal phases share that entry.
#[tokio::test]
async fn biped_and_quad_material_entries_consume_exact_dice_and_replay() {
    for source in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/GOL-1H.toml"),
    ] {
        let (_dir, config, mut base) = support::isolated_world().await;
        let id = base.create(&config, "Material target".into(), Kind::Thing);
        base.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(&mut base, id, MechTemplate::parse("test", source).unwrap()).unwrap();
        support::seed_object_dice(&mut base, id, support::FIXTURE_DICE_SEED);
        for scenario in ["zero", "armor", "internal", "missing_sections", "overflow"] {
            let mut before = base.clone();
            let section = MechSection::LeftArm;
            if matches!(scenario, "internal" | "overflow") {
                apply_damage_phase(
                    &mut before,
                    id,
                    section,
                    u16::MAX,
                    DamagePhase::Armor { rear: false },
                )
                .unwrap();
            }
            if scenario == "overflow" {
                let internal = before.btech.constructed_units()[&id].sections()[&section].internal;
                apply_damage_phase(
                    &mut before,
                    id,
                    section,
                    internal - 1,
                    DamagePhase::Internal,
                )
                .unwrap();
            }
            if scenario == "missing_sections" {
                apply_damage_phase(
                    &mut before,
                    id,
                    MechSection::LeftTorso,
                    u16::MAX,
                    DamagePhase::Internal,
                )
                .unwrap();
            }
            // A low internal roll isolates entry accounting from equipment critical dice.
            let seed = (0..=255)
                .find(|value| {
                    let mut dice = Dice::seeded([*value; 32]);
                    dice.two_d6();
                    dice.two_d6() <= 7
                })
                .unwrap();
            before
                .btech
                .set_unit_dice(id, Dice::seeded([seed; 32]))
                .unwrap();
            before.validate(&config).unwrap();
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            let mut world = before.clone();
            let amount = match scenario {
                "zero" => 0,
                "overflow" => 2,
                _ => 1,
            };
            let report = resolve_battle_impact(&mut world, id, hit(section), amount).unwrap();
            assert_eq!(
                resolve_battle_impact(&mut restored, id, hit(section), amount).unwrap(),
                report
            );
            assert_eq!(restored.btech, world.btech);
            assert!(report.criticals.is_empty(), "{scenario}");
            let entries = match scenario {
                "missing_sections" | "overflow" => 3,
                "internal" => 2,
                _ => 1,
            };
            let mut dice = Dice::seeded([seed; 32]);
            for _ in 0..entries {
                dice.two_d6();
            }
            assert_eq!(
                roll_unit_dice(&mut world, id, 1).unwrap(),
                [dice.d6()],
                "{scenario}"
            );
            let expected_sections: Vec<_> = match scenario {
                "zero" => vec![],
                "missing_sections" => vec![MechSection::CenterTorso],
                "overflow" => vec![section, section, MechSection::LeftTorso],
                "internal" => vec![section, section],
                _ => vec![section],
            };
            assert_eq!(
                report
                    .phases
                    .iter()
                    .map(|phase| phase.section)
                    .collect::<Vec<_>>(),
                expected_sections,
                "{scenario}"
            );
            world.validate(&config).unwrap();
        }
    }
}
