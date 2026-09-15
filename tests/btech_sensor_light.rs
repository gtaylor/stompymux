//! Map light transitions preserve reference slot checks and transactional cockpit warnings.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[allow(dead_code)]
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

const WARNING: &str = "The light's kinda too bright now to use Light-amplification!";

/// Read the shared selection through either supported storage family.
fn selection(world: &World, unit: ObjectId) -> BattleSensorSelection {
    world.btech.constructed_units().get(&unit).map_or_else(
        || world.btech.vehicles()[&unit].sensor_selection(),
        |u| u.sensor_selection(),
    )
}

/// Configure active and pending slots without advancing unrelated combat state.
fn configure(world: &mut World, unit: ObjectId, pair: BattleSensorPair) {
    firing::edit(world, unit, |s| {
        s["sensor_selection"] = serde_json::to_value(BattleSensorSelection {
            active: pair,
            pending: Some(BattleSensorChange {
                wanted: BattleSensorPair::default(),
                remaining: 7,
            }),
        })
        .unwrap();
    });
}

/// All chassis recheck distinct slots once, retain pending requests/locks, and persist the result.
#[tokio::test]
async fn changed_light_rechecks_active_slots_without_clearing_locks() {
    use BattleSensorMode::{LightAmplification as L, Visual as V};
    for template in firing::templates() {
        let (_dir, config, base, unit, _, _) =
            firing::fixture_with_target(&template, None, &template).await;
        let map = base.btech.units()[&unit].map.unwrap();
        for running in [false, true] {
            for (primary, secondary, expected) in [
                (L, L, (V, L)),
                (L, V, (V, V)),
                (V, L, (V, V)),
                (V, V, (V, V)),
            ] {
                let mut world = base.clone();
                if !running {
                    stop_battle_unit(
                        &mut world,
                        unit,
                        ObjectId(1),
                        BattleMovementRules::STANDARD.fall,
                    )
                    .unwrap();
                }
                set_battle_map_visibility(&mut world, map, BattleLight::Night, 30).unwrap();
                configure(&mut world, unit, BattleSensorPair { primary, secondary });
                let before = world.btech.clone();
                let notices =
                    set_battle_map_visibility(&mut world, map, BattleLight::Day, 30).unwrap();
                let changed = primary == L || secondary == L;
                assert_eq!(notices.len(), usize::from(running && changed));
                if running && changed {
                    assert_eq!(
                        notices[0],
                        BattleNotice {
                            unit,
                            text: WARNING.into()
                        }
                    );
                }
                let pair = BattleSensorPair {
                    primary: expected.0,
                    secondary: expected.1,
                };
                assert_eq!(selection(&world, unit).active, pair);
                assert_eq!(selection(&world, unit).pending.unwrap().remaining, 7);
                // Compare every unit field except the deliberately changed active slots.
                let mut unchanged = world.clone();
                configure(
                    &mut unchanged,
                    unit,
                    BattleSensorPair { primary, secondary },
                );
                assert_eq!(
                    unchanged.btech.constructed_units(),
                    before.constructed_units()
                );
                assert_eq!(unchanged.btech.vehicles(), before.vehicles());
                assert!(
                    set_battle_map_visibility(&mut world, map, BattleLight::Day, 15)
                        .unwrap()
                        .is_empty()
                );
                assert_eq!(selection(&world, unit).active, pair);
                persistence::save(&config.database(), &world).await.unwrap();
                let restored = persistence::load(&config.database()).await.unwrap();
                assert_eq!(selection(&restored, unit), selection(&world, unit));
            }
        }
    }
}

/// Both Lua entry points and native map conditions publish the same cockpit-only consequences.
#[tokio::test]
async fn map_light_actions_publish_and_rollback_together() {
    for template in firing::templates() {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&template, None, &template).await;
        let map = world.btech.units()[&unit].map.unwrap();
        set_battle_map_visibility(&mut world, map, BattleLight::Night, 30).unwrap();
        configure(
            &mut world,
            unit,
            BattleSensorPair {
                primary: BattleSensorMode::LightAmplification,
                secondary: BattleSensorMode::Visual,
            },
        );
        let passenger = world.create(&config, "Passenger".into(), Kind::Player);
        world.objects.get_mut(&passenger).unwrap().location = Some(unit);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        for call in [
            format!("btech.map.conditions({},'day',30)", map.0),
            format!("btech.map.set_field(1,{},'maplight','2')", map.0),
        ] {
            *scripts.world_mut() = world.clone();
            assert!(
                scripts
                    .eval_callback::<()>(&format!("{call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, world.btech);
            assert!(scripts.drain_outbox().is_empty());
            scripts.eval_callback::<()>(&call).unwrap();
            let output = scripts.drain_outbox();
            for who in [ObjectId(1), passenger] {
                assert_eq!(
                    output
                        .iter()
                        .filter(|(id, doc)| *id == who && text::plain(doc.source()) == WARNING)
                        .count(),
                    1
                );
            }
            assert!(
                output
                    .iter()
                    .all(|(id, _)| [unit, ObjectId(1), passenger].contains(id))
            );
        }
        *scripts.world_mut() = world;
        let reply = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech map-conditions #{}=day,30", map.0),
        );
        assert!(reply.contains(WARNING), "{reply}");
        assert_eq!(
            selection(&scripts.world(), unit).active,
            BattleSensorPair::default()
        );
    }
}
