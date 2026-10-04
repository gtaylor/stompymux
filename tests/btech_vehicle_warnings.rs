//! Vehicle warning preferences share material thresholds and launcher accounting with Mechs.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Suppress critical side effects while retaining ordinary armor admission and warning ordering.
fn rules() -> VehicleCriticalRules {
    VehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        table: VehicleCriticalTable::Standard,
        enabled: false,
        combat_safe: false,
        toughness: false,
    }
}

/// Warnings occur on severity changes, not every hit, and a saved opt-out changes no material result.
#[tokio::test]
async fn vehicle_armor_warnings_follow_thresholds_preferences_and_restart() {
    for source in firing::templates().into_iter().skip(2) {
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let original = world.btech.vehicles()[&id].sections()[&VehicleSection::Front].armor;
        let mut previous = original;
        for (remaining, message) in [
            (original / 2, None),
            (original / 2 - 1, Some("low.")),
            (original / 4, None),
            (original / 4 - 1, Some("critical!")),
            (0, Some("BREACHED!")),
        ] {
            let hit = VehicleArmorHit {
                damage_class: DamageClass::Ordinary,
                section: VehicleSection::Front,
                amount: u32::from(previous - remaining),
                through_armor_critical: false,
                armor_piercing: None,
            };
            let mut quiet = world.clone();
            set_battle_armor_warning(&mut quiet, id, ObjectId(1), false).unwrap();
            let quiet_report =
                resolve_battle_vehicle_armor_damage(&mut quiet, id, hit, rules()).unwrap();
            assert!(
                quiet_report
                    .notices
                    .iter()
                    .all(|n| !n.text.contains("WARNING: FS Armor"))
            );
            let report = resolve_battle_vehicle_armor_damage(&mut world, id, hit, rules()).unwrap();
            let warnings: Vec<_> = report
                .notices
                .iter()
                .filter(|n| n.text.contains("WARNING: FS Armor"))
                .collect();
            assert_eq!(warnings.len(), usize::from(message.is_some()));
            if let Some(message) = message {
                assert!(warnings[0].text.contains(message));
            }
            set_battle_armor_warning(&mut quiet, id, ObjectId(1), true).unwrap();
            assert_eq!(quiet.btech, world.btech);
            previous = remaining;
        }
        set_battle_armor_warning(&mut world, id, ObjectId(1), false).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, world.btech);
        assert!(!loaded.btech.vehicles()[&id].armor_warning());
    }
}

/// Native/Lua controls and shots use pre-expenditure supply; callback failure restores warnings and dice.
#[tokio::test]
async fn vehicle_ammunition_warnings_match_supply_and_native_lua_controls() {
    for source in firing::templates().into_iter().skip(2) {
        let (_dir, config, base, id, target, index) =
            firing::fixture_with_supply(&source, Some(Weapon::Srm4), &source, false, Some(""))
                .await;
        let bins: Vec<_> = base.btech.vehicles()[&id]
            .loadout()
            .unwrap()
            .ammunition
            .iter()
            .enumerate()
            .filter(|(_, b)| b.weapon == Weapon::Srm4)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(bins.len(), 1);
        for supply in [3, 5, 6, 7, 11, 12, 13] {
            let mut world = base.clone();
            firing::edit(&mut world, id, |u| {
                u["ammunition"][bins[0]] = serde_json::json!(supply)
            });
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let call = format!("btech.unit.fire({},1,{index},{})", id.0, target.0);
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, world.btech);
            assert!(lua.drain_outbox().is_empty());
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire {index} #{}", target.0),
            );
            lua.eval_callback::<()>(&call).unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            assert_eq!(
                text.contains("Ammo for"),
                matches!(supply, 6 | 12),
                "{text}"
            );
            let quiet = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            for (name, method) in [
                ("AmmoWarn", "ammunition_warning"),
                ("ArmorWarn", "armor_warning"),
            ] {
                assert!(
                    support::run_text(
                        &quiet,
                        &config,
                        ObjectId(1),
                        1,
                        &format!("mechprefs {name} OFF")
                    )
                    .contains("OFF")
                );
                assert!(
                    quiet
                        .eval_callback::<()>(&format!("btech.unit.{method}({},2,true)", id.0))
                        .is_err()
                );
            }
            let saved = quiet.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            let loaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(loaded.btech, saved.btech);
            assert!(!loaded.btech.vehicles()[&id].ammunition_warning());
            let text = support::run_text(
                &quiet,
                &config,
                ObjectId(1),
                1,
                &format!("fire {index} #{}", target.0),
            );
            assert!(!text.contains("Ammo for"));
            set_battle_ammunition_warning(&mut quiet.world_mut(), id, ObjectId(1), true).unwrap();
            set_battle_armor_warning(&mut quiet.world_mut(), id, ObjectId(1), true).unwrap();
            assert_eq!(quiet.world().btech, native.world().btech);
        }
    }
}
