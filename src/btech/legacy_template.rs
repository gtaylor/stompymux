//! One-time conversion of brace-delimited unit templates into TOML documents.
use super::template_document::{
    RenderSection, is_split_proxy, render, split_link_data,
};
use super::{
    BattleSection, BattleTemplate, BattleUnitTemplate, BattleVehicleTemplate, RawMovement,
    RawSectionCode, RawTemplate, RawUnitClass, SectionDefinition, template::LegacyTemplate,
};
use anyhow::{Context, Result, bail, ensure};
use std::collections::BTreeMap;

/// Convert one brace-delimited template into an equivalent TOML document.
pub fn convert_legacy_template(source: &str) -> Result<String> {
    let mut parsed = LegacyTemplate::parse(source)?;
    let class = RawUnitClass::parse(&parsed.required("type")?)?;
    let movement = RawMovement::parse(&parsed.required("move_type")?)?;
    link_legacy_splits(&mut parsed.sections, class)?;
    let mut sections = Vec::new();
    let mut remaining = parsed.sections.keys().cloned().collect::<Vec<_>>();
    for code in RawSectionCode::for_unit(class, movement) {
        let heading = code.name().replace(' ', "_").to_ascii_lowercase();
        let Some(layout) = parsed.sections.get(&heading) else {
            continue;
        };
        remaining.retain(|name| *name != heading);
        let mech = mech_section(class, &heading)?;
        sections.push(RenderSection {
            heading,
            mech,
            layout,
        });
    }
    // Headings outside the class anatomy are kept so the document fails to load exactly as
    // its source did.
    for heading in remaining {
        let layout = &parsed.sections[&heading];
        sections.push(RenderSection {
            heading,
            mech: None,
            layout,
        });
    }
    let mut fields = parsed.fields.clone();
    fields.remove("comment");
    fields.remove("reference");
    render(&fields, &sections)
}

/// Check that the TOML document decodes to the same unit state as its source.
pub fn verify_legacy_conversion(reference: &str, legacy: &str, document: &str) -> Result<()> {
    match (
        RawTemplate::parse_legacy(legacy),
        RawTemplate::parse(reference, document),
    ) {
        (Err(_), Err(_)) => {}
        (Ok(mut old_raw), Ok(mut new_raw)) => {
            link_raw_splits(&mut old_raw)?;
            normalize_raw(&mut old_raw, reference);
            normalize_raw(&mut new_raw, reference);
            ensure!(
                old_raw == new_raw,
                "raw templates differ:\n{old_raw:#?}\n{new_raw:#?}"
            );
        }
        (Ok(_), Err(error)) => return Err(error.context("raw document no longer loads")),
        (Err(error), Ok(_)) => return Err(error.context("raw document loads but source did not")),
    }
    let mut old_parsed = LegacyTemplate::parse(legacy)?;
    if let Ok(class) = RawUnitClass::parse(&old_parsed.required("type")?) {
        link_legacy_splits(&mut old_parsed.sections, class)?;
    }
    let old_unit = BattleUnitTemplate::from_parsed(old_parsed);
    let new_unit = BattleUnitTemplate::parse(reference, document);
    match (old_unit, new_unit) {
        (Err(_), Err(_)) => Ok(()),
        (Ok(_), Err(error)) => Err(error.context("converted document no longer loads")),
        (Err(error), Ok(_)) => Err(error.context("converted document loads but source did not")),
        (Ok(BattleUnitTemplate::Mech(old)), Ok(BattleUnitTemplate::Mech(new))) => {
            let (old, new) = (normalize_mech(old, reference), normalize_mech(new, reference));
            ensure!(old == new, "mech templates differ:\n{old:#?}\n{new:#?}");
            Ok(())
        }
        (Ok(BattleUnitTemplate::Vehicle(old)), Ok(BattleUnitTemplate::Vehicle(new))) => {
            let (old, new) = (
                normalize_vehicle(old, reference),
                normalize_vehicle(new, reference),
            );
            ensure!(old == new, "vehicle templates differ:\n{old:#?}\n{new:#?}");
            Ok(())
        }
        _ => bail!("converted document changed unit class"),
    }
}

/// The stable mech section for a heading, when the class has mech anatomy.
fn mech_section(class: RawUnitClass, heading: &str) -> Result<Option<BattleSection>> {
    if !matches!(class, RawUnitClass::Mech | RawUnitClass::MechWarrior) {
        return Ok(None);
    }
    let section = super::BattleMechChassis::Biped
        .parse_section(heading)
        .or_else(|_| super::BattleMechChassis::Quad.parse_section(heading))?;
    Ok(Some(section))
}

/// The primary section a legacy marker implied from its side and its own section.
fn legacy_parent(name: &str, section: BattleSection) -> Result<BattleSection> {
    use BattleSection::*;
    let left = name.eq_ignore_ascii_case(super::template_document::SPLIT_LEFT);
    Ok(match (left, section) {
        (true, LeftArm | LeftLeg | CenterTorso) => LeftTorso,
        (false, RightArm | RightLeg | CenterTorso) => RightTorso,
        (true, LeftTorso) => LeftArm,
        (false, RightTorso) => RightArm,
        _ => bail!("Invalid split critical section"),
    })
}

/// Rewrite numeric legacy split markers as explicit primary section links.
fn link_legacy_splits(
    sections: &mut BTreeMap<String, SectionDefinition>,
    class: RawUnitClass,
) -> Result<()> {
    for (heading, layout) in sections.iter_mut() {
        for critical in layout.criticals.values_mut() {
            if !is_split_proxy(&critical.equipment) {
                continue;
            }
            let section = mech_section(class, heading)?.context("split outside a mech")?;
            let parent = legacy_parent(&critical.equipment, section)?;
            let slot = critical.data.parse().context("invalid legacy split slot")?;
            critical.data = split_link_data(parent, slot);
        }
    }
    Ok(())
}

/// Apply [`link_legacy_splits`] to raw section codes.
fn link_raw_splits(template: &mut RawTemplate) -> Result<()> {
    let mut sections: BTreeMap<String, SectionDefinition> = BTreeMap::new();
    for (code, layout) in &template.sections {
        sections.insert(code.name().replace(' ', "_"), layout.clone());
    }
    link_legacy_splits(&mut sections, template.class)?;
    for (code, layout) in template.sections.iter_mut() {
        *layout = sections.remove(&code.name().replace(' ', "_")).unwrap();
    }
    Ok(())
}

/// Spellings that legitimately change: identity, comments, class case and numeric formatting.
fn normalize_attributes(attributes: &mut BTreeMap<String, String>, reference: &str) {
    attributes.remove("comment");
    attributes.insert("reference".into(), reference.into());
    for (key, value) in attributes.iter_mut() {
        if matches!(key.as_str(), "type" | "move_type") {
            *value = value.to_ascii_lowercase();
        } else if let Ok(number) = value.parse::<f64>() {
            *value = number.to_string();
        }
    }
    if attributes.get("specials").is_some_and(|value| value == "-") {
        attributes.remove("specials");
    }
}

fn normalize_raw(template: &mut RawTemplate, reference: &str) {
    template.reference = reference.into();
    normalize_attributes(&mut template.attributes, reference);
}

fn normalize_mech(mut template: BattleTemplate, reference: &str) -> BattleTemplate {
    template.reference = reference.into();
    normalize_attributes(&mut template.attributes, reference);
    template
}

fn normalize_vehicle(mut template: BattleVehicleTemplate, reference: &str) -> BattleVehicleTemplate {
    template.reference = reference.into();
    normalize_attributes(&mut template.attributes, reference);
    template
}
