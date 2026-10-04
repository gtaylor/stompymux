//! Plasma heat follows primary damage transfers and shares persistent target dice.
use crate::support;
use stompymux_rs::*;

/// Compare identical material hits, then account for exactly the post-damage plasma rolls.
#[tokio::test]
async fn plasma_heat_transfer_unwind_and_saved_dice_replay() {
    let (_dir, config, mut base) = support::isolated_world().await;
    let id = base.create(&config, "Plasma target".into(), Kind::Thing);
    let object = base.objects.get_mut(&id).unwrap();
    object.location = Some(ObjectId(config.start()));
    object.home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut base,
        id,
        BattleTemplate::parse("AS7-D", include_str!("fixtures/btech/mechs/AS7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut base, id, support::FIXTURE_DICE_SEED);
    let rules = BattleHitRules {
        inferno_penalty: false,
        exile_stun_mode: 0,
    };
    for (transfer, fatal, expected_rolls) in [(false, false, 1), (true, false, 3), (false, true, 0)]
    {
        let mut scenario = base.clone();
        scenario
            .btech
            .rewrite_unit_record(id, |record| {
                let sections = &mut record["sections"];
                if transfer {
                    for section in ["LeftArm", "LeftTorso"] {
                        sections[section]["armor"] = 0.into();
                        sections[section]["internal"] = 1.into();
                    }
                }
                if fatal {
                    sections["CenterTorso"]["armor"] = 0.into();
                    sections["CenterTorso"]["internal"] = 1.into();
                }
            })
            .unwrap();
        let section = if fatal {
            BattleSection::CenterTorso
        } else {
            BattleSection::LeftArm
        };
        let (before, ordinary, ordinary_report) = (0..=u8::MAX)
            .find_map(|seed| {
                let mut before = scenario.clone();
                before
                    .btech
                    .set_unit_dice(id, BattleDice::seeded([seed; 32]))
                    .unwrap();
                let mut ordinary = before.clone();
                let report = resolve_battle_salvo(
                    &mut ordinary,
                    id,
                    BattleWeapon::Ac10,
                    BattleHitArc::Front,
                    rules,
                )
                .ok()?;
                let group = &report.groups[0];
                (group.hit.section == section
                    && group.impact.criticals.is_empty()
                    && group.impact.phases.len()
                        == if transfer {
                            5
                        } else if fatal {
                            2
                        } else {
                            1
                        })
                .then_some((before, ordinary, report))
            })
            .expect("seed for an isolated material path");
        persistence::save(&config.database(), &before)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let mut clan = before.clone();
        let clan_report = resolve_battle_salvo(
            &mut clan,
            id,
            BattleWeapon::ClanPlasmaRifle,
            BattleHitArc::Front,
            rules,
        )
        .unwrap();
        assert_eq!(clan_report, ordinary_report);
        assert_eq!(clan.btech, ordinary.btech);
        let mut plasma = before.clone();
        let report = resolve_battle_salvo(
            &mut plasma,
            id,
            BattleWeapon::PlasmaRifle,
            BattleHitArc::Front,
            rules,
        )
        .unwrap();
        let replay = resolve_battle_salvo(
            &mut restored,
            id,
            BattleWeapon::PlasmaRifle,
            BattleHitArc::Front,
            rules,
        )
        .unwrap();
        assert_eq!(report, replay);
        assert_eq!(plasma.btech, restored.btech);
        let mut expected = serde_json::to_value(&ordinary.btech).unwrap();
        let unit = &mut expected["constructed"][id.0.to_string()];
        let mut dice: BattleDice = serde_json::from_value(unit["dice"].clone()).unwrap();
        let rolls: Vec<_> = (0..expected_rolls).map(|_| dice.d6()).collect();
        unit["dice"] = serde_json::to_value(dice).unwrap();
        unit["heat"]["stored"] = rolls.iter().map(|&v| f64::from(v)).sum::<f64>().into();
        assert_eq!(report.groups[0].impact.plasma_heat, rolls);
        assert_eq!(
            report.groups[0].impact.phases,
            ordinary_report.groups[0].impact.phases
        );
        assert_eq!(serde_json::to_value(&plasma.btech).unwrap(), expected);
    }
}

/// Plasma bins lose their supply on a critical without detonating or disabling a separate mount.
#[test]
fn plasma_ammunition_is_inert_and_mount_facts_are_distinct() {
    let weapon = BattleWeapon::PlasmaRifle;
    assert_eq!(weapon.mass(), 6144);
    assert_eq!(weapon.gunnery_skill(true), "Gunnery-Ballistic");
    assert!(weapon.supports_targeting_computer());
    assert!(!weapon.supports_heat_mode());
    for rounds in [0, 1, 10, u16::MAX] {
        assert_eq!(weapon.ammunition_explosion_damage(rounds), 0);
    }
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let arm = template.sections.get_mut(&BattleSection::LeftArm).unwrap();
    for slot in [2, 3] {
        arm.criticals.get_mut(&slot).unwrap().equipment = weapon.name().into();
    }
    let bin = template
        .sections
        .get_mut(&BattleSection::RightTorso)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap();
    bin.equipment = "Ammo_IS.PlasmaRifle".into();
    bin.data = "10".into();
    let mut unit = BattleUnit::from_template(template).unwrap();
    let loadout = unit.loadout().unwrap();
    let location = loadout.ammunition[0].location;
    let index = loadout
        .weapons
        .iter()
        .position(|m| m.weapon == weapon)
        .unwrap();
    assert_eq!(
        unit.destroy_critical(location).unwrap(),
        Some(BattleCriticalLoss::Ammunition {
            index: 0,
            rounds: 10,
            explosion_damage: 0,
        })
    );
    assert!(unit.weapon_readiness(index).unwrap().intact);
    assert_eq!(unit.weapon_readiness(index).unwrap().ammunition, 0);
}
