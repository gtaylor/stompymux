//! End-to-end autopilot use of side-wide spotting (Tactical Operations p. 191) through the
//! production heartbeat: automatic C3i formation, an attack order on an enemy only a teammate
//! can see, holding fire while the sighting is relayed, closing in, and locking once the hunter
//! perceives the enemy itself; and the refusal to attack an enemy nobody on the side spots.
use crate::support;
use stompymux_rs::*;

/// A JR7-D carrying a C3i computer in its otherwise empty left-torso slots.
fn networked_jenner() -> MechTemplate {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("../game/units/JR7-D.toml")).unwrap();
    let torso = template.sections.get_mut(&MechSection::LeftTorso).unwrap();
    for slot in [4, 5] {
        torso.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: "C3i".into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    }
    template
}

/// Place a running unit on `map`; `pilot` is assigned when given.
fn place(
    world: &mut World,
    config: &Config,
    map: ObjectId,
    name: &str,
    template: MechTemplate,
    (x, y): (i64, i64),
    pilot: Option<ObjectId>,
) -> ObjectId {
    let id = world.create(config, name.into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(world, id, template).unwrap();
    support::seed_object_dice(world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(world, id, map, x, y).unwrap();
    if let Some(pilot) = pilot {
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        assign_battle_pilot(world, id, pilot).unwrap();
        support::seed_object_dice(world, pilot, support::FIXTURE_DICE_SEED);
    }
    // Start directly in Running; the startup state machine has its own scenarios.
    world.btech.set_unit_power(id, Power::Running).unwrap();
    id
}

/// Hunter and spotter share C3i on a moonlit field without sensors: units see ten hexes, the
/// spotter stands beside the enemy, and the hunter starts eighteen hexes away.
async fn fixture() -> (tempfile::TempDir, Config, World, [ObjectId; 3]) {
    let (directory, _, mut world) = support::isolated_world().await;
    let config = Config::load(directory.path()).unwrap();
    let map = world.create(&config, "Network hunt".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "network.hunt",
        MapAsset::from_cells(&format!("7 26\n{}", (".0".repeat(7) + "\n").repeat(26))).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    set_battle_map_conditions(&mut world, map, Light::MoonlessNight, Weather::Clear).unwrap();
    set_battle_map_perception(&mut world, map, MapPerceptionFlag::Sensors, false).unwrap();
    let hunter = place(
        &mut world,
        &config,
        map,
        "Hunter",
        networked_jenner(),
        (3, 2),
        Some(ObjectId(1)),
    );
    let spotter_pilot = world.create(&config, "Spotter pilot".into(), Kind::Player);
    let spotter = place(
        &mut world,
        &config,
        map,
        "Spotter",
        networked_jenner(),
        (4, 20),
        Some(spotter_pilot),
    );
    let enemy = place(
        &mut world,
        &config,
        map,
        "Enemy",
        MechTemplate::parse("JR7-D", include_str!("../game/units/JR7-D.toml")).unwrap(),
        (3, 20),
        None,
    );
    set_battle_unit_signature(
        &mut world,
        enemy,
        UnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    world.validate(&config).unwrap();
    (directory, config, world, [hunter, spotter, enemy])
}

/// Whether `observer`'s contact list carries `target`, perceived itself or spotted by its side.
fn spotted(world: &World, observer: ObjectId, target: ObjectId) -> bool {
    visible_battle_contacts(world, observer)
        .unwrap()
        .iter()
        .any(|contact| contact.target == target)
}

/// Whether `observer` perceives `target` itself: the row carries a detection channel of its own.
fn holds(world: &World, observer: ObjectId, target: ObjectId) -> bool {
    visible_battle_contacts(world, observer)
        .unwrap()
        .iter()
        .any(|contact| contact.target == target && contact.detection.is_some())
}

/// Current row of the hunter.
fn row(world: &World, id: ObjectId) -> u16 {
    world.btech.constructed_units()[&id].position().unwrap().y
}

/// Advance the heartbeat one simulation second and insist it commits.
async fn step(harness: &mut HeartbeatHarness) -> HeartbeatMetrics {
    let now = harness.world().btech.simulation_time() + 1;
    let metrics = harness.step(now).await;
    assert!(metrics.committed, "heartbeat must commit");
    metrics
}

/// Hand the hunter an attack order on `enemy` through the Lua director.
fn order_attack(
    harness: &HeartbeatHarness,
    hunter: ObjectId,
    enemy: ObjectId,
) -> anyhow::Result<()> {
    harness.scripts().eval_callback::<()>(&format!(
        r#"
        local u = mux.world.object({hunter})
        local a = btech.autopilot
        a.attach(u)
        a.configure(u, {{ fire_mode = a.fire_modes.ASSIGNED_TARGET }})
        local result = a.submit(u, {{ {{ kind = a.orders.ATTACK, target = {enemy} }} }}, a.submission_modes.APPEND)
        assert(result.ids[1] == 1)
        a.resume(u)
        "#,
        hunter = hunter.0,
        enemy = enemy.0,
    ))
}

#[tokio::test]
async fn autopilot_hunts_an_enemy_only_a_teammate_can_see() {
    let (_directory, config, world, [hunter, spotter, enemy]) = fixture().await;
    let mut harness = HeartbeatHarness::new(config.clone(), world).unwrap();

    // The heartbeat links the pair with no pilot action and the spotter sees the enemy beside it.
    let mut ready = false;
    for _ in 0..30 {
        step(&mut harness).await;
        let world = harness.world();
        ready = battle_c3i_members(&world, hunter).unwrap() == vec![hunter, spotter]
            && holds(&world, spotter, enemy);
        if ready {
            break;
        }
    }
    assert!(ready, "network formed and spotter sees the enemy");
    let world = harness.world();
    assert!(
        !holds(&world, hunter, enemy),
        "hunter perceives nothing itself"
    );
    assert!(
        spotted(&world, hunter, enemy),
        "the side's sighting reaches the hunter"
    );
    let relayed = stompymux_rs::btech::autopilot::observations::observe(&world, hunter, 0)
        .unwrap()
        .contacts
        .into_iter()
        .find(|contact| contact.unit == enemy)
        .expect("teammate sighting reaches the autopilot");
    assert!(relayed.relayed && relayed.identified && !relayed.friendly);
    assert!((relayed.network_range.unwrap() - 1.0).abs() < 1e-8);

    // An attack order may name an enemy held only by a teammate.
    order_attack(&harness, hunter, enemy).unwrap();

    // A target only a teammate spots may be engaged (Tactical Operations p. 191): the hunter
    // locks it at once, closes in, and fires as soon as its weapons reach, before it sees the
    // enemy itself.
    let start = row(&harness.world(), hunter);
    let mut fired_at = None;
    for tick in 0..240 {
        let metrics = step(&mut harness).await;
        let world = harness.world();
        let controller = &world.btech.controllers()[&hunter];
        assert_eq!(
            controller.state(),
            stompymux_rs::btech::AutopilotState::Executing,
            "attack order stays active at tick {tick}"
        );
        assert!(
            world.btech.constructed_units()[&hunter]
                .target_lock()
                .is_some_and(|lock| lock.target == enemy),
            "the relayed enemy stays locked (tick {tick})"
        );
        if metrics.autopilot.autonomous_shots > 0 {
            fired_at = Some((tick, holds(&world, hunter, enemy)));
            break;
        }
    }
    let (_, own_sighting) = fired_at.expect("hunter fired on the relayed enemy");
    assert!(
        !own_sighting,
        "the hunter fired before it perceived the enemy itself"
    );
    assert!(
        row(&harness.world(), hunter) > start,
        "hunter advanced on the enemy"
    );
}

/// With the spotter shut down nobody on the side perceives the enemy: it is no contact, the
/// attack order is refused, and the heartbeat never locks or fires.
#[tokio::test]
async fn autopilot_cannot_attack_an_enemy_nobody_on_its_side_spots() {
    let (_directory, config, mut world, [hunter, spotter, enemy]) = fixture().await;
    world.btech.set_unit_power(spotter, Power::Off).unwrap();
    let mut harness = HeartbeatHarness::new(config.clone(), world).unwrap();
    for _ in 0..10 {
        step(&mut harness).await;
    }
    let world = harness.world();
    assert!(!spotted(&world, hunter, enemy), "nobody spots the enemy");
    assert!(
        !stompymux_rs::btech::autopilot::observations::observe(&world, hunter, 0)
            .unwrap()
            .contacts
            .iter()
            .any(|contact| contact.unit == enemy)
    );
    drop(world);
    let error = order_attack(&harness, hunter, enemy)
        .unwrap_err()
        .to_string();
    assert!(error.contains("hostile contact"), "{error}");
    let start = row(&harness.world(), hunter);
    for tick in 0..30 {
        let metrics = step(&mut harness).await;
        let world = harness.world();
        assert_eq!(
            metrics.autopilot.autonomous_shots, 0,
            "no shots at an unseen enemy (tick {tick})"
        );
        assert!(
            world.btech.constructed_units()[&hunter]
                .target_lock()
                .is_none(),
            "no lock on an unseen enemy (tick {tick})"
        );
    }
    assert_eq!(
        row(&harness.world(), hunter),
        start,
        "hunter has nowhere to go"
    );
}
