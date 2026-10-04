//! Automatic C3/C3i network formation: per-map, per-team pools, family capacity rules,
//! stability across passes, the pilot hold (`-`) and resume (`+`) controls, and the heartbeat.
use crate::support;
use stompymux_rs::*;

/// Running friendly units on one map. Every unit carries C3i; `masters[i]` classic masters
/// and, when `slaves` is set, one classic slave are added per unit.
async fn field(
    masters: &[usize],
    slaves: bool,
) -> (tempfile::TempDir, Config, World, Vec<(ObjectId, ObjectId)>) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Topology field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "topology.map",
        MapAsset::from_cells(&format!("20 20\n{}", (".0".repeat(20) + "\n").repeat(20))).unwrap(),
    )
    .unwrap();
    let critical = |equipment: &str| CriticalDefinition {
        equipment: equipment.into(),
        data: "-".into(),
        modes: vec![],
    };
    let mut units = Vec::new();
    for (i, &master_count) in masters.iter().enumerate() {
        let mut template =
            MechTemplate::parse("AS7-D", include_str!("fixtures/btech/mechs/AS7-D.toml")).unwrap();
        let torso = template
            .sections
            .get_mut(&MechSection::CenterTorso)
            .unwrap();
        for slot in [10, 11] {
            torso.criticals.insert(slot, critical("C3i"));
        }
        if slaves {
            template
                .sections
                .get_mut(&MechSection::LeftTorso)
                .unwrap()
                .criticals
                .insert(11, critical("C3Slave"));
        }
        for section in [MechSection::LeftArm, MechSection::RightArm]
            .into_iter()
            .take(master_count)
        {
            let arm = template.sections.get_mut(&section).unwrap();
            for slot in 2..7 {
                arm.criticals.insert(slot, critical("C3Master"));
            }
        }
        let id = world.create(&config, format!("Topology unit {i}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(&mut world, id, template).unwrap();
        place_battle_unit(&mut world, id, map, 10, 5 + i as i64).unwrap();
        let pilot = if i == 0 {
            ObjectId(1)
        } else {
            world.create(&config, format!("Topology pilot {i}"), Kind::Player)
        };
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, pilot).unwrap();
        start_battle_unit(&mut world, id, pilot, true).unwrap();
        units.push((id, pilot));
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    // Everyone already sees everyone, so manual joins have a visible target.
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    for &(id, _) in &units {
        for &(other, _) in &units {
            if id != other {
                encoded["constructed"][id.0.to_string()]["contacts"][other.0.to_string()] =
                    serde_json::json!({"identified":true});
            }
        }
    }
    world.btech = serde_json::from_value(encoded).unwrap();
    (dir, config, world, units)
}

/// Sorted C3i peers of `id`, including itself, or empty when unlinked.
fn c3i(world: &World, id: ObjectId) -> Vec<ObjectId> {
    battle_c3i_members(world, id).unwrap()
}

#[tokio::test]
async fn c3i_networks_form_per_team_hold_six_and_stay_stable() {
    let (_dir, config, mut world, units) = field(&[0; 7], false).await;
    let ids: Vec<_> = units.iter().map(|(id, _)| *id).collect();
    let notices = reconcile_battle_command_networks(&mut world).unwrap();
    // Six units form one network; the seventh cannot join and no singleton network is kept.
    for &id in &ids[..6] {
        assert_eq!(c3i(&world, id), ids[..6]);
    }
    assert!(c3i(&world, ids[6]).is_empty());
    assert_eq!(notices.len(), 6);
    assert!(
        notices
            .iter()
            .all(|n| n.text == "C3i network established with 6 units.")
    );
    // A second pass changes nothing and says nothing.
    let before = world.btech.clone();
    assert!(
        reconcile_battle_command_networks(&mut world)
            .unwrap()
            .is_empty()
    );
    assert_eq!(world.btech, before);
    // A team change frees a seat, which the waiting unit takes with a notice to everyone.
    set_battle_unit_signature(
        &mut world,
        ids[3],
        UnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    let notices = reconcile_battle_command_networks(&mut world).unwrap();
    let expected: Vec<_> = ids.iter().copied().filter(|id| *id != ids[3]).collect();
    assert_eq!(c3i(&world, ids[6]), expected);
    assert!(c3i(&world, ids[3]).is_empty());
    assert_eq!(notices.len(), 6);
    assert!(
        notices
            .iter()
            .any(|n| n.unit == ids[6] && n.text == "C3i network established with 6 units.")
    );
    assert!(
        notices
            .iter()
            .filter(|n| n.unit != ids[6])
            .all(|n| n.text.ends_with("connects to your C3i network."))
    );
    // A second hostile unit gives the lone one a partner on its own team.
    set_battle_unit_signature(
        &mut world,
        ids[4],
        UnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    reconcile_battle_command_networks(&mut world).unwrap();
    assert_eq!(c3i(&world, ids[3]), vec![ids[3], ids[4]]);
    assert_eq!(c3i(&world, ids[0]).len(), 5);
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
}

#[tokio::test]
async fn classic_networks_seed_from_masters_and_fill_to_capacity() {
    // Slaves alone never form a network.
    let (_dir, _config, mut world, units) = field(&[0; 3], true).await;
    reconcile_battle_command_networks(&mut world).unwrap();
    assert!(
        units
            .iter()
            .all(|(id, _)| battle_c3_members(&world, *id).unwrap().is_empty())
    );
    // One master carries three peers; the surplus slaves wait for another master.
    let (_dir, _config, mut world, units) = field(&[0, 0, 1, 0, 0, 0], true).await;
    let ids: Vec<_> = units.iter().map(|(id, _)| *id).collect();
    reconcile_battle_command_networks(&mut world).unwrap();
    let network = battle_c3_members(&world, ids[2]).unwrap();
    assert_eq!(network.len(), 4);
    assert!(network.contains(&ids[2]));
    let waiting = ids
        .iter()
        .filter(|id| battle_c3_members(&world, **id).unwrap().is_empty())
        .count();
    assert_eq!(waiting, 2);
    // C3i formed independently across all six.
    assert_eq!(c3i(&world, ids[0]).len(), 6);
    // Two masters carry the whole field in one network.
    let (_dir, _config, mut world, units) = field(&[1, 0, 0, 0, 1, 0, 0], true).await;
    let ids: Vec<_> = units.iter().map(|(id, _)| *id).collect();
    reconcile_battle_command_networks(&mut world).unwrap();
    assert_eq!(battle_c3_members(&world, ids[0]).unwrap(), ids);
}

#[tokio::test]
async fn pilots_can_hold_out_of_and_resume_automatic_linking() {
    let (_dir, config, mut world, units) = field(&[0; 4], false).await;
    let ids: Vec<_> = units.iter().map(|(id, _)| *id).collect();
    let (first, pilot) = units[0];
    reconcile_battle_command_networks(&mut world).unwrap();
    assert_eq!(c3i(&world, first), ids);
    // Leaving holds the unit out; later passes do not relink it.
    let notices = request_battle_network(
        &mut world,
        first,
        pilot,
        NetworkRequest::Leave,
        CommandNetwork::C3i,
    )
    .unwrap();
    assert_eq!(notices[0].text, "You disconnect from the C3i network.");
    assert_eq!(
        notices[1].text,
        "Automatic C3i linking is off until you run c3i +."
    );
    assert!(
        reconcile_battle_command_networks(&mut world)
            .unwrap()
            .is_empty()
    );
    assert!(c3i(&world, first).is_empty());
    assert_eq!(c3i(&world, ids[1]).len(), 3);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert!(
        reconcile_battle_command_networks(&mut restored)
            .unwrap()
            .is_empty()
    );
    assert!(c3i(&restored, first).is_empty());
    // A manual join still works while held, and the pass leaves it alone.
    request_battle_network(
        &mut world,
        first,
        pilot,
        NetworkRequest::Join(ids[1]),
        CommandNetwork::C3i,
    )
    .unwrap();
    assert_eq!(c3i(&world, first), ids);
    request_battle_network(
        &mut world,
        first,
        pilot,
        NetworkRequest::Leave,
        CommandNetwork::C3i,
    )
    .unwrap();
    // Resuming relinks at once and tells the network.
    let notices = request_battle_network(
        &mut world,
        first,
        pilot,
        NetworkRequest::Automatic,
        CommandNetwork::C3i,
    )
    .unwrap();
    assert_eq!(c3i(&world, first), ids);
    assert!(
        notices
            .iter()
            .any(|n| n.unit == first && n.text == "C3i network established with 4 units.")
    );
    assert_eq!(notices.len(), 4);
    // The cockpit commands drive the same requests.
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert!(
        support::run_text(&scripts, &config, pilot, 1, "c3i -")
            .contains("You disconnect from the C3i network.")
    );
    assert!(c3i(&scripts.world(), first).is_empty());
    assert!(
        support::run_text(&scripts, &config, pilot, 1, "c3i +")
            .contains("C3i network established with 4 units.")
    );
    assert_eq!(c3i(&scripts.world(), first), ids);
    // Resuming with nobody to link says so instead of staying silent.
    let lonely = support::run_text(&scripts, &config, pilot, 1, "c3 +");
    assert!(lonely.contains("not equipped with C3"));
}

#[tokio::test]
async fn heartbeat_links_running_units_without_pilot_action() {
    let (_dir, config, world, units) = field(&[1, 0, 0], true).await;
    let ids: Vec<_> = units.iter().map(|(id, _)| *id).collect();
    assert!(c3i(&world, ids[0]).is_empty());
    let snapshots = support::autopilot::heartbeat_snapshots(&config, &world, 1).await;
    let after = snapshots.last().unwrap();
    assert_eq!(c3i(after, ids[0]), ids);
    assert_eq!(battle_c3_members(after, ids[0]).unwrap(), ids);
}

/// Autopilot observations carry peer sightings, flagged as relayed, with the shared range.
#[tokio::test]
async fn autopilot_observation_includes_relayed_network_sightings() {
    let (_dir, config, mut world, units) = field(&[0; 3], false).await;
    let (observer, peer, enemy) = (units[0].0, units[1].0, units[2].0);
    set_battle_unit_signature(
        &mut world,
        enemy,
        UnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    reconcile_battle_command_networks(&mut world).unwrap();
    assert_eq!(c3i(&world, observer), vec![observer, peer]);
    // Only the peer holds the enemy; the observer's own sensors hold nothing.
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    for &(id, _) in &units {
        encoded["constructed"][id.0.to_string()]["contacts"] = serde_json::json!({});
    }
    encoded["constructed"][peer.0.to_string()]["contacts"][enemy.0.to_string()] =
        serde_json::json!({"identified":true});
    world.btech = serde_json::from_value(encoded).unwrap();
    let observe = |world: &World| {
        stompymux_rs::btech::autopilot::observations::observe(world, observer, 0).unwrap()
    };
    let observation = observe(&world);
    let contact = observation
        .contacts
        .iter()
        .find(|c| c.unit == enemy)
        .expect("peer sighting is relayed");
    assert!(contact.relayed && contact.identified && !contact.friendly);
    assert!((contact.range - 2.0).abs() < 1e-8);
    assert!((contact.network_range.unwrap() - 1.0).abs() < 1e-8);
    // The peer itself is a direct contact of nobody here, so it is not invented.
    assert!(!observation.contacts.iter().any(|c| c.unit == peer));
    // Leaving the network removes the relayed sighting.
    request_battle_network(
        &mut world,
        observer,
        units[0].1,
        NetworkRequest::Leave,
        CommandNetwork::C3i,
    )
    .unwrap();
    assert!(observe(&world).contacts.is_empty());
    request_battle_network(
        &mut world,
        observer,
        units[0].1,
        NetworkRequest::Automatic,
        CommandNetwork::C3i,
    )
    .unwrap();
    // Once the observer acquires the enemy itself, the contact is direct.
    world
        .btech
        .rewrite_unit_record(observer, |record| {
            record["contacts"][enemy.0.to_string()] = serde_json::json!({"identified":true});
        })
        .unwrap();
    let contact = observe(&world)
        .contacts
        .into_iter()
        .find(|c| c.unit == enemy)
        .unwrap();
    assert!(!contact.relayed);
    assert!((contact.network_range.unwrap() - 1.0).abs() < 1e-8);
    // Lua directors see the same flag.
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let relayed: bool = scripts
        .eval_callback(&format!(
            r#"
            local u = mux.world.object({observer})
            btech.autopilot.attach(u)
            for _, contact in ipairs(btech.autopilot.observe(u).contacts) do
                if contact.unit == {enemy} then
                    return contact.relayed
                end
            end
            error("enemy missing")
            "#,
            observer = observer.0,
            enemy = enemy.0,
        ))
        .unwrap();
    assert!(!relayed);
}
