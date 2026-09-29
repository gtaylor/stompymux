//! End-to-end autopilot use of C3i peer sightings through the production heartbeat:
//! automatic network formation, an attack order on an enemy only a peer can see, holding fire
//! while the sighting is relayed, closing in, and locking once its own sensors acquire it.
use crate::support;
use stompymux_rs::*;

/// Sensor and sight reach in hexes; short enough that the autopilot starts blind to the enemy.
const REACH: u8 = 5;

/// A JR7-D carrying a C3i computer in its otherwise empty left-torso slots.
fn networked_jenner() -> BattleTemplate {
    let mut template = BattleTemplate::parse(include_str!("../game/mechs/JR7-D")).unwrap();
    let torso = template
        .sections
        .get_mut(&BattleSection::LeftTorso)
        .unwrap();
    for slot in [4, 5] {
        torso.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: "C3i".into(),
                data: "-".into(),
                modes: vec![],
                brand: None,
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
    template: BattleTemplate,
    (x, y): (i64, i64),
    pilot: Option<ObjectId>,
) -> ObjectId {
    let id = world.create(config, name.into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(world, id, template).unwrap();
    place_battle_unit(world, id, map, x, y).unwrap();
    if let Some(pilot) = pilot {
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        assign_battle_pilot(world, id, pilot).unwrap();
    }
    // Start directly in Running; the startup state machine has its own scenarios.
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["power"] =
        serde_json::to_value(BattlePower::Running).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    id
}

/// Hunter and spotter share C3i; the enemy sits beside the spotter, far beyond the hunter's reach.
async fn fixture() -> (tempfile::TempDir, Config, World, [ObjectId; 3]) {
    let (directory, _, mut world) = support::isolated_world().await;
    // The runtime reapplies the configured sensor reach, so shorten it in the copied config.
    let settings = directory.path().join("stompymux.toml");
    let text = std::fs::read_to_string(&settings).unwrap();
    let edited = text.replacen(
        "[battletech]\n",
        &format!("[battletech]\nsensor_range = {REACH}\n"),
        1,
    );
    assert_ne!(text, edited, "fixture battletech table not found");
    std::fs::write(&settings, edited).unwrap();
    let config = Config::load(directory.path()).unwrap();
    let map = world.create(&config, "Network hunt".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "network.hunt",
        BattleMapAsset::parse(&format!("7 26\n{}", (".0".repeat(7) + "\n").repeat(26))).unwrap(),
    )
    .unwrap();
    set_battle_map_visibility(&mut world, map, BattleLight::Day, REACH).unwrap();
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
        (0, 17),
        Some(spotter_pilot),
    );
    let enemy = place(
        &mut world,
        &config,
        map,
        "Enemy",
        BattleTemplate::parse(include_str!("../game/mechs/JR7-D")).unwrap(),
        (3, 20),
        None,
    );
    set_battle_unit_signature(
        &mut world,
        enemy,
        BattleUnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    world.validate(&config).unwrap();
    (directory, config, world, [hunter, spotter, enemy])
}

/// Whether `observer` holds `target` through its own sensors.
fn holds(world: &World, observer: ObjectId, target: ObjectId) -> bool {
    visible_battle_contacts(world, observer)
        .unwrap()
        .iter()
        .any(|contact| contact.target == target)
}

/// Current row of the hunter.
fn row(world: &World, id: ObjectId) -> u16 {
    world.btech.constructed_units()[&id].position().unwrap().y
}

#[tokio::test]
async fn autopilot_hunts_an_enemy_only_its_c3i_peer_can_see() {
    let (_directory, config, world, [hunter, spotter, enemy]) = fixture().await;
    let mut harness = HeartbeatHarness::new(config.clone(), world).unwrap();
    let step = async |harness: &mut HeartbeatHarness| {
        let now = harness.world().btech.simulation_time() + 1;
        let metrics = harness.step(now).await;
        assert!(metrics.committed, "heartbeat must commit");
        metrics
    };

    // The heartbeat links the pair with no pilot action and the spotter acquires the enemy.
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
    assert!(ready, "network formed and spotter acquired the enemy");
    let world = harness.world();
    assert!(!holds(&world, hunter, enemy), "hunter starts blind");
    let relayed = stompymux_rs::btech::autopilot::observations::observe(&world, hunter, 0)
        .unwrap()
        .contacts
        .into_iter()
        .find(|contact| contact.unit == enemy)
        .expect("peer sighting reaches the autopilot");
    assert!(relayed.relayed && relayed.identified && !relayed.friendly);

    // An attack order may name an enemy held only by the network.
    harness
        .scripts()
        .eval_callback::<()>(&format!(
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
        .unwrap();

    // While the sighting is relayed the hunter closes in but neither locks nor fires.
    let start = row(&harness.world(), hunter);
    let mut acquired_at = None;
    for tick in 0..240 {
        let metrics = step(&mut harness).await;
        let world = harness.world();
        let controller = &world.btech.controllers()[&hunter];
        assert_eq!(
            controller.state(),
            stompymux_rs::btech::AutopilotState::Executing,
            "attack order stays active at tick {tick}"
        );
        if holds(&world, hunter, enemy) {
            acquired_at = Some(tick);
            break;
        }
        assert_eq!(
            metrics.autopilot.autonomous_shots, 0,
            "no shots at a relayed target (tick {tick})"
        );
        assert!(
            world.btech.constructed_units()[&hunter]
                .target_lock()
                .is_none_or(|lock| lock.target != enemy),
            "no lock on a relayed target (tick {tick})"
        );
    }
    assert!(
        acquired_at.is_some(),
        "hunter closed until its own sensors acquired the enemy"
    );
    assert!(
        row(&harness.world(), hunter) > start,
        "hunter advanced on the enemy"
    );

    // With its own contact, ordinary targeting takes over.
    let mut locked = false;
    for _ in 0..30 {
        step(&mut harness).await;
        locked = harness.world().btech.constructed_units()[&hunter]
            .target_lock()
            .is_some_and(|lock| lock.target == enemy);
        if locked {
            break;
        }
    }
    assert!(locked, "hunter locks the enemy once it holds it directly");
}
