//! Wizard LOS broadcasts reuse live acquired visibility and atomic observer publication.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Native and Lua broadcasts share exact recipients, literal text and source exclusion.
#[tokio::test]
async fn losemit_shares_all_chassis_observers_and_transactions() {
    for template in firing::templates() {
        let (dir, config, mut world, observer, source, _) =
            firing::fixture_with_target(&template, Some(BattleWeapon::MediumLaser), &template)
                .await;
        let actor = world.create(&config, "Announcer".into(), Kind::Player);
        world
            .objects
            .get_mut(&actor)
            .unwrap()
            .flags
            .insert(Flag::Wizard);
        world.objects.get_mut(&actor).unwrap().location = Some(source);
        let passenger = world.create(&config, "Passenger".into(), Kind::Player);
        world.objects.get_mut(&passenger).unwrap().location = Some(source);
        for message in ["waves.", "'s arm glows!", "[fg=red]literal[reset]", ""] {
            let expected = battle_observer_messages(&world, source, message);
            assert_eq!(expected.len(), 1);
            assert_eq!(expected[0].0, observer);
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            assert_eq!(
                battle_losemit_action(&scripts, actor, source, message).unwrap(),
                1
            );
            let notices: Vec<_> = scripts
                .drain_outbox()
                .into_iter()
                .map(|(id, text)| (id, text.source().to_owned()))
                .collect();
            assert!(
                notices.contains(&(ObjectId(1), text::escape(&expected[0].1))),
                "{notices:?}"
            );
            assert!(notices.contains(&(actor, "Broadcast done.".into())));
            assert!(!notices.iter().any(|(id, _)| *id == passenger));
            assert_eq!(scripts.world().btech, world.btech);
            let lua_message = format!("[=[{message}]=]");
            let count: usize = scripts
                .eval_callback(&format!(
                    "return btech.unit.losemit({}, {}, {lua_message})",
                    actor.0, source.0
                ))
                .unwrap();
            assert_eq!(count, 1);
            let lua_notices: Vec<_> = scripts
                .drain_outbox()
                .into_iter()
                .map(|(id, text)| (id, text.source().to_owned()))
                .collect();
            assert_eq!(lua_notices, notices);
            let native =
                support::run_text(&scripts, &config, actor, 1, &format!("@losemit {message}"));
            assert!(native.contains("Broadcast done."), "{native}");
            assert_eq!(scripts.world().btech, world.btech);
        }
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        assert!(battle_losemit_action(&scripts, passenger, source, "waves.").is_err());
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.losemit({}, {}, 'waves.'); error('abort')",
                    actor.0, source.0
                ))
                .is_err()
        );
        assert!(scripts.drain_outbox().is_empty());
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        let restarted = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
        assert_eq!(
            battle_losemit_action(&restarted, actor, source, "waves.").unwrap(),
            1
        );
        assert_eq!(restarted.drain_outbox().len(), 2);
        // Source power does not suppress an emote, but losing the contact suppresses delivery.
        firing::edit(&mut world, source, |state| {
            state["power"] = serde_json::to_value(BattlePower::Off).unwrap()
        });
        let stopped = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        assert_eq!(
            battle_losemit_action(&stopped, actor, source, "waves.").unwrap(),
            1
        );
        let mut contacts = serde_json::Value::Null;
        firing::edit(&mut world, observer, |state| {
            contacts = state["contacts"].clone();
            state["contacts"] = serde_json::json!({});
        });
        let unseen = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        assert_eq!(
            battle_losemit_action(&unseen, actor, source, "waves.").unwrap(),
            0
        );
        assert_eq!(unseen.drain_outbox().len(), 1);
        // One observer line succeeds, then confirmation exceeds capacity: no partial output survives.
        let path = dir.path().join("stompymux.toml");
        let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        table
            .entry("lua")
            .or_insert(toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .unwrap()
            .insert("output_entry_limit".into(), 2.into());
        std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
        let config = Config::load(dir.path()).unwrap();
        let limited = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        battle_losemit_action(&limited, actor, source, "first").unwrap();
        firing::edit(&mut limited.world_mut(), observer, |state| {
            state["contacts"] = contacts
        });
        let before = limited.world().btech.clone();
        let failure = battle_losemit_action(&limited, actor, source, "second").unwrap_err();
        assert!(failure.to_string().contains("output limit"), "{failure:#}");
        assert_eq!(limited.drain_outbox().len(), 1);
        assert_eq!(limited.world().btech, before);
    }
}
