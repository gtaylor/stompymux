//! Fusion-engine slot layouts, derived mass and damage availability. Engines fill only the
//! TechManual layouts (TechManual p. 50; Tactical Operations: Advanced Units & Equipment for
//! XXL engines).
use stompymux_rs::*;

/// The engine construction flag each family declares.
fn flag(engine: Engine) -> &'static str {
    match engine {
        Engine::Standard => "",
        Engine::Xl => "XLEngine_Tech",
        Engine::Light => "LightEngine_Tech",
        Engine::Xxl => "XXL_Tech",
        Engine::Compact => "CompactEngine_Tech",
    }
}

/// A complete biped with its engine slots laid out among its three torso sections and the
/// engine family `engine` declared.
fn definition(center: u8, left: u8, right: u8, engine: Engine) -> MechTemplate {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    template.max_speed = 96.75;
    template
        .attributes
        .insert("specials".into(), format!("FlipArms {}", flag(engine)));
    let engine = template.sections[&MechSection::CenterTorso].criticals[&0].clone();
    for (section, count) in [
        (MechSection::CenterTorso, center),
        (MechSection::LeftTorso, left),
        (MechSection::RightTorso, right),
    ] {
        let layout = template.sections.get_mut(&section).unwrap();
        layout
            .criticals
            .retain(|_, part| part.equipment != "Engine");
        if section == MechSection::CenterTorso {
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
    crate::support::templates::legal(template)
}

/// Each Inner Sphere family fills its own layout and weighs its share of the standard engine.
#[test]
fn engine_families_mass_and_invalid_layouts() {
    for (center, left, right, engine, mass) in [
        (6, 0, 0, Engine::Standard, 9216),
        (3, 0, 0, Engine::Compact, 13824),
        (6, 3, 3, Engine::Xl, 4608),
        (6, 2, 2, Engine::Light, 7168),
        (6, 6, 6, Engine::Xxl, 3072),
    ] {
        let unit = Mech::from_template(definition(center, left, right, engine)).unwrap();
        assert_eq!(unit.engine().unwrap(), engine);
        assert_eq!(unit.mass().unwrap().engine, mass);
        let loadout = unit.loadout().unwrap();
        let slots: Vec<_> = loadout
            .systems
            .iter()
            .filter(|part| part.system == System::Engine)
            .map(|part| part.location)
            .collect();
        assert_eq!(slots.len(), usize::from(center + left + right));
        let mut damaged = unit.clone();
        for (index, location) in slots.iter().take(3).enumerate() {
            damaged.destroy_critical(*location).unwrap();
            assert_eq!(damaged.system_hits(System::Engine), index as u8 + 1);
            assert_eq!(damaged.engine().unwrap(), engine);
            assert_eq!(damaged.is_destroyed(), index == 2);
        }
    }
    // Relocated slots, partial installations and a family the slots do not match are refused.
    for (center, left, right, engine) in [
        (6, 5, 1, Engine::Xl),
        (6, 4, 2, Engine::Xxl),
        (8, 2, 0, Engine::Light),
        (7, 2, 1, Engine::Xl),
        (8, 5, 5, Engine::Xxl),
        (7, 2, 0, Engine::Light),
        (6, 2, 2, Engine::Xl),
        (6, 3, 3, Engine::Standard),
    ] {
        assert!(
            Mech::from_template(definition(center, left, right, engine)).is_err(),
            "{center}/{left}/{right} {engine:?}"
        );
    }
}

/// The Cestus CES-4S record sheet fields a light engine beside its heavy Gauss rifle.
#[test]
fn cestus_constructs_with_its_published_engine() {
    let template =
        MechTemplate::parse("CES-4S", include_str!("../game/units/CES-4S.toml")).unwrap();
    let unit = Mech::from_template(template).unwrap();
    assert_eq!(unit.engine().unwrap(), Engine::Light);
    let loadout = unit.loadout().unwrap();
    assert!(
        loadout
            .weapons
            .iter()
            .any(|mount| mount.weapon == Weapon::HeavyGaussRifle)
    );
    assert_eq!(
        loadout
            .systems
            .iter()
            .filter(|part| part.system == System::Engine)
            .count(),
        10
    );
}

/// Clan side-torso slots identify XL/XXL masses and preserve their critical vulnerability.
#[test]
fn clan_engine_layouts_mass_and_side_loss() {
    for (center, left, right, engine, mass) in [
        (6, 0, 0, Engine::Standard, 9216),
        (6, 2, 2, Engine::Xl, 4608),
        (6, 4, 4, Engine::Xxl, 3072),
    ] {
        let mut template = definition(center, left, right, engine);
        template
            .attributes
            .insert("specials".into(), format!("Clan FlipArms {}", flag(engine)));
        template.heat_sinks = 20;
        for section in template.sections.values_mut() {
            section
                .criticals
                .retain(|_, part| part.equipment != "HeatSink");
        }
        let mut unit = Mech::from_template(crate::support::templates::legal(template)).unwrap();
        assert_eq!(unit.engine().unwrap(), engine);
        assert_eq!(unit.mass().unwrap().engine, mass);
        if left > 0 {
            unit.damage_phase(MechSection::LeftTorso, 100, DamagePhase::Internal);
            assert_eq!(unit.system_hits(System::Engine), left);
            assert_eq!(unit.is_destroyed(), left >= 3);
            assert_eq!(unit.engine().unwrap(), engine);
        }
    }
    // Clan engines have no compact or light family and no relocated layouts.
    for (center, left, right, engine) in [
        (3, 0, 0, Engine::Compact),
        (8, 2, 0, Engine::Xl),
        (7, 1, 2, Engine::Xl),
        (6, 3, 3, Engine::Xl),
    ] {
        let mut template = definition(center, left, right, engine);
        template
            .attributes
            .insert("specials".into(), format!("Clan FlipArms {}", flag(engine)));
        template.heat_sinks = 20;
        for section in template.sections.values_mut() {
            section
                .criticals
                .retain(|_, part| part.equipment != "HeatSink");
        }
        assert!(
            Mech::from_template(crate::support::templates::legal(template)).is_err(),
            "{center}/{left}/{right}"
        );
    }
}

/// Engine shielding changes internal defensive value independently of installed weapon offense.
#[test]
fn battle_value_engine_factors() {
    for (center, left, right, defensive) in [
        (6, 0, 0, 324.35),
        (8, 2, 0, 296.075),
        (8, 3, 1, 267.8),
        (7, 2, 1, 267.8),
        (7, 3, 2, 267.8),
        // An Inner Sphere XXL engine keeps a quarter of the structure value (TO:AUE p. 193).
        (7, 5, 6, 239.525),
    ] {
        let unit = Mech::from_template(definition(center, left, right)).unwrap();
        let value = unit.battle_value().unwrap();
        // Four medium lasers and 35 tons at 9 running and 5 jumping MP (Speed Factor 1.89).
        assert!((value.offensive - 413.91).abs() < 0.0001, "{value:?}");
        assert!((value.defensive - defensive).abs() < 0.0001, "{value:?}");
    }
}

/// The Barghest quads construct with their published XL engines and lose them to three hits.
#[test]
fn quad_engines_use_their_family_and_live_damage() {
    for source in [
        include_str!("../game/units/BGS-1T.toml"),
        include_str!("../game/units/BGS-2T.toml"),
    ] {
        let template = MechTemplate::parse("test", source).unwrap();
        assert!(check_battle_template(&template).constructible);
        let mut unit = Mech::from_template(template).unwrap();
        assert_eq!(unit.engine().unwrap(), Engine::Xl);
        let original_mass = unit.mass().unwrap();
        assert!(original_mass.engine > 0);
        let original_bv = unit.battle_value().unwrap();
        let restored: Mech = serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored.mass().unwrap(), original_mass);
        assert_eq!(restored.battle_value().unwrap(), original_bv);
        let slots: Vec<_> = unit
            .loadout()
            .unwrap()
            .systems
            .iter()
            .filter(|p| p.system == System::Engine)
            .map(|p| p.location)
            .collect();
        assert_eq!(slots.len(), 12);
        for (i, slot) in slots.into_iter().take(3).enumerate() {
            unit.destroy_critical(slot).unwrap();
            assert_eq!(unit.system_hits(System::Engine), i as u8 + 1);
            assert_eq!(unit.engine().unwrap(), Engine::Xl);
        }
        assert!(unit.is_destroyed());
    }
    // The Golem GOL-3S splits its LB 20-X between torsos, never into a leg (TechManual
    // p. 47), and an engine slot outside the torsos is refused.
    let mut template =
        MechTemplate::parse("GOL-3S", include_str!("../game/units/GOL-3S.toml")).unwrap();
    let unit = Mech::from_template(template.clone()).unwrap();
    assert_eq!(unit.engine().unwrap(), Engine::Light);
    let loadout = unit.loadout().unwrap();
    let cannon = loadout
        .weapons
        .iter()
        .find(|mount| mount.weapon == Weapon::Lbx20)
        .unwrap();
    assert_eq!(cannon.criticals.len(), 11);
    assert!(cannon.criticals.iter().all(|location| !matches!(
        location.section,
        MechSection::LeftLeg | MechSection::RightLeg
    )));
    let engine = template.sections[&MechSection::CenterTorso].criticals[&0].clone();
    template
        .sections
        .get_mut(&MechSection::LeftLeg)
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

/// Record-sheet light and XL engines keep their authored equipment and saved damage.
#[test]
fn published_engines_construct_and_preserve_saved_damage() {
    for (source, engine) in [
        (include_str!("../game/units/AXM-3S.toml"), Engine::Light),
        (include_str!("../game/units/HBK-5S.toml"), Engine::Light),
        (include_str!("../game/units/MAD-4S.toml"), Engine::Light),
        (include_str!("../game/units/STK-8S.toml"), Engine::Light),
        (include_str!("../game/units/BTZ-3F.toml"), Engine::Xl),
    ] {
        let definition = MechTemplate::parse("test", source).unwrap();
        assert!(check_battle_template(&definition).constructible);
        let mut unit = Mech::from_template(definition.clone()).unwrap();
        assert_eq!(unit.definition(), &definition);
        assert_eq!(unit.engine().unwrap(), engine);
        let mass = unit.mass().unwrap();
        let slots: Vec<_> = unit
            .loadout()
            .unwrap()
            .systems
            .iter()
            .filter(|part| part.system == System::Engine)
            .map(|part| part.location)
            .collect();
        for (index, slot) in slots.into_iter().take(3).enumerate() {
            unit.destroy_critical(slot).unwrap();
            assert_eq!(unit.system_hits(System::Engine), index as u8 + 1);
            assert_eq!(unit.engine().unwrap(), engine);
            assert_eq!(unit.mass().unwrap().engine, mass.engine);
        }
        assert!(unit.is_destroyed());
        let restored: Mech = serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored, unit);
    }
}

use crate::support;

/// The cockpit names each engine family.
#[tokio::test]
async fn critical_report_engine_names() {
    for (center, left, right, engine, name) in [
        (6, 2, 2, Engine::Light, "Engine (Light)"),
        (6, 6, 6, Engine::Xxl, "Engine (XXL)"),
        (3, 0, 0, Engine::Compact, "Engine (Compact)"),
        (6, 3, 3, Engine::Xl, "Engine (XL)"),
        (6, 0, 0, Engine::Standard, "Engine"),
    ] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let id = world.create(&config, "Engine report".into(), Kind::Thing);
        create_battle_unit(&mut world, id, definition(center, left, right, engine)).unwrap();
        let report = battle_critical_report(&world, id, "ct").unwrap();
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
