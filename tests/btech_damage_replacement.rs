//! Shared material replacement plans resolve against installed equipment without touching runtime state.
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
use crate::support;

/// Replacement is complete, ordered and independent of the current damaged material.
#[tokio::test]
async fn replacement_material_is_shared_across_all_chassis() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, _, _) = firing::fixture_with_supply(
            &source,
            Some(BattleWeapon::Mml3),
            &source,
            false,
            Some(""),
        )
        .await;
        let mech = world.btech.constructed_units().contains_key(&id);
        let ammo_slot = if mech {
            BattleWeapon::Mml3.profile().critical_slots
        } else {
            1
        };
        let bin = BattleDamageSlot {
            section: 2,
            slot: ammo_slot,
        };
        let weapon = BattleDamageSlot {
            section: 2,
            slot: 0,
        };
        let intact = prepare_battle_damage_field(&world, id, "").unwrap();
        firing::edit(&mut world, id, |state| {
            let section = if mech { "LeftTorso" } else { "front" };
            let armor = state["sections"][section]["armor"].as_u64().unwrap();
            state["sections"][section]["armor"] = (armor - 1).into();
        });
        world.validate(&config).unwrap();
        let before = world.btech.clone();
        assert_eq!(prepare_battle_damage_field(&world, id, "").unwrap(), intact);
        let text =
            format!("A:2/2147483647,A:2/1,I:2/1,R:2/{ammo_slot}(3),R:2/{ammo_slot}(1),G:2/0(7)");
        let replacement = prepare_battle_damage_field(&world, id, &text).unwrap();
        assert_eq!(
            replacement.sections[&2].armor,
            intact.sections[&2].armor - 1
        );
        assert_eq!(
            replacement.sections[&2].internal,
            intact.sections[&2].internal - 1
        );
        assert_eq!(replacement.ammunition[&bin], intact.ammunition[&bin] - 1);
        assert_eq!(replacement.failures[&weapon], 7);
        if mech && ammo_slot > 1 {
            let multi = prepare_battle_damage_field(&world, id, "G:2/0(7),G:2/1(2)").unwrap();
            assert_eq!(multi.failures, [(weapon, 2)].into_iter().collect());
        }

        let lost =
            prepare_battle_damage_field(&world, id, &format!("C:2/{ammo_slot},G:2/0(7),C:2/0"))
                .unwrap();
        assert_eq!(lost.ammunition[&bin], 0);
        assert!(lost.destroyed_criticals.contains(&weapon));
        assert!(lost.failures.is_empty());
        let original = &intact.sections[&2];
        let section_loss = prepare_battle_damage_field(
            &world,
            id,
            &format!(
                "A:2/{},A(R):2/{},I:2/{}",
                original.armor, original.rear, original.internal
            ),
        )
        .unwrap();
        assert_eq!(section_loss.ammunition[&bin], 0);
        assert_eq!(section_loss.sections[&2].internal, 0);
        let cleared = prepare_battle_damage_field(&world, id, "G:2/0(7),G:2/0(0)").unwrap();
        assert!(cleared.failures.is_empty());
        for invalid in [
            "A:2/-1".into(),
            "A:2/2147483647".into(),
            "R:2/0(1)".into(),
            format!("G:2/{ammo_slot}(1)"),
            "G:2/0(8)".into(),
            "C:2/11".into(),
            format!("I:2/{}", original.internal),
            format!("R:2/{ammo_slot}(-1)"),
            format!("R:2/{ammo_slot}(2147483647)"),
        ] {
            assert!(
                prepare_battle_damage_field(&world, id, &invalid).is_err(),
                "{invalid}"
            );
        }
        if !mech {
            assert!(prepare_battle_damage_field(&world, id, "A:7/1").is_err());
        }
        assert_eq!(world.btech, before);
    }
}

/// Whole heat-sink and improved-jet installations use the same loss grouping as combat.
#[tokio::test]
async fn replacement_critical_groups_and_weapon_failures_resolve_installed_slots() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let mut template = BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
    for section in template.sections.values_mut() {
        section
            .criticals
            .retain(|_, part| !matches!(part.equipment.as_str(), "HeatSink" | "JumpJet"));
    }
    template.attributes.insert(
        "specials".into(),
        "FlipArms DoubleHS ImprovedJJ_Tech".into(),
    );
    for slot in 0..10 {
        template
            .sections
            .get_mut(&BattleSection::LeftTorso)
            .unwrap()
            .criticals
            .insert(
                slot,
                CriticalDefinition {
                    equipment: "JumpJet".into(),
                    data: "-".into(),
                    modes: Vec::new(),
                    brand: None,
                },
            );
    }
    for slot in 6..9 {
        template
            .sections
            .get_mut(&BattleSection::RightArm)
            .unwrap()
            .criticals
            .insert(
                slot,
                CriticalDefinition {
                    equipment: "HeatSink".into(),
                    data: "-".into(),
                    modes: Vec::new(),
                    brand: None,
                },
            );
    }
    let id = world.create(&config, "Replacement groups".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(&mut world, id, template).unwrap();
    let before = world.btech.clone();
    let replacement = prepare_battle_damage_field(&world, id, "C:2/0,C:1/7").unwrap();
    assert_eq!(
        replacement.destroyed_criticals,
        [
            BattleDamageSlot {
                section: 2,
                slot: 0
            },
            BattleDamageSlot {
                section: 2,
                slot: 1
            },
            BattleDamageSlot {
                section: 1,
                slot: 6
            },
            BattleDamageSlot {
                section: 1,
                slot: 7
            },
            BattleDamageSlot {
                section: 1,
                slot: 8
            },
        ]
        .into_iter()
        .collect()
    );
    assert!(prepare_battle_damage_field(&world, id, "G:2/0(1)").is_ok());
    assert_eq!(world.btech, before);
}

/// A live report prepares its current protection and ammunition rather than accumulating losses.
#[tokio::test]
async fn preparing_current_reports_recovers_current_material() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let mech = world.btech.constructed_units().contains_key(&id);
        firing::edit(&mut world, id, |state| {
            let section = if mech { "LeftTorso" } else { "front" };
            let armor = state["sections"][section]["armor"].as_u64().unwrap();
            state["sections"][section]["armor"] = (armor - 1).into();
            if let Some(rounds) = state["ammunition"]
                .as_array_mut()
                .and_then(|bins| bins.first_mut())
            {
                let remaining = rounds.as_u64().unwrap();
                if remaining > 0 {
                    *rounds = (remaining - 1).into();
                }
            }
        });
        world.validate(&config).unwrap();
        let (sections, rounds): (Vec<_>, Vec<_>) = if mech {
            let unit = &world.btech.constructed_units()[&id];
            (
                unit.sections().values().cloned().collect(),
                unit.ammunition().to_vec(),
            )
        } else {
            let unit = &world.btech.vehicles()[&id];
            (
                unit.sections().values().cloned().collect(),
                unit.ammunition().to_vec(),
            )
        };
        let before = world.btech.clone();
        let text = battle_unit_damage_field(&world, id).unwrap();
        let replacement = prepare_battle_damage_field(&world, id, &text).unwrap();
        let totals = |sections: Vec<BattleSectionState>| {
            sections
                .into_iter()
                .fold((0u32, 0u32, 0u32), |total, section| {
                    (
                        total.0 + u32::from(section.armor),
                        total.1 + u32::from(section.rear),
                        total.2 + u32::from(section.internal),
                    )
                })
        };
        assert_eq!(
            totals(replacement.sections.values().cloned().collect()),
            totals(sections)
        );
        assert_eq!(
            replacement
                .ammunition
                .values()
                .copied()
                .map(u32::from)
                .sum::<u32>(),
            rounds.into_iter().map(u32::from).sum::<u32>()
        );
        assert_eq!(world.btech, before);
    }
}
