//! Preferred sections preserve shared feed fallback, cockpit admission and persistence.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Native/Lua controls and multi-bin plans share one contract on Mechs, ground vehicles and VTOLs.
#[tokio::test]
async fn preferred_sections_controls_feed_and_restart() {
    for (source, vehicle) in [
        (include_str!("../game/mechs/JR7-D.toml"), false),
        (include_str!("../game/mechs/GOL-1H.toml"), false),
        (include_str!("../game/mechs/Demolisher.toml"), true),
        (include_str!("../game/mechs/Kestrel.toml"), true),
    ] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let id = world.create(&config, "Ammo preference".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        let (weapon, preference, preferred_bin, other_bin) = if vehicle {
            let mut definition = BattleVehicleTemplate::parse("test", source).unwrap();
            let bin = definition
                .sections
                .values()
                .flat_map(|s| s.criticals.values())
                .find(|p| p.equipment.starts_with("Ammo_"))
                .unwrap()
                .clone();
            definition
                .sections
                .get_mut(&BattleVehicleSection::Left)
                .unwrap()
                .criticals
                .insert(0, bin.clone());
            create_battle_vehicle(&mut world, id, definition).unwrap();
            support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
            let loadout = world.btech.vehicles()[&id].loadout().unwrap();
            let weapon_type = loadout
                .ammunition
                .iter()
                .find(|b| b.location.section == BattleVehicleSection::Left)
                .unwrap()
                .weapon;
            (
                loadout
                    .weapons
                    .iter()
                    .position(|w| w.weapon == weapon_type)
                    .unwrap(),
                "left",
                loadout
                    .ammunition
                    .iter()
                    .position(|b| b.location.section == BattleVehicleSection::Left)
                    .unwrap(),
                loadout
                    .ammunition
                    .iter()
                    .position(|b| {
                        b.weapon == weapon_type && b.location.section != BattleVehicleSection::Left
                    })
                    .unwrap(),
            )
        } else {
            let mut definition = BattleTemplate::parse("test", source).unwrap();
            let bin = definition
                .sections
                .values()
                .flat_map(|s| s.criticals.values())
                .find(|p| p.equipment.starts_with("Ammo_"))
                .unwrap()
                .clone();
            definition
                .sections
                .get_mut(&BattleSection::Head)
                .unwrap()
                .criticals
                .insert(3, bin);
            create_battle_unit(&mut world, id, definition).unwrap();
            support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
            let loadout = world.btech.constructed_units()[&id].loadout().unwrap();
            let weapon_type = loadout
                .ammunition
                .iter()
                .find(|b| b.location.section == BattleSection::Head)
                .unwrap()
                .weapon;
            (
                loadout
                    .weapons
                    .iter()
                    .position(|w| w.weapon == weapon_type)
                    .unwrap(),
                "hd",
                loadout
                    .ammunition
                    .iter()
                    .position(|b| b.location.section == BattleSection::Head)
                    .unwrap(),
                loadout
                    .ammunition
                    .iter()
                    .position(|b| {
                        b.weapon == weapon_type && b.location.section != BattleSection::Head
                    })
                    .unwrap(),
            )
        };
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        assert!(
            set_battle_ammunition_section(&mut world, id, ObjectId(1), weapon, Some(preference))
                .is_err()
        );
        let map = world.create(&config, "Field".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "field",
            MapAsset::from_cells("1 1\n.0\n").unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.usebin({},1,{weapon},'{preference}'); error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        let text = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("usebin {weapon} {preference} ignored"),
        );
        assert!(text.contains("Preferred ammo source set"), "{text}");
        let inspected: String = scripts
            .eval_callback(&format!(
                "return btech.unit.weapon_states({})[{}].preferred_ammunition_section",
                id.0,
                weapon + 1
            ))
            .unwrap();
        assert!(!inspected.is_empty());
        let mut saved = scripts.world().clone();
        assert!(set_battle_ammunition_section(&mut saved, id, ObjectId(2), weapon, None).is_err());
        assert!(set_battle_ammunition_section(&mut saved, id, ObjectId(1), 95, None).is_err());
        let collection = if vehicle { "vehicles" } else { "constructed" };
        let mut state = serde_json::to_value(&saved.btech).unwrap();
        state[collection][id.0.to_string()]["ammunition"][preferred_bin] = 1.into();
        saved.btech = serde_json::from_value(state).unwrap();
        let feed = |world: &World, rounds| {
            if vehicle {
                world.btech.vehicles()[&id]
                    .ammunition_feed(weapon, rounds)
                    .unwrap()
            } else {
                world.btech.constructed_units()[&id]
                    .ammunition_feed(weapon, rounds)
                    .unwrap()
            }
        };
        assert_eq!(
            feed(&saved, 2),
            [
                BattleAmmunitionDraw {
                    bin_index: preferred_bin,
                    rounds: 1
                },
                BattleAmmunitionDraw {
                    bin_index: other_bin,
                    rounds: 1
                }
            ]
        );
        saved.validate(&config).unwrap();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, saved.btech);
        let mut firing = restored.clone();
        let mut powered = serde_json::to_value(&firing.btech).unwrap();
        powered[collection][id.0.to_string()]["power"] =
            serde_json::to_value(BattlePower::Running).unwrap();
        firing.btech = serde_json::from_value(powered).unwrap();
        let spent = if vehicle {
            reserve_battle_vehicle_weapon(&mut firing, id, ObjectId(1), weapon, true)
                .unwrap()
                .ammunition
        } else {
            spend_battle_weapon(&mut firing, id, ObjectId(1), weapon)
                .unwrap()
                .ammunition
        };
        assert_eq!(spent[0].bin_index, preferred_bin);
        assert_eq!(spent[0].rounds, 1);
        assert_eq!(feed(&firing, 1)[0].bin_index, other_bin);
        assert!(set_battle_ammunition_section(&mut firing, id, ObjectId(1), weapon, None).is_err());

        assert_eq!(feed(&restored, 2), feed(&saved, 2));
        if vehicle {
            let location =
                saved.btech.vehicles()[&id].loadout().unwrap().ammunition[preferred_bin].location;
            destroy_battle_vehicle_critical(&mut saved, id, location).unwrap();
        } else {
            let location = saved.btech.constructed_units()[&id]
                .loadout()
                .unwrap()
                .ammunition[preferred_bin]
                .location;
            destroy_battle_critical(&mut saved, id, location).unwrap();
        }
        assert_eq!(feed(&saved, 1)[0].bin_index, other_bin);
        saved.validate(&config).unwrap();
        let mut corrupt = serde_json::to_value(&saved.btech).unwrap();
        corrupt[collection][id.0.to_string()]["ammunition_sections"] =
            serde_json::json!({"95":if vehicle {"left"} else {"Head"}});
        if let Ok(state) = serde_json::from_value(corrupt) {
            let mut invalid = saved.clone();
            invalid.btech = state;
            assert!(invalid.validate(&config).is_err());
        }
        scripts
            .eval_callback::<()>(&format!("btech.unit.usebin({},1,{weapon},nil)", id.0))
            .unwrap();
        assert_eq!(scripts.world().btech, before);
        let _ = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("usebin {weapon} {preference}"),
        );
        let text = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("usebin {weapon} -reset ignored"),
        );
        assert!(text.contains("reset"), "{text}");
        assert_eq!(scripts.world().btech, before);
    }
}

/// Laser AMS draws no ammunition, so these energy weapons are not eligible for usebin.
#[tokio::test]
async fn laser_defense_rejects_preferred_ammunition() {
    for weapon in [BattleWeapon::LaserAms, BattleWeapon::ClanLaserAms] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let id = world.create(&config, "Laser defense".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        let source = include_str!("../game/mechs/Demolisher.toml")
            .replace(
                "    { at = \"3-6\", item = \"Ammo_IS.AC/20\", rounds = 5 },\n",
                "",
            )
            .replace("IS.AC/20", weapon.name());
        create_battle_vehicle(
            &mut world,
            id,
            BattleVehicleTemplate::parse("test", &source).unwrap(),
        )
        .unwrap();
        let map = world.create(&config, "Field".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "field",
            MapAsset::from_cells("1 1\n.0\n").unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        let before = world.btech.clone();
        assert!(
            set_battle_ammunition_section(&mut world, id, ObjectId(1), 0, Some("turret"))
                .unwrap_err()
                .to_string()
                .contains("Energy")
        );
        assert_eq!(world.btech, before);
    }
}
