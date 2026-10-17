//! Who spots for LRM indirect fire (Total Warfare pp. 111, 139, 142): a declared spotter with
//! its own line of sight, a friendly TAG designation, or a Narc pod for Narc-equipped missiles;
//! direct fire needing a clear line of sight even at a contact held on sensors; and guided
//! missiles drawn onto a friendly unit carrying a Nemesis pod.
use crate::support;
use stompymux_rs::*;

/// An LRM boat at row 9, a scout at row 3 and an enemy target at row 1 on a one-hex strip
/// laid out by `rows`, with the scout's sight of the target already acquired.
async fn strip(
    rows: &str,
    boat: MechTemplate,
    scout: MechTemplate,
) -> (tempfile::TempDir, Config, World, [ObjectId; 3]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Strip".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "strip.map",
        MapAsset::from_cells(rows).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let mut units = Vec::new();
    for (pilot, template, row, team) in [
        (Some(ObjectId(1)), boat, 9, 0),
        (Some(ObjectId(2)), scout, 3, 0),
        (None, jenner(), 1, 2),
    ] {
        let id = world.create(&config, format!("Strip unit {row}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(&mut world, id, template).unwrap();
        support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, id, map, 0, row).unwrap();
        if let Some(pilot) = pilot {
            world.objects.get_mut(&pilot).unwrap().location = Some(id);
            assign_battle_pilot(&mut world, id, pilot).unwrap();
        }
        set_battle_unit_signature(
            &mut world,
            id,
            UnitSignature {
                team,
                ..Default::default()
            },
        )
        .unwrap();
        world.btech.set_unit_power(id, Power::Running).unwrap();
        world
            .btech
            .set_unit_dice(id, Dice::seeded([31; 32]))
            .unwrap();
        units.push(id);
    }
    let [boat, scout, target] = [units[0], units[1], units[2]];
    let seen = (0..=255).any(|seed| {
        world
            .btech
            .set_unit_dice(scout, Dice::seeded([seed; 32]))
            .unwrap();
        refresh_battle_contacts(&mut world, &[scout, boat]).unwrap();
        visible_battle_contact(&world, scout, target)
            .unwrap()
            .is_some()
    });
    assert!(seen, "the scout never saw the target");
    (dir, config, world, [boat, scout, target])
}

/// The open strip with a wall between the boat and the others.
const WALLED: &str = "1 10\n.0\n.0\n.0\n.0\n.0\n.0\n=6\n.0\n.0\n.0\n";

/// Standard shooting rules.
fn shot_rules() -> ShotRules {
    ShotRules {
        tsm_tow_bonus: true,
        vehicle_impact: VehicleImpactRules::STANDARD,
        aim: AimRules {
            override_weapon_arcs: false,
        },
        extended_gunnery: false,
        extended_piloting: false,
        target_toughness: false,
    }
}

/// The boat's LRM-20.
fn lrm(world: &World, boat: ObjectId) -> usize {
    world.btech.constructed_units()[&boat]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::Lrm20)
        .unwrap()
}

/// A plain Jenner.
fn jenner() -> MechTemplate {
    MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap()
}

/// The Atlas, with Narc-equipped rounds in its LRM-20 bin when `narc`.
fn atlas(narc: bool) -> MechTemplate {
    let source = include_str!("fixtures/btech/units/AS7-D.toml");
    let source = if narc {
        source
            .replace(
                "{ at = \"1-5\", item = \"IS.LRM-20\" }",
                "{ at = \"1-5\", item = \"IS.LRM-20\", modes = [\"Narc/Smoke\"] }",
            )
            .replace(
                "{ at = \"7-8\", item = \"Ammo_IS.LRM-20\", rounds = 6 }",
                "{ at = \"7-8\", item = \"Ammo_IS.LRM-20\", rounds = 6, modes = [\"Narc/Smoke\"] }",
            )
    } else {
        source.into()
    };
    MechTemplate::parse("AS7-D", &source).unwrap()
}

/// The Atlas with an Artemis V system guiding its LRM-20, which fires Artemis rounds, and its
/// SRM-6 removed, since fire control must guide every launcher or none.
fn artemis_v_atlas() -> MechTemplate {
    let source = include_str!("fixtures/btech/units/AS7-D.toml")
        .replace(
            "    { at = 6, item = \"Ammo_IS.SRM-6\", rounds = 15 },\n",
            "",
        )
        .replace("    { at = \"9-10\", item = \"IS.SRM-6\" },\n", "")
        .replace(
            "{ at = \"1-5\", item = \"IS.LRM-20\" }",
            "{ at = \"1-5\", item = \"IS.LRM-20\", modes = [\"Artemis/Mine\"] }",
        )
        .replace(
            "{ at = \"7-8\", item = \"Ammo_IS.LRM-20\", rounds = 6 }",
            "{ at = \"7-8\", item = \"Ammo_IS.LRM-20\", rounds = 6, modes = [\"Artemis/Mine\"] }",
        )
        .replace(
            "    { at = 11, item = \"HeatSink\" },\n]\n\n[sections.right_torso]",
            "    { at = \"11-12\", item = \"ArtemisV\", link = 1 },\n]\n\n[sections.right_torso]",
        );
    crate::support::templates::legal(MechTemplate::parse("AS7-D", &source).unwrap())
}

/// A Jenner with TAG in its left torso, lightened to stay within its tonnage.
fn tag_jenner() -> MechTemplate {
    let mut template = jenner();
    template
        .sections
        .get_mut(&MechSection::LeftTorso)
        .unwrap()
        .criticals
        .insert(
            7,
            CriticalDefinition {
                equipment: "TAG".into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    crate::support::templates::legal(template)
}

/// Narc-equipped missiles fire indirectly at a target carrying a Narc pod with no spotter at
/// all (Total Warfare p. 139): +1 for indirect fire, nothing for a spotter's movement or
/// terrain. Without the pod the boat, which cannot see the target, may not fire.
#[tokio::test]
async fn narc_missiles_fire_indirectly_at_a_podded_target_without_a_spotter() {
    let (_dir, _config, mut world, [boat, _scout, target]) =
        strip(WALLED, atlas(true), jenner()).await;
    select_battle_target(&mut world, boat, ObjectId(1), Some(target)).unwrap();
    let lrm = lrm(&world, boat);
    let rules = shot_rules();
    let unpodded = resolve_battle_shot(&mut world.clone(), boat, ObjectId(1), target, lrm, rules)
        .unwrap_err()
        .to_string();
    assert!(unpodded.contains("indirect fire"), "{unpodded}");
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["beacons"] = serde_json::json!({ "CenterTorso": ["narc"] });
        })
        .unwrap();
    let aim = battle_aim_modifiers(&world, boat, target, lrm, 4, rules.aim).unwrap();
    assert_eq!(
        aim.indirect,
        Some(IndirectAim {
            spotter: None,
            movement: 0,
            spotter_attacked: false,
            tag_guided: false,
        })
    );
    assert_eq!(aim.perception.unwrap().modifier, 0);
    let shot = resolve_battle_shot(&mut world, boat, ObjectId(1), target, lrm, rules).unwrap();
    assert!(shot.launched);
    assert!(shot.aim.indirect.is_some());
}

/// A friendly unit's TAG designation spots the target for LRM indirect fire (Total Warfare
/// p. 142), with no declared spotter; TAG is used before the attack phase, so the designator's
/// own attacks add nothing.
#[tokio::test]
async fn a_tag_designation_spots_for_indirect_fire() {
    let (_dir, _config, mut world, [boat, scout, target]) =
        strip(WALLED, atlas(false), tag_jenner()).await;
    select_battle_target(&mut world, boat, ObjectId(1), Some(target)).unwrap();
    let lrm = lrm(&world, boat);
    let rules = shot_rules();
    assert!(
        battle_aim_modifiers(&world, boat, target, lrm, 4, rules.aim)
            .unwrap()
            .indirect
            .is_none()
    );
    world
        .btech
        .set_unit_dice(scout, Dice::seeded([support::roll_twelve_seed(); 32]))
        .unwrap();
    select_battle_tag(
        &mut world,
        scout,
        ObjectId(2),
        Some(target),
        TagRules {
            aim: AimRules::STANDARD,
            extended_gunnery: true,
        },
    )
    .unwrap();
    assert_eq!(battle_tagged_by(&world, target), Some(scout));
    let aim = battle_aim_modifiers(&world, boat, target, lrm, 4, rules.aim).unwrap();
    let indirect = aim.indirect.unwrap();
    assert_eq!(indirect.spotter, Some(scout));
    assert!(!indirect.spotter_attacked);
    let shot = resolve_battle_shot(&mut world, boat, ObjectId(1), target, lrm, rules).unwrap();
    assert!(shot.launched);
}

/// A declared spotter must have line of sight to the target itself (Total Warfare p. 111): a
/// contact a lookout beside the target relays will not do.
#[tokio::test]
async fn a_spotter_needs_its_own_line_of_sight() {
    // A wall between the scout and the target, which the lookout at row 0 sees.
    let rows = "1 10\n.0\n.0\n=6\n.0\n.0\n.0\n=6\n.0\n.0\n.0\n";
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Strip".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "strip.map",
        MapAsset::from_cells(rows).unwrap(),
    )
    .unwrap();
    let mut units = Vec::new();
    for (pilot, row, team) in [
        (Some(ObjectId(1)), 9, 0),
        (Some(ObjectId(2)), 3, 0),
        (None, 0, 0),
        (None, 1, 2),
    ] {
        let id = world.create(&config, format!("Strip unit {row}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        let source = if row == 9 {
            include_str!("fixtures/btech/units/AS7-D.toml")
        } else {
            include_str!("fixtures/btech/units/JR7-D.toml")
        };
        create_battle_unit(&mut world, id, MechTemplate::parse("test", source).unwrap()).unwrap();
        support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, id, map, 0, row).unwrap();
        if let Some(pilot) = pilot {
            world.objects.get_mut(&pilot).unwrap().location = Some(id);
            assign_battle_pilot(&mut world, id, pilot).unwrap();
        }
        set_battle_unit_signature(
            &mut world,
            id,
            UnitSignature {
                team,
                ..Default::default()
            },
        )
        .unwrap();
        world.btech.set_unit_power(id, Power::Running).unwrap();
        units.push(id);
    }
    let [boat, scout, lookout, target] = [units[0], units[1], units[2], units[3]];
    let seen = (0..=255).any(|seed| {
        world
            .btech
            .set_unit_dice(lookout, Dice::seeded([seed; 32]))
            .unwrap();
        refresh_battle_contacts(&mut world, &[lookout, scout, boat]).unwrap();
        visible_battle_contact(&world, lookout, target)
            .unwrap()
            .is_some()
    });
    assert!(seen, "the lookout never saw the target");
    assert!(
        battle_unit_terrain_los(&world, scout, target)
            .unwrap()
            .blocked
    );
    select_battle_target(&mut world, scout, ObjectId(2), Some(target)).unwrap();
    select_battle_spotter(&mut world, scout, ObjectId(2), Some(scout)).unwrap();
    select_battle_spotter(&mut world, boat, ObjectId(1), Some(scout)).unwrap();
    assert_eq!(
        battle_spotter_target(&world, boat).unwrap_err().to_string(),
        "Your spotter does not have a target in LOS!"
    );
}

/// Three points of woods break line of sight, so a target that a teammate beside it relays may
/// not take direct fire through them (Total Warfare pp. 99-100); two points leave a clear line.
#[tokio::test]
async fn direct_fire_needs_a_clear_line_through_the_woods() {
    for (rows, clear) in [
        ("2 6\n.0.0\n`0.0\n.0.0\n.0.0\n.0.0\n.0.0\n", true),
        ("2 6\n.0.0\n\"0.0\n`0.0\n.0.0\n.0.0\n.0.0\n", false),
    ] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let map = world.create(&config, "Woods".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "woods.map",
            MapAsset::from_cells(rows).unwrap(),
        )
        .unwrap();
        let mut units = Vec::new();
        for (pilot, (x, y), team) in [
            (Some(ObjectId(1)), (0, 4), 0),
            (Some(ObjectId(2)), (1, 0), 0),
            (None, (0, 0), 2),
        ] {
            let id = world.create(&config, format!("Woods unit {x},{y}"), Kind::Thing);
            world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
            create_battle_unit(&mut world, id, jenner()).unwrap();
            support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
            place_battle_unit(&mut world, id, map, x, y).unwrap();
            if let Some(pilot) = pilot {
                world.objects.get_mut(&pilot).unwrap().location = Some(id);
                assign_battle_pilot(&mut world, id, pilot).unwrap();
            }
            set_battle_unit_signature(
                &mut world,
                id,
                UnitSignature {
                    team,
                    ..Default::default()
                },
            )
            .unwrap();
            world.btech.set_unit_power(id, Power::Running).unwrap();
            units.push(id);
        }
        let [shooter, lookout, target] = [units[0], units[1], units[2]];
        let terrain = battle_unit_terrain_los(&world, shooter, target).unwrap();
        assert!(!terrain.blocked);
        assert_eq!(terrain.clear_line(), clear);
        let seen = (0..=255).any(|seed| {
            world
                .btech
                .set_unit_dice(lookout, Dice::seeded([seed; 32]))
                .unwrap();
            refresh_battle_contacts(&mut world, &[lookout, shooter]).unwrap();
            visible_battle_contact(&world, lookout, target)
                .unwrap()
                .is_some()
        });
        assert!(seen, "the lookout never saw the target");
        let aim = battle_aim_modifiers(&world, shooter, target, 0, 4, AimRules::STANDARD).unwrap();
        let perception = aim.perception.expect("the target is a contact");
        assert_eq!(perception.direct_fire, clear, "{rows}");
    }
}

/// Artemis V takes one off the to-hit number of the launcher it guides (Tactical Operations:
/// Advanced Units & Equipment p. 95), but like Artemis IV does nothing for indirect fire (Total
/// Warfare p. 130).
#[tokio::test]
async fn artemis_v_does_not_help_indirect_fire() {
    let open = "1 10\n.0\n.0\n.0\n.0\n.0\n.0\n.0\n.0\n.0\n.0\n";
    for (rows, accuracy) in [(open, -1), (WALLED, 0)] {
        let (_dir, _config, mut world, [boat, scout, target]) =
            strip(rows, artemis_v_atlas(), jenner()).await;
        let lrm = lrm(&world, boat);
        assert_eq!(
            world.btech.constructed_units()[&boat]
                .ammunition_mode(lrm)
                .unwrap(),
            AmmunitionMode::Artemis
        );
        select_battle_target(&mut world, scout, ObjectId(2), Some(target)).unwrap();
        select_battle_spotter(&mut world, scout, ObjectId(2), Some(scout)).unwrap();
        select_battle_spotter(&mut world, boat, ObjectId(1), Some(scout)).unwrap();
        let aim = battle_aim_modifiers(&world, boat, target, lrm, 4, AimRules::STANDARD).unwrap();
        assert_eq!(aim.indirect.is_some(), rows == WALLED);
        assert_eq!(aim.beacon_accuracy, accuracy, "{rows}");
    }
}

/// A friendly 'Mech carrying a Nemesis pod in the line between an attacker and its target draws
/// the attacker's Narc-equipped missiles onto itself, at +1 and with no multiple-targets term
/// (Total Warfare p. 142); ordinary missiles fly on to the target, and so do guided ones once
/// the pod is gone.
#[tokio::test]
async fn a_nemesis_pod_draws_friendly_guided_missiles() {
    let open = "1 10\n.0\n.0\n.0\n.0\n.0\n.0\n.0\n.0\n.0\n.0\n";
    for (narc, nemesis) in [(true, true), (false, true), (true, false)] {
        let (_dir, _config, mut world, [boat, scout, target]) =
            strip(open, atlas(narc), jenner()).await;
        if nemesis {
            world
                .btech
                .rewrite_unit_record(scout, |record| {
                    record["beacons"] = serde_json::json!({ "CenterTorso": ["nemesis"] });
                })
                .unwrap();
        }
        refresh_battle_contacts(&mut world, &[boat]).unwrap();
        select_battle_target(&mut world, boat, ObjectId(1), Some(target)).unwrap();
        let lrm = lrm(&world, boat);
        let diverted = narc && nemesis;
        let aim = battle_aim_modifiers(&world, boat, target, lrm, 4, AimRules::STANDARD).unwrap();
        assert_eq!(aim.beacon_accuracy, i8::from(diverted), "{narc} {nemesis}");
        assert_eq!(aim.target_lock, 0);
        let shot =
            resolve_battle_shot(&mut world, boat, ObjectId(1), target, lrm, shot_rules()).unwrap();
        assert_eq!(
            shot.target,
            if diverted { scout } else { target },
            "{narc} {nemesis}"
        );
    }
}
