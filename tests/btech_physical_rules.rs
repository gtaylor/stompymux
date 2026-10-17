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
