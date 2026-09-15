//! Map broadcasts use a shared cockpit audience, preserving state and atomic notification delivery.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// Inspect recipient identity and exact message source without socket rendering.
fn output(scripts: &Scripts) -> Vec<(ObjectId, String)> {
    scripts
        .drain_outbox()
        .into_iter()
        .map(|(id, text)| (id, text.source().to_owned()))
        .collect()
}

/// A map-side wizard and a second cockpit listener distinguish broadcasts from map-room emits.
fn listeners(world: &mut World, config: &Config, map: ObjectId, target: ObjectId) -> ObjectId {
    let actor = world.create(config, "Map announcer".into(), Kind::Player);
    world
        .objects
        .get_mut(&actor)
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    world.objects.get_mut(&actor).unwrap().location = Some(map);
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
    actor
}

/// Every supported chassis shares running, blindness and consciousness admission, without acquiring contacts.
#[tokio::test]
async fn map_broadcast_audience_native_lua_and_restart() {
    for source in firing::templates() {
        for mode in [
            "running",
            "stunned",
            "off",
            "starting",
            "blind",
            "pilot_recovery",
            "empty_recovery",
        ] {
            let (_dir, config, mut world, unit, target, _) =
                firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D"))
                    .await;
            let map = world.btech.units()[&unit].map.unwrap();
            let actor = listeners(&mut world, &config, map, target);
            if mode == "empty_recovery" {
                release_battle_pilot(&mut world, unit, ObjectId(1)).unwrap();
            }
            firing::edit(&mut world, unit, |state| {
                state["contacts"] = serde_json::json!({});
                state["target_lock"] = serde_json::Value::Null;
                if mode == "off" {
                    state["power"] = serde_json::to_value(BattlePower::Off).unwrap();
                }
                if mode == "starting" {
                    state["power"] =
                        serde_json::to_value(BattlePower::Starting { remaining: 10 }).unwrap();
                }
                if mode == "stunned" {
                    let field = if state.get("crew_stun_remaining").is_some() {
                        "crew_stun_remaining"
                    } else {
                        "stun_remaining"
                    };
                    state[field] = 5.into();
                }
                if mode == "blind" {
                    state["blinded_remaining"] = 3.into();
                }
                if mode == "empty_recovery" {
                    state["pilot_injuries"] = 1.into();
                    state["crew_recovery"]["mode"] =
                        serde_json::json!({"kind":"tactical","injuries":1});
                    state["crew_recovery"]["remaining"] = 10.into();
                }
            });
            if mode == "pilot_recovery" {
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["recoveries"]["1"] = serde_json::json!({"mode":{"kind":"tactical","injuries":1},"remaining":10,"pain_resistance":false,"toughness":false,"dice":BattleDice::seeded([63;32])});
                world.btech = serde_json::from_value(state).unwrap();
            }
            // Put the later-created target first to distinguish slot order from object-id order.
            let mut state = serde_json::to_value(&world.btech).unwrap();
            for (id, slot) in [(unit, 1), (target, 0)] {
                let kind = if world.btech.vehicles().contains_key(&id) {
                    "vehicles"
                } else {
                    "constructed"
                };
                state[kind][id.0.to_string()]["map_slot"] = slot.into();
            }
            world.btech = serde_json::from_value(state).unwrap();
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            let baseline = world.btech.clone();
            let native = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let lua = Scripts::new(
                &config,
                Rc::new(RefCell::new(
                    persistence::load(&config.database()).await.unwrap(),
                )),
            )
            .unwrap();
            let call = format!(
                "btech.map.emit({},{},'   Battlefield announcement')",
                actor.0, map.0
            );
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort broadcast')"))
                    .is_err()
            );
            assert!(output(&lua).is_empty());
            let audience: Vec<i64> = lua.eval_callback(&format!("return {call}")).unwrap();
            let audience: Vec<_> = audience.into_iter().map(ObjectId).collect();
            let expected = if matches!(mode, "running" | "stunned") {
                vec![target, unit]
            } else {
                vec![target]
            };
            assert_eq!(audience, expected, "{mode}");
            assert!(matches!(
                commands::run(
                    &native,
                    &config,
                    actor,
                    1,
                    "@mapemit   Battlefield announcement"
                )
                .unwrap(),
                CommandAction::Continue
            ));
            let native_output = output(&native);
            assert_eq!(native_output, output(&lua), "{mode}");
            assert_eq!(
                native_output
                    .iter()
                    .filter(|(_, text)| text == "Battlefield announcement")
                    .count(),
                if matches!(mode, "running" | "stunned") {
                    2
                } else {
                    1
                }
            );
            assert_eq!(native_output.last(), Some(&(actor, "Message sent!".into())));
            assert_eq!(native.world().btech, baseline);
            assert_eq!(lua.world().btech, baseline);
        }
    }
}

/// Broadcasts include every cockpit occupant, exclude room bystanders and reject unauthorized requests.
#[tokio::test]
async fn map_emit_authority_empty_maps_and_all_occupants() {
    let (_dir, config, mut world, unit, target, _) = firing::fixture_with_target(
        &firing::templates()[0],
        None,
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    let map = world.btech.units()[&unit].map.unwrap();
    let actor = listeners(&mut world, &config, map, target);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let passenger = world.create(&config, "Passenger".into(), Kind::Player);
    world.objects.get_mut(&passenger).unwrap().location = Some(unit);
    let bystander = world.create(&config, "Bystander".into(), Kind::Player);
    world.objects.get_mut(&bystander).unwrap().location = Some(map);
    let empty = world.create(&config, "Empty map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        empty,
        "empty",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (who, where_, text) in [
        (ObjectId(2), map, "hello"),
        (actor, unit, "hello"),
        (actor, map, ""),
        (actor, map, "   "),
        (actor, map, "\0"),
        (actor, empty, "\0"),
    ] {
        assert!(emit_battle_map_action(&scripts, who, where_, text).is_err());
        assert!(output(&scripts).is_empty());
    }
    let denied = support::run_text(&scripts, &config, actor, 1, "@mapemit/bad message");
    assert!(denied.contains("takes no switches"), "{denied}");
    let before = scripts.world().btech.clone();
    assert_eq!(
        emit_battle_map_action(&scripts, actor, map, "   Hello  ").unwrap(),
        vec![unit, target]
    );
    let messages = output(&scripts);
    for who in [ObjectId(1), ObjectId(2), passenger] {
        assert!(messages.contains(&(who, "Hello  ".into())));
    }
    assert!(
        !messages
            .iter()
            .any(|(who, _)| *who == bystander || *who == unit || *who == target)
    );
    scripts
        .world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .location = Some(map);
    assert_eq!(
        emit_battle_map_action(&scripts, actor, map, "Empty cockpit").unwrap(),
        vec![unit, target]
    );
    assert!(!output(&scripts).iter().any(|(id, _)| *id == ObjectId(2)));
    assert!(
        emit_battle_map_action(&scripts, actor, empty, "Nobody home")
            .unwrap()
            .is_empty()
    );
    assert_eq!(output(&scripts), vec![(actor, "Message sent!".into())]);
    assert_eq!(scripts.world().btech, before);
}

/// A failed confirmation retracts messages already staged for multiple cockpits.
#[tokio::test]
async fn failed_map_confirmation_rolls_back_every_recipient() {
    let (dir, config, mut world, unit, target, _) = firing::fixture_with_target(
        &firing::templates()[0],
        None,
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    let map = world.btech.units()[&unit].map.unwrap();
    let actor = listeners(&mut world, &config, map, target);
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("runtime")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_message_limit".into(), 1.into());
    std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let error = emit_battle_map_action(&scripts, actor, map, "X").unwrap_err();
    assert!(error.to_string().contains("output limit"), "{error:#}");
    assert!(output(&scripts).is_empty());
    assert_eq!(scripts.world().btech, before);
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.map.emit({},{},'X')", actor.0, map.0))
            .is_err()
    );
    assert!(output(&scripts).is_empty());
}
