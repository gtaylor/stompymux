//! Reactor blasts share damage across chassis, crews and neighboring terrain.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Change isolated scenario facts without altering the production control interfaces.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    let vehicle = world.btech.vehicles().contains_key(&id);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    change(&mut state[if vehicle { "vehicles" } else { "constructed" }][id.0.to_string()]);
    world.btech = serde_json::from_value(state).unwrap();
}

/// Construct supported targets with durable map membership and reproducible random streams.
fn unit(
    world: &mut World,
    config: &Config,
    map: ObjectId,
    chassis: &str,
    x: u16,
    y: u16,
) -> ObjectId {
    let id = world.create(config, chassis.into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    match chassis {
        "biped" | "quad" => create_battle_unit(
            world,
            id,
            BattleTemplate::parse(
                "test",
                if chassis == "quad" {
                    include_str!("../game/mechs/GOL-1H.toml")
                } else {
                    include_str!("../game/mechs/Daishi-H.toml")
                },
            )
            .unwrap(),
        )
        .unwrap(),
        other => {
            let source = match other {
                "vtol" => include_str!("../game/mechs/Kestrel.toml").to_owned(),
                "wheel" => include_str!("../game/mechs/Demolisher.toml")
                    .replace("movement = \"track\"", "movement = \"wheel\""),
                "hover" => include_str!("../game/mechs/Demolisher.toml")
                    .replace("movement = \"track\"", "movement = \"hover\""),
                "stationary" => include_str!("../game/mechs/Demolisher.toml")
                    .replace("movement = \"track\"", "movement = \"none\"")
                    .replace("walk_mp = 5", "walk_mp = 0"),
                _ => include_str!("../game/mechs/Demolisher.toml").to_owned(),
            };
            create_battle_vehicle(
                world,
                id,
                BattleVehicleTemplate::parse("test", &source).unwrap(),
            )
            .unwrap();
        }
    };
    place_battle_unit(world, id, map, i64::from(x), i64::from(y)).unwrap();
    edit(world, id, |state| {
        state["dice"] = serde_json::to_value(BattleDice::seeded([17; 32])).unwrap()
    });
    id
}

/// Full damage at the center, distance falloff, shared packets, boundaries and rollback.
#[tokio::test]
async fn reactor_blast_cross_chassis_and_atomic_replay() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Blast field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "blast",
        BattleMapAsset::from_cells(&format!("7 7\n{}", "\"0\"0\"0\"0\"0\"0\"0\n".repeat(7)))
            .unwrap(),
    )
    .unwrap();
    let source = unit(&mut world, &config, map, "biped", 3, 3);
    let mut targets = Vec::new();
    for chassis in [
        "biped",
        "quad",
        "track",
        "wheel",
        "hover",
        "stationary",
        "vtol",
    ] {
        targets.push(unit(&mut world, &config, map, chassis, 3, 3));
    }
    let near = unit(&mut world, &config, map, "biped", 3, 4);
    let far = unit(&mut world, &config, map, "biped", 3, 5);
    let outside = unit(&mut world, &config, map, "biped", 3, 6);
    let high = unit(&mut world, &config, map, "biped", 3, 3);
    edit(&mut world, high, |s| s["ground_elevation"] = 3.into());
    let low = unit(&mut world, &config, map, "biped", 3, 3);
    edit(&mut world, low, |s| s["ground_elevation"] = (-5).into());
    world.validate(&config).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.reactor_explode({}); error('abort')",
                source.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, world.btech);
    let report = reactor_explosion_action(&scripts, &config, source).unwrap();
    assert!(
        !report
            .hits
            .iter()
            .any(|h| [source, outside, high, low].contains(&h.unit))
    );
    assert_eq!(report.ignited.len(), 18);
    assert!(!report.ignited.contains(&BattleHexCoordinate { x: 3, y: 3 }));
    for coordinate in &report.ignited {
        let decoration = scripts.world().btech.maps()[&map]
            .decoration(*coordinate)
            .unwrap()
            .unwrap();
        assert_eq!(decoration.kind, BattleDecorationKind::Fire);
        assert!((60..=180).contains(&decoration.remaining));
    }
    for id in targets {
        let hit = report.hits.iter().find(|h| h.unit == id).unwrap();
        assert_eq!((hit.damage, hit.heat, hit.impacts.len()), (30, 12, 10));
    }
    assert_eq!(
        report.hits.iter().find(|h| h.unit == near).unwrap().damage,
        15
    );
    assert_eq!(
        report.hits.iter().find(|h| h.unit == far).unwrap().damage,
        10
    );
    assert!(
        scripts.world().btech.constructed_units()[&source]
            .sections()
            .values()
            .all(|s| s.internal == 0 && s.armor == 0 && s.rear == 0)
    );
    let after = scripts.world().clone();
    // The common blast path uses the recipient as its damage author in the
    // reference. Neither the reactor nor another recipient gains inflicted
    // damage from this explosion, including nested critical consequences.
    let statistics = serde_json::to_value(&after.btech).unwrap();
    for owner in ["constructed", "vehicles"] {
        for (id, unit) in statistics[owner].as_object().unwrap() {
            assert_eq!(unit["damage_counters"]["inflicted"], 0, "{owner} {id}");
            assert_eq!(
                unit["units_killed"], 0,
                "{owner} {id}: self-attributed blast"
            );
        }
    }
    for id in [source, outside, high, low] {
        assert_eq!(
            statistics["constructed"][id.0.to_string()]["damage_counters"]["taken"],
            0,
            "excluded recipient {id:?}"
        );
    }
    for hit in &report.hits {
        let owner = if after.btech.vehicles().contains_key(&hit.unit) {
            "vehicles"
        } else {
            "constructed"
        };
        assert!(
            statistics[owner][hit.unit.0.to_string()]["damage_counters"]["taken"]
                .as_i64()
                .unwrap()
                > 0,
            "admitted recipient {:?} must record incoming damage",
            hit.unit
        );
    }
    assert!(reactor_explosion_action(&scripts, &config, source).is_err());
    assert_eq!(scripts.world().btech, after.btech);
    persistence::save(&config.database(), &after).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, after.btech);
    let replay = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert_eq!(
        reactor_explosion_action(&replay, &config, source).unwrap(),
        report
    );
    assert_eq!(replay.world().btech, after.btech);
}

/// A failed casualty departure restores the source, neighboring damage and all staged effects.
#[tokio::test]
async fn reactor_casualties_and_callback_failure_are_atomic() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Crew field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "crew",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let source = unit(&mut world, &config, map, "quad", 0, 0);
    unit(&mut world, &config, map, "track", 0, 0);
    world
        .objects
        .get_mut(&source)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let pilot = ObjectId(2);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&pilot).unwrap().location = Some(source);
    set_battle_character(
        &mut world,
        pilot,
        BattleCharacter {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    assign_battle_pilot(&mut world, source, pilot).unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    injure_battle_character_pilot(&mut world, source, 2, false).unwrap();
    let before = world.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let parents: mlua::Table = scripts
        .inspect_lua()
        .named_registry_value("mux.parents")
        .unwrap();
    let parent: mlua::Table = parents
        .get(scripts.world().objects[&source].lua_parent.as_str())
        .unwrap();
    let previous: mlua::Value = parent.get("events").unwrap();
    let events = scripts.inspect_lua().create_table().unwrap();
    let failure = scripts
        .inspect_lua()
        .create_function(|_, _: mlua::Value| -> mlua::Result<()> {
            Err(mlua::Error::external("reactor casualty departure failed"))
        })
        .unwrap();
    events.set("on_leave", failure).unwrap();
    parent.set("events", events).unwrap();
    assert!(reactor_explosion_action(&scripts, &config, source).is_err());
    assert_eq!(scripts.world().btech, before.btech);
    assert_eq!(scripts.world().objects[&pilot].location, Some(source));
    parent.set("events", previous).unwrap();
    let report = reactor_explosion_action(&scripts, &config, source).unwrap();
    // Two existing pilot injuries plus the reactor event's four-hit packet.
    assert_eq!(report.crew_injury.as_ref().unwrap().injuries, 6);
    assert!(report.crew_injury.as_ref().unwrap().killed);
    assert_eq!(
        scripts.world().btech.characters(),
        before.btech.characters()
    );
    assert_eq!(
        scripts.world().btech.constructed_units()[&source]
            .crew_recovery()
            .remaining,
        0
    );
    assert_eq!(
        scripts.world().objects[&pilot].location,
        Some(ObjectId(config.battletech.afterlife_dbref))
    );
    scripts.world().validate(&config).unwrap();
}

/// A compact battlefield for reactor reentry and chains, with the same construction adapters as radial tests.
async fn chain_fixture() -> (
    tempfile::TempDir,
    Config,
    World,
    ObjectId,
    ObjectId,
    ObjectId,
) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Chain field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "chain",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let first = unit(&mut world, &config, map, "biped", 0, 0);
    let second = unit(&mut world, &config, map, "biped", 0, 0);
    let vehicle = unit(&mut world, &config, map, "track", 0, 0);
    (dir, config, world, first, second, vehicle)
}

/// A manual detonation can trigger one earlier blast through its own engine compartment loss.
#[tokio::test]
async fn reactor_manual_destruction_preserves_bounded_reentry() {
    let (_dir, config, world, source, _, vehicle) = chain_fixture().await;
    for (roll, power, elapsed, window, repeats) in [
        (8, BattlePower::Running, 0, None, false),
        (9, BattlePower::Running, 0, None, true),
        (9, BattlePower::Starting { remaining: 3 }, 0, None, true),
        (9, BattlePower::Off, 0, None, false),
        (9, BattlePower::Running, 31, None, false),
        (9, BattlePower::Running, 31, Some(1), true),
    ] {
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
            .unwrap();
        let mut candidate = world.clone();
        for _ in 0..elapsed {
            advance_battle_reactor_windows(&mut candidate);
        }
        edit(&mut candidate, source, |state| {
            state["power"] = serde_json::to_value(power).unwrap();
            state["reactor_instability_remaining"] = serde_json::to_value(window).unwrap();
            state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        });
        let before = candidate.clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(candidate))).unwrap();
        let report = reactor_explosion_action(&scripts, &config, source).unwrap();
        assert_eq!(
            report.section_explosion.is_some(),
            repeats,
            "{roll} {power:?} {elapsed} {window:?}"
        );
        if let Some(nested) = &report.section_explosion {
            assert_eq!(nested.unit, source);
            assert!(nested.section_explosion.is_none());
            assert_eq!(nested.crew_injury.as_ref().unwrap().injuries, 4);
            assert_eq!(report.crew_injury.as_ref().unwrap().injuries, 8);
            assert!(nested.hits.iter().any(|hit| hit.unit == vehicle));
        } else {
            assert_eq!(report.crew_injury.as_ref().unwrap().injuries, 4);
        }
        assert!(report.hits.iter().any(|hit| hit.unit == vehicle));
        let replay = Scripts::new(&config, Rc::new(RefCell::new(before))).unwrap();
        assert_eq!(
            reactor_explosion_action(&replay, &config, source).unwrap(),
            report
        );
        assert_eq!(replay.world().btech, scripts.world().btech);
        let after = scripts.world().clone();
        assert!(reactor_explosion_action(&scripts, &config, source).is_err());
        assert_eq!(scripts.world().btech, after.btech);
        after.validate(&config).unwrap();
    }
}

/// Locate a blast caused by damage to a neighboring Mech, retaining its report inside the initiating packet.
fn neighbor_blast(
    report: &BattleReactorExplosion,
    neighbor: ObjectId,
) -> Option<&BattleReactorExplosion> {
    report
        .hits
        .iter()
        .flat_map(|hit| &hit.impacts)
        .find_map(|impact| {
            let BattleBlastImpact::Mech(impact) = impact else {
                return None;
            };
            impact
                .impact
                .reactor_explosions
                .iter()
                .find(|blast| blast.unit == neighbor)
        })
}

/// Chained reactors share one transaction, terminate on lost engines, and replay across callback failure and reload.
#[tokio::test]
async fn reactor_chain_replay_and_casualty_rollback() {
    let (_dir, config, mut world, first, second, vehicle) = chain_fixture().await;
    let mut target = world.btech.constructed_units()[&second].clone();
    for slot in [0, 1] {
        target
            .destroy_critical(CriticalLocation {
                section: BattleSection::CenterTorso,
                slot,
            })
            .unwrap();
    }
    edit(&mut world, second, |state| {
        *state = serde_json::to_value(target).unwrap();
        state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
        state["reactor_instability_remaining"] = 31.into();
        state["sections"]["CenterTorso"]["internal"] = 1.into();
        state["sections"]["CenterTorso"]["armor"] = 0.into();
        state["sections"]["CenterTorso"]["rear"] = 0.into();
    });
    world
        .objects
        .get_mut(&second)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(second);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let seed = (0..=255)
        .find(|seed| {
            let mut candidate = world.clone();
            edit(&mut candidate, second, |state| {
                state["dice"] = serde_json::to_value(BattleDice::seeded([*seed; 32])).unwrap()
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(candidate))).unwrap();
            neighbor_blast(
                &reactor_explosion_action(&scripts, &config, first).unwrap(),
                second,
            )
            .is_some()
        })
        .expect("neighboring reactor chain should be reachable");
    edit(&mut world, second, |state| {
        state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
    });
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let parents: mlua::Table = scripts
        .inspect_lua()
        .named_registry_value("mux.parents")
        .unwrap();
    let parent: mlua::Table = parents
        .get(world.objects[&second].lua_parent.as_str())
        .unwrap();
    let prior: mlua::Value = parent.get("events").unwrap();
    let events = scripts.inspect_lua().create_table().unwrap();
    events
        .set(
            "on_leave",
            scripts
                .inspect_lua()
                .create_function(|_, _: mlua::Value| -> mlua::Result<()> {
                    Err(mlua::Error::external("chain departure failure"))
                })
                .unwrap(),
        )
        .unwrap();
    parent.set("events", events).unwrap();
    let error = reactor_explosion_action(&scripts, &config, first).unwrap_err();
    assert!(format!("{error:#}").contains("chain departure failure"));
    assert_eq!(scripts.world().btech, world.btech);
    assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(second));
    assert!(scripts.drain_outbox().is_empty());
    parent.set("events", prior).unwrap();
    let report = reactor_explosion_action(&scripts, &config, first).unwrap();
    assert!(report.section_explosion.is_none());
    let nested = neighbor_blast(&report, second).unwrap();
    assert!(nested.section_explosion.is_none());
    assert!(nested.hits.iter().any(|hit| hit.unit == first));
    assert!(nested.hits.iter().any(|hit| hit.unit == vehicle));
    assert!(
        nested
            .hits
            .iter()
            .flat_map(|hit| &hit.impacts)
            .all(|impact| match impact {
                BattleBlastImpact::Mech(impact) => impact.impact.reactor_explosions.is_empty(),
                BattleBlastImpact::Vehicle(_) => true,
            })
    );
    assert_ne!(scripts.world().objects[&ObjectId(2)].location, Some(second));
    let after = scripts.world().clone();
    persistence::save(&config.database(), &after).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        after.btech
    );
    let replay = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert_eq!(
        reactor_explosion_action(&replay, &config, first).unwrap(),
        report
    );
    assert_eq!(replay.world().btech, after.btech);
}

/// Nested reactor warnings precede neighboring pilot rolls without leaking them to passengers.
#[tokio::test]
async fn reactor_chain_preserves_private_packet_feedback() {
    let (_dir, config, mut baseline, source, target, _) = chain_fixture().await;
    let pilot = ObjectId(1);
    baseline.objects.get_mut(&pilot).unwrap().location = Some(target);
    baseline
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(&mut baseline, target, pilot).unwrap();
    let passenger = baseline.create(&config, "Passenger".into(), Kind::Player);
    baseline.objects.get_mut(&passenger).unwrap().location = Some(target);
    baseline
        .objects
        .get_mut(&passenger)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let source_seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 9)
        .unwrap();
    edit(&mut baseline, source, |state| {
        state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
        state["dice"] = serde_json::to_value(BattleDice::seeded([source_seed; 32])).unwrap();
    });
    edit(&mut baseline, target, |state| {
        state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
        for section in ["LeftLeg", "RightLeg"] {
            state["sections"][section]["armor"] = 0.into();
            state["sections"][section]["internal"] = 1.into();
        }
    });
    for seed in 0..64 {
        let mut world = baseline.clone();
        edit(&mut world, target, |state| {
            state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        });
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let report = reactor_explosion_action(&scripts, &config, source).unwrap();
        let nested = report
            .section_explosion
            .as_ref()
            .expect("source instability causes an earlier blast");
        if nested.pilot_notices.is_empty() {
            continue;
        }
        let output = scripts.drain_outbox();
        let actual: Vec<_> = output
            .iter()
            .filter(|(_, message)| {
                message.source() == "You make a piloting skill roll!"
                    || message.source().starts_with("Modified Pilot Skill:")
            })
            .map(|(who, message)| (*who, message.source().to_owned()))
            .collect();
        assert_eq!(
            actual,
            report
                .pilot_notices
                .iter()
                .map(|notice| (notice.pilot, notice.text.clone()))
                .collect::<Vec<_>>()
        );
        assert!(actual.iter().all(|(who, _)| *who == pilot));
        assert!(output.iter().any(|(who, _)| *who == passenger));
        let warning = output
            .iter()
            .position(|(_, message)| message.source().contains("last safety systems"))
            .unwrap();
        let first_check = output
            .iter()
            .position(|(_, message)| message.source() == "You make a piloting skill roll!")
            .unwrap();
        assert!(warning < first_check);
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.reactor_explode({}); error('abort')",
                source.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        assert!(lua.drain_outbox().is_empty());
        lua.eval_callback::<()>(&format!("btech.unit.reactor_explode({})", source.0))
            .unwrap();
        assert_eq!(lua.world().btech, scripts.world().btech);
        assert_eq!(lua.drain_outbox(), output);
        return;
    }
    panic!("nested reactor blast must exercise private control feedback");
}
