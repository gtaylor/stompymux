//! Detached C-compatible defaults for registered units before a template is loaded.
use super::{CriticalDefinition, MechSection, MechTemplate, SectionDefinition};
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result};
use std::collections::BTreeMap;

fn critical(equipment: &str) -> CriticalDefinition {
    CriticalDefinition {
        equipment: equipment.into(),
        data: "-".into(),
        modes: Vec::new(),
    }
}

/// Build the pristine raw state established by C's MECH registration initializer.
///
/// This is a detached inspection value. It does not admit the unit to Rust simulation or
/// manufacture a persisted template merely because the special object was registered.
pub fn registered_unit_default_template(world: &World, id: ObjectId) -> Option<MechTemplate> {
    let object = world.objects.get(&id)?;
    if object.kind != Kind::Thing
        || object.flags.contains(Flag::Going)
        || world.btech.registrations().get(&id).map(String::as_str) != Some("MECH")
        || world.btech.constructed.contains_key(&id)
        || world.btech.vehicles.contains_key(&id)
    {
        return None;
    }
    let mut sections: BTreeMap<_, _> = MechSection::ALL
        .into_iter()
        .map(|section| (section, SectionDefinition::default()))
        .collect();
    let mut install = |section, values: &[(u8, &str)]| {
        let criticals = &mut sections.get_mut(&section).expect("all sections").criticals;
        for &(slot, equipment) in values {
            criticals.insert(slot, critical(equipment));
        }
    };
    install(
        MechSection::Head,
        &[
            (0, "LifeSupport"),
            (1, "Sensors"),
            (2, "Cockpit"),
            (4, "Sensors"),
            (5, "LifeSupport"),
        ],
    );
    install(
        MechSection::CenterTorso,
        &[
            (0, "Engine"),
            (1, "Engine"),
            (2, "Engine"),
            (3, "Gyro"),
            (4, "Gyro"),
            (5, "Gyro"),
            (6, "Gyro"),
            (7, "Engine"),
            (8, "Engine"),
            (9, "Engine"),
        ],
    );
    for section in [
        MechSection::LeftArm,
        MechSection::RightArm,
        MechSection::LeftLeg,
        MechSection::RightLeg,
    ] {
        install(
            section,
            &[
                (0, "ShoulderOrHip"),
                (1, "UpperActuator"),
                (2, "LowerActuator"),
                (3, "HandOrFootActuator"),
            ],
        );
    }
    Some(MechTemplate {
        name: String::new(),
        reference: String::new(),
        tons: 0,
        max_speed: 0.0,
        jump_speed: 0.0,
        heat_sinks: 0,
        sections,
        attributes: BTreeMap::from([
            ("type".into(), "Mech".into()),
            ("move_type".into(), "Biped".into()),
        ]),
    })
}

/// Materialize the C default raw Mech state for an administrative mutation.
pub fn ensure_registered_unit_runtime(world: &mut World, id: ObjectId) -> Result<()> {
    if world.btech.constructed_units().contains_key(&id) || world.btech.vehicles().contains_key(&id)
    {
        return Ok(());
    }
    let definition =
        registered_unit_default_template(world, id).context("Unit runtime state is unavailable")?;
    let unit = super::Mech::from_contract_template(definition)?;
    world.btech.units.insert(id, unit.identity());
    world.btech.constructed.insert(id, unit);
    Ok(())
}
