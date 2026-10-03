//! Temporary weapon conditions use the same state and recovery contract on every supported chassis.
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Observe either owned unit through the common temporary-failure contract.
fn failure(world: &World, id: ObjectId, index: usize) -> Option<BattleEquipmentFailure> {
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        assert!(!unit.weapon_readiness(index).unwrap().ready);
        return unit.weapon_failures().get(&index).copied();
    }
    let unit = &world.btech.vehicles()[&id];
    assert!(!unit.weapon_readiness(index).unwrap().ready);
    unit.weapon_failures().get(&index).copied()
}

#[tokio::test]
async fn conditions_preserve_material_and_recover_only_from_existing_clocks() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, _, index) =
            firing::fixture_with_target(&source, Some(BattleWeapon::Mml3), &source).await;
        firing::edit(&mut world, id, |state| {
            state["target_lock"] = serde_json::Value::Null;
        });
        let pristine = world.btech.clone();
        for code in 1..=7 {
            let condition = BattleEquipmentFailure::from_code(code).unwrap();
            set_battle_weapon_failure(&mut world, id, index, condition).unwrap();
            assert_eq!(failure(&world, id, index), condition);
            assert!(
                battle_unit_damage_field(&world, id)
                    .unwrap()
                    .contains(&format!("G:2/0({code})"))
            );
            let expected = match code {
                1 => BattleEquipmentCondition::Jammed,
                2 => BattleEquipmentCondition::Shorted,
                3 => BattleEquipmentCondition::Broken,
                4 => BattleEquipmentCondition::Empty,
                5 => BattleEquipmentCondition::Destroyed,
                _ => BattleEquipmentCondition::AmmoJam,
            };
            assert_eq!(
                battle_weapon_diagnostics(&world, id).unwrap()[index].condition,
                expected
            );
            let section = if world.btech.constructed_units().contains_key(&id) {
                "LT"
            } else {
                "Front"
            };
            assert_eq!(
                battle_critical_report(&world, id, section).unwrap().slots[0].condition,
                expected
            );

            world.validate(&config).unwrap();
            let checkpoint = world.btech.clone();
            assert!(advance_battle_recycle(&mut world).is_empty());
            assert_eq!(world.btech, checkpoint);
            world.btech =
                serde_json::from_value(serde_json::to_value(&world.btech).unwrap()).unwrap();
            assert_eq!(world.btech, checkpoint);
            assert!(set_battle_weapon_failure(&mut world, id, usize::MAX, condition).is_err());
            assert_eq!(world.btech, checkpoint);
            set_battle_weapon_failure(&mut world, id, index, None).unwrap();
            assert_eq!(world.btech, pristine);

            set_battle_weapon_failure(&mut world, id, index, condition).unwrap();
            firing::edit(&mut world, id, |state| {
                state["weapon_recycle"] = serde_json::json!({(index.to_string()):2});
                state["power"] = serde_json::to_value(BattlePower::Off).unwrap();
            });
            let paused = world.btech.clone();
            assert!(advance_battle_recycle(&mut world).is_empty());
            assert_eq!(world.btech, paused);
            firing::edit(&mut world, id, |state| {
                state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            });
            let mut notices = advance_battle_recycle(&mut world);
            if code != 5 {
                assert!(notices.is_empty());
                assert_eq!(failure(&world, id, index), condition);
                notices = advance_battle_recycle(&mut world);
            }
            assert_eq!(notices.len(), 1);
            assert!(notices[0].text.contains("is operational again"));
            assert_eq!(world.btech, pristine);
        }
    }
}

#[tokio::test]
async fn crew_recovery_clears_ordinary_feed_failures_and_preserves_critical_failures() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, _, index) = firing::fixture_with_supply(
            &source,
            Some(BattleWeapon::Mml3),
            &source,
            false,
            Some(""),
        )
        .await;
        set_battle_weapon_failure(
            &mut world,
            id,
            index,
            Some(BattleEquipmentFailure::CriticalAmmunitionJam),
        )
        .unwrap();
        let checkpoint = world.btech.clone();
        assert!(begin_battle_unjam(&mut world, id, ObjectId(1), index).is_err());
        assert_eq!(world.btech, checkpoint);
        set_battle_weapon_failure(
            &mut world,
            id,
            index,
            Some(BattleEquipmentFailure::AmmunitionJam),
        )
        .unwrap();
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        firing::edit(&mut world, id, |state| {
            state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        });
        begin_battle_unjam(&mut world, id, ObjectId(1), index).unwrap();
        for _ in 0..59 {
            assert!(
                advance_battle_unjamming(&mut world, false, false)
                    .unwrap()
                    .is_empty()
            );
        }
        let mut replay = world.clone();
        replay.btech = serde_json::from_value(serde_json::to_value(&world.btech).unwrap()).unwrap();
        let messages = advance_battle_unjamming(&mut world, false, false).unwrap();
        assert!(
            messages
                .iter()
                .any(|(_, text)| text.contains("manage to clear the jam"))
        );
        assert_eq!(
            messages,
            advance_battle_unjamming(&mut replay, false, false).unwrap()
        );
        assert_eq!(world.btech, replay.btech);
        world.validate(&config).unwrap();
        let failures = if let Some(unit) = world.btech.constructed_units().get(&id) {
            unit.weapon_failures()
        } else {
            world.btech.vehicles()[&id].weapon_failures()
        };
        assert!(failures.is_empty());
    }
}
