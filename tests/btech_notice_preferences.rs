//! Shared cockpit notice preferences preserve sensor facts, cursors and restart behavior.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Both notice preferences use native/Lua controls and actual delivery rules on every chassis.
#[tokio::test]
async fn notice_preferences_share_delivery_and_persistence_across_chassis() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, target, _) =
            firing::fixture_with_target(&source, None, &source).await;
        firing::edit(&mut world, target, |unit| {
            unit["power"] = serde_json::to_value(Power::Off).unwrap();
            unit["signature"]["team"] = serde_json::json!(2);
        });
        let event = ContactEvent {
            experience_message: None,
            identified: true,
            observer: id,
            target,
            acquired: true,
            lock_lost: false,
        };
        assert!(event.notice(&world).is_none());
        assert!(
            ContactEvent {
                lock_lost: true,
                ..event.clone()
            }
            .notice(&world)
            .unwrap()
            .text
            .contains("lock has been lost")
        );
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for (command, method) in [
            ("SLWarn", "searchlight_warning"),
            ("AutoconShutdown", "autocon_shutdown"),
        ] {
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("mechprefs {command} ON"),
            );
            assert!(text.contains("ON"), "{text}");
            lua.eval_callback::<()>(&format!("btech.unit.{method}({},1,true)", id.0))
                .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
        }
        assert!(event.notice(&native.world()).is_some());
        assert!(lua.eval_callback::<()>(&format!("btech.unit.searchlight_warning({},1,false); btech.unit.autocon_shutdown({},1,false); error('abort')",id.0,id.0)).is_err());
        assert_eq!(native.world().btech, lua.world().btech);
        assert!(
            set_battle_searchlight_warning(&mut lua.world_mut(), id, ObjectId(2), false).is_err()
        );
        assert!(set_battle_autocon_shutdown(&mut lua.world_mut(), id, ObjectId(2), false).is_err());
        assert_eq!(native.world().btech, lua.world().btech);
        let mut world = native.world().clone();
        assert!(refresh_battle_illumination(&mut world).is_empty());
        let contacts = visible_battle_contacts(&world, id).unwrap();
        firing::edit(&mut world, id, |unit| {
            unit["signature"]["illuminated"] = serde_json::json!(true)
        });
        assert!(battle_illumination_pending(&world));
        assert_eq!(
            refresh_battle_illumination(&mut world),
            vec![Notice {
                unit: id,
                text: "You are being illuminated!".into()
            }]
        );
        assert!(!battle_illumination_pending(&world));
        assert!(refresh_battle_illumination(&mut world).is_empty());
        assert_eq!(visible_battle_contacts(&world, id).unwrap(), contacts);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, world.btech);
        assert!(refresh_battle_illumination(&mut loaded).is_empty());
        firing::edit(&mut loaded, id, |unit| {
            unit["signature"]["illuminated"] = serde_json::json!(false)
        });
        assert_eq!(
            refresh_battle_illumination(&mut loaded),
            vec![Notice {
                unit: id,
                text: "You are no longer being illuminated.".into()
            }]
        );
        set_battle_searchlight_warning(&mut loaded, id, ObjectId(1), false).unwrap();
        firing::edit(&mut loaded, id, |unit| {
            unit["signature"]["illuminated"] = serde_json::json!(true)
        });
        assert!(refresh_battle_illumination(&mut loaded).is_empty());
        assert!(!battle_illumination_pending(&loaded));
        set_battle_autocon_shutdown(&mut loaded, id, ObjectId(1), false).unwrap();
        assert!(event.notice(&loaded).is_none());
    }
}
