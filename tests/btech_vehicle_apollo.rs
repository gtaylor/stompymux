//! MRM "Apollo" fire control on vehicle and VTOL shooters: the negated MRM to-hit penalty, the
//! one taken from the Cluster Hits roll, and damage that leaves the launcher a plain MRM
//! (Tactical Operations: Advanced Units & Equipment p. 143).
use crate::support;
use stompymux_rs::*;

/// Front slots of an MRM-10, its ammunition and the Apollo system linked to it.
const FRONT: &str = "{ at = 1, item = \"IS.MRM-10\" },\n    { at = 2, item = \"Ammo_IS.MRM-10\", rounds = 24 },\n    { at = 3, item = \"ApolloFCS\", link = 1 },";

/// A tracked tank and a VTOL whose front carries `front`.
pub(crate) fn shooters(front: &str) -> [String; 2] {
    let tank = format!(
        "name = \"Zhukov\"\nclass = \"vehicle\"\nmovement = \"track\"\ntons = 75\nwalk_mp = 3\n\n\
         [construction]\nengine = \"ice\"\n\n[sections.left_side]\narmor = 32\n\n\
         [sections.right_side]\narmor = 32\n\n[sections.front_side]\narmor = 28\nslots = [\n    {front}\n]\n\n\
         [sections.aft_side]\narmor = 24\n\n[sections.turret]\narmor = 44\nslots = [\n    \
         {{ at = \"1-2\", item = \"IS.AC/10\" }},\n    {{ at = \"3-4\", item = \"Ammo_IS.AC/10\", rounds = 10 }},\n]\n"
    );
    let vtol = format!(
        "name = \"Yellow Jacket\"\nclass = \"vtol\"\nmovement = \"vtol\"\ntons = 30\nwalk_mp = 6\n\n\
         [construction]\nengine = \"ice\"\n\n[sections.left_side]\narmor = 13\n\n\
         [sections.right_side]\narmor = 13\n\n[sections.front_side]\narmor = 20\nslots = [\n    {front}\n]\n\n\
         [sections.aft_side]\narmor = 8\n\n[sections.rotor]\narmor = 2\n"
    );
    [tank, vtol]
}

/// The shooter one hex south of a Jenner, with the index of its weapon `weapon`.
pub(crate) async fn engagement(
    source: &str,
    weapon: Weapon,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    let (dir, config, world, shooter, target, _) = support::btech_firing::fixture_with_target(
        source,
        None,
        include_str!("fixtures/btech/units/JR7-D.toml"),
    )
    .await;
    let index = world.btech.vehicles()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == weapon)
        .unwrap();
    (dir, config, world, shooter, target, index)
}

/// Standard vehicle shot policy.
pub(crate) fn rules() -> VehicleShotRules {
    VehicleShotRules {
        shot: ShotRules {
            tsm_tow_bonus: true,
            vehicle_impact: VehicleImpactRules::STANDARD,
            aim: AimRules::STANDARD,
            extended_gunnery: false,
            extended_piloting: false,
            target_toughness: false,
        },
        shooter_criticals: VehicleImpactRules::STANDARD.criticals,
    }
}

/// Seed `id`'s dice with the first uniform seed whose first 2D6 roll is `roll`.
pub(crate) fn seed_roll(world: &mut World, id: ObjectId, roll: u8) {
    let value = (0..=255)
        .find(|value| Dice::seeded([*value; 32]).two_d6() == roll)
        .unwrap();
    world
        .btech
        .set_unit_dice(id, Dice::seeded([value; 32]))
        .unwrap();
}

/// A working Apollo system negates the MRM's +1 to hit; once destroyed the launcher fires as
/// a plain MRM again.
#[tokio::test]
async fn vehicle_apollo_negates_the_mrm_to_hit_penalty_until_destroyed() {
    for source in shooters(FRONT) {
        let (_dir, config, mut world, shooter, target, index) =
            engagement(&source, Weapon::Mrm10).await;
        let accuracy = |world: &World| {
            battle_aim_modifiers(world, shooter, target, index, 4, AimRules::STANDARD)
                .unwrap()
                .weapon_accuracy
        };
        assert!(
            world.btech.vehicles()[&shooter]
                .apollo_operational(index)
                .unwrap()
        );
        assert_eq!(accuracy(&world), 0);
        destroy_battle_vehicle_critical(
            &mut world,
            shooter,
            VehicleCriticalLocation {
                section: VehicleSection::Front,
                slot: 2,
            },
        )
        .unwrap();
        assert!(
            !world.btech.vehicles()[&shooter]
                .apollo_operational(index)
                .unwrap()
        );
        assert_eq!(accuracy(&world), 1);
        world.validate(&config).unwrap();
    }
}

/// The Apollo system takes one from the Cluster Hits roll the table reads, never below 2;
/// the destroyed system no longer does.
#[tokio::test]
async fn vehicle_apollo_takes_one_from_the_cluster_roll() {
    // The Cluster Hits Table's 10 column (Total Warfare p. 114).
    let column = [3, 3, 4, 6, 6, 6, 6, 8, 8, 10, 10];
    for source in shooters(FRONT) {
        let (_dir, config, base, shooter, target, index) = engagement(&source, Weapon::Mrm10).await;
        for working in [true, false] {
            let mut initial = base.clone();
            if !working {
                destroy_battle_vehicle_critical(
                    &mut initial,
                    shooter,
                    VehicleCriticalLocation {
                        section: VehicleSection::Front,
                        slot: 2,
                    },
                )
                .unwrap();
            }
            let mut discriminating = false;
            for value in 0..=63u8 {
                let mut world = initial.clone();
                seed_roll(&mut world, shooter, 12);
                world
                    .btech
                    .set_unit_dice(target, Dice::seeded([value; 32]))
                    .unwrap();
                let report = fire_battle_vehicle_shot(
                    &mut world,
                    shooter,
                    ObjectId(1),
                    target,
                    index,
                    rules(),
                )
                .unwrap();
                assert!(report.hit);
                let Some(TargetSalvo::Mech(salvo)) = &report.salvo else {
                    panic!("a Mech salvo: {:?}", report.salvo);
                };
                let roll = salvo.cluster_roll.unwrap();
                let read = if working { (roll - 1).max(2) } else { roll };
                // Later groups may find the target already destroyed, so count the missiles
                // that struck rather than the damage dealt.
                assert_eq!(
                    salvo.missiles_before_defense,
                    Some(column[usize::from(read) - 2]),
                    "roll {roll}"
                );
                world.validate(&config).unwrap();
                if column[usize::from(roll) - 2] != column[usize::from((roll - 1).max(2)) - 2] {
                    discriminating = true;
                    break;
                }
            }
            assert!(
                discriminating,
                "no seed rolled a discriminating cluster roll"
            );
        }
    }
}

/// A vehicle's Apollo system raises its launcher's BV by 15 percent (Tactical Operations:
/// Advanced Units & Equipment p. 195, note L), and a machine gun array adds its guns' BV
/// times 0.1 (Tactical Operations: Advanced Units & Equipment p. 197, note L), as on a 'Mech.
#[test]
fn vehicle_apollo_and_array_battle_value() {
    let vehicle = |source: &str| {
        Vehicle::new(support::templates::fitted(
            VehicleTemplate::parse("test", source).unwrap(),
        ))
        .unwrap()
    };
    let [tank, _] = shooters(FRONT);
    let guided = vehicle(&tank);
    // Against the same tank without the system, which fires a plain MRM.
    let idle = vehicle(&tank.replace("\n    { at = 3, item = \"ApolloFCS\", link = 1 },", ""));
    let index = 0;
    assert!(guided.apollo_operational(index).unwrap());
    assert!(!idle.apollo_operational(index).unwrap());
    let launcher = f64::from(Weapon::Mrm10.battle_value());
    let gained = guided.battle_value().unwrap().offensive - idle.battle_value().unwrap().offensive;
    // The Zhukov's flanking MP of 5 has a Speed Factor of 1.
    assert!((gained - launcher * 0.15).abs() < 0.001, "{gained}");
    let guns = "{ at = \"1-3\", item = \"IS.MachineGun\" },\n    { at = 4, item = \"IS.MGA\" },\n    { at = 5, item = \"Ammo_IS.MachineGun\", rounds = 200 },";
    let [linked, _] = shooters(guns);
    let [loose, _] = shooters(&guns.replace("{ at = 4, item = \"IS.MGA\" },\n    ", ""));
    let gained = vehicle(&linked).battle_value().unwrap().offensive
        - vehicle(&loose).battle_value().unwrap().offensive;
    let value = 0.1 * 3.0 * f64::from(Weapon::MachineGun.battle_value());
    assert!((gained - value).abs() < 0.001, "{gained}");
}
