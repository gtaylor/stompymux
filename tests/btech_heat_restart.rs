//! Restarting after a heat shutdown (Total Warfare p. 160): the MechWarrior keeps the cockpit,
//! the reactor restarts by itself below 14 heat, and from 14 to 29 a restart needs a 2d6
//! Avoid Roll once a turn.
use crate::support;
use stompymux_rs::*;

/// A running, piloted Jenner on a one-hex field.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Battlefield".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test.map",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, "Hot reactor".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    world.btech.set_unit_power(id, Power::Running).unwrap();
    // No ammunition, so the Ammunition Avoid Rolls at high heat have nothing to detonate.
    let bins = world.btech.constructed_units()[&id].ammunition().len();
    for index in 0..bins {
        world.btech.set_unit_ammunition_bin(id, index, 0).unwrap();
    }
    (dir, config, world, id)
}

fn rules() -> OverheatRules {
    OverheatRules {
        vehicle_impact: VehicleImpactRules::STANDARD,
        extended_piloting: false,
    }
}

/// Set a sampled heat and make either the unit's Avoid Rolls due, or only its turn boundary.
fn heat_due(world: &mut World, id: ObjectId, heat: f64, turn_start: bool) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["heat"] = serde_json::json!({"stored": heat + 10.0, "excess": heat});
            record["overheat_clock"] = serde_json::json!({
                "elapsed": if turn_start { 0 } else { 30 },
                "phase": 0,
                "injury_due": turn_start
            });
        })
        .unwrap();
}

/// A seed whose first 2d6 roll satisfies the predicate.
fn seed(accept: impl Fn(u8) -> bool) -> Dice {
    (0..=255)
        .map(|byte| Dice::seeded([byte; 32]))
        .find(|dice| accept(dice.clone().two_d6()))
        .unwrap()
}

/// A heat shutdown keeps the pilot; restarting at 14-29 heat takes one Avoid Roll a turn, a
/// failure binds until the next turn, and the turn boundary frees the roll again.
#[tokio::test]
async fn restart_avoid_roll_binds_for_the_turn_and_keeps_the_pilot() {
    let (_dir, config, mut world, id) = fixture().await;
    heat_due(&mut world, id, 30.0, false);
    let report = advance_battle_overheat(&mut world, rules())
        .unwrap()
        .remove(0);
    assert!(report.shutdown);
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.power(), Power::Off);
    assert_eq!(unit.pilot(), Some(ObjectId(1)));
    assert!(unit.heat_shutdown());
    assert!(!unit.restart_denied());
    world.validate(&config).unwrap();
    // At 30 or more heat there is no restart at all.
    assert!(
        start_battle_unit(&mut world, id, ObjectId(1), false)
            .unwrap_err()
            .to_string()
            .contains("too hot")
    );
    // At 22 heat the Avoid Number is 8: a 7 fails and binds the turn.
    heat_due(&mut world, id, 22.0, false);
    world
        .btech
        .set_unit_dice(id, seed(|roll| roll == 7))
        .unwrap();
    let notice = start_battle_unit(&mut world, id, ObjectId(1), false).unwrap();
    assert!(
        notice
            .text
            .contains("Restart Avoid Roll fails: 7 against 8"),
        "{}",
        notice.text
    );
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.power(), Power::Off);
    assert!(unit.restart_denied());
    assert!(unit.heat_shutdown());
    assert!(
        start_battle_unit(&mut world, id, ObjectId(1), false)
            .unwrap_err()
            .to_string()
            .contains("next turn")
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    // The next turn frees the roll; 22 heat is still too hot for an automatic restart.
    heat_due(&mut world, id, 22.0, true);
    assert!(
        advance_battle_overheat(&mut world, rules())
            .unwrap()
            .is_empty()
    );
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.power(), Power::Off);
    assert!(!unit.restart_denied());
    world
        .btech
        .set_unit_dice(id, seed(|roll| roll == 8))
        .unwrap();
    let notice = start_battle_unit(&mut world, id, ObjectId(1), false).unwrap();
    assert_eq!(notice.text, "Startup Cycle commencing...");
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.power(), Power::Starting { remaining: 30 });
    assert!(!unit.heat_shutdown());
    world.validate(&config).unwrap();
    // A voluntary shutdown restarts under the same Avoid Roll (Total Warfare p. 160,
    // Voluntary Shutdown), but never by itself.
    world.btech.set_unit_power(id, Power::Running).unwrap();
    stop_battle_unit(&mut world, id, ObjectId(1), FallRules::STANDARD).unwrap();
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    heat_due(&mut world, id, 13.0, true);
    assert!(
        advance_battle_overheat(&mut world, rules())
            .unwrap()
            .is_empty()
    );
    assert_eq!(world.btech.constructed_units()[&id].power(), Power::Off);
    heat_due(&mut world, id, 14.0, false);
    world
        .btech
        .set_unit_dice(id, seed(|roll| roll == 3))
        .unwrap();
    let notice = start_battle_unit(&mut world, id, ObjectId(1), false).unwrap();
    assert!(notice.text.contains("3 against 4"), "{}", notice.text);
    assert_eq!(world.btech.constructed_units()[&id].power(), Power::Off);
}

/// Below 14 heat a heat-shutdown reactor restarts at the turn boundary without the pilot,
/// even an unconscious one; at 14 it waits.
#[tokio::test]
async fn heat_shutdown_restarts_automatically_below_fourteen() {
    for (heat, unconscious, restarts) in [
        (14.0, false, false),
        (13.99, false, true),
        (13.0, true, true),
    ] {
        let (_dir, config, mut world, id) = fixture().await;
        heat_due(&mut world, id, 30.0, false);
        assert!(
            advance_battle_overheat(&mut world, rules())
                .unwrap()
                .remove(0)
                .shutdown
        );
        if unconscious {
            // A consciousness roll of 2 fails for every pilot.
            let knockout = seed(|roll| roll == 2);
            let mut encoded = serde_json::to_value(&world.btech).unwrap();
            encoded["recoveries"]["1"] = serde_json::json!({
                "mode":{"kind":"tactical","injuries":0}, "remaining":0,
                "pain_resistance":false,"toughness":false,"dice":knockout
            });
            world.btech = serde_json::from_value(encoded).unwrap();
            injure_battle_tactical_pilot(&mut world, id, 1, false).unwrap();
            assert!(world.btech.unconscious(ObjectId(1)));
        }
        heat_due(&mut world, id, heat, true);
        let reports = advance_battle_overheat(&mut world, rules()).unwrap();
        let unit = &world.btech.constructed_units()[&id];
        if !restarts {
            assert!(reports.is_empty(), "{heat}");
            assert_eq!(unit.power(), Power::Off);
            assert!(unit.heat_shutdown());
            continue;
        }
        assert_eq!(reports.len(), 1, "{heat}");
        assert!(reports[0].restart);
        assert!(
            reports[0]
                .notices
                .iter()
                .any(|notice| notice.unit == id && notice.text.contains("restarts automatically"))
        );
        assert_eq!(unit.power(), Power::Starting { remaining: 30 });
        assert!(!unit.heat_shutdown());
        assert_eq!(unit.pilot(), Some(ObjectId(1)));
        world.validate(&config).unwrap();
        for _ in 0..30 {
            advance_battle_units(&mut world, 0, true);
        }
        assert_eq!(world.btech.constructed_units()[&id].power(), Power::Running);
    }
}
