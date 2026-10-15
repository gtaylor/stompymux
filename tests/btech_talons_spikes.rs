//! Talons raise kicks by half while their leg's talons and foot survive; spikes blunt the
//! blows that land on them and wound the kicking leg (Tactical Operations: Advanced Units &
//! Equipment p. 103).
use crate::btech_motion_common::{kick_fixture, kick_rules, shot_seed};
use stompymux_rs::*;

/// Install `item` in each listed slot of a saved unit's design, balancing the design.
fn install(world: &mut World, id: ObjectId, item: &str, slots: &[(MechSection, u8)]) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            let mut definition: MechTemplate =
                serde_json::from_value(record["definition"].clone()).unwrap();
            for &(section, slot) in slots {
                definition
                    .sections
                    .get_mut(&section)
                    .unwrap()
                    .criticals
                    .insert(
                        slot,
                        CriticalDefinition {
                            equipment: item.into(),
                            data: "-".into(),
                            modes: vec![],
                        },
                    );
            }
            crate::support::templates::set_record_definition(record, definition);
        })
        .unwrap();
}

/// Total damage an impact's phases absorbed.
fn absorbed(impact: &TacticalImpact) -> u16 {
    impact
        .impact
        .phases
        .iter()
        .map(|phase| phase.absorbed)
        .sum()
}

#[tokio::test]
async fn talons_raise_kicks_until_the_talons_or_foot_are_lost() {
    let (_dir, config, mut world, id, target) = kick_fixture().await;
    let kick = |world: &World, leg| {
        battle_kick_profile(world, id, ObjectId(1), target, leg, kick_rules())
            .unwrap()
            .damage
    };
    assert_eq!(kick(&world, Leg::Right), 7);
    install(
        &mut world,
        id,
        "Talons",
        &[
            (MechSection::LeftLeg, 4),
            (MechSection::LeftLeg, 5),
            (MechSection::RightLeg, 4),
            (MechSection::RightLeg, 5),
        ],
    );
    // Seven points and half again, rounded up.
    assert_eq!(kick(&world, Leg::Right), 11);
    assert_eq!(kick(&world, Leg::Left), 11);
    destroy_battle_critical(
        &mut world,
        id,
        CriticalLocation {
            section: MechSection::RightLeg,
            slot: 4,
        },
    )
    .unwrap();
    assert_eq!(kick(&world, Leg::Right), 7);
    assert_eq!(kick(&world, Leg::Left), 11);
    assert!(
        world.btech.constructed_units()[&id]
            .any_talons_working()
            .unwrap()
    );
    destroy_battle_critical(
        &mut world,
        id,
        CriticalLocation {
            section: MechSection::LeftLeg,
            slot: 3,
        },
    )
    .unwrap();
    assert_eq!(kick(&world, Leg::Left), 7);
    assert!(
        !world.btech.constructed_units()[&id]
            .any_talons_working()
            .unwrap()
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn spikes_blunt_a_kick_and_wound_the_kicking_leg() {
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    install(
        &mut base,
        target,
        "Spikes",
        &[(MechSection::LeftLeg, 4), (MechSection::RightLeg, 4)],
    );
    let hit = (0..=255)
        .find_map(|seed| {
            let mut world = base.clone();
            shot_seed(&mut world, id, seed);
            let report = resolve_battle_kick(
                &mut world,
                id,
                ObjectId(1),
                target,
                Leg::Right,
                kick_rules(),
            )
            .unwrap();
            (report.hit && !report.glancing).then_some((world, report))
        })
        .expect("some seed lands a full kick");
    let (world, report) = hit;
    // Seven points less the four the spikes take.
    assert_eq!(absorbed(report.impact.as_ref().unwrap()), 3);
    let [backlash] = report.spike_backlash.as_slice() else {
        panic!("one backlash impact: {:?}", report.spike_backlash);
    };
    assert_eq!(absorbed(backlash), 2);
    assert_eq!(backlash.impact.phases[0].section, MechSection::RightLeg);
    world.validate(&config).unwrap();
}

/// Each working torso spike cluster adds two points to a charge.
#[tokio::test]
async fn torso_spikes_sharpen_a_charge() {
    let (_dir, config, mut world, id, target) = kick_fixture().await;
    crate::btech_motion_common::prepare_test_charge(&mut world, id);
    let rules = ChargeRules {
        distance: 4.0,
        physical: kick_rules(),
    };
    let plain = battle_charge_profile(&world, id, target, rules)
        .unwrap()
        .inflicted_damage;
    install(
        &mut world,
        id,
        "Spikes",
        &[(MechSection::LeftTorso, 6), (MechSection::RightTorso, 6)],
    );
    assert_eq!(
        world.btech.constructed_units()[&id]
            .torso_spike_clusters()
            .unwrap(),
        2
    );
    assert_eq!(
        battle_charge_profile(&world, id, target, rules)
            .unwrap()
            .inflicted_damage,
        plain + 4
    );
    world.validate(&config).unwrap();
}
