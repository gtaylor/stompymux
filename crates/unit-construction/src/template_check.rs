//! Read-only construction diagnostics for a Mech template: whether construction would accept
//! it and why not. Operator and Lua inspection and the template tools report these without
//! changing the source.
use super::{MechChassis, MechLoadout, MechTemplate};
use serde::Serialize;

/// Construction readiness under currently implemented biped rules, not a claim of complete gameplay parity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TemplateCheck {
    pub name: String,
    pub reference: String,
    /// Parsed anatomy, independent of whether live simulation is ready.
    pub chassis: Option<MechChassis>,
    pub constructible: bool,
    pub rejection: Option<String>,
    /// Walking MP the template states (TM p. 48).
    pub walking_mp: u32,
    /// Running MP: walking MP x 1.5, rounded up (TM p. 48).
    pub running_mp: u32,
    /// Engine rating: tonnage x walking MP (TM p. 48).
    pub engine_rating: u32,
    /// The engine rating the template's `[construction]` table declares, which construction
    /// requires to equal `engine_rating`; absent when the template derives it.
    pub declared_engine_rating: Option<u32>,
    pub weapons: usize,
    pub ammunition_bins: usize,
}

/// Validate a parsed template using unit construction without registering an object or changing the source.
pub fn check_template(template: &MechTemplate) -> TemplateCheck {
    let mut report = TemplateCheck {
        name: template.name.clone(),
        reference: template.reference.clone(),
        chassis: template.chassis().ok(),
        constructible: false,
        rejection: None,
        walking_mp: template.walking_mp(),
        running_mp: template.running_mp(),
        engine_rating: template.engine_rating(),
        declared_engine_rating: template.declared_engine_rating().ok().flatten(),
        weapons: 0,
        ammunition_bins: 0,
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
    report
}

/// Run construction in the order the game constructs a live Mech and return the loadout it
/// would field.
fn construct(template: &MechTemplate) -> anyhow::Result<MechLoadout> {
    template.validate_definition()?;
    let loadout = MechLoadout::resolve(template)?;
    template.validate_loadout(&loadout)?;
    Ok(loadout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MechSection;

    const JENNER: &str = include_str!("../tests/fixtures/JR7-D.toml");

    /// A machine gun bin holds a full ton, or a half-ton lot marked `Halfton`; construction
    /// refuses any other count rather than refilling or resizing the bin, and loads other bins
    /// with the rounds they list (TechManual p. 57). The source is never changed.
    #[test]
    fn bins_hold_what_their_lot_holds() {
        for (bin, rounds, modes, expected) in [
            ("Ammo_IS.SRM-4", 7, "", None),
            (
                "Ammo_IS.MachineGun",
                100,
                "",
                Some(
                    "lists 100 rounds; a machine gun bin holds 200 rounds a ton, or 100 in a half-ton lot marked Halfton",
                ),
            ),
            (
                "Ammo_IS.MachineGun",
                200,
                ", modes = [\"Halfton\"]",
                Some("Ammunition exceeds bin capacity"),
            ),
            (
                "Ammo_IS.MachineGun",
                40,
                ", modes = [\"Halfton\"]",
                Some("lists 40 rounds"),
            ),
            ("Ammo_IS.MachineGun", 100, ", modes = [\"Halfton\"]", None),
            ("Ammo_IS.MachineGun", 200, "", None),
        ] {
            let source = JENNER.replace(
                "item = \"Ammo_IS.SRM-4\", rounds = 25",
                &format!("item = \"{bin}\", rounds = {rounds}{modes}"),
            );
            assert_ne!(source, JENNER);
            let template = MechTemplate::parse("JR7-D", &source).unwrap();
            let original = template.clone();
            let report = check_template(&template);
            assert_eq!(template, original);
            let Some(expected) = expected else {
                assert!(report.constructible, "{:?}", report.rejection);
                assert_eq!(report.chassis, Some(MechChassis::Biped));
                assert_eq!((report.weapons, report.ammunition_bins), (5, 1));
                continue;
            };
            assert!(!report.constructible);
            let rejection = report.rejection.unwrap();
            assert!(rejection.contains(expected), "{rejection}");
        }
    }

    /// An explicit half-ton lot on anything but a machine gun is a construction failure.
    #[test]
    fn half_ton_bins_on_other_weapons_are_rejected() {
        let source = JENNER.replace(
            "item = \"Ammo_IS.SRM-4\", rounds = 25",
            "item = \"Ammo_IS.SRM-4\", rounds = 12, modes = [\"Halfton\"]",
        );
        assert_ne!(source, JENNER);
        let report = check_template(&MechTemplate::parse("JR7-D", &source).unwrap());
        assert!(!report.constructible);
        assert!(
            report
                .rejection
                .as_deref()
                .is_some_and(|rejection| rejection
                    .contains("Half-ton ammunition bins are limited to machine guns")),
            "{:?}",
            report.rejection
        );
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
            assert_eq!(report.ammunition_bins, 0);
        }
    }
}
