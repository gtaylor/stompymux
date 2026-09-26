//! Observer administration and disclosure share the same role across supported chassis.
use crate::support;
use stompymux_rs::*;

#[tokio::test]
async fn observers_share_admin_disclosure_radio_and_saved_role() {
    for template in [
        include_str!("fixtures/btech/mechs/JR7-D"),
        include_str!("../game/mechs/SCP-1N"),
        include_str!("../game/mechs/Demolisher"),
        include_str!("../game/mechs/Kestrel"),
    ] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let map = world.create(&config, "Observer field".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "observer",
            BattleMapAsset::parse("1 5\n.0\n.0\n.0\n.0\n.0\n").unwrap(),
        )
        .unwrap();
        let id = world.create(&config, "Observer".into(), Kind::Thing);
        let target = world.create(&config, "Subject".into(), Kind::Thing);
        BattleUnitTemplate::parse(template)
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        BattleUnitTemplate::parse(include_str!("../game/mechs/Demolisher"))
            .unwrap()
            .create(&mut world, target)
            .unwrap();
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
        place_battle_unit(&mut world, target, map, 0, 3).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        let key = if world.btech.vehicles().contains_key(&id) {
            "vehicles"
        } else {
            "constructed"
        };
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved[key][id.0.to_string()]["contacts"][target.0.to_string()] =
            serde_json::json!({"identified": true});
        saved[key][id.0.to_string()]["definition"]["attributes"]["scan_range"] = "1".into();
        saved[key][id.0.to_string()]["definition"]["attributes"]["tac_range"] = "1".into();
        saved["vehicles"][target.0.to_string()]["radio"][0]["frequency"] = 42.into();
        saved["vehicles"][target.0.to_string()]["radio"][0]["mode"]["digital"] = true.into();
        saved["vehicles"][target.0.to_string()]["definition"]["attributes"]["radio_range"] =
            "1".into();
        world.btech = serde_json::from_value(saved).unwrap();
        world.validate(&config).unwrap();
        assert!(scan_battle_unit(&world, id, ObjectId(1), target, "A").is_err());
        assert!(
            !resolve_digital_radio(&world, target, 0, "Unmatched")
                .unwrap()
                .receptions
                .iter()
                .any(|r| r.receiver == id)
        );
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let output = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("@btech unit-observer #{}=on", id.0),
        );
        assert!(output.contains("observer role enabled"), "{output}");
        assert!(
            lua.eval_callback::<bool>(&format!("return btech.unit.observer({},true)", id.0))
                .unwrap()
        );
        assert_eq!(native.world().btech, lua.world().btech);
        assert!(
            lua.eval_callback::<bool>(&format!(
                "return btech.unit.observer({}) and btech.unit.state({}).observer",
                id.0, id.0
            ))
            .unwrap()
        );
        assert!(
            support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("@btech inspect #{}", id.0)
            )
            .contains("Observer: true")
        );
        let mut observing = lua.world().clone();
        let exact = scan_battle_unit(&observing, id, ObjectId(1), target, "A").unwrap();
        assert!(
            stompymux_rs::text::plain(&exact)
                .contains("         ,`.40,'.                              ,`. 8,'."),
            "{exact}"
        );
        assert!(
            parse_battle_view_center(
                &observing,
                id,
                ObjectId(1),
                BattleViewKind::Tactical,
                "180 1000"
            )
            .is_ok()
        );
        assert!(
            parse_battle_view_center(
                &observing,
                id,
                ObjectId(1),
                BattleViewKind::Tactical,
                &format!("#{}", target.0)
            )
            .is_err()
        );
        assert!(
            resolve_targeted_radio(&observing, id, ObjectId(1), target, "Forbidden")
                .unwrap_err()
                .to_string()
                .contains("can't radio")
        );
        let digital = resolve_digital_radio(&observing, target, 0, "Clear text").unwrap();
        let receiver = digital
            .receptions
            .iter()
            .find(|r| r.receiver == id)
            .unwrap();
        assert!(
            receiver.text.contains("AB:42> <> Clear text"),
            "{}",
            receiver.text
        );
        let event = BattleContactEvent {
            identified: true,
            observer: id,
            target,
            acquired: true,
            lock_lost: false,
            experience_message: None,
        };
        assert!(event.notice(&observing).is_none());
        let lost = BattleContactEvent {
            acquired: false,
            lock_lost: true,
            ..event
        };
        assert!(
            lost.notice(&observing)
                .unwrap()
                .text
                .contains("lock has been lost")
        );
        lua.drain_outbox();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.observer({},false); error('abort')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, observing.btech);
        assert!(lua.drain_outbox().is_empty());
        let ordinary = native
            .world_mut()
            .create(&config, "Ordinary player".into(), Kind::Player);
        let denied = support::run_text(
            &native,
            &config,
            ordinary,
            2,
            &format!("@btech unit-observer #{}=off", id.0),
        );
        assert!(denied.contains("Permission"), "{denied}");
        assert!(battle_unit_observer(&native.world(), id).unwrap());
        persistence::save(&config.database(), &observing)
            .await
            .unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert!(battle_unit_observer(&restored, id).unwrap());
        assert_eq!(
            scan_battle_unit(&restored, id, ObjectId(1), target, "A").unwrap(),
            exact
        );
        assert_eq!(
            resolve_digital_radio(&restored, target, 0, "Clear text").unwrap(),
            digital
        );
        let mut analog_world = observing.clone();
        let mut saved = serde_json::to_value(&analog_world.btech).unwrap();
        saved["vehicles"][target.0.to_string()]["radio"][0]["mode"]["digital"] = false.into();
        analog_world.btech = serde_json::from_value(saved).unwrap();
        persistence::save(&config.database(), &analog_world)
            .await
            .unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        let analog =
            resolve_analog_radio(&mut analog_world, target, 0, "Clear analog text").unwrap();
        assert!(analog.interfered_receivers.contains(&id));
        assert!(
            analog
                .receptions
                .iter()
                .find(|r| r.receiver == id)
                .unwrap()
                .text
                .contains("AB:42> <> Clear analog text")
        );
        assert_eq!(
            resolve_analog_radio(&mut replay, target, 0, "Clear analog text").unwrap(),
            analog
        );
        assert_eq!(analog_world.btech, replay.btech);
        assert!(set_battle_observer(&mut observing, ObjectId(-1), true).is_err());
        set_battle_observer(&mut observing, id, false).unwrap();
        assert!(scan_battle_unit(&observing, id, ObjectId(1), target, "A").is_err());
        assert!(
            !resolve_digital_radio(&observing, target, 0, "Clear text")
                .unwrap()
                .receptions
                .iter()
                .any(|r| r.receiver == id)
        );
    }
}
