//! Read-only construction diagnostics for a Mech template: whether construction would accept
//! it, why not, and the ammunition bins construction would resize or refill. Operator and Lua
//! inspection and the template tools report these without changing the source.
use super::{CriticalLocation, MechChassis, MechLoadout, MechTemplate};
use serde::Serialize;

/// A quantity or inferred bin-size adjustment that construction would make to an owned template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AmmunitionAdjustment {
    pub location: CriticalLocation,
    pub supplied: u16,
    pub normalized: u16,
    pub inferred_half_ton: bool,
}

/// Construction readiness under currently implemented biped rules, not a claim of complete gameplay parity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TemplateCheck {
    pub name: String,
    pub reference: String,
    /// Parsed anatomy, independent of whether live simulation is ready.
    pub chassis: Option<MechChassis>,
    pub constructible: bool,
    pub rejection: Option<String>,
    pub weapons: usize,
    pub ammunition_bins: usize,
    pub ammunition_adjustments: Vec<AmmunitionAdjustment>,
}

/// Validate a parsed template using unit construction without registering an object or changing the source.
pub fn check_template(template: &MechTemplate) -> TemplateCheck {
    let mut report = TemplateCheck {
        name: template.name.clone(),
        reference: template.reference.clone(),
        chassis: template.chassis().ok(),
        constructible: false,
        rejection: None,
        weapons: 0,
        ammunition_bins: 0,
        ammunition_adjustments: Vec::new(),
    };
    let loadout = match construct(template) {
        Ok(loadout) => loadout,
        Err(error) => {
            report.rejection = Some(format!("{error:#}"));
            return report;
        }
    };
    report.constructible = true;
    report.weapons = loadout.weapons.len();
    report.ammunition_bins = loadout.ammunition.len();
    for bin in &loadout.ammunition {
        let source = &template.sections[&bin.location.section].criticals[&bin.location.slot];
        let supplied = source
            .data
            .parse()
            .expect("construction validated ammunition syntax");
        let inferred_half_ton = bin.half_ton && !source.modes.iter().any(|flag| flag == "Halfton");
        if supplied != bin.rounds || inferred_half_ton {
            report.ammunition_adjustments.push(AmmunitionAdjustment {
                location: bin.location,
                supplied,
                normalized: bin.rounds,
                inferred_half_ton,
            });
        }
    }
    report
}

/// Run construction on a copy of the template, in the order the game constructs a live Mech,
/// and return the normalized loadout it would field.
fn construct(template: &MechTemplate) -> anyhow::Result<MechLoadout> {
    let mut normalized = template.clone();
    normalized.normalize_ammunition()?;
    normalized.validate_definition()?;
    let loadout = MechLoadout::resolve(&normalized)?;
    normalized.validate_loadout(&loadout)?;
    Ok(loadout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MechSection;

    const JENNER: &str = include_str!("../tests/fixtures/JR7-D.toml");

    #[test]
    fn constructible_templates_report_normalized_bins_and_keep_the_source() {
        let source = JENNER.replace(
            "item = \"Ammo_IS.SRM-4\", rounds = 25",
            "item = \"Ammo_IS.SRM-4\", rounds = 7",
        );
        assert_ne!(source, JENNER);
        let template = MechTemplate::parse("JR7-D", &source).unwrap();
        let original = template.clone();
        let report = check_template(&template);
        assert!(report.constructible, "{:?}", report.rejection);
        assert_eq!(report.chassis, Some(MechChassis::Biped));
        assert_eq!((report.weapons, report.ammunition_bins), (5, 1));
        assert_eq!(
            report.ammunition_adjustments,
            [AmmunitionAdjustment {
                location: CriticalLocation {
                    section: MechSection::RightTorso,
                    slot: 0
                },
                supplied: 7,
                normalized: 12,
                inferred_half_ton: true,
            }]
        );
        assert_eq!(template, original);
    }

    #[test]
    fn rejections_name_the_failing_rule() {
        let original = MechTemplate::parse("JR7-D", JENNER).unwrap();
        let unknown_weapon =
            MechTemplate::parse("JR7-D", &JENNER.replace("IS.MediumLaser", "IS.Unknown")).unwrap();
        let mut missing_gyro = original.clone();
        for critical in missing_gyro
            .sections
            .get_mut(&MechSection::CenterTorso)
            .unwrap()
            .criticals
            .values_mut()
            .filter(|critical| critical.equipment == "Gyro")
        {
            critical.equipment = "HeatSink".into();
        }
        let mut unknown_technology = original.clone();
        unknown_technology
            .attributes
            .insert("specials".into(), "UnknownTechnology".into());
        let mut heavy = original;
        heavy.tons = 101;
        for (template, expected) in [
            (unknown_weapon, "Unsupported weapon IS.Unknown"),
            (missing_gyro, "Incomplete conventional Gyro system"),
            (unknown_technology, "Unsupported chassis specials"),
            (heavy, "Unsupported biped tonnage"),
        ] {
            let report = check_template(&template);
            assert!(!report.constructible);
            let rejection = report.rejection.unwrap();
            assert!(rejection.contains(expected), "{rejection}");
            assert!(report.ammunition_adjustments.is_empty());
        }
    }
}
