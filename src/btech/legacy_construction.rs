//! One-time conversion of literal TOML templates into construction-based documents.
//!
//! Technology types follow the critical slots a unit carries when its flags are silent:
//! engine slots in the side torsos make an XL, light or XXL engine, and enough endo
//! steel or ferro-fibrous slots make that structure or armor. Flippable arms follow
//! the arm actuators.
use super::template_construction::{FLIP_ARMS, canonical_infantry_special, canonical_special};
use super::template_document::{DocumentMode, ParsedTemplate, RenderSection, render};
use super::{
    BattleMechChassis, BattleSection, BattleUnitTemplate, RawMovement, RawSectionCode, RawTemplate,
    RawUnitClass,
};
use anyhow::{Context, Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

/// Convert a literal document and verify the result decodes to the same layout.
pub fn convert_constructed_template(reference: &str, source: &str) -> Result<String> {
    let mut literal = ParsedTemplate::parse_mode(reference, source, DocumentMode::Literal)?;
    let class = literal
        .fields
        .get("type")
        .map(|class| RawUnitClass::parse(class))
        .transpose()?
        .context("missing class")?;
    let movement = RawMovement::parse(
        literal
            .fields
            .get("move_type")
            .context("missing movement")?,
    )?;
    decide_by_slots(&mut literal, class, movement)?;
    let mut ordered = Vec::new();
    let mut remaining: BTreeSet<_> = literal.sections.keys().cloned().collect();
    for code in RawSectionCode::for_unit(class, movement) {
        let heading = code.name().replace(' ', "_").to_ascii_lowercase();
        if remaining.remove(&heading) {
            ordered.push(heading);
        }
    }
    ordered.extend(remaining);
    let sections: Vec<_> = ordered
        .iter()
        .map(|heading| RenderSection {
            heading: heading.clone(),
            mech: mech_section(class, heading),
            layout: &literal.sections[heading],
        })
        .collect();
    let mut fields = literal.fields.clone();
    fields.remove("reference");
    let document = render(&fields, &sections)?;
    let constructed = ParsedTemplate::parse(reference, &document)
        .with_context(|| format!("converted document does not parse:\n{document}"))?;
    ensure!(
        constructed.sections == literal.sections,
        "sections differ:\n{:#?}\n{:#?}",
        literal.sections,
        constructed.sections
    );
    ensure!(
        normalized(&constructed.fields) == normalized(&literal.fields),
        "fields differ:\n{:#?}\n{:#?}",
        literal.fields,
        constructed.fields
    );
    let literal_unit = BattleUnitTemplate::from_parsed(ParsedTemplate::parse_mode(
        reference,
        source,
        DocumentMode::Literal,
    )?);
    let constructed_unit = BattleUnitTemplate::parse(reference, &document);
    ensure!(
        constructed_unit.is_ok() || literal_unit.is_err(),
        "converted document no longer loads: {:#}",
        constructed_unit.err().unwrap()
    );
    let constructed_raw = RawTemplate::parse(reference, &document);
    let literal_raw = RawTemplate::from_parsed(ParsedTemplate::parse_mode(
        reference,
        source,
        DocumentMode::Literal,
    )?);
    ensure!(
        constructed_raw.is_ok() == literal_raw.is_ok(),
        "raw loading changed"
    );
    Ok(document)
}

/// The stable mech section for a heading, when the class has mech anatomy.
fn mech_section(class: RawUnitClass, heading: &str) -> Option<BattleSection> {
    if class != RawUnitClass::Mech {
        return None;
    }
    BattleMechChassis::Biped
        .parse_section(heading)
        .or_else(|_| BattleMechChassis::Quad.parse_section(heading))
        .ok()
}

/// Fields compared with flags as a canonical set and numbers by value.
fn normalized(fields: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    fields
        .iter()
        .map(|(key, value)| {
            let value = match key.as_str() {
                "specials" | "infantryspecials" => {
                    let flags: BTreeSet<_> = value
                        .split_ascii_whitespace()
                        .map(str::to_ascii_lowercase)
                        .collect();
                    flags.into_iter().collect::<Vec<_>>().join(" ")
                }
                _ => value
                    .parse::<f64>()
                    .map_or_else(|_| value.clone(), |number| number.to_string()),
            };
            (key.clone(), value)
        })
        .collect()
}

/// Apply the conversion rules to a literal document's flags: technology types follow the
/// slots when the flags are silent, flippable arms follow the actuators, Clan units drop
/// the redundant double heat sink flag, and every flag takes its canonical spelling.
fn decide_by_slots(
    parsed: &mut ParsedTemplate,
    class: RawUnitClass,
    movement: RawMovement,
) -> Result<()> {
    let mut flags: Vec<String> = parsed
        .fields
        .get("specials")
        .map(|specials| {
            specials
                .split_ascii_whitespace()
                .filter(|flag| *flag != "-")
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let has =
        |flags: &[String], name: &str| flags.iter().any(|flag| flag.eq_ignore_ascii_case(name));
    let clan = has(&flags, "Clan");
    if class == RawUnitClass::Mech {
        let count = |heading: &str, item: &str| {
            parsed.sections.get(heading).map_or(0, |section| {
                section
                    .criticals
                    .values()
                    .filter(|critical| critical.equipment == item)
                    .count()
            })
        };
        let total = |item: &str| {
            parsed
                .sections
                .values()
                .flat_map(|section| section.criticals.values())
                .filter(|critical| critical.equipment == item)
                .count()
        };
        let engine_flags = [
            "XLEngine_Tech",
            "LightEngine_Tech",
            "XXL_Tech",
            "CompactEngine_Tech",
            "ICEEngine_Tech",
        ];
        if !engine_flags.iter().any(|name| has(&flags, name)) {
            let layout = (
                count("center_torso", "Engine"),
                count("left_torso", "Engine"),
                count("right_torso", "Engine"),
                clan,
            );
            match layout {
                (6, 3, 3, false) | (6, 2, 2, true) => flags.push("XLEngine_Tech".into()),
                (6, 2, 2, false) => flags.push("LightEngine_Tech".into()),
                (6, 6, 6, false) | (6, 4, 4, true) => flags.push("XXL_Tech".into()),
                _ => {}
            }
        }
        let material = if clan { 7 } else { 14 };
        let structure_flags = [
            "EndoSteel_Tech",
            "CompositeInternal_Tech",
            "CINT",
            "ReinforcedInternal_Tech",
            "RINT",
        ];
        if !structure_flags.iter().any(|name| has(&flags, name)) && total("EndoSteel") >= material {
            flags.push("EndoSteel_Tech".into());
        }
        let armor_flags = [
            "FerroFibrous_Tech",
            "LtFerroFibrous_Tech",
            "HvyFerroFibrous_Tech",
            "StealthArmor_Tech",
            "HardenedArmor_Tech",
            "HARM",
            "LaserRefArmor_Tech",
            "LRARM",
            "ReactiveArmor_Tech",
        ];
        if !armor_flags.iter().any(|name| has(&flags, name)) {
            if total("FerroFibrous") >= material {
                flags.push("FerroFibrous_Tech".into());
            } else if total("HvyFerroFibrous") >= 21 {
                flags.push("HvyFerroFibrous_Tech".into());
            } else if total("LtFerroFibrous") >= 7 {
                flags.push("LtFerroFibrous_Tech".into());
            }
        }
        flags.retain(|flag| !flag.eq_ignore_ascii_case(FLIP_ARMS));
        if movement == RawMovement::Biped
            && let (Some(left), Some(right)) = (
                parsed.sections.get("left_arm"),
                parsed.sections.get("right_arm"),
            )
            && super::template_construction::arms_flip([left, right])
        {
            flags.push(FLIP_ARMS.into());
        }
    }
    if class != RawUnitClass::Mech {
        flags.retain(|flag| !flag.eq_ignore_ascii_case(FLIP_ARMS));
    }
    if clan {
        flags.retain(|flag| !flag.eq_ignore_ascii_case("DoubleHS"));
    }
    for flag in &mut flags {
        if let Ok(canonical) = canonical_special(flag) {
            *flag = canonical.into();
        }
    }
    if flags.is_empty() {
        parsed.fields.remove("specials");
    } else {
        parsed.fields.insert("specials".into(), flags.join(" "));
    }
    if let Some(infantry) = parsed.fields.get_mut("infantryspecials") {
        let canonical: Result<Vec<_>> = infantry
            .split_ascii_whitespace()
            .map(|flag| canonical_infantry_special(flag).map(str::to_owned))
            .collect();
        *infantry = canonical?.join(" ");
    }
    Ok(())
}
