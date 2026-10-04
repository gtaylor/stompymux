//! Operator visibility persists across chassis without replacing ordinary sensor acquisition.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Representative construction for every supported movement class.
fn templates() -> Vec<String> {
    let tracked = include_str!("../game/mechs/Demolisher.toml");
    vec![
        include_str!("../game/mechs/JR7-D.toml").into(),
        include_str!("../game/mechs/GOL-1H.toml").into(),
        tracked.into(),
        tracked.replace("movement = \"track\"", "movement = \"wheel\""),
        tracked.replace("movement = \"track\"", "movement = \"hover\""),
        tracked
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 5", "walk_mp = 0"),
        include_str!("../game/mechs/Kestrel.toml").into(),
    ]
}

/// Set isolated runtime facts through the persisted representation for either anatomy.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    world.btech.rewrite_unit_record(id, change).unwrap();
}

/// A piloted observer and target on opposite sides of an optional obstructing hill.
async fn fixture(
    observer: &str,
    target: &str,
    blocked: bool,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Visibility field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "visibility",
        BattleMapAsset::from_cells(if blocked {
            "1 3\n.0\n.9\n.0\n"
        } else {
            "1 3\n.0\n.0\n.0\n"
        })
        .unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for (index, source) in [observer, target].into_iter().enumerate() {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        BattleUnitTemplate::parse("test", source)
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        place_battle_unit(&mut world, id, map, 0, if index == 0 { 2 } else { 0 }).unwrap();
        edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
        });
        ids.push(id);
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ids[0]);
    assign_battle_pilot(&mut world, ids[0], ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    world.validate(&config).unwrap();
    (dir, config, world, ids[0], ids[1])
}

/// Native edits and trusted Lua share validation, callback rollback and exact saved state.
#[tokio::test]
async fn visibility_controls_all_chassis_are_atomic_and_durable() {
    for source in templates() {
        let (_dir, config, mut world, id, _) = fixture(&source, &source, false).await;
        world
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let command = format!("@btech unit-visibility #{id}=both", id = id.0);
        let denied = support::run_text(&scripts, &config, ObjectId(2), 2, &command);
        assert!(denied.contains("Permission"), "{denied}");
        assert_eq!(scripts.world().btech, world.btech);
        let reply = support::run_text(&scripts, &config, ObjectId(1), 1, &command);
        assert!(
            reply.contains("invisible=true, clairvoyant=true"),
            "{reply}"
        );
        let before = scripts.world().btech.clone();
        scripts.drain_outbox();
        let call = format!(
            "btech.unit.visibility({}, {{invisible=false,clairvoyant=false}})",
            id.0
        );
        assert!(
            scripts
                .eval_callback::<()>(&(call.clone() + "; error('abort')"))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.visibility({}, {{invisible=false}})",
                    id.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        let flags: (bool, bool) = scripts
            .eval_callback(&format!(
                "local v=btech.unit.state({}).visibility; return v.invisible,v.clairvoyant",
                id.0
            ))
            .unwrap();
        assert_eq!(flags, (true, true));
        let detached: bool = scripts.eval_callback(&format!("local v=btech.unit.visibility({}); v.invisible=false; return btech.unit.visibility({}).invisible",id.0,id.0)).unwrap();
        assert!(detached);
        let saved_world = scripts.world().clone();
        persistence::save(&config.database(), &saved_world)
            .await
            .unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, scripts.world().btech);
        assert_eq!(
            battle_visibility(&loaded, id).unwrap(),
            BattleVisibility {
                invisible: true,
                clairvoyant: true
            }
        );
        scripts.eval_callback::<()>(&call).unwrap();
        assert_eq!(scripts.world().btech, world.btech);
        let mut going = world.clone();
        going
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::Going);
        assert!(
            set_battle_visibility(
                &mut going,
                id,
                BattleVisibility {
                    invisible: true,
                    clairvoyant: false
                }
            )
            .is_err()
        );
        assert_eq!(going.btech, world.btech);
        loaded.validate(&config).unwrap();
    }
}

/// Invisibility hides cached contacts immediately and the next scan reports their loss without new rolls.
#[tokio::test]
async fn visibility_sensor_loss_and_clairvoyant_contacts_cross_all_chassis() {
    for observer_source in templates() {
        for target_source in templates() {
            let (_dir, config, mut world, observer, target) =
                fixture(&observer_source, &target_source, false).await;
            edit(&mut world, observer, |state| {
                state["contacts"] = serde_json::json!({target.0.to_string(): {"identified":true}});
            });
            let geometry = battle_unit_terrain_los(&world, observer, target).unwrap();
            assert!(
                visible_battle_contact(&world, observer, target)
                    .unwrap()
                    .is_some()
            );
            assert_eq!(
                battle_perceive(&world, observer, target)
                    .unwrap()
                    .unwrap()
                    .channel,
                BattleDetectionChannel::Sensors
            );
            set_battle_visibility(
                &mut world,
                target,
                BattleVisibility {
                    invisible: true,
                    clairvoyant: false,
                },
            )
            .unwrap();
            let hidden = world.btech.clone();
            assert!(
                visible_battle_contact(&world, observer, target)
                    .unwrap()
                    .is_none()
            );
            assert!(battle_observer_messages(&world, target, "moves").is_empty());
            assert!(battle_perceive(&world, observer, target).unwrap().is_none());
            assert_eq!(world.btech, hidden);
            let events = refresh_battle_contacts(&mut world, &[observer]).unwrap();
            assert!(
                events
                    .iter()
                    .any(|event| event.target == target && !event.acquired)
            );
            set_battle_visibility(
                &mut world,
                observer,
                BattleVisibility {
                    invisible: false,
                    clairvoyant: true,
                },
            )
            .unwrap();
            let unchanged = world.btech.clone();
            let contact = visible_battle_contact(&world, observer, target)
                .unwrap()
                .unwrap();
            assert!(contact.identified);
            assert!(contact.detection.is_none());
            assert!(
                contact.short_text.starts_with(' '),
                "{}",
                contact.short_text
            );
            assert_eq!(visible_battle_contacts(&world, observer).unwrap().len(), 1);
            assert_eq!(battle_observer_messages(&world, target, "moves").len(), 1);
            assert_eq!(
                battle_unit_terrain_los(&world, observer, target).unwrap(),
                geometry
            );
            assert_eq!(world.btech, unchanged);
            world.validate(&config).unwrap();
        }
    }
}

/// Clairvoyance exposes blocked terrain and unacquired units but leaves the firing penalty and geometry intact.
#[tokio::test]
async fn clairvoyance_preserves_physical_los_and_unacquired_aim() {
    for source in templates() {
        let (_dir, config, mut world, observer, target) =
            fixture(&source, include_str!("../game/mechs/JR7-D.toml"), true).await;
        let hex = BattleHexCoordinate { x: 0, y: 0 };
        assert!(
            battle_unit_terrain_los(&world, observer, target)
                .unwrap()
                .blocked
        );
        assert!(!battle_hex_visible(&world, observer, hex).unwrap());
        assert!(
            visible_battle_contacts(&world, observer)
                .unwrap()
                .is_empty()
        );
        set_battle_visibility(
            &mut world,
            observer,
            BattleVisibility {
                invisible: false,
                clairvoyant: true,
            },
        )
        .unwrap();
        let before = world.btech.clone();
        assert!(battle_hex_visible(&world, observer, hex).unwrap());
        assert_eq!(
            battle_hex_perception(&world, observer, hex).unwrap(),
            Some(BattleDetectionChannel::Sight)
        );
        assert!(battle_perceive(&world, observer, target).unwrap().is_none());
        assert!(battle_hex_visible(&world, observer, BattleHexCoordinate { x: 9, y: 9 }).is_err());
        let view = visible_battle_contact(&world, observer, target)
            .unwrap()
            .unwrap();
        assert!(view.identified);
        assert_eq!(view.detection, None);
        let rules = BattleAimRules {
            woods_damage: false,
            dig_bonus: 3,
            dig_only_front: false,
            hit_arc_mode: 0,
            fasa_turning: false,
            extended_movement: false,
            extended_ranges: false,
            hotload_half_minimum: false,
            override_weapon_arcs: true,
        };
        let aim = battle_aim_modifiers(&world, observer, target, 0, 4, rules).unwrap();
        assert_eq!(
            aim.perception,
            Some(BattlePerceptionAim {
                channel: None,
                direct_fire: true,
                modifier: 10_000,
            })
        );
        assert!(
            battle_unit_terrain_los(&world, observer, target)
                .unwrap()
                .blocked
        );
        assert_eq!(world.btech, before);
        world.validate(&config).unwrap();
    }
}

/// Every perception channel rejects invisible targets, even for a clairvoyant operator with working hardware.
#[tokio::test]
async fn invisibility_suppresses_all_perception_channels_without_acquisition() {
    let source = include_str!("../game/mechs/JR7-D.toml").replace(
        r#"{ at = "1-2", item = "JumpJet" },"#,
        r#"{ at = "1-2", item = "JumpJet" },
    { at = "3-4", item = "BeagleProbe" },
    { at = 5, item = "Light_BAP" },
    { at = "6-8", item = "BloodhoundProbe" },"#,
    );
    let source = support::templates::with_flags(&source, &["AntiAircraft"]);
    let (_dir, config, base, observer, target) =
        fixture(&source, include_str!("../game/mechs/JR7-D.toml"), false).await;
    let map = base.btech.units()[&observer].map.unwrap();
    for channel in BattleDetectionChannel::ALL {
        let mut world = base.clone();
        // Silence every channel that would win the tie-break ahead of the one under test.
        if channel != BattleDetectionChannel::Sensors {
            set_battle_map_perception(&mut world, map, BattleMapPerceptionFlag::Sensors, false)
                .unwrap();
        }
        if matches!(
            channel,
            BattleDetectionChannel::Radar | BattleDetectionChannel::Probe
        ) {
            set_battle_map_visibility(&mut world, map, BattleLight::Day, 0).unwrap();
        }
        if channel == BattleDetectionChannel::Radar {
            set_battle_map_perception(&mut world, map, BattleMapPerceptionFlag::Probes, false)
                .unwrap();
            edit(&mut world, target, |state| {
                state["ground_elevation"] = 5.into()
            });
        }
        let ordinary = battle_perceive(&world, observer, target).unwrap();
        assert_eq!(
            ordinary.map(|perception| perception.channel),
            Some(channel),
            "{channel:?} must have a detectable control target"
        );
        set_battle_visibility(
            &mut world,
            target,
            BattleVisibility {
                invisible: true,
                clairvoyant: false,
            },
        )
        .unwrap();
        set_battle_visibility(
            &mut world,
            observer,
            BattleVisibility {
                invisible: false,
                clairvoyant: true,
            },
        )
        .unwrap();
        let before = world.btech.clone();
        assert!(
            battle_perceive(&world, observer, target).unwrap().is_none(),
            "{channel:?}"
        );
        assert!(
            refresh_battle_contacts(&mut world, &[observer])
                .unwrap()
                .is_empty()
        );
        assert_eq!(world.btech, before);
        world.validate(&config).unwrap();
    }
}
