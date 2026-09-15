//! Persisted drops share altitude ownership, scenario edits and deferred map cleanup across chassis.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// Author a drop through the same validated snapshot format used for restart.
fn insert_drop(world: &mut World, id: ObjectId) {
    firing::edit(world, id, |state| {
        state["orbital_drop"] =
            serde_json::to_value(BattleOrbitalDrop::new(35 * 1024, 300).unwrap()).unwrap();
        state["ground_elevation"] = serde_json::Value::Null;
    });
}

/// One accessor keeps every acceptance case independent of anatomy storage.
fn drop_state(world: &World, id: ObjectId) -> Option<BattleOrbitalDrop> {
    world
        .btech
        .constructed_units()
        .get(&id)
        .and_then(BattleUnit::orbital_drop)
        .or_else(|| {
            world
                .btech
                .vehicles()
                .get(&id)
                .and_then(BattleVehicle::orbital_drop)
        })
}

#[tokio::test]
async fn saved_drops_own_geometry_and_survive_scenario_relocation() {
    for source in firing::templates().into_iter().take(6) {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        insert_drop(&mut world, unit);
        if !world
            .btech
            .vehicles()
            .get(&unit)
            .is_some_and(|unit| unit.definition().movement == BattleVehicleMovement::Stationary)
        {
            firing::edit(&mut world, unit, |state| {
                state["motion"]["speed"] = 1.0.into();
                state["motion"]["desired_speed"] = 2.0.into();
            });
        }
        world.validate(&config).unwrap();
        assert_eq!(battle_unit_altitude(&world, unit).unwrap(), Some(300.0));
        assert_eq!(battle_unit_elevation(&world, unit).unwrap(), Some(300));
        advance_battle_jumps(&mut world, BattleMovementRules::STANDARD).unwrap();
        assert_eq!(battle_unit_elevation(&world, unit).unwrap(), Some(298));
        let before = world.btech.clone();
        advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        assert_eq!(world.btech, before);
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, world.btech);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
        let _: mlua::Table = scripts
            .eval_callback(&format!(
                "return btech.unit.setxy(1, {}, 0, 9, 123)",
                unit.0
            ))
            .unwrap();
        let moved = drop_state(&scripts.world(), unit).unwrap();
        let inspected: mlua::Table = scripts
            .eval_callback(&format!("return btech.unit.state({}).orbital_drop", unit.0))
            .unwrap();
        assert_eq!(
            serde_json::to_value(inspected).unwrap(),
            serde_json::to_value(moved).unwrap()
        );
        assert_eq!(moved.elevation(), 123);
        assert_eq!(
            moved.protection(),
            BattleDropProtection::Cocoon { integrity: 8 }
        );
        assert_eq!(
            battle_unit_altitude(&scripts.world(), unit).unwrap(),
            Some(123.0)
        );
        let before = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.setxy(1, {}, 0, 8, 222); error('abort')",
                    unit.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        scripts.world().validate(&config).unwrap();
    }
}

#[tokio::test]
async fn reassignment_preserves_drops_and_detached_update_retires_them() {
    for source in firing::templates().into_iter().take(6) {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        insert_drop(&mut world, unit);
        let destination = world.create(&config, "Drop destination".into(), Kind::Room);
        create_battle_map(
            &mut world,
            destination,
            "drop",
            BattleMapAsset::parse("1 2\n.0\n.0\n").unwrap(),
        )
        .unwrap();
        let report = reassign_battle_map(&mut world, unit, destination, None).unwrap();
        assert!(report.reset_origin);
        assert_eq!(drop_state(&world, unit).unwrap().elevation(), 300);
        world.validate(&config).unwrap();
        remove_battle_map_membership(&mut world, unit).unwrap();
        assert_eq!(battle_unit_altitude(&world, unit).unwrap(), None);
        let saved = serde_json::to_value(&world.btech).unwrap();
        world.btech = serde_json::from_value(saved).unwrap();
        assert!(drop_state(&world, unit).is_some());
        let notices = advance_battle_units(&mut world, 0);
        assert!(
            notices
                .iter()
                .any(|notice| notice.unit == unit && notice.text.contains("invalid map"))
        );
        assert!(drop_state(&world, unit).is_none());
        world.validate(&config).unwrap();
        reassign_battle_map(&mut world, unit, destination, None).unwrap();
        assert_eq!(battle_unit_altitude(&world, unit).unwrap(), Some(300.0));
    }
}

#[tokio::test]
async fn restart_rejects_competing_altitude_owners_and_aircraft_cocoons() {
    for (index, source) in firing::templates().into_iter().enumerate() {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let group = if index < 2 { "constructed" } else { "vehicles" };
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state[group][unit.0.to_string()]["orbital_drop"] =
            serde_json::to_value(BattleOrbitalDrop::new(1024, 300).unwrap()).unwrap();
        if index == 6 {
            assert!(serde_json::from_value::<BtechState>(state).is_err());
            continue;
        }
        for (field, value) in [
            ("ground_elevation", serde_json::json!(12.0)),
            (
                "free_fall",
                serde_json::to_value(BattleFreeFall::new(300)).unwrap(),
            ),
            (
                "orbital_drop",
                serde_json::json!({"elevation":300,"protection":{"state":"breached"}}),
            ),
        ] {
            let mut invalid = state.clone();
            invalid[group][unit.0.to_string()][field] = value;
            if let Ok(decoded) = serde_json::from_value::<BtechState>(invalid) {
                let mut candidate = world.clone();
                candidate.btech = decoded;
                assert!(candidate.validate(&config).is_err(), "{index}: {field}");
            }
        }
        world.btech = serde_json::from_value(state).unwrap();
        world.validate(&config).unwrap();
    }
}
