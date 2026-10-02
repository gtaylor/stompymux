//! Relocated fusion-engine slot layouts, derived mass and damage availability.
use stompymux_rs::*;

/// Preserve a complete biped while redistributing engine slots among its three torso sections.
fn definition(center: u8, left: u8, right: u8) -> BattleTemplate {
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    template.max_speed = 96.75;
    let engine = template.sections[&BattleSection::CenterTorso].criticals[&0].clone();
    for (section, count) in [
        (BattleSection::CenterTorso, center),
        (BattleSection::LeftTorso, left),
        (BattleSection::RightTorso, right),
    ] {
        let layout = template.sections.get_mut(&section).unwrap();
        layout
            .criticals
            .retain(|_, part| part.equipment != "Engine");
        if section == BattleSection::CenterTorso {
            layout.criticals.remove(&10);
            layout.criticals.remove(&11);
        }
        let slots: Vec<_> = (0..12)
            .filter(|slot| !layout.criticals.contains_key(slot))
            .take(usize::from(count))
            .collect();
        assert_eq!(slots.len(), usize::from(count));
        for slot in slots {
            layout.criticals.insert(slot, engine.clone());
        }
    }
    template
}

/// Full engine totals and consistent side-torso evidence are required after redistribution.
#[test]
fn relocated_engine_families_mass_and_invalid_layouts() {
    for (center, left, right, engine, mass) in [
        (6, 5, 1, BattleEngine::Xl, 4608),
        (6, 1, 5, BattleEngine::Xl, 4608),
        (6, 4, 2, BattleEngine::Xxl, 3072),
        (6, 2, 4, BattleEngine::Xxl, 3072),
        (8, 2, 0, BattleEngine::Light, 7168),
        (8, 0, 2, BattleEngine::Light, 7168),
        (7, 2, 1, BattleEngine::Xl, 4608),
        (7, 1, 2, BattleEngine::Xl, 4608),
        (8, 1, 1, BattleEngine::Xl, 4608),
        (7, 2, 3, BattleEngine::Xl, 4608),
        (7, 3, 2, BattleEngine::Xl, 4608),
        (8, 3, 1, BattleEngine::Xl, 4608),
        (8, 1, 3, BattleEngine::Xl, 4608),
        (7, 5, 6, BattleEngine::Xxl, 3072),
        (7, 6, 5, BattleEngine::Xxl, 3072),
        (8, 4, 6, BattleEngine::Xxl, 3072),
        (8, 6, 4, BattleEngine::Xxl, 3072),
        (8, 5, 5, BattleEngine::Xxl, 3072),
    ] {
        let unit = BattleUnit::from_template(definition(center, left, right)).unwrap();
        assert_eq!(unit.engine().unwrap(), engine);
        assert_eq!(unit.mass().unwrap().engine, mass);
        let loadout = unit.loadout().unwrap();
        let slots: Vec<_> = loadout
            .systems
            .iter()
            .filter(|part| part.system == BattleSystem::Engine)
            .map(|part| part.location)
            .collect();
        assert_eq!(slots.len(), usize::from(center + left + right));
        let mut damaged = unit.clone();
        for (index, location) in slots.iter().take(3).enumerate() {
            damaged.destroy_critical(*location).unwrap();
            assert_eq!(damaged.system_hits(BattleSystem::Engine), index as u8 + 1);
            assert_eq!(damaged.engine().unwrap(), engine);
            assert_eq!(damaged.is_destroyed(), index == 2);
        }
    }
    for (center, left, right) in [
        (7, 2, 0),
        (8, 1, 0),
        (8, 2, 2),
        (8, 3, 0),
        (6, 6, 5),
        (8, 4, 5),
    ] {
        assert!(
            BattleUnit::from_template(definition(center, left, right)).is_err(),
            "{center}/{left}/{right}"
        );
    }
}

/// Existing Heavy Gauss assets preserve their installed engine identity and weapon loadout.
#[test]
fn relocated_cestus_constructs_without_asset_changes() {
    let template =
        BattleTemplate::parse("CES-4S", include_str!("../game/mechs/CES-4S.toml")).unwrap();
    let unit = BattleUnit::from_template(template).unwrap();
    assert_eq!(unit.engine().unwrap(), BattleEngine::Light);
    let loadout = unit.loadout().unwrap();
    assert!(
        loadout
            .weapons
            .iter()
            .any(|mount| mount.weapon == BattleWeapon::HeavyGaussRifle)
    );
    assert_eq!(
        loadout
            .systems
            .iter()
            .filter(|part| part.system == BattleSystem::Engine)
            .count(),
        10
    );
}

/// Clan side-torso slots identify XL/XXL masses and preserve their critical vulnerability.
#[test]
fn clan_engine_layouts_mass_and_side_loss() {
    for (center, left, right, engine, mass) in [
        (6, 0, 0, BattleEngine::Standard, 9216),
        (3, 0, 0, BattleEngine::Compact, 13824),
        (6, 2, 2, BattleEngine::Xl, 4608),
        (8, 2, 0, BattleEngine::Xl, 4608),
        (7, 1, 2, BattleEngine::Xl, 4608),
        (6, 4, 4, BattleEngine::Xxl, 3072),
    ] {
        let mut template = definition(center, left, right);
        template
            .attributes
            .insert("specials".into(), "Clan FlipArms".into());
        template.heat_sinks = 20;
        for section in template.sections.values_mut() {
            section
                .criticals
                .retain(|_, part| part.equipment != "HeatSink");
        }
        let mut unit = BattleUnit::from_template(template).unwrap();
        assert_eq!(unit.engine().unwrap(), engine);
        assert_eq!(unit.mass().unwrap().engine, mass);
        if left > 0 {
            unit.damage_phase(BattleSection::LeftTorso, 100, BattleDamagePhase::Internal);
            assert_eq!(unit.system_hits(BattleSystem::Engine), left);
            assert_eq!(unit.is_destroyed(), left >= 3);
            assert_eq!(unit.engine().unwrap(), engine);
        }
    }
    for (center, left, right) in [(8, 2, 4), (7, 3, 4), (8, 3, 3)] {
        let mut template = definition(center, left, right);
        template.attributes.insert("specials".into(), "Clan".into());
        template.heat_sinks = 20;
        for section in template.sections.values_mut() {
            section
                .criticals
                .retain(|_, part| part.equipment != "HeatSink");
        }
        assert!(BattleUnit::from_template(template).is_err());
    }
}

/// Engine shielding changes internal defensive value independently of installed weapon offense.
#[test]
fn battle_value_engine_factors() {
    for (center, left, right, defensive) in [
        (6, 0, 0, 324.35),
        (8, 2, 0, 296.08),
        (8, 3, 1, 267.8),
        (7, 2, 1, 267.8),
        (7, 3, 2, 267.8),
        (7, 5, 6, 267.8),
    ] {
        let unit = BattleUnit::from_template(definition(center, left, right)).unwrap();
        let value = unit.battle_value(None).unwrap();
        assert_eq!(value.offensive, 196.0);
        assert!((value.defensive - defensive).abs() < 0.0001, "{value:?}");
    }
}

/// Asymmetric complete Barghest engines use characterized mass/BV precedence and retain slot damage.
#[test]
fn asymmetric_quad_engines_use_effective_family_and_live_damage() {
    for (source, expected) in [
        (include_str!("../game/mechs/BGS-1T.toml"), BattleEngine::Xl),
        (include_str!("../game/mechs/BGS-2T.toml"), BattleEngine::Xxl),
    ] {
        let template = BattleTemplate::parse("test", source).unwrap();
        assert!(check_battle_template(&template).constructible);
        let mut unit = BattleUnit::from_template(template).unwrap();
        assert_eq!(unit.engine().unwrap(), expected);
        let original_mass = unit.mass().unwrap();
        assert!(original_mass.engine > 0);
        let original_bv = unit.battle_value(None).unwrap();
        let restored: BattleUnit =
            serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored.mass().unwrap(), original_mass);
        assert_eq!(restored.battle_value(None).unwrap(), original_bv);
        let slots: Vec<_> = unit
            .loadout()
            .unwrap()
            .systems
            .iter()
            .filter(|p| p.system == BattleSystem::Engine)
            .map(|p| p.location)
            .collect();
        assert_eq!(slots.len(), 12);
        for (i, slot) in slots.into_iter().take(3).enumerate() {
            unit.destroy_critical(slot).unwrap();
            assert_eq!(unit.system_hits(BattleSystem::Engine), i as u8 + 1);
            assert_eq!(unit.engine().unwrap(), expected);
        }
        assert!(unit.is_destroyed());
    }
    let mut template =
        BattleTemplate::parse("GOL-3S", include_str!("../game/mechs/GOL-3S.toml")).unwrap();
    let unit = BattleUnit::from_template(template.clone()).unwrap();
    assert_eq!(unit.engine().unwrap(), BattleEngine::Light);
    let loadout = unit.loadout().unwrap();
    let cannon = loadout
        .weapons
        .iter()
        .find(|mount| mount.weapon == BattleWeapon::Lbx20)
        .unwrap();
    assert_eq!(cannon.criticals.len(), 11);
    assert_eq!(
        cannon.criticals.last().unwrap().section,
        BattleSection::LeftLeg
    );
    let engine = template.sections[&BattleSection::CenterTorso].criticals[&0].clone();
    template
        .sections
        .get_mut(&BattleSection::LeftLeg)
        .unwrap()
        .criticals
        .insert(5, engine);
    assert!(
        check_battle_template(&template)
            .rejection
            .unwrap()
            .contains("Engine critical outside torso: Left_Leg critical 6")
    );
}

/// Mixed side evidence retains authored equipment and uses XL mass and combat behavior.
#[test]
fn mixed_relocated_asset_engines_construct_and_preserve_saved_damage() {
    for source in [
        include_str!("../game/mechs/AXM-3S.toml"),
        include_str!("../game/mechs/HBK-5S.toml"),
        include_str!("../game/mechs/MAD-4S.toml"),
        include_str!("../game/mechs/STK-8S.toml"),
        include_str!("../game/mechs/BTZ-3F.toml"),
    ] {
        let definition = BattleTemplate::parse("test", source).unwrap();
        assert!(check_battle_template(&definition).constructible);
        let mut unit = BattleUnit::from_template(definition.clone()).unwrap();
        assert_eq!(unit.definition(), &definition);
        assert_eq!(unit.engine().unwrap(), BattleEngine::Xl);
        let mass = unit.mass().unwrap();
        let slots: Vec<_> = unit
            .loadout()
            .unwrap()
            .systems
            .iter()
            .filter(|part| part.system == BattleSystem::Engine)
            .map(|part| part.location)
            .collect();
        for (index, slot) in slots.into_iter().take(3).enumerate() {
            unit.destroy_critical(slot).unwrap();
            assert_eq!(unit.system_hits(BattleSystem::Engine), index as u8 + 1);
            assert_eq!(unit.engine().unwrap(), BattleEngine::Xl);
            assert_eq!(unit.mass().unwrap().engine, mass.engine);
        }
        assert!(unit.is_destroyed());
        let restored: BattleUnit =
            serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored, unit);
    }
}

use crate::support;

/// Cockpit naming preserves display precedence without changing the effective mass/combat family.
#[tokio::test]
async fn critical_report_mixed_engine_names() {
    for (center, left, right, name) in [
        (7, 2, 1, "Engine (Light)"),
        (6, 4, 2, "Engine (Light)"),
        (6, 5, 1, "Engine (XXL)"),
        (3, 0, 0, "Engine (Compact)"),
        (6, 3, 3, "Engine (XL)"),
        (6, 0, 0, "Engine"),
    ] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let id = world.create(&config, "Engine report".into(), Kind::Thing);
        create_battle_unit(&mut world, id, definition(center, left, right)).unwrap();
        let report = battle_critical_report(&world, id, "ct", true).unwrap();
        assert_eq!(
            report
                .slots
                .iter()
                .find(|slot| slot.equipment.starts_with("Engine"))
                .unwrap()
                .equipment,
            name
        );
    }
}
