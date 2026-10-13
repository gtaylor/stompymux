//! Vehicle hit routing, motive effects and protection changes commit as one replayable impact.
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
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        VehicleTemplate::parse("test", template).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    (dir, config, world, id)
}

/// Find a deterministic stream for a specific multi-stage explosion path.
fn matching_seed(predicate: impl Fn(&mut Dice) -> bool) -> [u8; 32] {
    for value in 0u32..100000 {
        let mut seed = [0; 32];
        seed[..4].copy_from_slice(&value.to_le_bytes());
        if predicate(&mut Dice::seeded(seed)) {
            return seed;
        }
    }
    panic!("No deterministic stream matched");
}

/// Change only the victim's random stream.
fn set_seed(world: &mut World, id: ObjectId, seed: [u8; 32]) {
    world.btech.set_unit_dice(id, Dice::seeded(seed)).unwrap();
}

fn rules() -> VehicleImpactRules {
    VehicleImpactRules::STANDARD
}

#[tokio::test]
async fn complete_impacts_apply_hit_effects_and_damage_with_saved_replay() {
    for (arc, roll, section, motive) in [
        (HitArc::Left, 7, VehicleSection::Left, false),
        (HitArc::Front, 5, VehicleSection::Right, true),
    ] {
        let (_dir, config, mut world, id) =
            fixture(include_str!("../game/units/Demolisher.toml")).await;
        // Location roll, then a motive roll too low to matter on a dagger row.
        let stream = matching_seed(|dice| dice.two_d6() == roll && (!motive || dice.two_d6() <= 5));
        set_seed(&mut world, id, stream);
        let before = world.btech.vehicles()[&id].clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let report = resolve_battle_vehicle_impact(&mut world, id, arc, 1, None, rules()).unwrap();
        assert_eq!(
            report,
            resolve_battle_vehicle_impact(&mut restored, id, arc, 1, None, rules()).unwrap()
        );
        assert_eq!(world.btech, restored.btech);
        assert_eq!(report.rolls, vec![roll]);
        let hit = report.hit.unwrap();
        assert_eq!(hit.section, section);
        assert_eq!(hit.motive.is_some(), motive);
        assert_eq!(
            world.btech.vehicles()[&id].sections()[&section].armor,
            before.sections()[&section].armor - 1
        );
        assert_eq!(
            world.btech.vehicles()[&id].maximum_speed(),
            before.maximum_speed()
        );
        assert!(!world.btech.vehicles()[&id].turret_locked());
        let mut dice = Dice::seeded(stream);
        dice.two_d6();
        if motive {
            dice.two_d6();
        }
        dice.two_d6();
        assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            world.btech,
            persistence::load(&config.database()).await.unwrap().btech
        );
    }
}

#[tokio::test]
async fn hull_impacts_preserve_occupants_and_combat_safety_preserves_material() {
    let (_dir, _config, mut world, id) =
        fixture(include_str!("../game/units/Demolisher.toml")).await;
    let stream = matching_seed(|dice| dice.two_d6() == 7);
    set_seed(&mut world, id, stream);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let before = world.btech.clone();
    let report =
        resolve_battle_vehicle_impact(&mut world, id, HitArc::Left, 40, None, rules()).unwrap();
    assert!(report.damage.unwrap().unit_destroyed);
    assert!(!world.btech.vehicles()[&id].crew_killed());
    assert_eq!(world.objects[&ObjectId(1)].location, Some(id));
    world.btech = before.clone();
    assert!(resolve_battle_vehicle_impact(&mut world, id, HitArc::Left, 0, None, rules()).is_err());
    assert_eq!(world.btech, before);
    let sections = world.btech.vehicles()[&id].sections().clone();
    let mut policy = rules();
    policy.criticals.combat_safe = true;
    let report =
        resolve_battle_vehicle_impact(&mut world, id, HitArc::Left, 40, None, policy).unwrap();
    assert!(report.hit.is_none());
    assert!(report.damage.is_none());
    assert!(report.notices.is_empty());
    assert_eq!(world.btech.vehicles()[&id].sections(), &sections);
    assert_eq!(world.btech.vehicles()[&id].motive_speed_loss(), 0.0);
    let mut dice = Dice::seeded(stream);
    dice.two_d6();
    dice.two_d6();
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
}

#[tokio::test]
async fn motive_impacts_commit_steering_speed_and_penetration_together() {
    let text = include_str!("../game/units/Demolisher.toml").replace("armor = 40", "armor = 0");
    let (_dir, config, mut world, id) = fixture(&text).await;
    // Location 3 (front, motive roll), motive 8 (tracked: −1 MP and +2 driving), entry roll,
    // then a critical roll below six for the point of internal damage.
    let stream = matching_seed(|dice| {
        dice.two_d6() == 3 && dice.two_d6() == 8 && {
            dice.two_d6();
            dice.two_d6() < 6
        }
    });
    set_seed(&mut world, id, stream);
    world
        .btech
        .edit_unit_motion(id, |motion| {
            motion.speed = 10.0;
            motion.desired_speed = 10.0;
        })
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    let report =
        resolve_battle_vehicle_impact(&mut world, id, HitArc::Front, 1, None, rules()).unwrap();
    assert_eq!(
        report,
        resolve_battle_vehicle_impact(&mut restored, id, HitArc::Front, 1, None, rules()).unwrap()
    );
    assert_eq!(world.btech, restored.btech);
    let motive = report.hit.unwrap().motive.unwrap();
    assert_eq!(motive.roll, 8);
    assert_eq!(motive.modifier, 0);
    assert_eq!(
        motive.hit,
        Some(VehicleMotiveHit::SpeedLoss { movement_points: 1 })
    );
    assert_eq!(motive.driving_penalty, 2);
    assert_eq!(world.btech.vehicles()[&id].piloting_damage(), 2);
    assert_eq!(world.btech.vehicles()[&id].maximum_speed(), 43.0);
    assert_eq!(
        world.btech.vehicles()[&id].sections()[&VehicleSection::Front].internal,
        7
    );
    assert!(
        report
            .broadcasts
            .iter()
            .any(|notice| notice.text == "wobbles violently.")
    );
    let mut dice = Dice::seeded(stream);
    for _ in 0..4 {
        dice.two_d6();
    }
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        world.btech,
        persistence::load(&config.database()).await.unwrap().btech
    );
}

#[tokio::test]
async fn driving_penalties_apply_once_per_tier_and_an_immobilized_hovercraft_sinks() {
    let text = include_str!("../game/units/Demolisher.toml")
        .replace("movement = \"track\"", "movement = \"hover\"");
    let (_dir, config, mut world, id) = fixture(&text).await;
    // Hover +3 from the front: a motive roll of 3 is a 6 (+1 driving) each time.
    for expected in [1, 1] {
        set_seed(
            &mut world,
            id,
            matching_seed(|dice| dice.two_d6() == 3 && dice.two_d6() == 3),
        );
        let _ =
            resolve_battle_vehicle_impact(&mut world, id, HitArc::Front, 1, None, rules()).unwrap();
        assert_eq!(world.btech.vehicles()[&id].piloting_damage(), expected);
    }
    // The −1 MP tier still stacks its movement loss while its +2 applies only once.
    for (expected_penalty, expected_speed) in [(3, 43.0), (3, 32.25)] {
        set_seed(
            &mut world,
            id,
            matching_seed(|dice| dice.two_d6() == 3 && dice.two_d6() == 5),
        );
        let _ =
            resolve_battle_vehicle_impact(&mut world, id, HitArc::Front, 1, None, rules()).unwrap();
        assert_eq!(
            world.btech.vehicles()[&id].piloting_damage(),
            expected_penalty
        );
        assert_eq!(world.btech.vehicles()[&id].maximum_speed(), expected_speed);
    }
    // Immobilized over deep water, the hovercraft sinks (TW p. 193).
    let deep = world.create(&config, "Deep water".into(), Kind::Room);
    create_battle_map(
        &mut world,
        deep,
        "deep",
        MapAsset::from_cells("1 1\n~1\n").unwrap(),
    )
    .unwrap();
    stop_battle_unit(&mut world, id, ObjectId(1), MovementRules::STANDARD.fall).unwrap();
    place_battle_unit(&mut world, id, deep, 0, 0).unwrap();
    set_seed(
        &mut world,
        id,
        matching_seed(|dice| dice.two_d6() == 3 && dice.two_d6() == 9),
    );
    let report =
        resolve_battle_vehicle_impact(&mut world, id, HitArc::Front, 1, None, rules()).unwrap();
    assert_eq!(
        report.hit.unwrap().motive.unwrap().hit,
        Some(VehicleMotiveHit::Immobilize)
    );
    let vehicle = &world.btech.vehicles()[&id];
    assert!(vehicle.immobilized() && vehicle.flooded() && vehicle.is_destroyed());
    assert!(
        report
            .notices
            .iter()
            .any(|notice| notice.text.contains("sinks"))
    );
}
