//! Whole-hit material/critical cascades, deterministic replay and database transaction boundaries.
use crate::support;
use stompymux_rs::{
    BattleDice, BattleHit, BattleImpactEffect as Effect, BattleSection as Section, BattleTemplate,
    Kind, ObjectId, create_battle_unit, persistence, resolve_battle_impact,
};

/// Seed an isolated owned unit stream for deterministic damage scenarios.
fn seed(world: &mut stompymux_rs::World, id: ObjectId, value: u8) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
}

/// A supported constructed unit in an isolated world.
async fn fixture() -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
) {
    let (dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Impact Jenner".into(), Kind::Thing);
    let object = world.objects.get_mut(&id).unwrap();
    object.location = Some(ObjectId(config.start()));
    object.home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap(),
    )
    .unwrap();
    seed(&mut world, id, 0);
    (dir, config, world, id)
}

/// An already-selected conventional weapon hit, with no head-graze reroll.
fn hit(section: Section, tac: bool) -> BattleHit {
    BattleHit {
        section,
        rear_armor: false,
        through_armor_critical: tac,
        crew_stun: false,
    }
}

#[tokio::test]
async fn normal_hits_replay_and_failed_commits_preserve_all_phases_and_dice() {
    use sqlx::Connection;
    let (_dir, config, mut world, id) = fixture().await;
    persistence::save(&config.database(), &world).await.unwrap();
    let before = world.clone();
    assert!(
        resolve_battle_impact(&mut world, ObjectId(-1), hit(Section::LeftArm, false), 10).is_err()
    );
    let mut empty = before.clone();
    let zero = resolve_battle_impact(&mut empty, id, hit(Section::LeftArm, true), 0).unwrap();
    assert!(zero.phases.is_empty());
    stompymux_rs::roll_unit_dice(&mut world, id, 2).unwrap();
    assert_eq!(empty.btech, world.btech);
    world = before.clone();
    assert_eq!(world.btech, before.btech);
    let report = resolve_battle_impact(&mut world, id, hit(Section::LeftArm, false), 20).unwrap();
    let mut repeated = before.clone();
    assert_eq!(
        resolve_battle_impact(&mut repeated, id, hit(Section::LeftArm, false), 20).unwrap(),
        report
    );
    assert_eq!(repeated.btech, world.btech);
    assert!(
        report
            .pending_effects
            .contains(&Effect::SectionLost(Section::LeftArm))
    );
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER reject_impact BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'impact failure'); END;").execute(&mut sql).await.unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        before.btech
    );
    sqlx::query("DROP TRIGGER reject_impact")
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn ammunition_cascade_bypasses_torso_armor_and_consumes_bin_once() {
    let (_dir, _config, mut baseline, id) = fixture().await;
    for slot in [1, 2] {
        stompymux_rs::destroy_battle_critical(
            &mut baseline,
            id,
            stompymux_rs::CriticalLocation {
                section: Section::RightTorso,
                slot,
            },
        )
        .unwrap();
    }
    let mut found = false;
    for value in 0..64 {
        let mut world = baseline.clone();
        seed(&mut world, id, value);
        let report =
            resolve_battle_impact(&mut world, id, hit(Section::RightTorso, true), 1).unwrap();
        if !report.pending_effects.contains(&Effect::ExplosionInjury) {
            continue;
        }
        assert!(report.destroyed);
        assert_eq!(world.btech.constructed_units()[&id].ammunition(), &[0]);
        let center = report
            .phases
            .iter()
            .find(|phase| phase.section == Section::CenterTorso)
            .unwrap();
        assert_eq!(center.absorbed, 11);
        assert_eq!(
            report
                .pending_effects
                .iter()
                .filter(|effect| **effect == Effect::ExplosionInjury)
                .count(),
            1
        );
        assert!(
            report
                .pending_effects
                .contains(&Effect::SectionLost(Section::RightArm))
        );
        found = true;
        break;
    }
    assert!(
        found,
        "seeded scenarios must exercise an ammunition critical"
    );
}

#[tokio::test]
async fn internal_twelve_severs_limb_without_transferring_overflow() {
    let (_dir, _config, baseline, id) = fixture().await;
    let mut found = false;
    for value in 0..128 {
        let mut world = baseline.clone();
        seed(&mut world, id, value);
        let report =
            resolve_battle_impact(&mut world, id, hit(Section::LeftArm, false), 30).unwrap();
        if report.phases.len() != 2 {
            continue;
        }
        assert_eq!(
            world.btech.constructed_units()[&id].sections()[&Section::LeftTorso],
            baseline.btech.constructed_units()[&id].sections()[&Section::LeftTorso]
        );
        assert_eq!(report.phases[1].remaining, 0);
        found = true;
        break;
    }
    assert!(found, "seeded scenarios must exercise limb severing");
}

/// A wreck alone does not imply crew death; head and cockpit losses are explicit casualty triggers.
#[tokio::test]
async fn crew_casualties_distinguish_head_cockpit_and_center_torso() {
    use stompymux_rs::BattleCrewCasualty;
    let (_dir, config, base, id) = fixture().await;
    let mut head = base.clone();
    let report = resolve_battle_impact(&mut head, id, hit(Section::Head, false), 100).unwrap();
    assert!(report.destroyed);
    assert_eq!(
        report.crew_casualty(),
        Some(BattleCrewCasualty::HeadDestroyed)
    );
    let mut torso = base.clone();
    let report =
        resolve_battle_impact(&mut torso, id, hit(Section::CenterTorso, false), 100).unwrap();
    assert!(report.destroyed);
    assert_eq!(report.crew_casualty(), None);
    let mut cockpit = None;
    for dice in 0..=255 {
        let mut candidate = base.clone();
        seed(&mut candidate, id, dice);
        let report =
            resolve_battle_impact(&mut candidate, id, hit(Section::Head, true), 1).unwrap();
        if report.crew_casualty() == Some(BattleCrewCasualty::CockpitDestroyed) {
            cockpit = Some((candidate, report));
            break;
        }
    }
    let (cockpit, report) = cockpit.expect("A deterministic cockpit critical should be reachable");
    assert!(report.destroyed);
    assert!(
        !report
            .phases
            .iter()
            .any(|phase| phase.destroyed_sections.contains(&Section::Head))
    );
    cockpit.validate(&config).unwrap();
    let report =
        resolve_battle_impact(&mut base.clone(), id, hit(Section::Head, false), 1).unwrap();
    assert_eq!(report.crew_casualty(), None);
}

/// A surviving ammunition explosion applies RPG injury in the cascade and honors Pain Resistance.
#[tokio::test]
async fn character_explosion_injuries_are_applied_once() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id) = fixture().await;
    base.objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    base.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    for player in [ObjectId(1), ObjectId(2)] {
        base.objects.get_mut(&player).unwrap().location = Some(id);
        base.objects
            .get_mut(&player)
            .unwrap()
            .flags
            .insert(Flag::Connected);
    }
    set_battle_character(
        &mut base,
        ObjectId(1),
        BattleCharacter {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    assign_battle_pilot(&mut base, id, ObjectId(1)).unwrap();
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][id.0.to_string()]["ammunition"] = serde_json::json!([1]);
    base.btech = serde_json::from_value(state).unwrap();
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
    )
    .unwrap();
    for resistant in [false, true] {
        let mut found = false;
        for dice in 0..=255 {
            let mut candidate = base.clone();
            seed(&mut candidate, id, dice);
            set_battle_character_value(
                &mut candidate,
                ObjectId(1),
                "Pain_Resistance",
                BattleCharacterValue {
                    value: u8::from(resistant),
                    ..Default::default()
                },
            )
            .unwrap();
            *scripts.world_mut() = candidate;
            let report = resolve_battle_impact_action(
                &scripts,
                &config,
                id,
                hit(Section::RightTorso, true),
                1,
            )
            .unwrap();
            if report.character_injuries.is_empty() {
                continue;
            }
            assert_eq!(report.character_injuries.len(), 1);
            let messages = scripts.drain_outbox();
            assert_eq!(
                messages
                    .iter()
                    .filter(|(to, message)| *to == ObjectId(1)
                        && message.source().contains("attempt to keep consciousness"))
                    .count(),
                1
            );
            assert!(
                !messages
                    .iter()
                    .any(|(to, message)| *to == ObjectId(2)
                        && message.source().contains("conscious"))
            );
            assert!(!report.pending_effects.contains(&Effect::ExplosionInjury));
            assert!(!report.destroyed);
            assert_eq!(
                report.character_injuries[0].injury.bruise_added,
                if resistant { 10 } else { 20 }
            );
            assert_eq!(
                scripts.world().btech.constructed_units()[&id].ammunition(),
                &[0]
            );
            scripts.world().validate(&config).unwrap();
            found = true;
            break;
        }
        assert!(found, "A surviving explosion should be reachable");
    }
}

/// Character actions consume stun once, publish its notice, and roll it back with failed casualties.
#[tokio::test]
async fn character_stun_actions_publish_and_roll_back() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = fixture().await;
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    for player in [ObjectId(1), ObjectId(2)] {
        let object = world.objects.get_mut(&player).unwrap();
        object.location = Some(id);
        object.flags.insert(Flag::Connected);
    }
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let stunned = |section| BattleHit {
        section,
        crew_stun: true,
        ..hit(section, false)
    };
    let report =
        resolve_battle_impact_action(&scripts, &config, id, stunned(Section::LeftArm), 0).unwrap();
    assert!(report.phases.is_empty());
    roll_unit_dice(&mut world, id, 2).unwrap();
    assert_eq!(scripts.world().btech, world.btech);
    assert!(scripts.drain_outbox().is_empty());
    scripts
        .world_mut()
        .objects
        .remove(&ObjectId(config.battletech.afterlife_dbref));
    assert!(
        resolve_battle_impact_action(&scripts, &config, id, stunned(Section::Head), 100).is_err()
    );
    assert_eq!(scripts.world().btech, world.btech);
    assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(id));
    assert!(scripts.drain_outbox().is_empty());
    *scripts.world_mut() = world;
    let report =
        resolve_battle_impact_action(&scripts, &config, id, stunned(Section::LeftArm), 1).unwrap();
    assert!(!report.pending_effects.contains(&Effect::CrewStun));
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].stun_remaining(),
        10
    );
    let messages = scripts.drain_outbox();
    assert_eq!(
        messages
            .iter()
            .filter(|(to, message)| *to == ObjectId(1)
                && message.source().contains("momentarily stunned"))
            .count(),
        1
    );
    world = scripts.world().clone();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    for _ in 0..10 {
        assert_eq!(
            advance_battle_stun(&mut world),
            advance_battle_stun(&mut loaded)
        );
    }
    assert_eq!(world.btech, loaded.btech);
    assert_eq!(world.btech.constructed_units()[&id].stun_remaining(), 0);
}

/// CASE II takes one internal point, vents the rest through the torso's rear armor and loses
/// any excess, so the center torso survives and the pilot is injured once instead of twice.
#[tokio::test]
async fn case_ii_vents_ammunition_explosion_through_local_armor() {
    let (dir, config, mut baseline) = support::isolated_world().await;
    let id = baseline.create(&config, "Vented Jenner".into(), Kind::Thing);
    let object = baseline.objects.get_mut(&id).unwrap();
    object.location = Some(ObjectId(config.start()));
    object.home = Some(ObjectId(config.home()));
    let source = include_str!("fixtures/btech/mechs/JR7-D").replace(
        "    CRIT_2-3\t\t  { JumpJet - - }\nCenter_Torso",
        "    CRIT_2-3\t\t  { JumpJet - - }\n    CRIT_4\t\t  { CASE-II - - }\nCenter_Torso",
    );
    let template = BattleTemplate::parse(&source).unwrap();
    assert!(
        template.sections[&Section::RightTorso]
            .criticals
            .values()
            .any(|part| part.equipment == "CASE-II")
    );
    create_battle_unit(&mut baseline, id, template).unwrap();
    assert!(baseline.btech.constructed_units()[&id].has_case_ii(Section::RightTorso));
    for slot in [1, 2] {
        stompymux_rs::destroy_battle_critical(
            &mut baseline,
            id,
            stompymux_rs::CriticalLocation {
                section: Section::RightTorso,
                slot,
            },
        )
        .unwrap();
    }
    let mut found = false;
    for value in 0..64 {
        let mut world = baseline.clone();
        seed(&mut world, id, value);
        let report =
            resolve_battle_impact(&mut world, id, hit(Section::RightTorso, true), 1).unwrap();
        if !report
            .pending_effects
            .contains(&Effect::VentedExplosionInjury)
        {
            continue;
        }
        assert!(!report.destroyed);
        assert!(!report.pending_effects.contains(&Effect::ExplosionInjury));
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.ammunition(), &[0]);
        let torso = &unit.sections()[&Section::RightTorso];
        assert_eq!(torso.internal, 7);
        assert_eq!(torso.rear, 0);
        assert_eq!(torso.armor, 7);
        assert!(
            report
                .phases
                .iter()
                .all(|phase| phase.section == Section::RightTorso)
        );
        assert_eq!(
            unit.sections()[&Section::CenterTorso],
            baseline.btech.constructed_units()[&id].sections()[&Section::CenterTorso]
        );
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        found = true;
        break;
    }
    drop(dir);
    assert!(
        found,
        "seeded scenarios must exercise the ammunition critical"
    );
}

/// Build the Jenner fixture with extra chassis technology flags.
async fn technology_fixture(
    specials: &str,
) -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
) {
    let (dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Technology Jenner".into(), Kind::Thing);
    let object = world.objects.get_mut(&id).unwrap();
    object.location = Some(ObjectId(config.start()));
    object.home = Some(ObjectId(config.home()));
    let source = include_str!("fixtures/btech/mechs/JR7-D").replace(
        "Specials\t { FlipArms }",
        &format!("Specials\t {{ FlipArms {specials} }}"),
    );
    create_battle_unit(&mut world, id, BattleTemplate::parse(&source).unwrap()).unwrap();
    seed(&mut world, id, 0);
    (dir, config, world, id)
}

/// Each hardened armor point stops two damage and overflow passes at full value; reinforced
/// structure halves and composite structure doubles internal damage. Full names and reference abbreviations behave the same.
#[tokio::test]
async fn armor_and_structure_technologies_modify_mech_damage() {
    // Left arm: 4 armor and 6 internal structure on the Jenner.
    for (specials, damage, armor, internal) in [
        ("", 2, 2, 6),
        ("HardenedArmor_Tech", 2, 3, 6),
        ("HARM", 2, 3, 6),
        // Four hardened points stop eight of twelve; four reach the structure.
        ("HardenedArmor_Tech", 12, 0, 2),
        ("HARM", 9, 0, 5),
        ("", 6, 0, 4),
        ("ReinforcedInternal_Tech", 6, 0, 5),
        ("RINT", 6, 0, 5),
        ("CompositeInternal_Tech", 6, 0, 2),
        ("CINT", 6, 0, 2),
    ] {
        let (_dir, _config, mut world, id) = technology_fixture(specials).await;
        let before = world.btech.constructed_units()[&id].sections()[&Section::LeftArm].clone();
        assert_eq!((before.armor, before.internal), (4, 6), "{specials}");
        resolve_battle_impact(&mut world, id, hit(Section::LeftArm, false), damage).unwrap();
        let after = &world.btech.constructed_units()[&id].sections()[&Section::LeftArm];
        assert_eq!(
            (after.armor, after.internal),
            (armor, internal),
            "{specials} {damage}"
        );
    }
}

/// Construction mass follows the technology flags: hardened armor weighs double, reinforced
/// structure double and composite structure half, and the small cockpit weighs two tons.
#[tokio::test]
async fn technology_flags_change_mech_mass() {
    let (_dir, _config, world, id) = technology_fixture("").await;
    let base = world.btech.constructed_units()[&id].mass().unwrap();
    for (specials, armor, structure, cockpit) in [
        (
            "HardenedArmor_Tech",
            base.armor * 2,
            base.structure,
            base.cockpit,
        ),
        (
            "ReinforcedInternal_Tech",
            base.armor,
            base.structure * 2,
            base.cockpit,
        ),
        (
            "CompositeInternal_Tech",
            base.armor,
            // Half of 3.5 tons rounds up to the next half ton, as Endo Steel does.
            2048,
            base.cockpit,
        ),
        ("SmallCockpit_Tech", base.armor, base.structure, 2048),
        ("SMCPIT", base.armor, base.structure, 2048),
    ] {
        let (_dir, _config, world, id) = technology_fixture(specials).await;
        let mass = world.btech.constructed_units()[&id].mass().unwrap();
        assert_eq!(
            (mass.armor, mass.structure, mass.cockpit),
            (armor, structure, cockpit),
            "{specials}"
        );
    }
}

/// Running laser heat sinks glow. The Nightgyr ships
/// with laser heat sinks; the abbreviation behaves the same and removing the flag stops the glow.
#[tokio::test]
async fn laser_heat_sinks_glow_while_running() {
    let original = include_str!("../game/mechs/NightGyr-A");
    assert!(original.contains("LaserHS_Tech"));
    for (source, glows) in [
        (original.to_string(), true),
        (original.replace("LaserHS_Tech", "LHS"), true),
        (original.replace(" LaserHS_Tech", ""), false),
    ] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let id = world.create(&config, "Glowing Nightgyr".into(), Kind::Thing);
        let object = world.objects.get_mut(&id).unwrap();
        object.location = Some(ObjectId(config.start()));
        object.home = Some(ObjectId(config.home()));
        create_battle_unit(&mut world, id, BattleTemplate::parse(&source).unwrap()).unwrap();
        assert!(
            world.btech.constructed_units()[&id]
                .definition()
                .has_double_heat_sinks()
        );
        assert!(!stompymux_rs::battle_unit_illuminated(&world, id));
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["power"] = serde_json::json!({"state": "running"});
        world.btech = serde_json::from_value(state).unwrap();
        assert_eq!(stompymux_rs::battle_unit_illuminated(&world, id), glows);
    }
}
