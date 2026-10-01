//! Reactor cascades retain the initiating kill without crediting self-attributed blast victims.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Read saved counters from either owning chassis store.
fn kills(world: &World, id: ObjectId) -> i64 {
    let owner = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    serde_json::to_value(&world.btech).unwrap()[owner][id.0.to_string()]["units_killed"]
        .as_i64()
        .unwrap()
}

/// A shot-triggered reactor awards one kill even when its radial blast destroys another unit.
#[tokio::test]
async fn reactor_shot_and_neighbor_death_have_distinct_attribution() {
    for shooter in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/Demolisher.toml"),
    ] {
        for target_source in [
            include_str!("../game/mechs/JR7-D.toml"),
            include_str!("../game/mechs/GOL-1H.toml"),
        ] {
            let (_dir, config, mut base, id, target, index) = firing::fixture_with_target(
                shooter,
                Some(BattleWeapon::MediumLaser),
                target_source,
            )
            .await;
            firing::edit(&mut base, target, |state| {
                state["sections"]["CenterTorso"]["armor"] = 0.into();
                state["sections"]["CenterTorso"]["rear"] = 0.into();
                state["sections"]["CenterTorso"]["internal"] = 1.into();
                for rounds in state["ammunition"].as_array_mut().unwrap() {
                    *rounds = 0.into();
                }
            });
            let position = base.btech.constructed_units()[&target].position().unwrap();
            let neighbor = base.create(&config, "Fragile blast neighbor".into(), Kind::Thing);
            base.objects.get_mut(&neighbor).unwrap().home = Some(ObjectId(config.home()));
            create_battle_vehicle(
                &mut base,
                neighbor,
                BattleVehicleTemplate::parse(
                    "Demolisher",
                    include_str!("../game/mechs/Demolisher.toml"),
                )
                .unwrap(),
            )
            .unwrap();
            place_battle_unit(
                &mut base,
                neighbor,
                position.map,
                i64::from(position.x),
                i64::from(position.y),
            )
            .unwrap();
            firing::edit(&mut base, neighbor, |state| {
                for section in state["sections"].as_object_mut().unwrap().values_mut() {
                    section["armor"] = 0.into();
                    section["internal"] = 1.into();
                }
                for rounds in state["ammunition"].as_array_mut().unwrap() {
                    *rounds = 0.into();
                }
                state["dice"] = serde_json::to_value(BattleDice::seeded([17; 32])).unwrap();
            });
            let hit_seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
                .unwrap();
            firing::edit(&mut base, id, |state| {
                state["dice"] = serde_json::to_value(BattleDice::seeded([hit_seed; 32])).unwrap()
            });
            let command = format!("btech.unit.fire({},1,{index},{})", id.0, target.0);
            let mut chosen = None;
            for seed in 0..=255 {
                firing::edit(&mut base, target, |state| {
                    state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
                });
                let scripts = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
                let report: mlua::Table =
                    scripts.eval_callback(&format!("return {command}")).unwrap();
                let report = serde_json::to_value(report).unwrap();
                let detonated = report["salvo"]["report"]["groups"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|group| {
                        group["impact"]["reactor_explosions"]
                            .as_array()
                            .is_some_and(|blasts| !blasts.is_empty())
                    });
                if detonated {
                    chosen = Some(scripts);
                    break;
                }
            }
            let scripts =
                chosen.expect("seed matrix must include a shot-triggered reactor explosion");
            assert!(scripts.world().btech.constructed_units()[&target].is_destroyed());
            assert!(scripts.world().btech.vehicles()[&neighbor].is_destroyed());
            assert_eq!(kills(&scripts.world(), id), 1);
            assert_eq!(kills(&scripts.world(), target), 0);
            assert_eq!(kills(&scripts.world(), neighbor), 0);
            let native = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
            support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire {index} #{}", target.0),
            );
            assert!(native.world().btech == scripts.world().btech);
            let saved = scripts.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            let loaded = persistence::load(&config.database()).await.unwrap();
            for unit in [id, target, neighbor] {
                assert_eq!(kills(&loaded, unit), kills(&saved, unit));
            }
            let abort = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
            assert!(
                abort
                    .eval_callback::<()>(&format!("{command}; error('abort')"))
                    .is_err()
            );
            assert!(abort.world().btech == base.btech);
            assert!(abort.drain_outbox().is_empty());
        }
    }
}
