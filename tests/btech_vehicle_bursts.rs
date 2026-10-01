//! Vehicle firing cycles reserve ordered multi-bin draws and deterministic mode-specific preparation.
use crate::support;
use stompymux_rs::*;

/// A running vehicle with a present pilot, initially disconnected from a session.
async fn fixture(template: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Test field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("test", template).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

/// Set inventory and the saved random stream without changing construction or mode selections.
fn supply(world: &mut World, id: ObjectId, amounts: &[u16]) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["ammunition"] = serde_json::to_value(amounts).unwrap();
    state["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([41; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
}

#[tokio::test]
async fn vehicle_bursts_span_bins_and_persist_single_shot_supply_fallback() {
    for (weapon, flag, mode, rounds) in [
        ("IS.UltraAC/2", "UltraMode", BattleFireMode::Ultra, 2),
        ("IS.AC/2", "RapidFire", BattleFireMode::Rapid, 2),
        (
            "IS.RotaryAC/2",
            "Rotary_TwoShot",
            BattleFireMode::Rotary2,
            2,
        ),
        (
            "IS.RotaryAC/2",
            "Rotary_FourShot",
            BattleFireMode::Rotary4,
            4,
        ),
        (
            "IS.RotaryAC/2",
            "Rotary_SixShot",
            BattleFireMode::Rotary6,
            6,
        ),
    ] {
        let template = include_str!("../game/mechs/Demolisher.toml")
            .replace("IS.AC/20", weapon)
            .replace(
                &format!("item = \"{weapon}\" }}"),
                &format!("item = \"{weapon}\", modes = [\"{flag}\"] }}"),
            )
            .replace(
                "[sections.left_side]\n",
                &format!(
                    "[sections.left_side]\nslots = [{{ at = 1, item = \"Ammo_{weapon}\", rounds = 2 }}]\n"
                ),
            );
        let (_dir, config, base, id) = fixture(&template).await;
        for enough in [true, false] {
            let mut world = base.clone();
            supply(
                &mut world,
                id,
                if enough {
                    &[2, 1, 1, 1, 1]
                } else {
                    &[0, 1, 0, 0, 0]
                },
            );
            let before = world.btech.clone();
            let draws = world.btech.vehicles()[&id]
                .ammunition_feed(0, rounds)
                .unwrap();
            assert_eq!(
                draws[0],
                BattleAmmunitionDraw {
                    bin_index: 1,
                    rounds: 1
                }
            );
            assert_eq!(world.btech, before);
            persistence::save(&config.database(), &world).await.unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            let cycle =
                reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).unwrap();
            assert_eq!(
                cycle,
                reserve_battle_vehicle_weapon(&mut restored, id, ObjectId(1), 0, true).unwrap()
            );
            assert_eq!(world.btech, restored.btech);
            assert_eq!(
                cycle.fire_mode,
                if enough { mode } else { BattleFireMode::Normal }
            );
            assert_eq!(
                cycle.ammunition.iter().map(|draw| draw.rounds).sum::<u16>(),
                if enough { rounds } else { 1 }
            );
            if enough && rounds == 6 {
                assert_eq!(
                    cycle
                        .ammunition
                        .iter()
                        .map(|draw| draw.bin_index)
                        .collect::<Vec<_>>(),
                    [1, 2, 3, 4, 0]
                );
                assert_eq!(cycle.ammunition.last().unwrap().rounds, 2);
            }
            assert_eq!(cycle.gatling_damage, None);
            assert_eq!(
                world.btech.vehicles()[&id].fire_mode(0).unwrap(),
                cycle.fire_mode
            );
            assert_eq!(
                roll_unit_dice(&mut world, id, 1).unwrap(),
                [BattleDice::seeded([41; 32]).d6()]
            );
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
        }
    }
}

#[tokio::test]
async fn vehicle_gatling_caps_damage_by_supply_and_replays_its_single_roll() {
    let template = include_str!("../game/mechs/Demolisher.toml")
        .replace("IS.AC/20", "IS.MachineGun")
        .replace(
            "item = \"IS.MachineGun\" }",
            "item = \"IS.MachineGun\", modes = [\"Gattling\"] }",
        );
    let (_dir, config, base, id) = fixture(&template).await;
    for rounds in [0u16, 1, 2, 3, 5, 6, 17, 18, 30] {
        let mut world = base.clone();
        supply(&mut world, id, &[rounds, 0, 0, 0]);
        let before = world.btech.clone();
        let rejected = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, false);
        assert!(rejected.is_err());
        assert_eq!(world.btech, before);
        if rounds == 0 {
            assert!(reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).is_err());
            assert_eq!(world.btech, before);
            continue;
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let mut dice = BattleDice::seeded([41; 32]);
        let damage = dice.d6().min((rounds / 3).max(1) as u8);
        let cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).unwrap();
        assert_eq!(
            cycle,
            reserve_battle_vehicle_weapon(&mut restored, id, ObjectId(1), 0, true).unwrap()
        );
        assert_eq!(restored.btech, world.btech);
        assert_eq!(cycle.gatling_damage, Some(damage));
        assert_eq!(cycle.fire_mode, BattleFireMode::Gatling);
        assert_eq!(
            cycle.ammunition[0].rounds,
            (u16::from(damage) * 3).min(rounds)
        );
        assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
        let before = world.btech.clone();
        assert!(reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).is_err());
        assert_eq!(world.btech, before);
    }
}

#[tokio::test]
async fn vehicle_feed_skips_lost_and_incompatible_bins_and_heat_cycles_need_no_ammo() {
    let template = include_str!("../game/mechs/Demolisher.toml").replace(
        "item = \"Ammo_IS.AC/20\", rounds = 5 }",
        "item = \"Ammo_IS.AC/20\", rounds = 1, modes = [\"Precision\"] }",
    );
    let (_dir, _config, mut world, id) = fixture(&template).await;
    assert!(
        world.btech.vehicles()[&id]
            .ammunition_feed(0, 6)
            .unwrap()
            .is_empty()
    );
    toggle_battle_precision(&mut world, id, ObjectId(1), 0).unwrap();
    destroy_battle_vehicle_critical(
        &mut world,
        id,
        VehicleCriticalLocation {
            section: BattleVehicleSection::Turret,
            slot: 2,
        },
    )
    .unwrap();
    let vehicle = &world.btech.vehicles()[&id];
    assert!(vehicle.ammunition_feed(0, 0).unwrap().is_empty());
    assert!(vehicle.ammunition_feed(99, 1).is_err());
    let draws = vehicle.ammunition_feed(0, u16::MAX).unwrap();
    assert_eq!(
        draws.iter().map(|draw| draw.bin_index).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    let template = include_str!("../game/mechs/Demolisher.toml").replace(
        "item = \"IS.AC/20\" }",
        "item = \"IS.Flamer\", modes = [\"Heat\"] }",
    );
    let (_dir, _config, mut world, id) = fixture(&template).await;
    let before = world.btech.vehicles()[&id].ammunition().to_vec();
    let cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).unwrap();
    assert_eq!(cycle.fire_mode, BattleFireMode::Heat);
    assert!(cycle.ammunition.is_empty());
    assert_eq!(world.btech.vehicles()[&id].ammunition(), before);
}
