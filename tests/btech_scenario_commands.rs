//! Native scenario command coverage reuses shared flamer and team controls across supported chassis.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// HEAT is the same ordered flamer control as FLAMERHEAT, including failures and duplicate selections.
#[tokio::test]
async fn heat_alias_shares_flamer_control() {
    for template in firing::templates() {
        let (_dir, config, world, _, _, weapon) =
            firing::fixture_with_target(&template, Some(BattleWeapon::Flamer), &template).await;
        for argument in [
            weapon.to_string(),
            format!("{weapon},{weapon}"),
            "invalid".into(),
            "96".into(),
        ] {
            let canonical = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let alias = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let expected = support::run_text(
                &canonical,
                &config,
                ObjectId(1),
                1,
                &format!("flamerheat {argument}"),
            );
            let actual =
                support::run_text(&alias, &config, ObjectId(1), 1, &format!("heat {argument}"));
            assert_eq!(actual, expected);
            assert_eq!(alias.world().btech, canonical.world().btech);
        }
    }
}

/// SETTEAM preserves other signature facts, normalizes negative input and commits through one action.
#[tokio::test]
async fn native_team_control_matches_lua_and_restart() {
    for template in firing::templates() {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&template, Some(BattleWeapon::MediumLaser), &template)
                .await;
        let visitor = world.create(&config, "Visitor".into(), Kind::Player);
        set_battle_unit_signature(
            &mut world,
            unit,
            BattleUnitSignature {
                team: 9,
                hidden: true,
                illuminated: true,
            },
        )
        .unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for team in [7, -42, i32::MAX] {
            let text =
                support::run_text(&native, &config, ObjectId(1), 1, &format!("setteam {team}"));
            assert_eq!(text.trim(), format!("Team set to {}", team.max(0)));
            let value: i32 = lua
                .eval_callback(&format!(
                    "return btech.unit.set_team(1, {}, {team})",
                    unit.0
                ))
                .unwrap();
            assert_eq!(value, team.max(0));
            lua.drain_outbox();
            assert_eq!(native.world().btech, lua.world().btech);
            let current = native.world();
            let signature = current
                .btech
                .constructed_units()
                .get(&unit)
                .map(|unit| unit.signature())
                .or_else(|| {
                    current
                        .btech
                        .vehicles()
                        .get(&unit)
                        .map(|unit| unit.signature())
                })
                .unwrap();
            assert_eq!(
                signature,
                BattleUnitSignature {
                    team: team.max(0),
                    hidden: true,
                    illuminated: true
                }
            );
        }
        let before = native.world().btech.clone();
        for invalid in ["", "1 2", "2147483648", "#1", "1.5"] {
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("setteam {invalid}"),
            );
            assert!(text.contains("Invalid team"), "{text}");
            assert_eq!(native.world().btech, before);
        }
        assert!(set_battle_team_action(&native, &config, visitor, unit, 1).is_err());
        assert!(
            native
                .eval_callback::<()>(&format!(
                    "btech.unit.set_team(1, {}, 17); error('abort')",
                    unit.0
                ))
                .is_err()
        );
        assert_eq!(native.world().btech, before);
        assert!(native.drain_outbox().is_empty());
        let saved = native.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, saved.btech);
    }
}
