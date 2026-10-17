//! Physical attack eligibility by the book: quad rear kicks, trips, the wrecking ball's arc,
//! attacks on vehicles and VTOLs by level, charge and death-from-above restrictions, and
//! glancing charges and landings (Total Warfare pp. 144-152, 198; Tactical Operations:
//! Advanced Rules pp. 78, 89).
use crate::btech_motion_common::{
    kick_fixture, kick_rules, pass_seconds, prepare_test_charge, shot_seed,
};
use crate::btech_motion_physical::{
    dfa_dice, dfa_piloting, install_test_handweapons, prepare_test_club,
};
use crate::support;
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// A seed whose first two-dice roll is `roll`.
fn roll_seed(roll: u8) -> u8 {
    (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == roll)
        .unwrap()
}

/// Add a running enemy vehicle built from `source` at `(x, y)` on the attacker's map, and let
/// the attacker see it.
fn add_vehicle(
    world: &mut World,
    config: &Config,
    attacker: ObjectId,
    source: &str,
    (x, y): (i64, i64),
) -> ObjectId {
    let map = world.btech.constructed_units()[&attacker]
        .position()
        .unwrap()
        .map;
    let id = world.create(config, "Physical target".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    let mut template = UnitTemplate::parse("vehicle", source).unwrap();
    if let UnitTemplate::Vehicle(definition) = &mut template {
        support::templates::fit_vehicle(definition);
    }
    template.create(world, id).unwrap();
    support::seed_object_dice(world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(world, id, map, x, y).unwrap();
    world.btech.set_unit_power(id, Power::Running).unwrap();
    set_battle_unit_signature(
        world,
        id,
        UnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    refresh_battle_contacts(world, &[attacker]).unwrap();
    id
}

/// Set the level of the hex at `(x, y)` on the attacker's map.
fn raise_hex(world: &mut World, attacker: ObjectId, (x, y): (usize, usize), level: u8) {
    let map = world.btech.constructed_units()[&attacker]
        .position()
        .unwrap()
        .map;
    let width = world.btech.maps()[&map].width as usize;
    world
        .btech
        .rewrite_map_record(map, |record| {
            support::set_hex_elevation(&mut record["terrain"][y * width + x], level);
        })
        .unwrap();
}

/// The error text of a failed physical profile.
fn refusal<T: std::fmt::Debug>(result: anyhow::Result<T>) -> String {
    format!("{:#}", result.unwrap_err())
}

/// A quad kicking into its rear arc kicks with its rear leg on that side, at +1: that leg's
/// actuators set the modifiers, the front leg's do not, and both rear legs are tied up for
/// the turn (Total Warfare p. 147).
#[tokio::test]
async fn quad_rear_kicks_use_the_rear_legs() {
    let (_dir, config, mut base, quad, target, _) = firing::fixture_with_target(
        include_str!("../game/units/SCP-1N.toml"),
        None,
        include_str!("../game/units/JR7-D.toml"),
    )
    .await;
    // The target stands due north; turning about puts it in the quad's rear arc.
    let kick = |world: &World| {
        battle_kick_profile(world, quad, ObjectId(1), target, Leg::Left, kick_rules())
    };
    let front = kick(&base).unwrap();
    assert_eq!(front.rear_kick, 0);
    assert_eq!(front.actuators, 0);
    base.btech
        .edit_unit_motion(quad, |motion| {
            motion.heading = 180.0;
            motion.desired_heading = 180.0;
        })
        .unwrap();
    let rear = kick(&base).unwrap();
    assert_eq!(rear.rear_kick, 1);
    assert_eq!(rear.actuators, 0);
    assert_eq!(rear.target_number, front.target_number + 1);
    for (section, rear_actuators) in [(MechSection::LeftLeg, 2), (MechSection::LeftArm, 0)] {
        let mut world = base.clone();
        destroy_battle_critical(&mut world, quad, CriticalLocation { section, slot: 1 }).unwrap();
        assert_eq!(
            kick(&world).unwrap().actuators,
            rear_actuators,
            "{section:?}"
        );
    }
    let mut world = base.clone();
    resolve_battle_kick(
        &mut world,
        quad,
        ObjectId(1),
        target,
        Leg::Left,
        kick_rules(),
    )
    .unwrap();
    assert_eq!(
        world.btech.constructed_units()[&quad].limb_recycle(),
        &std::collections::BTreeMap::from([
            (MechSection::LeftLeg, 30),
            (MechSection::RightLeg, 30)
        ])
    );
    world.validate(&config).unwrap();
}

/// Only biped 'Mechs trip, and only biped targets (Tactical Operations: Advanced Rules p. 89).
#[tokio::test]
async fn trips_are_made_by_and_against_bipeds_only() {
    let quad = include_str!("../game/units/SCP-1N.toml");
    let biped = include_str!("../game/units/JR7-D.toml");
    for (attacker, defender, refusal_text) in [
        (quad, biped, "Only two-legged"),
        (biped, quad, "cannot trip a four-legged"),
    ] {
        let (_dir, _config, world, id, target, _) =
            firing::fixture_with_target(attacker, None, defender).await;
        let error = refusal(battle_trip_profile(
            &world,
            id,
            ObjectId(1),
            target,
            Leg::Right,
            kick_rules(),
        ));
        assert!(error.contains(refusal_text), "{error}");
    }
}

/// A wrecking ball swings only into the forward arc, where a sword in the same arm also reaches
/// that arm's side arc (Total Warfare p. 146).
#[tokio::test]
async fn wrecking_balls_swing_only_forward() {
    let (_dir, _config, base, id, target) = kick_fixture().await;
    for (kind, reaches_side) in [(ArmAttack::Sword, true), (ArmAttack::WreckingBall, false)] {
        let mut world = base.clone();
        install_test_handweapons(&mut world, id, kind);
        let swing = |world: &World| {
            battle_arm_attack_profile(
                world,
                id,
                ObjectId(1),
                target,
                Arm::Right,
                kind,
                kick_rules(),
            )
        };
        swing(&world).unwrap();
        world
            .btech
            .edit_unit_motion(target, |motion| motion.point.x += 0.25)
            .unwrap();
        let side = swing(&world);
        assert_eq!(side.is_ok(), reaches_side, "{kind:?}");
        if !reaches_side {
            assert!(refusal(side).contains("outside the attacking limb's arc"));
        }
    }
}

/// A ground vehicle on the 'Mech's own level can be kicked, clubbed or struck with a physical
/// weapon but not punched; one level up it can be punched, clubbed or struck but not kicked
/// (Total Warfare pp. 147 and 150).
#[tokio::test]
async fn ground_vehicles_follow_the_level_rules() {
    let (_dir, config, mut base, id, _) = kick_fixture().await;
    prepare_test_club(&mut base, id);
    grab_battle_club(&mut base, id, ObjectId(1), Some("right")).unwrap();
    pass_seconds(&mut base, 30);
    for level in [0, 1] {
        let mut world = base.clone();
        raise_hex(&mut world, id, (5, 4), level);
        let tank = add_vehicle(
            &mut world,
            &config,
            id,
            include_str!("../game/units/Demolisher.toml"),
            (5, 4),
        );
        let kick = battle_kick_profile(&world, id, ObjectId(1), tank, Leg::Right, kick_rules());
        let punch = battle_punch_profile(&world, id, ObjectId(1), tank, Arm::Left, kick_rules());
        let club = battle_club_profile(&world, id, ObjectId(1), tank, kick_rules());
        assert!(club.unwrap().vehicle_target);
        if level == 0 {
            kick.unwrap();
            let error = refusal(punch);
            assert!(error.contains("too low for this arm attack"), "{error}");
        } else {
            punch.unwrap();
            assert!(refusal(kick).contains("only kick a vehicle on your own level"));
        }
        world.validate(&config).unwrap();
    }
}

/// A flying VTOL can be attacked by level difference: level with the 'Mech everything but a
/// punch, one or two levels up everything but a kick, three levels up only clubs and physical
/// weapons, and nothing higher. A landed VTOL is attacked as a ground vehicle, and any
/// successful physical attack destroys the rotor (Total Warfare p. 198).
#[tokio::test]
async fn vtols_follow_their_table_and_lose_their_rotor() {
    let (_dir, config, mut base, id, _) = kick_fixture().await;
    prepare_test_club(&mut base, id);
    grab_battle_club(&mut base, id, ObjectId(1), Some("right")).unwrap();
    pass_seconds(&mut base, 30);
    let kestrel = add_vehicle(
        &mut base,
        &config,
        id,
        include_str!("../game/units/Kestrel.toml"),
        (5, 4),
    );
    // Altitude (None for landed), and whether a kick, a punch and a club reach.
    for (altitude, kicks, punches, clubs) in [
        (None, true, false, true),
        (Some(0), true, false, true),
        (Some(1), false, true, true),
        (Some(2), false, true, true),
        (Some(3), false, false, true),
        (Some(4), false, false, false),
    ] {
        let mut world = base.clone();
        if let Some(altitude) = altitude {
            world
                .btech
                .rewrite_unit_record(kestrel, |record| {
                    record["vtol_flight"]["phase"] = serde_json::json!({"kind": "airborne"});
                    record["vtol_flight"]["altitude"] = f64::from(altitude).into();
                })
                .unwrap();
        }
        let kick = battle_kick_profile(&world, id, ObjectId(1), kestrel, Leg::Right, kick_rules());
        let punch = battle_punch_profile(&world, id, ObjectId(1), kestrel, Arm::Left, kick_rules());
        let club = battle_club_profile(&world, id, ObjectId(1), kestrel, kick_rules());
        assert_eq!(kick.is_ok(), kicks, "{altitude:?} {kick:?}");
        assert_eq!(punch.is_ok(), punches, "{altitude:?} {punch:?}");
        assert_eq!(club.is_ok(), clubs, "{altitude:?} {club:?}");
    }
    // A blow that lands anywhere but the rotor still tears the rotor off.
    let (world, report) = (0..=255)
        .find_map(|seed| {
            let mut world = base.clone();
            shot_seed(&mut world, id, roll_seed(12));
            shot_seed(&mut world, kestrel, seed);
            let report = resolve_battle_kick(
                &mut world,
                id,
                ObjectId(1),
                kestrel,
                Leg::Right,
                kick_rules(),
            )
            .unwrap();
            let struck = report.vehicle_impact.as_ref()?.hit.as_ref()?.section;
            (struck != VehicleSection::Rotor).then_some((world, report))
        })
        .unwrap();
    assert!(report.hit);
    assert!(report.rotor.is_some());
    assert!(world.btech.vehicles()[&kestrel].rotor_destroyed());
    world.validate(&config).unwrap();
}

/// A 'Mech may not charge in a turn it jumped, at a target more than one level away, or with
/// its pilot unconscious (Total Warfare pp. 148, 150).
#[tokio::test]
async fn charges_respect_jumping_levels_and_consciousness() {
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    prepare_test_charge(&mut base, id);
    let rules = ChargeRules {
        distance: 2.0,
        physical: kick_rules(),
    };
    battle_charge_profile(&base, id, target, rules).unwrap();

    let mut jumped = base.clone();
    let now = jumped.btech.simulation_time();
    jumped
        .btech
        .rewrite_unit_record(id, |record| record["last_jump_launch"] = now.into())
        .unwrap();
    pass_seconds(&mut jumped, 29);
    assert!(
        refusal(battle_charge_profile(&jumped, id, target, rules)).contains("jumped this turn")
    );
    pass_seconds(&mut jumped, 1);
    battle_charge_profile(&jumped, id, target, rules).unwrap();

    for (height, allowed) in [(1, true), (2, false)] {
        let mut world = base.clone();
        world
            .btech
            .rewrite_unit_record(target, |record| record["ground_elevation"] = height.into())
            .unwrap();
        let profile = battle_charge_profile(&world, id, target, rules);
        assert_eq!(profile.is_ok(), allowed, "{height}");
        if !allowed {
            assert!(refusal(profile).contains("more than one level away"));
        }
    }

    // Four hits on the pilot, and recovery dice that fail the consciousness roll.
    let world = (0..=255)
        .find_map(|seed| {
            let mut world = base.clone();
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["recoveries"]["1"] = serde_json::json!({"mode":{"kind":"tactical","injuries":0},"remaining":0,"pain_resistance":false,"toughness":false,"dice":Dice::seeded([seed;32])});
            world.btech = serde_json::from_value(state).unwrap();
            injure_battle_tactical_pilot(&mut world, id, 4, false).unwrap();
            world.btech.unconscious(ObjectId(1)).then_some(world)
        })
        .unwrap();
    assert!(refusal(battle_charge_profile(&world, id, target, rules)).contains("unconscious"));
    world.validate(&config).unwrap();
}

/// Total damage a set of five-point packets brought: each packet's first phase holds all of it.
fn dealt(impacts: &[TacticalImpact]) -> u16 {
    impacts
        .iter()
        .map(|impact| {
            let phase = &impact.impact.phases[0];
            phase.absorbed + phase.remaining
        })
        .sum()
}

/// A charge that meets its target number exactly is a glancing blow: the target takes half the
/// damage, rounded down, while the charger's own damage is unchanged (Tactical Operations:
/// Advanced Rules p. 78).
#[tokio::test]
async fn glancing_charges_halve_the_target_damage() {
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    prepare_test_charge(&mut base, id);
    let rules = ChargeRules {
        distance: 2.0,
        physical: kick_rules(),
    };
    let profile = battle_charge_profile(&base, id, target, rules).unwrap();
    let target_number = u8::try_from(profile.target_number).unwrap();
    for roll in [target_number, 12] {
        let mut world = base.clone();
        shot_seed(&mut world, id, roll_seed(roll));
        let report = resolve_battle_charge(&mut world, id, target, rules).unwrap();
        assert!(report.hit);
        let glancing = roll == target_number;
        assert_eq!(report.glancing, glancing);
        let expected = if glancing {
            profile.inflicted_damage / 2
        } else {
            profile.inflicted_damage
        };
        assert_eq!(dealt(&report.target_impacts), expected, "{roll}");
        assert_eq!(report.received_damage, profile.received_damage);
        world.validate(&config).unwrap();
    }
}

/// A landing that meets its target number exactly halves the target's damage (Tactical
/// Operations: Advanced Rules p. 78).
#[tokio::test]
async fn glancing_landings_halve_the_target_damage() {
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    dfa_piloting(&mut base);
    shot_seed(&mut base, target, 0);
    launch_battle_jump(&mut base, id, ObjectId(1), 0, 2.0).unwrap();
    let profile = battle_dfa_profile(&base, id, target, kick_rules()).unwrap();
    let target_number = u8::try_from(profile.target_number).unwrap();
    let mut world = base.clone();
    let dice = dfa_dice(&world, id, target, |report, _| report.roll == target_number);
    world.btech.set_unit_dice(id, dice).unwrap();
    let report = resolve_battle_dfa(&mut world, id, target, kick_rules()).unwrap();
    assert!(report.hit && report.glancing);
    assert_eq!(dealt(&report.target_impacts), profile.inflicted_damage / 2);
    world.validate(&config).unwrap();
}

/// After a successful landing each 'Mech rolls on its own: when the attacker's own landing
/// damage knocks it down, the target still makes its +2 roll (Total Warfare pp. 150-151).
#[tokio::test]
async fn a_felled_dfa_attacker_leaves_the_target_its_roll() {
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    dfa_piloting(&mut base);
    shot_seed(&mut base, target, 0);
    // Bare legs with one point of structure: the landing blows a leg off.
    base.btech
        .rewrite_unit_record(id, |record| {
            for section in ["LeftLeg", "RightLeg"] {
                record["sections"][section]["armor"] = 0.into();
                record["sections"][section]["internal"] = 1.into();
            }
        })
        .unwrap();
    launch_battle_jump(&mut base, id, ObjectId(1), 0, 2.0).unwrap();
    let mut world = base.clone();
    let dice = dfa_dice(&world, id, target, |report, _| report.hit);
    world.btech.set_unit_dice(id, dice).unwrap();
    let report = resolve_battle_dfa(&mut world, id, target, kick_rules()).unwrap();
    assert!(report.hit);
    assert_eq!(
        world.btech.constructed_units()[&id].posture(),
        Posture::Prone
    );
    assert_eq!(
        report
            .balance
            .iter()
            .map(|balance| (balance.unit, balance.check.situational))
            .collect::<Vec<_>>(),
        vec![(target, 2)]
    );
    world.validate(&config).unwrap();
}

/// A death from above needs Jumping MP to clear the target: its hex level plus two for a
/// standing 'Mech (Total Warfare p. 149). The fixture Jenner has five Jumping MP.
#[tokio::test]
async fn dfa_needs_jumping_mp_to_clear_the_target() {
    let (_dir, config, base, id, target) = kick_fixture().await;
    for (level, allowed) in [(3, true), (4, false)] {
        let mut world = base.clone();
        raise_hex(&mut world, id, (5, 3), level);
        let map = world.btech.constructed_units()[&id].position().unwrap().map;
        place_battle_unit(&mut world, target, map, 5, 3).unwrap();
        refresh_battle_contacts(&mut world, &[id]).unwrap();
        let launch = launch_battle_dfa(&mut world, id, ObjectId(1), Some(target));
        assert_eq!(launch.is_ok(), allowed, "{level}: {launch:?}");
        if !allowed {
            assert!(refusal(launch).contains("jumping MP to clear your target"));
        }
        world.validate(&config).unwrap();
    }
}

/// Add a shut-down enemy Jenner at `(x, y)` on the attacker's map, and let the attacker see it.
fn add_mech(
    world: &mut World,
    config: &Config,
    attacker: ObjectId,
    (x, y): (i64, i64),
) -> ObjectId {
    let map = world.btech.constructed_units()[&attacker]
        .position()
        .unwrap()
        .map;
    let id = world.create(config, "Second target".into(), Kind::Thing);
    create_battle_unit(
        world,
        id,
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(world, id, map, x, y).unwrap();
    set_battle_unit_signature(
        world,
        id,
        UnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    refresh_battle_contacts(world, &[attacker]).unwrap();
    id
}

/// The `(x, y)` hex a unit stands in.
fn hex(world: &World, id: ObjectId) -> (u16, u16) {
    let position = world.btech.unit(id).unwrap().position().unwrap();
    (position.x, position.y)
}

/// A push needs the target in the hex directly ahead, takes Piloting -1, and on a hit drives
/// the target one hex onward while the attacker steps into the hex it left; the target then
/// rolls to stay standing. A push ties up both arms for the turn, and the target cannot be
/// pushed, charged or landed on again that turn (Total Warfare pp. 147-148).
#[tokio::test]
async fn pushes_displace_the_target_and_the_attacker_follows() {
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    let map = base.btech.constructed_units()[&id].position().unwrap().map;
    let push = |world: &World| battle_push_profile(world, id, ObjectId(1), target, kick_rules());
    // Not in the hex ahead: the fixture target shares the attacker's hex.
    assert!(refusal(push(&base)).contains("directly ahead"));
    place_battle_unit(&mut base, target, map, 5, 4).unwrap();
    refresh_battle_contacts(&mut base, &[id]).unwrap();
    let kick = battle_kick_profile(&base, id, ObjectId(1), target, Leg::Right, kick_rules());
    let profile = push(&base).unwrap();
    assert_eq!(profile.base, kick.unwrap().base + 1);
    assert_eq!(profile.direction, 0);
    assert_eq!(profile.shoulders, 0);

    let mut world = base.clone();
    shot_seed(&mut world, id, roll_seed(12));
    let report = resolve_battle_push(&mut world, id, ObjectId(1), target, kick_rules()).unwrap();
    assert!(report.hit && report.displaced);
    assert_eq!(hex(&world, target), (5, 3));
    assert_eq!(hex(&world, id), (5, 4));
    assert_eq!(
        report
            .balance
            .iter()
            .map(|balance| balance.unit)
            .collect::<Vec<_>>(),
        vec![target]
    );
    assert_eq!(
        world.btech.constructed_units()[&id].limb_recycle(),
        &std::collections::BTreeMap::from([
            (MechSection::LeftArm, 30),
            (MechSection::RightArm, 30)
        ])
    );
    world.validate(&config).unwrap();

    // Once pushed, the target cannot be pushed again this turn.
    let mut again = base.clone();
    let now = again.btech.simulation_time();
    again
        .btech
        .rewrite_unit_record(target, |record| record["assaulted_at"] = now.into())
        .unwrap();
    assert!(refusal(push(&again)).contains("already been charged, pushed or landed on"));
    pass_seconds(&mut again, 30);
    push(&again).unwrap();

    // A slope too steep to climb stops the displacement, but the target still rolls.
    let mut blocked = base.clone();
    raise_hex(&mut blocked, id, (5, 3), 3);
    shot_seed(&mut blocked, id, roll_seed(12));
    let report = resolve_battle_push(&mut blocked, id, ObjectId(1), target, kick_rules()).unwrap();
    assert!(report.hit && !report.displaced);
    assert!(report.blocked.is_some());
    assert_eq!(hex(&blocked, target), (5, 4));
    assert_eq!(hex(&blocked, id), (5, 5));
    assert_eq!(report.balance.len(), 1);
    blocked.validate(&config).unwrap();

    // A 'Mech in the hex beyond is shoved out of its far side, and rolls too.
    let mut domino = base.clone();
    let third = add_mech(&mut domino, &config, id, (5, 3));
    shot_seed(&mut domino, id, roll_seed(12));
    let report = resolve_battle_push(&mut domino, id, ObjectId(1), target, kick_rules()).unwrap();
    assert!(report.displaced);
    assert_eq!(hex(&domino, third), (5, 2));
    assert_eq!(hex(&domino, target), (5, 3));
    assert_eq!(
        report
            .balance
            .iter()
            .map(|balance| balance.unit)
            .collect::<Vec<_>>(),
        vec![target, third]
    );
    domino.validate(&config).unwrap();
}

/// Pushed off a drop of more than two levels, a 'Mech falls rather than rolling (Total
/// Warfare p. 147).
#[tokio::test]
async fn pushes_over_a_cliff_are_falls() {
    let (_dir, config, mut world, id, target) = kick_fixture().await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    for y in [4, 5] {
        raise_hex(&mut world, id, (5, y), 3);
    }
    for unit in [id, target] {
        world
            .btech
            .rewrite_unit_record(unit, |record| record["ground_elevation"] = 3.into())
            .unwrap();
    }
    place_battle_unit(&mut world, target, map, 5, 4).unwrap();
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    shot_seed(&mut world, id, roll_seed(12));
    let report = resolve_battle_push(&mut world, id, ObjectId(1), target, kick_rules()).unwrap();
    assert!(report.displaced);
    assert!(report.balance.is_empty());
    assert_eq!(report.falls.len(), 1);
    assert_eq!(
        world.btech.constructed_units()[&target].posture(),
        Posture::Prone
    );
    world.validate(&config).unwrap();
}

/// A prone 'Mech may punch a ground vehicle in its own hex, propping itself on the other arm,
/// and no other physical attack (Total Warfare p. 151).
#[tokio::test]
async fn prone_mechs_punch_vehicles_in_their_own_hex() {
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    base.btech
        .rewrite_unit_record(id, |record| {
            record["posture"] = serde_json::to_value(Posture::Prone).unwrap();
        })
        .unwrap();
    let tank = add_vehicle(
        &mut base,
        &config,
        id,
        include_str!("../game/units/Demolisher.toml"),
        (5, 5),
    );
    let profile =
        battle_punch_profile(&base, id, ObjectId(1), tank, Arm::Left, kick_rules()).unwrap();
    assert!(profile.vehicle_target);
    assert_eq!(profile.limbs, [MechSection::LeftArm, MechSection::RightArm]);
    let kick = battle_kick_profile(&base, id, ObjectId(1), tank, Leg::Right, kick_rules());
    assert!(refusal(kick).contains("prone position"));
    let mech = battle_punch_profile(&base, id, ObjectId(1), target, Arm::Left, kick_rules());
    assert!(refusal(mech).contains("prone position"));
    let mut world = base.clone();
    let report = resolve_battle_punch(
        &mut world,
        id,
        ObjectId(1),
        tank,
        ArmSelection::Both,
        kick_rules(),
    )
    .unwrap();
    assert_eq!(report.attacks.len(), 1);
    assert_eq!(report.rejections.len(), 1);
    world.validate(&config).unwrap();
    // A vehicle in the next hex is out of a prone 'Mech's reach.
    let mut away = base.clone();
    let map = away.btech.constructed_units()[&id].position().unwrap().map;
    away.btech.set_unit_power(tank, Power::Off).unwrap();
    place_battle_unit(&mut away, tank, map, 5, 4).unwrap();
    away.btech.set_unit_power(tank, Power::Running).unwrap();
    refresh_battle_contacts(&mut away, &[id]).unwrap();
    let error = refusal(battle_punch_profile(
        &away,
        id,
        ObjectId(1),
        tank,
        Arm::Left,
        kick_rules(),
    ));
    assert!(error.contains("in your own hex"), "{error}");
}

/// Two arms may punch two different targets, each rolled on its own (Total Warfare p. 145).
#[tokio::test]
async fn split_punches_strike_two_targets() {
    let (_dir, config, mut world, id, target) = kick_fixture().await;
    let second = add_mech(&mut world, &config, id, (5, 4));
    let report =
        resolve_battle_split_punch(&mut world, id, ObjectId(1), [target, second], kick_rules())
            .unwrap();
    assert_eq!(
        report
            .attacks
            .iter()
            .map(|attack| (attack.profile.attack, attack.target))
            .collect::<Vec<_>>(),
        vec![
            (PhysicalAttack::Punch { arm: Arm::Left }, target),
            (PhysicalAttack::Punch { arm: Arm::Right }, second)
        ]
    );
    world.validate(&config).unwrap();
}

/// A death from above may land on a vehicle, on the Front column of its Hit Location Table,
/// and a VTOL loses its rotor; vehicles make no roll to stay standing (Total Warfare pp. 150,
/// 198).
#[tokio::test]
async fn dfa_lands_on_vehicles_and_vtols() {
    for (source, vtol) in [
        (include_str!("../game/units/Demolisher.toml"), false),
        (include_str!("../game/units/Kestrel.toml"), true),
    ] {
        let (_dir, config, mut world, id, _) = kick_fixture().await;
        dfa_piloting(&mut world);
        let vehicle = add_vehicle(&mut world, &config, id, source, (5, 5));
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
        let profile = battle_dfa_profile(&world, id, vehicle, kick_rules()).unwrap();
        assert!(profile.vehicle_target);
        assert_eq!(profile.target_arc, HitArc::Front);
        // A landing whose damage spares the rotor still tears it off a VTOL.
        shot_seed(&mut world, id, roll_seed(12));
        let (world, report) = (0..=255)
            .find_map(|seed| {
                let mut probe = world.clone();
                shot_seed(&mut probe, vehicle, seed);
                let report = resolve_battle_dfa(&mut probe, id, vehicle, kick_rules()).unwrap();
                (!vtol || report.rotor.is_some()).then_some((probe, report))
            })
            .unwrap();
        assert!(report.hit && !report.glancing);
        assert!(!report.vehicle_impacts.is_empty());
        assert!(report.balance.iter().all(|balance| balance.unit == id));
        assert_eq!(report.rotor.is_some(), vtol);
        world.validate(&config).unwrap();
    }
}

/// A vehicle may charge (ram) a 'Mech or another vehicle: tonnage over ten per hex to the
/// target, the target's tonnage over ten to the rammer, a +2 roll for a rammed 'Mech, and the
/// rammer's weapons held for the turn. A 'Mech may not charge a vehicle (Total Warfare p. 148).
#[tokio::test]
async fn vehicles_ram_mechs_and_vehicles() {
    for target_source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/Demolisher.toml"),
    ] {
        let (_dir, config, mut world, tank, target, _) = firing::fixture_with_target(
            include_str!("../game/units/Demolisher.toml"),
            None,
            target_source,
        )
        .await;
        let map = world.btech.unit(tank).unwrap().position().unwrap().map;
        world.btech.set_unit_power(target, Power::Off).unwrap();
        place_battle_unit(&mut world, target, map, 0, 11).unwrap();
        world.btech.set_unit_power(target, Power::Running).unwrap();
        refresh_battle_contacts(&mut world, &[tank]).unwrap();
        select_battle_charge(
            &mut world,
            tank,
            ObjectId(1),
            ChargeSelection::Target(target),
        )
        .unwrap();
        world
            .btech
            .edit_unit_motion(tank, |motion| {
                motion.speed = 21.5;
                motion.desired_speed = 21.5;
            })
            .unwrap();
        let rules = ChargeRules {
            distance: 1.0,
            physical: kick_rules(),
        };
        let profile = battle_ram_profile(&world, tank, target, rules).unwrap();
        let tons = world.btech.vehicles()[&tank].definition().tons;
        assert_eq!(profile.inflicted_damage, tons.div_ceil(10));
        let mech_target = world.btech.constructed_units().contains_key(&target);
        assert_eq!(profile.vehicle_target, !mech_target);
        shot_seed(&mut world, tank, roll_seed(12));
        let report = resolve_battle_ram(&mut world, tank, target, rules).unwrap();
        assert!(report.hit);
        assert_eq!(report.balance.is_some(), mech_target);
        assert!(!report.target_impacts.is_empty() && !report.attacker_impacts.is_empty());
        let vehicle = &world.btech.vehicles()[&tank];
        assert!(
            vehicle
                .weapon_recycle()
                .values()
                .all(|&seconds| seconds >= 30)
        );
        assert!(!vehicle.weapon_recycle().is_empty());
        // A unit that charged may not spot for the rest of the turn (Total Warfare p. 112),
        // even once it has dropped the charge.
        select_battle_charge(&mut world, tank, ObjectId(1), ChargeSelection::Cancel).unwrap();
        let error = select_battle_spotter(&mut world, tank, ObjectId(1), Some(tank)).unwrap_err();
        assert!(format!("{error:#}").contains("cannot spot"), "{error:#}");
        world.validate(&config).unwrap();
    }
    // A 'Mech may not charge a vehicle.
    let (_dir, config, mut world, id, _) = kick_fixture().await;
    let tank = add_vehicle(
        &mut world,
        &config,
        id,
        include_str!("../game/units/Demolisher.toml"),
        (5, 4),
    );
    let error = select_battle_charge(&mut world, id, ObjectId(1), ChargeSelection::Target(tank))
        .unwrap_err();
    assert!(format!("{error:#}").contains("cannot charge a vehicle"));
}

/// Armor points a vehicle has left over all its locations.
fn vehicle_armor(world: &World, id: ObjectId) -> u32 {
    world.btech.vehicles()[&id]
        .sections()
        .values()
        .map(|section| u32::from(section.armor))
        .sum()
}

/// Laser-reflective armor takes physical attacks doubled (Tactical Operations: Advanced Units
/// & Equipment p. 93): a death from above landing on a vehicle and a ram strip twice the armor
/// they strip from standard armor under the same dice.
#[tokio::test]
async fn reflective_armor_takes_vehicle_landings_and_rams_doubled() {
    let plain = include_str!("../game/units/Demolisher.toml");
    let reflective = support::templates::with_flags(plain, &["LaserRefArmor_Tech"]);
    let mut landings = Vec::new();
    let mut rams = Vec::new();
    for source in [plain, reflective.as_str()] {
        let (_dir, config, mut world, id, _) = kick_fixture().await;
        dfa_piloting(&mut world);
        let vehicle = add_vehicle(&mut world, &config, id, source, (5, 5));
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
        shot_seed(&mut world, id, roll_seed(12));
        let before = vehicle_armor(&world, vehicle);
        let report = resolve_battle_dfa(&mut world, id, vehicle, kick_rules()).unwrap();
        assert!(report.hit && !report.glancing);
        landings.push(before - vehicle_armor(&world, vehicle));

        let (_dir, _config, mut world, tank, target, _) =
            firing::fixture_with_target(plain, None, source).await;
        let map = world.btech.unit(tank).unwrap().position().unwrap().map;
        world.btech.set_unit_power(target, Power::Off).unwrap();
        place_battle_unit(&mut world, target, map, 0, 11).unwrap();
        world.btech.set_unit_power(target, Power::Running).unwrap();
        refresh_battle_contacts(&mut world, &[tank]).unwrap();
        world
            .btech
            .edit_unit_motion(tank, |motion| {
                motion.speed = 21.5;
                motion.desired_speed = 21.5;
            })
            .unwrap();
        let rules = ChargeRules {
            distance: 1.0,
            physical: kick_rules(),
        };
        shot_seed(&mut world, tank, roll_seed(12));
        let before = vehicle_armor(&world, target);
        let report = resolve_battle_ram(&mut world, tank, target, rules).unwrap();
        assert!(report.hit && !report.glancing);
        rams.push(before - vehicle_armor(&world, target));
    }
    assert!(landings[0] > 0 && rams[0] > 0);
    assert_eq!(landings[1], 2 * landings[0], "{landings:?}");
    assert_eq!(rams[1], 2 * rams[0], "{rams:?}");
}

/// Modifiers of the critical rolls the hit-location results of `impacts` caused.
fn critical_modifiers<'a>(impacts: impl IntoIterator<Item = &'a VehicleImpact>) -> Vec<i8> {
    impacts
        .into_iter()
        .filter_map(|impact| impact.damage.as_ref())
        .flat_map(|damage| &damage.criticals)
        .map(|critical| critical.selection.modifier)
        .collect()
}

/// The vehicle impacts among a ram's packets.
fn vehicle_impacts(impacts: &[BlastImpact]) -> impl Iterator<Item = &VehicleImpact> {
    impacts.iter().filter_map(|impact| match impact {
        BlastImpact::Vehicle(impact) => Some(impact),
        BlastImpact::Mech(_) => None,
    })
}

/// A glancing kick, death from above or ram takes two off every critical roll it causes on a
/// vehicle target, as on a 'Mech (Tactical Operations: Advanced Rules p. 78).
#[tokio::test]
async fn glancing_landings_and_rams_lower_vehicle_critical_rolls() {
    let tank = include_str!("../game/units/Demolisher.toml");
    let (_dir, config, mut base, id, _) = kick_fixture().await;
    let vehicle = add_vehicle(&mut base, &config, id, tank, (5, 4));
    let target_number =
        battle_kick_profile(&base, id, ObjectId(1), vehicle, Leg::Right, kick_rules())
            .unwrap()
            .target_number;
    for (roll, modifier) in [(u8::try_from(target_number).unwrap(), -2), (12, 0)] {
        let modifiers = (0..=255)
            .find_map(|seed| {
                let mut world = base.clone();
                shot_seed(&mut world, id, roll_seed(roll));
                shot_seed(&mut world, vehicle, seed);
                let report = resolve_battle_kick(
                    &mut world,
                    id,
                    ObjectId(1),
                    vehicle,
                    Leg::Right,
                    kick_rules(),
                )
                .unwrap();
                assert!(report.hit);
                assert_eq!(report.glancing, modifier != 0);
                let modifiers = critical_modifiers(&report.vehicle_impact);
                (!modifiers.is_empty()).then_some(modifiers)
            })
            .unwrap();
        assert!(
            modifiers.iter().all(|&found| found == modifier),
            "{modifiers:?}"
        );
    }

    let (_dir, config, mut base, id, _) = kick_fixture().await;
    dfa_piloting(&mut base);
    let vehicle = add_vehicle(&mut base, &config, id, tank, (5, 5));
    launch_battle_jump(&mut base, id, ObjectId(1), 0, 2.0).unwrap();
    let target_number = battle_dfa_profile(&base, id, vehicle, kick_rules())
        .unwrap()
        .target_number;
    for (roll, modifier) in [(u8::try_from(target_number).unwrap(), -2), (12, 0)] {
        let modifiers = (0..=255)
            .find_map(|seed| {
                let mut world = base.clone();
                shot_seed(&mut world, id, roll_seed(roll));
                shot_seed(&mut world, vehicle, seed);
                let report = resolve_battle_dfa(&mut world, id, vehicle, kick_rules()).unwrap();
                assert_eq!(report.glancing, modifier != 0);
                let modifiers = critical_modifiers(&report.vehicle_impacts);
                (!modifiers.is_empty()).then_some(modifiers)
            })
            .unwrap();
        assert!(
            modifiers.iter().all(|&found| found == modifier),
            "{modifiers:?}"
        );
    }

    let (_dir, _config, mut base, rammer, target, _) =
        firing::fixture_with_target(tank, None, tank).await;
    let map = base.btech.unit(rammer).unwrap().position().unwrap().map;
    base.btech.set_unit_power(target, Power::Off).unwrap();
    place_battle_unit(&mut base, target, map, 0, 11).unwrap();
    base.btech.set_unit_power(target, Power::Running).unwrap();
    refresh_battle_contacts(&mut base, &[rammer]).unwrap();
    base.btech
        .edit_unit_motion(rammer, |motion| {
            motion.speed = 21.5;
            motion.desired_speed = 21.5;
        })
        .unwrap();
    let rules = ChargeRules {
        distance: 1.0,
        physical: kick_rules(),
    };
    let target_number = battle_ram_profile(&base, rammer, target, rules)
        .unwrap()
        .target_number;
    let modifiers = (0..=255)
        .find_map(|seed| {
            let mut world = base.clone();
            shot_seed(
                &mut world,
                rammer,
                roll_seed(u8::try_from(target_number).unwrap()),
            );
            shot_seed(&mut world, target, seed);
            let report = resolve_battle_ram(&mut world, rammer, target, rules).unwrap();
            assert!(report.hit && report.glancing);
            let modifiers = critical_modifiers(vehicle_impacts(&report.target_impacts));
            (!modifiers.is_empty()).then_some(modifiers)
        })
        .unwrap();
    assert!(modifiers.iter().all(|&found| found == -2), "{modifiers:?}");
}

/// An artillery spotter's own physical attacks take +1 (Tactical Operations: Advanced Rules
/// p. 149), and every attack it makes, hit or miss, marks it as having attacked that turn for
/// the artillery attack it spots. Spotting for a unit with no artillery, as for LRM indirect
/// fire, leaves its physical attacks alone: that rule counts only the Weapon Attack Phase
/// (Total Warfare p. 111).
#[tokio::test]
async fn artillery_spotters_take_one_more_on_their_physical_attacks() {
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    let kick = |world: &World| {
        battle_kick_profile(world, id, ObjectId(1), target, Leg::Right, kick_rules()).unwrap()
    };
    let plain = kick(&base);
    assert_eq!(plain.spotting, 0);
    select_battle_spotter(&mut base, id, ObjectId(1), Some(id)).unwrap();
    // A friendly 'Mech without artillery firing through the spotter adds nothing.
    let lrm = add_mech(&mut base, &config, id, (1, 1));
    base.btech
        .rewrite_unit_record(lrm, |record| record["spotter"] = id.0.into())
        .unwrap();
    assert_eq!(kick(&base).spotting, 0);
    let mut world = base.clone();
    let piece = add_vehicle(
        &mut world,
        &config,
        id,
        include_str!("../game/units/Demolisher-Arrow.toml"),
        (1, 2),
    );
    world
        .btech
        .rewrite_unit_record(piece, |record| record["spotter"] = id.0.into())
        .unwrap();
    let spotted = kick(&world);
    assert_eq!(spotted.spotting, 1);
    assert_eq!(spotted.target_number, plain.target_number + 1);
    let mut pushing = world.clone();
    let map = pushing.btech.constructed_units()[&id]
        .position()
        .unwrap()
        .map;
    place_battle_unit(&mut pushing, target, map, 5, 4).unwrap();
    refresh_battle_contacts(&mut pushing, &[id]).unwrap();
    let push = battle_push_profile(&pushing, id, ObjectId(1), target, kick_rules()).unwrap();
    assert_eq!(push.spotting, 1);
    // A missed kick still counts as the spotter's attack this turn.
    assert_eq!(
        world.btech.constructed_units()[&id].last_physical_attack(),
        None
    );
    shot_seed(&mut world, id, roll_seed(2));
    let report = resolve_battle_kick(
        &mut world,
        id,
        ObjectId(1),
        target,
        Leg::Right,
        kick_rules(),
    )
    .unwrap();
    assert!(!report.hit);
    assert_eq!(
        world.btech.constructed_units()[&id].last_physical_attack(),
        Some(world.btech.simulation_time())
    );
    world.validate(&config).unwrap();
}

/// A 'Mech's backhoe is a physical weapon: Piloting +1 for six points on the standard Hit
/// Location Table, in the forward arc or its arm's side arc, with triple strength myomer
/// doubling the damage and each damaged arm actuator adding +2 and halving it (Total Warfare
/// p. 146). It replaces the hand, so the missing hand costs nothing.
#[tokio::test]
async fn backhoes_swing_at_plus_one_for_six() {
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    let punch =
        battle_punch_profile(&base, id, ObjectId(1), target, Arm::Right, kick_rules()).unwrap();
    install_test_handweapons(&mut base, id, ArmAttack::Backhoe);
    let swing = |world: &World| {
        battle_arm_attack_profile(
            world,
            id,
            ObjectId(1),
            target,
            Arm::Right,
            ArmAttack::Backhoe,
            kick_rules(),
        )
        .unwrap()
    };
    let profile = swing(&base);
    assert_eq!(profile.base, punch.base);
    assert_eq!(profile.weapon_modifier, 1);
    assert_eq!(profile.actuators, 0);
    // The fixture's Jenner punches without its lower arm and hand actuators; the backhoe arm
    // keeps the lower actuator and needs no hand.
    assert_eq!(
        profile.target_number,
        punch.target_number - i32::from(punch.actuators) + 1
    );
    assert_eq!(profile.damage, 6);
    assert_eq!(profile.hit_table, HitTable::Weapon);
    let mut damaged = base.clone();
    destroy_battle_critical(
        &mut damaged,
        id,
        CriticalLocation {
            section: MechSection::RightArm,
            slot: 2,
        },
    )
    .unwrap();
    let hurt = swing(&damaged);
    assert_eq!(hurt.actuators, 2);
    assert_eq!(hurt.damage, 3);

    let mut world = base.clone();
    shot_seed(&mut world, id, roll_seed(12));
    let report = resolve_battle_arm_attack(
        &mut world,
        id,
        ObjectId(1),
        target,
        ArmSelection::Right,
        ArmAttack::Backhoe,
        kick_rules(),
    )
    .unwrap();
    let attack = &report.attacks[0];
    assert!(attack.hit);
    assert_eq!(dealt(attack.impact.as_slice()), 6);
    world.validate(&config).unwrap();
}
