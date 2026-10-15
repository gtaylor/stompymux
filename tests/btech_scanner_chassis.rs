//! Automatic perception applies the same rules to every supported observer and target chassis.
use crate::support;
use stompymux_rs::*;

/// Representative supported chassis with stationary movement explicitly authored.
fn templates() -> Vec<String> {
    let tracked = include_str!("../game/units/Demolisher.toml");
    vec![
        include_str!("../game/units/JR7-D.toml").into(),
        include_str!("../game/units/GOL-1H.toml").into(),
        tracked.into(),
        tracked.replace("movement = \"track\"", "movement = \"wheel\""),
        crate::support::templates::hovercraft(tracked),
        tracked
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 5", "walk_mp = 0"),
        include_str!("../game/units/Kestrel.toml").into(),
    ]
}

/// Add probes to unoccupied slots without changing the unit's weapons or required equipment.
fn equipment(source: &str) -> UnitTemplate {
    let mut template =
        crate::support::templates::fitted_unit(UnitTemplate::parse("test", source).unwrap());
    let (section, parts) = match &mut template {
        UnitTemplate::Mech(definition) => {
            let section = definition
                .sections
                .iter_mut()
                .find(|(name, section)| {
                    matches!(
                        **name,
                        MechSection::LeftTorso | MechSection::RightTorso | MechSection::CenterTorso
                    ) && (0..12)
                        .filter(|slot| !section.criticals.contains_key(slot))
                        .count()
                        >= 6
                })
                .unwrap()
                .1;
            (
                section,
                vec![
                    "BeagleProbe",
                    "BeagleProbe",
                    "Light_BAP",
                    "BloodhoundProbe",
                    "BloodhoundProbe",
                    "BloodhoundProbe",
                ],
            )
        }
        UnitTemplate::Vehicle(definition) => (
            definition.sections.get_mut(&VehicleSection::Front).unwrap(),
            vec!["BeagleProbe", "Light_BAP", "BloodhoundProbe"],
        ),
    };
    for name in parts {
        let slot = (0..12)
            .find(|slot| !section.criticals.contains_key(slot))
            .unwrap();
        section.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: name.into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    }
    template
}

/// Change only explicitly selected scenario state, using the same representation as restart.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    world.btech.rewrite_unit_record(id, change).unwrap();
}

/// A running, equipped observer faces an ordinary target down a lane, optionally behind a hill.
async fn fixture(
    source: &str,
    target: &str,
    distance: u16,
    blocked: bool,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Sensor lane".into(), Kind::Room);
    let mut rows = vec![".0\n"; usize::from(distance + 1)];
    if blocked {
        rows[1] = ".9\n";
    }
    create_battle_map(
        &mut world,
        map,
        "sensors",
        MapAsset::from_cells(&format!("1 {}\n{}", distance + 1, rows.concat())).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for (index, template) in [
        equipment(source),
        crate::support::templates::fitted_unit(UnitTemplate::parse("test", target).unwrap()),
    ]
    .into_iter()
    .enumerate()
    {
        let id = world.create(&config, format!("Sensor unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        crate::support::templates::fitted_unit(template)
            .create(&mut world, id)
            .unwrap();
        place_battle_unit(
            &mut world,
            id,
            map,
            0,
            if index == 0 { i64::from(distance) } else { 0 },
        )
        .unwrap();
        edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(Power::Running).unwrap();
        });
        ids.push(id);
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ids[0]);
    assign_battle_pilot(&mut world, ids[0], ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    world.validate(&config).unwrap();
    (dir, config, world, ids[0], ids[1])
}

/// Lift a rotorcraft target into the air at the given altitude.
fn airborne(world: &mut World, id: ObjectId, altitude: f64) {
    edit(world, id, |state| {
        state["vtol_flight"] = serde_json::to_value(VtolFlight {
            phase: VtolFlightPhase::Airborne,
            altitude,
            ..Default::default()
        })
        .unwrap()
    });
}

/// The observer's saved dice stream, which perception never advances.
fn observer_dice(world: &World, id: ObjectId) -> serde_json::Value {
    let key = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    serde_json::to_value(&world.btech).unwrap()[key][id.0.to_string()]["dice"].clone()
}

/// Every observer sees every target chassis at once without dice, airborne VTOLs included.
async fn automatic_scanning_matrix(source: &str) {
    for target_source in templates() {
        let (_dir, config, base, observer, target) =
            fixture(source, &target_source, 3, false).await;
        let vtol = base
            .btech
            .vehicles()
            .get(&target)
            .is_some_and(|unit| unit.definition().is_vtol());
        for flying in [false, true] {
            if flying && !vtol {
                continue;
            }
            let mut world = base.clone();
            if flying {
                airborne(&mut world, target, 5.0);
            }
            assert!(battle_contact_observers(&world).contains(&observer));
            let channel = DetectionChannel::Sight;
            let perception = battle_perceive(&world, observer, target).unwrap().unwrap();
            assert_eq!(
                (
                    perception.channel,
                    perception.aim_modifier,
                    perception.identified,
                    perception.line_of_fire
                ),
                (channel, 0, true, true),
                "flying={flying}"
            );
            let dice = observer_dice(&world, observer);
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            let events = refresh_battle_contacts(&mut world, &[observer]).unwrap();
            assert!(
                events
                    .iter()
                    .any(|event| event.target == target && event.acquired),
                "flying={flying}"
            );
            assert_eq!(
                events,
                refresh_battle_contacts(&mut replay, &[observer]).unwrap()
            );
            assert_eq!(world.btech, replay.btech);
            assert_eq!(observer_dice(&world, observer), dice);
            assert_eq!(
                visible_battle_contact(&world, observer, target)
                    .unwrap()
                    .unwrap()
                    .detection,
                Some(channel)
            );
            world.validate(&config).unwrap();
        }
    }
}

/// One shard per observer chassis; templates are listed in `templates`.
#[tokio::test]
async fn automatic_scanning_covers_all_supported_chassis_pairs_01() {
    automatic_scanning_matrix(&templates()[0]).await;
}

/// Second observer chassis shard.
#[tokio::test]
async fn automatic_scanning_covers_all_supported_chassis_pairs_02() {
    automatic_scanning_matrix(&templates()[1]).await;
}

/// Third observer chassis shard.
#[tokio::test]
async fn automatic_scanning_covers_all_supported_chassis_pairs_03() {
    automatic_scanning_matrix(&templates()[2]).await;
}

/// Fourth observer chassis shard.
#[tokio::test]
async fn automatic_scanning_covers_all_supported_chassis_pairs_04() {
    automatic_scanning_matrix(&templates()[3]).await;
}

/// Fifth observer chassis shard.
#[tokio::test]
async fn automatic_scanning_covers_all_supported_chassis_pairs_05() {
    automatic_scanning_matrix(&templates()[4]).await;
}

/// Sixth observer chassis shard.
#[tokio::test]
async fn automatic_scanning_covers_all_supported_chassis_pairs_06() {
    automatic_scanning_matrix(&templates()[5]).await;
}

/// Seventh observer chassis shard.
#[tokio::test]
async fn automatic_scanning_covers_all_supported_chassis_pairs_07() {
    automatic_scanning_matrix(&templates()[6]).await;
}

/// Add one machine gun and a normal bin to unused slots without replacing probe hardware.
fn install_gatling(world: &mut World, id: ObjectId, supply: u16) -> usize {
    let vehicle = world.btech.vehicles().contains_key(&id);
    let mut template = if vehicle {
        UnitTemplate::Vehicle(world.btech.vehicles()[&id].definition().clone())
    } else {
        UnitTemplate::Mech(world.btech.constructed_units()[&id].definition().clone())
    };
    let section = match &mut template {
        UnitTemplate::Mech(definition) => {
            definition
                .sections
                .iter_mut()
                .find(|(name, section)| {
                    matches!(name, MechSection::LeftTorso | MechSection::RightTorso)
                        && (0..12)
                            .filter(|slot| !section.criticals.contains_key(slot))
                            .count()
                            >= 2
                })
                .unwrap()
                .1
        }
        UnitTemplate::Vehicle(definition) => {
            definition.sections.get_mut(&VehicleSection::Front).unwrap()
        }
    };
    let slots: Vec<_> = (0..12)
        .filter(|slot| !section.criticals.contains_key(slot))
        .take(2)
        .collect();
    section.criticals.insert(
        slots[0],
        CriticalDefinition {
            equipment: "IS.MachineGun".into(),
            data: "-".into(),
            modes: vec!["Gattling".into()],
        },
    );
    section.criticals.insert(
        slots[1],
        CriticalDefinition {
            equipment: "Ammo_IS.MachineGun".into(),
            data: supply.to_string(),
            modes: vec![],
        },
    );
    let (definition, ammunition, index) = match template {
        UnitTemplate::Mech(definition) => {
            let unit = Mech::from_template(definition.clone()).unwrap();
            let index = unit
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|mount| {
                    mount.weapon == Weapon::MachineGun
                        && mount.initial_fire_mode == FireMode::Gatling
                })
                .unwrap();
            let mut ammunition = unit.ammunition().to_vec();
            let loadout = unit.loadout().unwrap();
            let bin_index = loadout
                .ammunition
                .iter()
                .rposition(|bin| bin.weapon == Weapon::MachineGun && bin.capacity >= supply)
                .unwrap();
            for (i, bin) in loadout.ammunition.iter().enumerate() {
                if bin.weapon == Weapon::MachineGun {
                    ammunition[i] = if i == bin_index { supply } else { 0 };
                }
            }
            (
                serde_json::to_value(unit.definition()).unwrap(),
                serde_json::to_value(ammunition).unwrap(),
                index,
            )
        }
        UnitTemplate::Vehicle(definition) => {
            // The saved record keeps the edited definition; a constructible copy of it, with
            // the same slots, locates the weapon and bins.
            let unit = Vehicle::new(crate::support::templates::fitted(definition.clone())).unwrap();
            let index = unit
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|mount| {
                    mount.weapon == Weapon::MachineGun
                        && mount.initial_fire_mode == FireMode::Gatling
                })
                .unwrap();
            let mut ammunition = unit.ammunition().to_vec();
            let loadout = unit.loadout().unwrap();
            let bin_index = loadout
                .ammunition
                .iter()
                .rposition(|bin| bin.weapon == Weapon::MachineGun && bin.capacity >= supply)
                .unwrap();
            for (i, bin) in loadout.ammunition.iter().enumerate() {
                if bin.weapon == Weapon::MachineGun {
                    ammunition[i] = if i == bin_index { supply } else { 0 };
                }
            }
            (
                serde_json::to_value(&definition).unwrap(),
                serde_json::to_value(ammunition).unwrap(),
                index,
            )
        }
    };
    edit(world, id, |state| {
        state["definition"] = definition;
        state["ammunition"] = ammunition;
        state["fire_modes"][index.to_string()] = serde_json::to_value(FireMode::Gatling).unwrap();
    });
    index
}

/// Gatling preparation precedes the attack roll on every chassis, with one roll and atomic replay.
#[tokio::test]
async fn gatling_attack_order_replays_across_chassis() {
    for source in templates() {
        for supply in [2, 200] {
            let (_dir, config, mut world, shooter, target) =
                fixture(&source, include_str!("../game/units/JR7-D.toml"), 1, false).await;
            let index = install_gatling(&mut world, shooter, supply);
            edit(&mut world, shooter, |state| {
                state["dice"] = serde_json::to_value(Dice::seeded([17; 32])).unwrap();
            });
            edit(&mut world, target, |state| {
                state["dice"] = serde_json::to_value(Dice::seeded([42; 32])).unwrap();
            });
            refresh_battle_contacts(&mut world, &[shooter]).unwrap();
            let perception = battle_perceive(&world, shooter, target).unwrap().unwrap();
            assert_eq!(
                (perception.channel, perception.aim_modifier),
                (DetectionChannel::Sight, 0)
            );
            let mut expected = Dice::seeded([17; 32]);
            let damage = expected.d6().min((supply.min(18) / 3).max(1) as u8);
            let roll = expected.two_d6();
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let before = scripts.world().clone();
            let preview = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(before.clone())),
            )
            .unwrap();
            let sight: (u8, u8, i16) = preview.eval_callback(&format!("local r=btech.unit.sight({},1,{index},{}); return r.roll,r.gatling_roll,r.aim.perception.modifier", shooter.0, target.0)).unwrap();
            assert_eq!(
                sight,
                (roll, Dice::seeded([17; 32]).d6(), perception.aim_modifier)
            );
            let mut sight_expected = before.clone();
            edit(&mut sight_expected, shooter, |state| {
                state["dice"] = serde_json::to_value(&expected).unwrap()
            });
            assert_eq!(preview.world().btech, sight_expected.btech);

            let command = format!("btech.unit.fire({},1,{index},{})", shooter.0, target.0);
            assert!(
                scripts
                    .eval_callback::<()>(&format!("{command}; error('abort')"))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, before.btech);
            assert!(scripts.drain_outbox().is_empty());
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            let native =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
            let result: (u8, u8, i16, bool) = scripts.eval_callback(&format!("local r={command}; return r.roll,r.expenditure.gatling_damage,r.aim.perception.modifier,r.missed_terrain ~= nil")).unwrap();
            assert_eq!(
                (result.0, result.1, result.2),
                (roll, damage, perception.aim_modifier)
            );
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire {index} #{}", target.0),
            );
            assert!(text.contains("You fire"), "{text}");
            assert_eq!(native.world().btech, scripts.world().btech);
            let key = if scripts.world().btech.vehicles().contains_key(&shooter) {
                "vehicles"
            } else {
                "constructed"
            };
            assert_eq!(
                serde_json::to_value(&scripts.world().btech).unwrap()[key][shooter.0.to_string()]["dice"],
                serde_json::to_value(expected).unwrap()
            );
            scripts.world().validate(&config).unwrap();
        }
    }
}
