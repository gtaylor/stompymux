//! Explicit split weapon links, stable mounts and damage across section boundaries.
use crate::support;
use stompymux_rs::*;

/// Install a supported split mount with a zero-based parent pointer in each extension slot.
fn definition(
    weapon: BattleWeapon,
    parent: BattleSection,
    extension: BattleSection,
) -> BattleTemplate {
    use BattleSection::*;
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let count = weapon.profile().critical_slots;
    let first = if parent == CenterTorso {
        10
    } else if count == 8 {
        6
    } else if matches!(extension, LeftLeg | RightLeg | CenterTorso) && count == 11 {
        3
    } else {
        4
    };
    let mut part = template.sections[&LeftArm].criticals[&2].clone();
    part.equipment = weapon.name().into();
    for slot in first..12 {
        template
            .sections
            .get_mut(&parent)
            .unwrap()
            .criticals
            .insert(slot, part.clone());
    }
    let left = |section| matches!(section, LeftArm | LeftTorso | LeftLeg);
    part.equipment = if left(parent) || left(extension) {
        "SplitCrit_Left"
    } else {
        "SplitCrit_Right"
    }
    .into();
    part.data = format!("{}:{first}", parent.name());
    let extension_first = match extension {
        LeftArm | RightArm => 8,
        LeftTorso => 2,
        RightTorso => 3,
        LeftLeg | RightLeg => 4,
        CenterTorso => 10,
        Head => unreachable!(),
    };
    for slot in extension_first..extension_first + count - (12 - first) {
        template
            .sections
            .get_mut(&extension)
            .unwrap()
            .criticals
            .insert(slot, part.clone());
    }
    template
}

/// Both section traversal orders and every permitted link direction resolve a single mount.
#[test]
fn split_mount_locations_metadata_and_critical_availability() {
    use BattleSection::*;
    for weapon in [
        BattleWeapon::Ac20,
        BattleWeapon::HeavyGaussRifle,
        BattleWeapon::Lbx20,
        BattleWeapon::UltraAc20,
        BattleWeapon::ClanUltraAc20,
    ] {
        for (parent, extension) in [
            (LeftArm, LeftTorso),
            (RightArm, RightTorso),
            (LeftTorso, LeftArm),
            (RightTorso, RightArm),
            (LeftTorso, LeftLeg),
            (RightTorso, RightLeg),
            (LeftTorso, CenterTorso),
            (RightTorso, CenterTorso),
            (CenterTorso, LeftTorso),
            (CenterTorso, RightTorso),
        ] {
            let template = definition(weapon, parent, extension);
            let original = BattleUnit::from_template(template.clone()).unwrap();
            let loadout = original.loadout().unwrap();
            let mut mixed = template;
            for section in mixed.sections.values_mut() {
                for (&slot, part) in &mut section.criticals {
                    part.equipment = if slot % 2 == 0 {
                        part.equipment.to_ascii_lowercase()
                    } else {
                        part.equipment.to_ascii_uppercase()
                    };
                }
            }
            assert_eq!(BattleLoadout::resolve(&mixed).unwrap(), loadout);
            let mounts: Vec<_> = loadout
                .weapons
                .iter()
                .enumerate()
                .filter(|(_, mount)| mount.weapon == weapon)
                .collect();
            assert_eq!(mounts.len(), 1);
            let (index, mount) = mounts[0];
            assert_eq!(
                mount.criticals.len(),
                usize::from(weapon.profile().critical_slots)
            );
            assert_eq!(mount.criticals[0].section, parent);
            let child = *mount.criticals.last().unwrap();
            assert_eq!(child.section, extension);
            assert!(original.critical_candidates(extension).contains(&child));
            let mut damaged = original.clone();
            assert_eq!(
                damaged.destroy_critical(child).unwrap(),
                Some(BattleCriticalLoss::Weapon {
                    index,
                    explosion_damage: weapon.weapon_explosion_damage()
                })
            );
            assert!(!damaged.weapon_intact(index).unwrap());
            assert_eq!(
                damaged.mass().unwrap().equipment,
                original.mass().unwrap().equipment
            );
            if weapon == BattleWeapon::HeavyGaussRifle {
                assert!(
                    mount
                        .criticals
                        .iter()
                        .all(|slot| damaged.lost_criticals().contains(slot))
                );
                assert_eq!(damaged.destroy_critical(mount.criticals[0]).unwrap(), None);
            }
        }
    }
}

/// Broken links, partial runs and inconsistent metadata cannot create operational weapons.
#[test]
fn invalid_split_links_are_rejected() {
    use BattleSection::*;
    let source = definition(BattleWeapon::HeavyGaussRifle, LeftArm, LeftTorso);
    for variant in 0..8 {
        let mut template = source.clone();
        let child = template.sections.get_mut(&LeftTorso).unwrap();
        match variant {
            0 => {
                child.criticals.remove(&4);
            }
            1 => {
                child.criticals.get_mut(&2).unwrap().data = "Left_Arm:5".into();
            }
            2 => {
                child.criticals.get_mut(&2).unwrap().data = "Left_Arm:99".into();
            }
            3 => {
                child.criticals.get_mut(&2).unwrap().equipment = "SplitCrit_Right".into();
            }
            4 => {
                child
                    .criticals
                    .get_mut(&2)
                    .unwrap()
                    .modes
                    .push("RearMount".into());
            }
            5 => {
                let part = child.criticals.remove(&3).unwrap();
                child.criticals.insert(5, part);
            }
            6 => {
                child.criticals.get_mut(&2).unwrap().data = "Right_Arm:4".into();
            }
            7 => {
                child.criticals.get_mut(&2).unwrap().data = "4".into();
            }
            _ => unreachable!(),
        }
        assert!(
            BattleUnit::from_template(template).is_err(),
            "variant {variant}"
        );
    }
    let mut template = source;
    for part in template
        .sections
        .get_mut(&LeftArm)
        .unwrap()
        .criticals
        .values_mut()
    {
        if part.equipment == "IS.HeavyGaussRifle" {
            part.equipment = "IS.GaussRifle".into();
        }
    }
    assert!(BattleUnit::from_template(template).is_err());
}

/// An extension critical detonates a Gauss weapon in its primary section, where CASE applies.
#[tokio::test]
async fn split_gauss_explosion_origin_case_and_restart() {
    use BattleSection::*;
    let (_dir, config, mut base) = support::isolated_world().await;
    let id = base.create(&config, "Split Gauss".into(), Kind::Thing);
    base.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    let mut template = definition(BattleWeapon::HeavyGaussRifle, LeftArm, LeftTorso);
    template
        .sections
        .get_mut(&LeftArm)
        .unwrap()
        .criticals
        .get_mut(&3)
        .unwrap()
        .equipment = "CASE".into();
    create_battle_unit(&mut base, id, template).unwrap();
    let hit = BattleHit {
        section: LeftTorso,
        rear_armor: false,
        through_armor_critical: true,
        crew_stun: false,
    };
    for seed in 0..=255 {
        let mut state = serde_json::to_value(&base.btech).unwrap();
        state["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        base.btech = serde_json::from_value(state).unwrap();
        let mut fired = base.clone();
        let report = resolve_battle_impact(&mut fired, id, hit, 1).unwrap();
        if !report.criticals.iter().any(|(slot, loss)| {
            slot.section == LeftTorso
                && matches!(
                    loss,
                    BattleCriticalLoss::Weapon {
                        explosion_damage: 25,
                        ..
                    }
                )
        }) {
            continue;
        }
        let unit = &fired.btech.constructed_units()[&id];
        assert_eq!(unit.sections()[&LeftArm].internal, 0);
        assert_eq!(unit.sections()[&LeftTorso].internal, 8);
        assert_eq!(
            unit.sections()[&CenterTorso],
            base.btech.constructed_units()[&id].sections()[&CenterTorso]
        );
        assert_eq!(
            report
                .pending_effects
                .iter()
                .filter(|effect| **effect == BattleImpactEffect::ExplosionInjury)
                .count(),
            1
        );
        assert!(!unit.is_destroyed());
        persistence::save(&config.database(), &base).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            resolve_battle_impact(&mut restored, id, hit, 1).unwrap(),
            report
        );
        assert_eq!(restored.btech, fired.btech);
        restored.validate(&config).unwrap();
        return;
    }
    panic!("No seed selected the split extension");
}

/// Losing either section disables the mount while surviving weapon slots retain their physical mass.
#[tokio::test]
async fn split_section_loss_preserves_remaining_slot_mass() {
    use BattleSection::*;
    for (parent, extension, lost_mass) in [(LeftArm, LeftTorso, 13512), (LeftTorso, LeftArm, 4914)]
    {
        let (_dir, config, mut world) = support::isolated_world().await;
        let id = world.create(&config, "Split mass".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(
            &mut world,
            id,
            definition(BattleWeapon::Ac20, parent, extension),
        )
        .unwrap();
        let before = &world.btech.constructed_units()[&id];
        let mass = before.mass().unwrap().equipment;
        let index = before
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == BattleWeapon::Ac20)
            .unwrap();
        apply_damage_phase(&mut world, id, LeftArm, 100, BattleDamagePhase::Internal).unwrap();
        let after = &world.btech.constructed_units()[&id];
        assert!(!after.weapon_intact(index).unwrap());
        assert_eq!(mass - after.mass().unwrap().equipment, lost_mass);
        world.validate(&config).unwrap();
    }
}

/// A fifteen-slot artillery mount shares construction and critical loss across its linked sections.
#[test]
fn arrow_mount_can_end_before_the_primary_section_boundary() {
    let template =
        BattleTemplate::parse("CPLT-C5", include_str!("../game/mechs/CPLT-C5.toml")).unwrap();
    let unit = BattleUnit::from_template(template.clone()).unwrap();
    let loadout = unit.loadout().unwrap();
    let index = loadout
        .weapons
        .iter()
        .position(|mount| mount.weapon == BattleWeapon::ArrowIv)
        .unwrap();
    let mount = &loadout.weapons[index];
    assert_eq!(mount.criticals.len(), 15);
    assert_eq!(
        mount
            .criticals
            .iter()
            .filter(|slot| slot.section == BattleSection::RightArm)
            .count(),
        9
    );
    assert_eq!(
        mount
            .criticals
            .iter()
            .filter(|slot| slot.section == BattleSection::RightTorso)
            .count(),
        6
    );
    assert_eq!(
        loadout
            .weapons
            .iter()
            .filter(|mount| mount.weapon == BattleWeapon::ArrowIv)
            .count(),
        1
    );
    for &slot in &mount.criticals {
        let mut damaged = unit.clone();
        damaged.destroy_critical(slot).unwrap();
        assert!(!damaged.weapon_intact(index).unwrap());
        let restored: BattleUnit =
            serde_json::from_value(serde_json::to_value(&damaged).unwrap()).unwrap();
        assert_eq!(restored, damaged);
    }
    for slot in [6, 11] {
        let mut incomplete = template.clone();
        incomplete
            .sections
            .get_mut(&BattleSection::RightTorso)
            .unwrap()
            .criticals
            .remove(&slot);
        assert!(BattleUnit::from_template(incomplete).is_err());
    }
}

/// Damage in either half contributes to one launcher and excludes exactly the damaged slots.
#[test]
fn split_weapon_degradation_shares_one_effect_set() {
    use BattleSection::*;
    for (parent, extension) in [
        (LeftArm, LeftTorso),
        (RightArm, RightTorso),
        (LeftTorso, LeftLeg),
        (RightTorso, CenterTorso),
    ] {
        let base =
            BattleUnit::from_template(definition(BattleWeapon::Ac20, parent, extension)).unwrap();
        let loadout = base.loadout().unwrap();
        let index = loadout
            .weapons
            .iter()
            .position(|mount| mount.weapon == BattleWeapon::Ac20)
            .unwrap();
        let mount = &loadout.weapons[index];
        let first = mount.criticals[0];
        let second = *mount
            .criticals
            .iter()
            .find(|location| location.section == extension)
            .unwrap();
        let mut state = serde_json::to_value(&base).unwrap();
        state["weapon_damage"] = serde_json::json!([
            BattleWeaponDamage::new(first, BattleWeaponDamageKind::Barrel),
            BattleWeaponDamage::new(second, BattleWeaponDamageKind::Feed),
        ]);
        let damaged: BattleUnit = serde_json::from_value(state).unwrap();
        assert!(damaged.weapon_intact(index).unwrap());
        assert_eq!(damaged.mass().unwrap(), base.mass().unwrap());
        let effects = damaged.weapon_damage_effects(index).unwrap();
        assert_eq!(effects.jam, 1);
        assert_eq!(effects.explosion, 1);
        assert!(effects.feed_locked);
        for location in [first, second] {
            assert!(
                !damaged
                    .critical_candidates(location.section)
                    .contains(&location)
            );
        }
        let restored: BattleUnit =
            serde_json::from_str(&serde_json::to_string(&damaged).unwrap()).unwrap();
        assert_eq!(restored.weapon_damage_effects(index).unwrap(), effects);
    }
}

/// Select one requested critical and, while the mount works, its exact enhanced table roll.
fn critical_stream(
    unit: &BattleUnit,
    location: CriticalLocation,
    weapon_roll: Option<u8>,
) -> BattleDice {
    let candidates = unit.critical_candidates(location.section);
    let selected = candidates
        .iter()
        .position(|candidate| *candidate == location)
        .unwrap() as u16
        + 1;
    (0u32..100_000)
        .find_map(|value| {
            let mut bytes = [0; 32];
            bytes[..4].copy_from_slice(&value.to_le_bytes());
            let dice = BattleDice::seeded(bytes);
            let mut trial = dice.clone();
            trial.two_d6(); // Material entry precedes critical selection.
            (matches!(trial.two_d6(), 8 | 9)
                && trial.die(candidates.len() as u16).unwrap() == selected
                && weapon_roll.is_none_or(|roll| trial.two_d6() == roll))
            .then_some(dice)
        })
        .expect("seed for the requested critical sequence")
}

/// Repeated proxy hits accumulate flags on one parent, then destroy damaged slots and an additional slot.
#[tokio::test]
async fn repeated_split_proxy_criticals_accumulate_once_and_replay() {
    for weapon in [
        BattleWeapon::Ac20,
        BattleWeapon::Lbx20,
        BattleWeapon::UltraAc20,
        BattleWeapon::ClanUltraAc20,
    ] {
        for (parent, extension) in [
            (BattleSection::LeftArm, BattleSection::LeftTorso),
            (BattleSection::RightTorso, BattleSection::RightArm),
        ] {
            let (_dir, config, mut world) = support::isolated_world().await;
            let id = world.create(&config, "Split damage".into(), Kind::Thing);
            world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
            create_battle_unit(&mut world, id, definition(weapon, parent, extension)).unwrap();
            let loadout = world.btech.constructed_units()[&id].loadout().unwrap();
            let index = loadout
                .weapons
                .iter()
                .position(|mount| mount.weapon == weapon)
                .unwrap();
            let mount = &loadout.weapons[index];
            let primary = mount.criticals[0];
            let proxy = *mount
                .criticals
                .iter()
                .find(|location| location.section == extension)
                .unwrap();
            let hit = BattleHit {
                section: extension,
                rear_armor: false,
                through_armor_critical: true,
                crew_stun: false,
            };
            for (step, roll) in [Some(5), Some(6), Some(2), Some(2), Some(12), None]
                .into_iter()
                .enumerate()
            {
                let unit = &world.btech.constructed_units()[&id];
                let candidates = unit.critical_candidates(extension).len() as u16;
                let dice = critical_stream(unit, proxy, roll);
                let mut expected = dice.clone();
                expected.two_d6(); // Material entry.
                expected.two_d6();
                expected.die(candidates).unwrap();
                if roll.is_some() {
                    expected.two_d6();
                }
                // Penetrating damage consumes its own roll even when the TAC supplied a critical.
                if unit.sections()[&extension].armor == 0 {
                    expected.two_d6();
                }
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["constructed"][id.0.to_string()]["dice"] =
                    serde_json::to_value(dice).unwrap();
                world.btech = serde_json::from_value(state).unwrap();
                persistence::save(&config.database(), &world).await.unwrap();
                let mut replay = persistence::load(&config.database()).await.unwrap();
                let report = resolve_battle_impact(&mut world, id, hit, 1).unwrap();
                assert_eq!(
                    resolve_battle_impact(&mut replay, id, hit, 1).unwrap(),
                    report
                );
                assert_eq!(world.btech, replay.btech);
                assert_eq!(report.criticals.len(), 1);
                assert_eq!(report.criticals[0].0, proxy);
                let unit = &world.btech.constructed_units()[&id];
                if step < 4 {
                    assert_eq!(unit.weapon_damage().len(), 1);
                    assert_eq!(unit.weapon_damage()[0].location, primary);
                    assert_eq!(unit.weapon_damage()[0].effects.len(), (step + 1).min(3));
                    let effects = unit.weapon_damage_effects(index).unwrap();
                    assert_eq!(effects.jam, 1);
                    assert_eq!(effects.damage, 0);
                    assert_eq!(effects.heat, 0);
                    assert_eq!(effects.feed_locked, step > 0);
                    assert_eq!(effects.explosion, u8::from(step > 0));
                    assert_eq!(effects.moderate, u8::from(step > 1));
                } else {
                    assert!(unit.weapon_damage().is_empty());
                    assert_eq!(
                        unit.weapon_damage_effects(index).unwrap(),
                        BattleWeaponDamageEffects::default()
                    );
                }
                let criticals = battle_critical_report(&world, id, primary.section.name()).unwrap();
                assert_eq!(
                    criticals.slots[usize::from(primary.slot)].condition,
                    if step < 4 {
                        BattleEquipmentCondition::Damaged
                    } else {
                        BattleEquipmentCondition::Destroyed
                    }
                );
                let extensions = battle_critical_report(&world, id, proxy.section.name()).unwrap();
                let proxy_row = &extensions.slots[usize::from(proxy.slot)];
                assert_eq!(proxy_row.weapon_index, Some(index));
                assert!(!proxy_row.equipment.contains("SplitCrit"));
                assert_eq!(proxy_row.condition, BattleEquipmentCondition::Operational);
                let diagnostic = &battle_weapon_diagnostics(&world, id).unwrap()[index];
                assert_eq!(diagnostic.damaged_slots, u8::from(step < 4));
                assert_eq!(
                    diagnostic.destroyed_slots,
                    if step < 4 {
                        0
                    } else if step == 4 {
                        2
                    } else {
                        3
                    }
                );
                assert_eq!(
                    diagnostic.effects,
                    unit.weapon_damage_effects(index).unwrap()
                );
                assert_eq!(unit.weapon_intact(index).unwrap(), step < 4);
                assert_eq!(unit.critical_destroyed(primary), step >= 4);
                assert_eq!(unit.critical_destroyed(mount.criticals[1]), step >= 4);
                assert_eq!(unit.critical_destroyed(proxy), step >= 5);
                assert_eq!(
                    roll_unit_dice(&mut world, id, 2).unwrap(),
                    vec![expected.d6(), expected.d6()],
                    "{parent:?}->{extension:?} step {step}: {report:?}"
                );
                world.validate(&config).unwrap();
            }
        }
    }
}

/// A centre torso weapon continuing into a side torso loads from TOML and survives a save.
#[test]
fn center_torso_split_mounts_load_and_round_trip_from_documents() {
    use BattleSection::*;
    let source = include_str!("fixtures/btech/mechs/JR7-D.toml")
        .replace(
            "    { at = 11, item = \"IS.SRM-4\" },\n    { at = 12, item = \"JumpJet\" },\n",
            "",
        )
        .replace("    { at = \"1-2\", item = \"JumpJet\" },\n", "")
        + "\n[[split_mounts]]\nitem = \"IS.AC/20\"\nplacements = [\n    { section = \"center_torso\", at = \"11-12\" },\n    { section = \"left_torso\", at = \"1-8\" },\n]\n";
    let template = BattleTemplate::parse("JR7-D", &source).unwrap();
    let loadout = BattleLoadout::resolve(&template).unwrap();
    let mount = loadout
        .weapons
        .iter()
        .find(|mount| mount.weapon == BattleWeapon::Ac20)
        .unwrap();
    assert_eq!(mount.criticals.len(), 10);
    assert_eq!(mount.criticals[0].section, CenterTorso);
    assert_eq!(mount.criticals.last().unwrap().section, LeftTorso);

    let document = template.to_document().unwrap();
    assert!(document.contains("{ section = \"center_torso\", at = \"11-12\" }"));
    assert_eq!(BattleTemplate::parse("JR7-D", &document).unwrap(), template);

    let reversed = source.replace(
        "{ section = \"center_torso\", at = \"11-12\" },\n    { section = \"left_torso\", at = \"1-8\" },",
        "{ section = \"left_torso\", at = \"1-8\" },\n    { section = \"center_torso\", at = \"11-12\" },",
    );
    assert_ne!(reversed, source);
    let loadout =
        BattleLoadout::resolve(&BattleTemplate::parse("JR7-D", &reversed).unwrap()).unwrap();
    let mount = loadout
        .weapons
        .iter()
        .find(|mount| mount.weapon == BattleWeapon::Ac20)
        .unwrap();
    assert_eq!(mount.criticals[0].section, LeftTorso);
    assert_eq!(mount.criticals.last().unwrap().section, CenterTorso);

    let distant = source.replace(
        "\"left_torso\", at = \"1-8\"",
        "\"left_arm\", at = \"5-12\"",
    );
    assert!(BattleTemplate::parse("JR7-D", &distant).is_err());
}
