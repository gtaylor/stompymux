//! Conventional current mass, equipment loss and fixed-point construction accounting.
use stompymux_rs::*;

/// A supported standard-fusion fixture, independent of map state.
fn unit(source: &str) -> Mech {
    Mech::from_template(MechTemplate::parse("test", source).unwrap()).unwrap()
}
const JENNER: &str = include_str!("fixtures/btech/units/JR7-D.toml");
const ATLAS: &str = include_str!("fixtures/btech/units/AS7-D.toml");

#[test]
fn intact_mass_uses_catalog_components_and_per_slot_weapon_rounding() {
    let jenner = unit(JENNER).mass().unwrap();
    assert_eq!(
        jenner,
        Mass {
            engine: 12 * 1024,
            cockpit: 3 * 1024,
            gyro: 3 * 1024,
            structure: 3584,
            armor: 4 * 1024,
            equipment: 8704,
            ammunition: 1024,
            cargo: 0,
            total: 35 * 1024
        }
    );
    let atlas = unit(ATLAS).mass().unwrap();
    assert_eq!(atlas.engine, 19 * 1024);
    assert_eq!(atlas.ammunition, 5 * 1024);
    assert_eq!(atlas.total, 100 * 1024 - 6); // AC/20 criticals each truncate 1/10 of fourteen tons.
}

#[test]
fn current_mass_tracks_armor_structure_ammunition_and_missing_sections() {
    let original = unit(JENNER);
    let base = original.mass().unwrap();
    let mut state = serde_json::to_value(&original).unwrap();
    state["ammunition"][0] = 24.into();
    let changed: Mech = serde_json::from_value(state.clone()).unwrap();
    assert_eq!(changed.mass().unwrap().ammunition, 983);
    state["sections"]["CenterTorso"]["armor"] = 2.into();
    let changed: Mech = serde_json::from_value(state.clone()).unwrap();
    assert_eq!(changed.mass().unwrap().armor, 3584);
    state["sections"]["LeftArm"]["internal"] = 0.into();
    state["sections"]["LeftArm"]["armor"] = 0.into();
    let changed: Mech = serde_json::from_value(state).unwrap();
    assert_eq!(changed.mass().unwrap().equipment, base.equipment - 2048);
    assert!(changed.mass().unwrap().total < base.total);
}

#[test]
fn destroyed_weapon_slots_retain_mass_but_missing_sections_do_not() {
    let original = unit(JENNER);
    let mut state = serde_json::to_value(&original).unwrap();
    state["lost_criticals"] = serde_json::json!([{"section":"LeftArm","slot":2}]);
    let damaged: Mech = serde_json::from_value(state).unwrap();
    assert_eq!(damaged.mass().unwrap(), original.mass().unwrap());
}

#[test]
fn sink_losses_and_destroyed_core_preserve_component_accounting() {
    let atlas = unit(ATLAS);
    let mut state = serde_json::to_value(&atlas).unwrap();
    state["lost_criticals"] = serde_json::json!([{"section":"LeftArm","slot":5}]);
    let damaged: Mech = serde_json::from_value(state).unwrap();
    assert_eq!(
        damaged.mass().unwrap().equipment,
        atlas.mass().unwrap().equipment - 1024
    );
    let mut state = serde_json::to_value(unit(JENNER)).unwrap();
    state["sections"]["CenterTorso"] = serde_json::json!({"armor":0,"internal":0,"rear":0});
    let core: Mech = serde_json::from_value(state).unwrap();
    let mass = core.mass().unwrap();
    assert_eq!(mass.engine, 0);
    assert_eq!(mass.gyro, -1024);
    assert_eq!(mass.cockpit, 3072);
    assert!(mass.total > 0);
}

/// Add distributed construction slots without replacing weapons or conventional systems.
fn with_material(name: &str, count: usize) -> Mech {
    let mut template = MechTemplate::parse("JR7-D", JENNER).unwrap();
    let mut part = template.sections[&MechSection::Head].criticals[&3].clone();
    part.equipment = name.into();
    let mut remaining = count;
    for (&section, layout) in &mut template.sections {
        let size = match section {
            MechSection::Head | MechSection::LeftLeg | MechSection::RightLeg => 6,
            _ => 12,
        };
        for slot in 0..size {
            if remaining == 0 {
                break;
            }
            if let std::collections::btree_map::Entry::Vacant(entry) = layout.criticals.entry(slot)
            {
                entry.insert(part.clone());
                remaining -= 1;
            }
        }
    }
    assert_eq!(remaining, 0);
    Mech::from_template(template).unwrap()
}

#[test]
fn structural_material_thresholds_mass_rounding_and_critical_exclusion() {
    let standard = unit(JENNER);
    for (name, slots, intact_armor, damaged_armor, structure) in [
        ("FerroFibrous", 14, 4096, 3584, 3584),
        ("LtFerroFibrous", 7, 4096, 3584, 3584),
        ("HvyFerroFibrous", 21, 3584, 3072, 3584),
        ("EndoSteel", 14, 4096, 4096, 2048),
    ] {
        let mut enhanced = with_material(name, slots);
        assert_eq!(enhanced.mass().unwrap().armor, intact_armor, "{name}");
        assert_eq!(enhanced.mass().unwrap().structure, structure, "{name}");
        assert_eq!(
            enhanced.mass().unwrap().equipment,
            standard.mass().unwrap().equipment
        );
        assert_eq!(
            with_material(name, slots - 1).mass().unwrap(),
            standard.mass().unwrap()
        );
        for &section in standard.sections().keys() {
            assert_eq!(
                enhanced.critical_candidates(section),
                standard.critical_candidates(section)
            );
        }
        enhanced.damage_phase(MechSection::Head, 7, DamagePhase::Armor { rear: false });
        assert_eq!(enhanced.mass().unwrap().armor, damaged_armor, "{name}");
        let before = enhanced.mass().unwrap();
        let material = enhanced
            .loadout()
            .unwrap()
            .systems
            .into_iter()
            .find(|part| {
                !standard.definition().sections[&part.location.section]
                    .criticals
                    .contains_key(&part.location.slot)
            })
            .unwrap();
        enhanced.destroy_critical(material.location).unwrap();
        assert_eq!(enhanced.mass().unwrap(), before, "{name}");
    }
    let mut endo = with_material("EndoSteel", 14);
    endo.damage_phase(MechSection::CenterTorso, 10, DamagePhase::Internal);
    assert_eq!(endo.mass().unwrap().structure, 1536);
}

#[test]
fn material_flags_alone_do_not_replace_required_slots() {
    let standard = unit(JENNER);
    let mut template = standard.definition().clone();
    template.attributes.insert(
        "specials".into(),
        "FlipArms FerroFibrous_Tech EndoSteel_Tech HvyFerroFibrous_Tech LtFerroFibrous_Tech".into(),
    );
    let declared = Mech::from_template(template).unwrap();
    assert_eq!(declared.mass().unwrap(), standard.mass().unwrap());
}

/// The Jenner with empty side torsos for engine slots, no jump jets, and a `[construction]`
/// table.
fn jenner_with(construction: &str, slots: &str) -> Mech {
    let source = JENNER
        .replace("jump_mp = 5\n", "")
        .replace("slots = [\n    { at = \"1-2\", item = \"JumpJet\" },\n]\n", "")
        .replace(
            "    { at = 1, item = \"Ammo_IS.SRM-4\", rounds = 25 },\n    { at = \"2-3\", item = \"JumpJet\" },\n",
            "    { at = 12, item = \"Ammo_IS.SRM-4\", rounds = 25 },\n",
        )
        .replace("    { at = 11, item = \"IS.SRM-4\" },\n    { at = 12, item = \"JumpJet\" },\n", slots);
    unit(&format!("{source}\n[construction]\n{construction}\n"))
}

/// The Jenner's own center torso launcher.
const SRM: &str = "    { at = 11, item = \"IS.SRM-4\" },\n";

/// An engine's technology base, not the chassis's, sets its side slots and mass: a Clan XL on
/// an Inner Sphere chassis weighs half a standard engine, where a light engine with the same
/// side slots weighs three quarters.
#[test]
fn mixed_technology_engines_use_their_own_base() {
    let standard = jenner_with("", SRM).mass().unwrap().engine;
    assert_eq!(standard, 12 * 1024);
    for (construction, engine, mass) in [
        ("engine = \"xl\"", Engine::Xl, 6 * 1024),
        (
            "engine = \"xl\"\nengine_tech = \"clan\"",
            Engine::Xl,
            6 * 1024,
        ),
        ("engine = \"light\"", Engine::Light, 9 * 1024),
    ] {
        let unit = jenner_with(construction, SRM);
        assert_eq!(unit.engine().unwrap(), engine, "{construction}");
        assert_eq!(unit.mass().unwrap().engine, mass, "{construction}");
    }
    let clan = jenner_with("engine = \"xl\"\nengine_tech = \"clan\"", SRM);
    let side = clan.definition().sections[&MechSection::LeftTorso]
        .criticals
        .values()
        .filter(|critical| critical.equipment == "Engine")
        .count();
    assert_eq!(side, 2);
}

/// A combustion engine weighs twice a fusion engine, carries no free heat sinks and needs power
/// amplifiers for its energy weapons.
#[test]
fn combustion_engines_weigh_double_without_free_sinks() {
    let fusion = jenner_with("", SRM).mass().unwrap();
    let ice = jenner_with("engine = \"ice\"", SRM).mass().unwrap();
    assert_eq!(ice.engine, 2 * fusion.engine);
    // Ten heat sinks at a ton each, and half a ton of amplifiers for four medium lasers.
    assert_eq!(ice.equipment, fusion.equipment + 10 * 1024 + 512);
}

/// A one-shot launcher weighs half a ton more than the launcher and carries no bin.
#[test]
fn one_shot_launchers_add_half_a_ton() {
    let fed = jenner_with("", SRM).mass().unwrap();
    let one_shot = jenner_with(
        "",
        "    { at = 11, item = \"IS.SRM-4\", modes = [\"OneShot\"] },\n",
    )
    .mass()
    .unwrap();
    assert_eq!(one_shot.equipment, fed.equipment + 512);
}
