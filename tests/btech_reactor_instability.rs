//! Bulk engine-loss explosions retain timing, shared blast consequences and atomic replay.
use crate::support;
use stompymux_rs::*;

/// Adjust isolated saved scenario facts while retaining production construction and impact paths.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    world.btech.rewrite_unit_record(id, change).unwrap();
}

/// A running reactor with two real engine critical losses and an already damaged center torso.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Instability field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "instability",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, "Unstable Jenner".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        MechTemplate::parse("JR7-D", include_str!("../game/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    let mut unit = world.btech.constructed_units()[&id].clone();
    for slot in [0, 1] {
        unit.destroy_critical(CriticalLocation {
            section: MechSection::CenterTorso,
            slot,
        })
        .unwrap();
    }
    edit(&mut world, id, |state| {
        *state = serde_json::to_value(unit).unwrap();
        state["power"] = serde_json::to_value(Power::Running).unwrap();
        state["sections"]["CenterTorso"]["armor"] = 0.into();
        state["sections"]["CenterTorso"]["internal"] = 10.into();
    });
    world.validate(&config).unwrap();
    (dir, config, world, id)
}

/// Exhaust the remaining core structure through ordinary criticals followed by section destruction.
fn strike(world: &mut World, config: &Config, id: ObjectId) -> TacticalImpact {
    let damage =
        world.btech.constructed_units()[&id].sections()[&MechSection::CenterTorso].internal;
    resolve_battle_tactical_impact(
        world,
        id,
        Hit {
            section: MechSection::CenterTorso,
            rear_armor: false,
            through_armor_critical: false,
            crew_stun: false,
        },
        damage,
        FallRules::configured(config),
    )
    .unwrap()
}

/// Find a reproducible section-loss explosion through the actual damage and critical paths.
fn successful_seed(world: &World, config: &Config, id: ObjectId) -> u8 {
    for seed in 0..=255 {
        let mut candidate = world.clone();
        edit(&mut candidate, id, |state| {
            state["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
        });
        let impact = strike(&mut candidate, config, id).impact;
        if !impact.reactor_explosions.is_empty() && impact.criticals.is_empty() {
            return seed;
        }
    }
    panic!("No engine explosion seed found")
}

/// Initial startup grace and damage windows include their thirtieth second, with power checked last.
#[tokio::test]
async fn instability_window_order_power_and_replay() {
    let (_dir, config, mut world, id) = fixture().await;
    let seed = successful_seed(&world, &config, id);
    edit(&mut world, id, |state| {
        state["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
    });
    for (elapsed, window, power, enabled, detonates) in [
        (0, None, Power::Running, true, true),
        (30, None, Power::Running, true, true),
        (31, None, Power::Running, true, false),
        (31, Some(31), Power::Running, true, true),
        (31, Some(1), Power::Starting { remaining: 3 }, true, true),
        (31, Some(0), Power::Running, true, false),
        (0, None, Power::Off, true, false),
        (0, None, Power::Running, false, false),
    ] {
        let mut candidate = world.clone();
        for _ in 0..elapsed {
            advance_battle_reactor_windows(&mut candidate);
        }
        configure_battle_reactor_policy(&mut candidate, enabled, false);
        edit(&mut candidate, id, |state| {
            state["reactor_instability_remaining"] = serde_json::to_value(window).unwrap();
            state["power"] = serde_json::to_value(power).unwrap();
        });
        let before = candidate.clone();
        let report = strike(&mut candidate, &config, id);
        assert_eq!(
            !report.impact.reactor_explosions.is_empty(),
            detonates,
            "{elapsed} {window:?} {power:?} {enabled}"
        );
        assert!(candidate.btech.constructed_units()[&id].is_destroyed());
        if detonates {
            assert_eq!(report.impact.reactor_explosions.len(), 1);
            assert!(
                candidate.btech.constructed_units()[&id]
                    .sections()
                    .values()
                    .all(|section| section.internal == 0)
            );
        } else {
            assert_eq!(
                candidate.btech.constructed_units()[&id].reactor_instability_remaining(),
                window
            );
        }
        let mut replay = before;
        assert_eq!(strike(&mut replay, &config, id), report);
        assert_eq!(replay.btech, candidate.btech);
        candidate.validate(&config).unwrap();
    }
    // The same failed power gate consumes the eligible roll, while the disabled gate does not.
    let mut off = world.clone();
    edit(&mut off, id, |state| {
        state["power"] = serde_json::to_value(Power::Off).unwrap()
    });
    let mut disabled = off.clone();
    configure_battle_reactor_policy(&mut disabled, false, false);
    strike(&mut off, &config, id);
    strike(&mut disabled, &config, id);
    let a = serde_json::to_value(&off.btech).unwrap();
    let b = serde_json::to_value(&disabled.btech).unwrap();
    assert_ne!(
        a["constructed"][id.0.to_string()]["dice"],
        b["constructed"][id.0.to_string()]["dice"]
    );
}

/// Both initial grace and damaged-reactor timing persist; malformed countdowns are rejected.
#[tokio::test]
async fn instability_clock_and_damage_window_persist() {
    let (_dir, config, mut world, id) = fixture().await;
    for _ in 0..31 {
        advance_battle_reactor_windows(&mut world);
    }
    edit(&mut world, id, |state| {
        state["reactor_instability_remaining"] = 31.into()
    });
    for _ in 0..30 {
        advance_battle_reactor_windows(&mut world);
    }
    assert_eq!(
        world.btech.constructed_units()[&id].reactor_instability_remaining(),
        Some(1)
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    advance_battle_reactor_windows(&mut loaded);
    assert!(!reactor_windows_pending(&loaded));
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, loaded.btech);
    let mut invalid = restored.clone();
    edit(&mut invalid, id, |state| {
        state["reactor_instability_remaining"] = 32.into()
    });
    assert!(invalid.validate(&config).is_err());
    let mut state = serde_json::to_value(&restored.btech).unwrap();
    state["reactor"]["startup_remaining"] = 32.into();
    invalid.btech = serde_json::from_value(state).unwrap();
    assert!(invalid.validate(&config).is_err());
}

/// An ammunition critical cascade reaches the same blast and rolls back casualties and neighbors together.
#[tokio::test]
async fn instability_nested_ammunition_blast_callback_rollback() {
    use std::{cell::RefCell, rc::Rc};
    let (_dir, config, mut world, id) = fixture().await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let neighbor = world.create(&config, "Blast neighbor".into(), Kind::Thing);
    world.objects.get_mut(&neighbor).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        neighbor,
        VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
            .unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, neighbor, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, neighbor, map, 0, 0).unwrap();
    let seed = (0..=255)
        .find(|seed| {
            let mut candidate = world.clone();
            edit(&mut candidate, id, |state| {
                state["dice"] = serde_json::to_value(Dice::seeded([*seed; 32])).unwrap()
            });
            !explode_battle_ammunition(&mut candidate, id, 0, FallRules::configured(&config))
                .unwrap()
                .impact
                .reactor_explosions
                .is_empty()
        })
        .expect("ammunition cascade should be able to destroy the reactor");
    edit(&mut world, id, |state| {
        state["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
    });
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let passenger = world.objects.get_mut(&ObjectId(2)).unwrap();
    passenger.location = Some(id);
    passenger.flags.remove(Flag::Wizard);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let parents: mlua::Table = scripts
        .inspect_lua()
        .named_registry_value("mux.parents")
        .unwrap();
    let parent: mlua::Table = parents.get(world.objects[&id].lua_parent.as_str()).unwrap();
    let previous: mlua::Value = parent.get("events").unwrap();
    let events = scripts.inspect_lua().create_table().unwrap();
    events
        .set(
            "on_leave",
            scripts
                .inspect_lua()
                .create_function(|_, _: mlua::Value| -> mlua::Result<()> {
                    Err(mlua::Error::external("instability casualty failure"))
                })
                .unwrap(),
        )
        .unwrap();
    parent.set("events", events).unwrap();
    let error =
        explode_battle_ammunition_action(&scripts, &config, id, 0, FallRules::configured(&config))
            .unwrap_err();
    assert!(format!("{error:#}").contains("instability casualty failure"));
    assert_eq!(scripts.world().btech, world.btech);
    assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(id));
    assert!(scripts.drain_outbox().is_empty());
    parent.set("events", previous).unwrap();
    let report =
        explode_battle_ammunition_action(&scripts, &config, id, 0, FallRules::configured(&config))
            .unwrap();
    assert_eq!(report.impact.reactor_explosions.len(), 1);
    assert!(
        report.impact.reactor_explosions[0]
            .hits
            .iter()
            .any(|hit| hit.unit == neighbor)
    );
    assert_ne!(
        scripts.world().btech.vehicles()[&neighbor],
        world.btech.vehicles()[&neighbor]
    );
    assert_ne!(scripts.world().objects[&ObjectId(2)].location, Some(id));
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}

/// A stopped reactor's idle clock expires only after the world and clock rows commit together.
#[tokio::test(flavor = "current_thread")]
async fn instability_idle_server_clock_commit_retry() {
    use sqlx::Connection;
    use std::{cell::Cell, rc::Rc};
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world, id) = fixture().await;
        for _ in 0..30 { advance_battle_reactor_windows(&mut world); }
        edit(&mut world, id, |state| {
            state["reactor_instability_remaining"] = 1.into();
            state["power"] = serde_json::to_value(Power::Off).unwrap();
        });
        persistence::save(&config.database(), &world).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("CREATE TRIGGER deny_reactor_clock BEFORE UPDATE ON btech_reactor_clock BEGIN SELECT RAISE(ABORT,'reactor clock commit failure'); END").execute(&mut sql).await.unwrap();
        let (_, shutdown, task, _, mut heartbeats) = support::start(&config, Rc::new(Cell::new(1))).await;
        heartbeats.attempt().await;
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, world.btech);
        sqlx::query("DROP TRIGGER deny_reactor_clock").execute(&mut sql).await.unwrap();
        let loaded = heartbeats.until_saved(&config, 4, |loaded| !reactor_windows_pending(loaded)).await;
        assert_eq!(loaded.btech.constructed_units()[&id].reactor_instability_remaining(), Some(0));
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

/// Section loss samples the threshold after the internal-damage critical roll.
#[tokio::test]
async fn instability_roll_threshold() {
    let (_dir, config, world, id) = fixture().await;
    for threshold in [8, 9] {
        let (seed, expected) = (0..=255)
            .find_map(|seed| {
                let mut candidate = world.clone();
                configure_battle_reactor_policy(&mut candidate, false, false);
                edit(&mut candidate, id, |state| {
                    state["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
                });
                let report = strike(&mut candidate, &config, id);
                if !report.impact.criticals.is_empty() {
                    return None;
                }
                let saved = serde_json::to_value(&candidate.btech).unwrap();
                let mut dice: Dice =
                    serde_json::from_value(saved["constructed"][id.0.to_string()]["dice"].clone())
                        .unwrap();
                (dice.two_d6() == threshold).then_some((seed, threshold >= 9))
            })
            .expect("threshold scenario must be reachable");
        let mut candidate = world.clone();
        edit(&mut candidate, id, |state| {
            state["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
        });
        assert_eq!(
            !strike(&mut candidate, &config, id)
                .impact
                .reactor_explosions
                .is_empty(),
            expected,
            "roll {threshold}"
        );
    }
}

/// Ordinary engine criticals never roll stackpole; a pristine core opens its window before section loss.
#[tokio::test]
async fn instability_distinguishes_critical_and_section_destruction() {
    let (_dir, config, world, id) = fixture().await;
    let mut checked = 0;
    for seed in 0..=255 {
        let mut candidate = world.clone();
        edit(&mut candidate, id, |state| {
            state["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
        });
        let mut disabled = candidate.clone();
        configure_battle_reactor_policy(&mut disabled, false, false);
        let hit = Hit {
            section: MechSection::CenterTorso,
            rear_armor: false,
            through_armor_critical: false,
            crew_stun: false,
        };
        let report = resolve_battle_tactical_impact(
            &mut candidate,
            id,
            hit,
            1,
            FallRules::configured(&config),
        )
        .unwrap();
        if !matches!(
            &report.impact.criticals[..],
            [(
                _,
                CriticalLoss::System {
                    system: System::Engine
                }
            )]
        ) {
            continue;
        }
        let control = resolve_battle_tactical_impact(
            &mut disabled,
            id,
            hit,
            1,
            FallRules::configured(&config),
        )
        .unwrap();
        assert!(report.impact.reactor_explosions.is_empty());
        assert_eq!(report, control);
        assert_eq!(
            candidate.btech.constructed_units()[&id],
            disabled.btech.constructed_units()[&id]
        );
        checked += 1;
    }
    assert!(checked > 0);
    let seed = successful_seed(&world, &config, id);
    let mut pristine = world.clone();
    for _ in 0..31 {
        advance_battle_reactor_windows(&mut pristine);
    }
    edit(&mut pristine, id, |state| {
        state["sections"]["CenterTorso"]["internal"] = 11.into();
        state["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
    });
    assert_eq!(
        strike(&mut pristine, &config, id)
            .impact
            .reactor_explosions
            .len(),
        1
    );
    assert_eq!(
        pristine.btech.constructed_units()[&id].reactor_instability_remaining(),
        Some(31)
    );
}

/// Flooded engine compartments use bulk-loss admission without requiring destroyed internal structure.
#[tokio::test]
async fn instability_flooded_engine_compartment() {
    let (_dir, config, mut world, id) = fixture().await;
    let water = world.create(&config, "Deep water".into(), Kind::Room);
    create_battle_map(
        &mut world,
        water,
        "water",
        MapAsset::from_cells("1 1\n~3\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, water, support::FIXTURE_DICE_SEED);
    edit(&mut world, id, |state| {
        state["power"] = serde_json::to_value(Power::Off).unwrap()
    });
    place_battle_unit(&mut world, id, water, 0, 0).unwrap();
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 9)
        .unwrap();
    for (elapsed, enabled, expected) in [(0, true, true), (31, true, false), (0, false, false)] {
        let mut candidate = world.clone();
        for _ in 0..elapsed {
            advance_battle_reactor_windows(&mut candidate);
        }
        configure_battle_reactor_policy(&mut candidate, enabled, false);
        edit(&mut candidate, id, |state| {
            state["power"] = serde_json::to_value(Power::Running).unwrap();
            state["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        });
        let reports =
            flood_battle_unit(&mut candidate, id, FallRules::configured(&config)).unwrap();
        assert_eq!(
            reports
                .iter()
                .any(|report| report.reactor_explosion.is_some()),
            expected
        );
        assert!(candidate.btech.constructed_units()[&id].is_destroyed());
        if !expected {
            assert_eq!(
                candidate.btech.constructed_units()[&id].sections()[&MechSection::CenterTorso]
                    .internal,
                10
            );
        }
        candidate.validate(&config).unwrap();
    }
}
