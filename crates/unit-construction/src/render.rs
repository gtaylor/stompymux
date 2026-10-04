//! Render constructed templates back into TOML template documents, folding administrative
//! edits into the saved identity.
use super::{
    BattleMechChassis, BattleSection, BattleTemplate, BattleVehicleTemplate,
    document::{RenderSection, render},
};
use anyhow::Result;
use std::collections::BTreeMap;

impl BattleTemplate {
    /// Render this mech as a TOML template document, folding in administrative edits.
    pub fn to_document(&self) -> Result<String> {
        unit_template_source(self)
    }
}

impl BattleVehicleTemplate {
    /// Render this vehicle as a TOML template document, folding in administrative edits.
    pub fn to_document(&self) -> Result<String> {
        vehicle_template_source(self)
    }
}

/// Render a constructed mech as a TOML template document.
fn unit_template_source(template: &BattleTemplate) -> Result<String> {
    let chassis = template.chassis()?;
    let default_movement = match chassis {
        BattleMechChassis::Biped => "Biped",
        BattleMechChassis::Quad => "Quad",
    };
    let attributes = saved_attributes(
        &template.attributes,
        "Mech",
        default_movement,
        template.tons,
    );
    let sections: Vec<_> = BattleSection::ALL
        .into_iter()
        .map(|section| RenderSection {
            heading: chassis.section_name(section).to_ascii_lowercase(),
            mech: Some(section),
            layout: &template.sections[&section],
        })
        .collect();
    render(&attributes, &sections)
}

/// Render a constructed vehicle as a TOML template document.
fn vehicle_template_source(template: &BattleVehicleTemplate) -> Result<String> {
    let (class, movement) = if template.is_vtol() {
        ("VTOL", "VTOL")
    } else {
        ("Vehicle", "Track")
    };
    let attributes = saved_attributes(&template.attributes, class, movement, template.tons);
    let sections: Vec<_> = template
        .sections
        .iter()
        .map(|(section, layout)| RenderSection {
            heading: section.name().to_ascii_lowercase(),
            mech: None,
            layout,
        })
        .collect();
    render(&attributes, &sections)
}

/// Fold administrative class, movement and tonnage edits over the authored identity.
fn saved_attributes(
    attributes: &BTreeMap<String, String>,
    class: &str,
    movement: &str,
    tons: u16,
) -> BTreeMap<String, String> {
    let mut saved = attributes.clone();
    for (field, administrative, default) in [
        ("type", "administrative_unit_type", class.to_owned()),
        (
            "move_type",
            "administrative_movement_type",
            movement.to_owned(),
        ),
        ("tons", "administrative_tonnage", tons.to_string()),
    ] {
        let value = saved
            .remove(administrative)
            .or_else(|| saved.get(field).cloned())
            .unwrap_or(default);
        saved.insert(field.into(), value);
    }
    saved
}
