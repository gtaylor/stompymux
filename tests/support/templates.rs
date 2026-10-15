//! Template document edits that scenarios build variants from.
use stompymux_rs::UnitTemplate;

/// Re-render a template document with extra chassis flags. Construction choices such as
/// `HardenedArmor_Tech` land in `[construction]` and features in `specials`, exactly as a
/// saved unit carrying those flags would be written.
pub fn with_flags(source: &str, flags: &[&str]) -> String {
    let add = |attributes: &mut std::collections::BTreeMap<String, String>| {
        let specials = attributes.entry("specials".into()).or_default();
        for flag in flags.iter().filter(|flag| !flag.is_empty()) {
            if !specials.is_empty() {
                specials.push(' ');
            }
            specials.push_str(flag);
        }
    };
    match UnitTemplate::parse("variant", source).unwrap() {
        UnitTemplate::Mech(mut template) => {
            add(&mut template.attributes);
            template.to_document().unwrap()
        }
        UnitTemplate::Vehicle(mut template) => {
            add(&mut template.attributes);
            template.to_document().unwrap()
        }
    }
}

/// Re-render a template document without the named chassis flags.
pub fn without_flags(source: &str, flags: &[&str]) -> String {
    let remove = |attributes: &mut std::collections::BTreeMap<String, String>| {
        if let Some(specials) = attributes.get_mut("specials") {
            *specials = specials
                .split_ascii_whitespace()
                .filter(|flag| {
                    !flags
                        .iter()
                        .any(|removed| removed.eq_ignore_ascii_case(flag))
                })
                .collect::<Vec<_>>()
                .join(" ");
        }
    };
    match UnitTemplate::parse("variant", source).unwrap() {
        UnitTemplate::Mech(mut template) => {
            remove(&mut template.attributes);
            template.to_document().unwrap()
        }
        UnitTemplate::Vehicle(mut template) => {
            remove(&mut template.attributes);
            template.to_document().unwrap()
        }
    }
}

/// Give a mech a small cockpit under `flag` (`SmallCockpit_Tech` or `SMCPIT`), with the head
/// laid out as one: life support, sensors, cockpit and sensors, then the head's other
/// equipment in its original order.
pub fn small_cockpit(template: &mut stompymux_rs::MechTemplate, flag: &str) {
    use stompymux_rs::{CriticalDefinition, MechSection};
    let specials = template.attributes.entry("specials".into()).or_default();
    if !specials.is_empty() {
        specials.push(' ');
    }
    specials.push_str(flag);
    let head = template.sections.get_mut(&MechSection::Head).unwrap();
    let fixed = ["LifeSupport", "Sensors", "Cockpit"];
    let others: Vec<_> = head
        .criticals
        .values()
        .filter(|critical| !fixed.contains(&critical.equipment.as_str()))
        .cloned()
        .collect();
    head.criticals.clear();
    for (slot, item) in ["LifeSupport", "Sensors", "Cockpit", "Sensors"]
        .into_iter()
        .enumerate()
    {
        let critical = CriticalDefinition {
            equipment: item.into(),
            data: "-".into(),
            modes: Vec::new(),
        };
        head.criticals.insert(slot as u8, critical);
    }
    for (offset, critical) in others.into_iter().enumerate() {
        head.criticals.insert(4 + offset as u8, critical);
    }
}

/// The Demolisher rebuilt as a legal hovercraft for chassis-coverage tests: hovercraft weigh
/// at most 50 tons (TechManual p. 96), so it keeps its armor and turret but carries one AC/20
/// with two tons of ammunition, which brings it to exactly 50 tons.
pub fn hovercraft(tracked: &str) -> String {
    let hover = tracked
        .replace("movement = \"track\"", "movement = \"hover\"")
        .replace("tons = 80", "tons = 50")
        .replace(
            "{ at = \"1-2\", item = \"IS.AC/20\" }",
            "{ at = 1, item = \"IS.AC/20\" }",
        )
        .replace(
            "{ at = \"3-6\", item = \"Ammo_IS.AC/20\", rounds = 5 }",
            "{ at = \"3-4\", item = \"Ammo_IS.AC/20\", rounds = 5 }",
        );
    assert!(
        hover.contains("tons = 50") && hover.contains("{ at = 1, item = \"IS.AC/20\" }"),
        "expected the Demolisher template"
    );
    hover
}

/// Keep a vehicle that a test arms with extra equipment constructible under the TechManual
/// rules, changing as little as it can. It adds a heat sink per point of energy weapon heat
/// (TechManual p. 104); while the vehicle is still illegal, usually over its tonnage
/// (TechManual p. 97), it trims up to two tons of armor, fits a standard fusion engine in
/// place of an ICE, trims armor down to four points a location, empties the turret, slows it
/// to 1 cruising MP and finally raises it to its motive type's maximum tonnage.
pub fn fit_vehicle(template: &mut stompymux_rs::VehicleTemplate) {
    use stompymux_rs::{VehicleLoadout, VehicleSection, VehicleTemplate};
    let legal = |template: &VehicleTemplate| {
        stompymux_rs::check_battle_vehicle_template(template).constructible
    };
    let cover = |template: &mut VehicleTemplate| {
        let Ok(loadout) = VehicleLoadout::resolve(template) else {
            return;
        };
        let heat = u16::try_from(VehicleTemplate::energy_heat(&loadout)).unwrap();
        if template.heat_sink_capacity() < heat {
            template.heat_sinks = Some(heat);
        }
    };
    // Remove armor a point at a time from the most heavily armored location other than the
    // front, which tests usually hit, leaving every location at least four points.
    let trim = |template: &mut VehicleTemplate, limit: u32| {
        for _ in 0..limit {
            if legal(template) {
                return;
            }
            let Some(section) = template
                .sections
                .iter_mut()
                .filter(|(section, layout)| **section != VehicleSection::Rotor && layout.armor > 4)
                .max_by_key(|(section, layout)| (**section != VehicleSection::Front, layout.armor))
                .map(|(_, layout)| layout)
            else {
                return;
            };
            section.armor -= 1;
        }
    };
    cover(template);
    trim(template, 32);
    if legal(template) {
        return;
    }
    let specials = template.attributes.entry("specials".into()).or_default();
    let kept: Vec<_> = specials
        .split_ascii_whitespace()
        .filter(|flag| !flag.eq_ignore_ascii_case("ICEEngine_Tech"))
        .map(str::to_owned)
        .collect();
    *specials = if kept.is_empty() {
        "-".into()
    } else {
        kept.join(" ")
    };
    cover(template);
    trim(template, u32::MAX);
    if legal(template) {
        return;
    }
    if let Some(turret) = template.sections.get_mut(&VehicleSection::Turret) {
        turret.criticals.clear();
    }
    if legal(template) {
        return;
    }
    template.max_speed = 2.0 * 10.75;
    if legal(template) {
        return;
    }
    if let Some(maximum) = template.movement.maximum_tonnage() {
        template.tons = template.tons.max(maximum);
    }
}

/// [`fit_vehicle`] applied to a template a test is about to construct.
pub fn fitted(mut template: stompymux_rs::VehicleTemplate) -> stompymux_rs::VehicleTemplate {
    fit_vehicle(&mut template);
    template
}

/// [`fit_vehicle`] applied to a vehicle among the templates a test is about to construct;
/// Mechs pass unchanged.
pub fn fitted_unit(template: stompymux_rs::UnitTemplate) -> stompymux_rs::UnitTemplate {
    match template {
        stompymux_rs::UnitTemplate::Vehicle(vehicle) => {
            stompymux_rs::UnitTemplate::Vehicle(fitted(vehicle))
        }
        mech => mech,
    }
}

/// The Demolisher with its `movement` replaced; a hovercraft becomes [`hovercraft`], since an
/// 80-ton hovercraft cannot be built (TechManual p. 96).
pub fn with_movement(tracked: &str, movement: &str) -> String {
    if movement == "hover" {
        return hovercraft(tracked);
    }
    tracked.replace(
        "movement = \"track\"",
        &format!("movement = \"{movement}\""),
    )
}

/// The Demolisher template for a [`stompymux_rs::VehicleMovement`], with the same hovercraft
/// substitution as [`with_movement`].
pub fn demolisher(movement: stompymux_rs::VehicleMovement) -> stompymux_rs::VehicleTemplate {
    let source = include_str!("../../game/units/Demolisher.toml");
    let source = if movement == stompymux_rs::VehicleMovement::Hover {
        hovercraft(source)
    } else {
        source.to_owned()
    };
    let mut template = stompymux_rs::VehicleTemplate::parse("Demolisher", &source).unwrap();
    template.movement = movement;
    template
}
