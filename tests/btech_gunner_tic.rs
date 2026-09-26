//! Reserved station TIC commands remain inert and never grant physical cockpit group control.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Catalogued station TIC entries ignore arguments, assignment and the parent's live weapons.
#[tokio::test]
async fn station_tic_commands_are_inert_across_supported_chassis() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, target, weapon) =
            firing::fixture_with_target(&template, Some(BattleWeapon::MediumLaser), &template)
                .await;
        edit_battle_tic(
            &mut world,
            parent,
            ObjectId(1),
            0,
            BattleTicEdit::Add(vec![weapon]),
        )
        .unwrap();
        let station = world.create(&config, "Station".into(), Kind::Thing);
        let gunner = world.create(&config, "Gunner".into(), Kind::Player);
        world.objects.get_mut(&gunner).unwrap().location = Some(station);
        register_gunner_station(&mut world, ObjectId(1), station, parent, 0).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for initialized in [false, true] {
            if initialized {
                gunner_station_action(&scripts, station, gunner, true).unwrap();
                select_battle_target(&mut scripts.world_mut(), station, gunner, Some(target))
                    .unwrap();
                scripts.drain_outbox();
            }
            let before = scripts.world().btech.clone();
            for name in ["addtic", "deltic", "cleartic", "listtic", "firetic"] {
                for argument in [
                    "".into(),
                    "invalid arguments".into(),
                    format!("0 {weapon}"),
                    format!("0 #{}", target.0),
                ] {
                    let text = support::run_text(
                        &scripts,
                        &config,
                        gunner,
                        1,
                        &format!("{name} {argument}"),
                    );
                    assert!(text.is_empty(), "{template}: {name}: {text}");
                    assert_eq!(scripts.world().btech, before);
                    assert!(scripts.drain_outbox().is_empty());
                }
            }
            assert!(battle_tic(&scripts.world(), parent, gunner, 0).is_err());
            assert!(battle_tic(&scripts.world(), station, gunner, 0).is_err());
            assert_eq!(
                battle_tic(&scripts.world(), parent, ObjectId(1), 0).unwrap(),
                [weapon]
            );
        }
        let text = support::run_text(&scripts, &config, ObjectId(1), 1, "listtic 0");
        assert!(text.contains("TIC #0:"), "{text}");
    }
}
