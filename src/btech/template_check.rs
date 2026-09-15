//! Read-only construction diagnostics shared by operator and Lua asset inspection.
use super::{BattleTemplate, BattleUnit, CriticalLocation};
use serde::Serialize;

/// A quantity or inferred bin-size adjustment that construction would make to an owned template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleAmmunitionAdjustment {
    pub location: CriticalLocation,
    pub supplied: u16,
    pub normalized: u16,
    pub inferred_half_ton: bool,
}

/// Construction readiness under currently implemented biped rules, not a claim of complete gameplay parity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleTemplateCheck {
    pub name: String,
    pub reference: String,
    /// Parsed anatomy, independent of whether live simulation is ready.
    pub chassis: Option<super::BattleMechChassis>,
    pub constructible: bool,
    pub rejection: Option<String>,
    pub weapons: usize,
    pub ammunition_bins: usize,
    pub ammunition_adjustments: Vec<BattleAmmunitionAdjustment>,
}

/// Validate a parsed template using unit construction without registering an object or changing the source.
pub fn check_template(template: &BattleTemplate) -> BattleTemplateCheck {
    let mut report = BattleTemplateCheck {
        name: template.name.clone(),
        reference: template.reference.clone(),
        chassis: template.chassis().ok(),
        constructible: false,
        rejection: None,
        weapons: 0,
        ammunition_bins: 0,
        ammunition_adjustments: Vec::new(),
    };
    let unit = match BattleUnit::from_template(template.clone()) {
        Ok(unit) => unit,
        Err(error) => {
            report.rejection = Some(format!("{error:#}"));
            return report;
        }
    };
    let loadout = unit
        .loadout()
        .expect("construction validated the normalized loadout");
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
            report
                .ammunition_adjustments
                .push(BattleAmmunitionAdjustment {
                    location: bin.location,
                    supplied,
                    normalized: bin.rounds,
                    inferred_half_ton,
                });
        }
    }
    report
}
