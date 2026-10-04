//! Administrative template edits: tonnage, movement and technology codes that override a
//! template's authored identity without rebuilding its construction.
use super::{BattleSection, BattleSystem, BattleTemplate, BattleVehicleMovement};
use std::collections::BTreeMap;

pub fn administrative_template_tonnage(
    attributes: &BTreeMap<String, String>,
    fallback: u16,
) -> u32 {
    attributes
        .get("administrative_tonnage")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(u32::from(fallback))
}

/// Apply the optional administrative movement override used by the C template fields.
///
/// The Rust vehicle model currently admits the ground/VTOL movement families represented by
/// `BattleVehicleMovement`. Other C movement values remain stored verbatim and use the admitted
/// template's movement for vehicle-only calculations.
pub fn administrative_template_movement(
    attributes: &BTreeMap<String, String>,
    fallback: BattleVehicleMovement,
) -> BattleVehicleMovement {
    attributes
        .get("administrative_movement_type")
        .and_then(|value| BattleVehicleMovement::parse(value).ok())
        .unwrap_or(fallback)
}

pub fn administrative_technology(code: i32) -> Option<(&'static str, &'static str)> {
    let group = if code <= 30 {
        "primary"
    } else if code <= 56 {
        "secondary"
    } else {
        "infantry"
    };
    let name = match code {
        0 => "TripleMyomerTech",
        1 => "CL_AMS",
        2 => "IS_AMS",
        3 => "DoubleHS",
        4 => "Masc",
        5 => "Clan",
        6 => "FlipArms",
        7 => "C3MasterTech",
        8 => "C3SlaveTech",
        9 => "ArtemisIV",
        10 => "ECM",
        11 => "BeagleProbe",
        12 => "SalvageTech",
        13 => "CargoTech",
        14 => "SearchLight",
        15 => "LightBAP",
        16 => "AntiAircraft",
        17 => "NoSensors",
        18 => "SS_Ability",
        19 => "FerroFibrous_Tech",
        20 => "EndoSteel_Tech",
        21 => "XLEngine_Tech",
        22 => "ICEEngine_Tech",
        23 => "ForceSingleHS",
        24 => "LightEngine_Tech",
        25 => "XXL_Tech",
        26 => "CompactEngine_Tech",
        27 => "ReinforcedInternal_Tech",
        28 => "CompositeInternal_Tech",
        29 => "HardenedArmor_Tech",
        30 => "CritProof_Tech",
        31 => "StealthArmor_Tech",
        32 => "HvyFerroFibrous_Tech",
        33 => "LaserRefArmor_Tech",
        34 => "ReactiveArmor_Tech",
        35 => "NullSigSys_Tech",
        36 => "C3I_Tech",
        37 => "SuperCharger_Tech",
        38 => "ImprovedJJ_Tech",
        39 => "MechanicalJJ_Tech",
        40 => "CompactHS",
        41 => "LaserHS_Tech",
        42 => "BloodhoundProbe_Tech",
        43 => "AngelECM_Tech",
        44 => "WatchDog_Tech",
        45 => "LtFerroFibrous_Tech",
        46 => "TAG_Tech",
        47 => "OmniMech_Tech",
        48 => "ArtemisV_Tech",
        49 => "Camo_Tech",
        50 => "Carrier_Tech",
        51 => "Waterproof_Tech",
        52 => "XLGyro_Tech",
        53 => "HDGyro_Tech",
        54 => "CompactGyro_Tech",
        55 => "TargComp_Tech",
        56 => "SmallCockpit_Tech",
        57 => "Swarm_Attack_Tech",
        58 => "Mount_Friends_Tech",
        59 => "AntiLeg_Attack_Tech",
        60 => "CS_Purifier_Stealth_Tech",
        61 => "DC_Kage_Stealth_Tech",
        62 => "FWL_Achileus_Stealth_Tech",
        63 => "FC_Infiltrator_Stealth_Tech",
        64 => "FC_InfiltratorII_Stealth_Tech",
        65 => "Must_Jettison_Pack_Tech",
        66 => "Can_Jettison_Pack_Tech",
        _ => return None,
    };
    Some((name, group))
}

/// Toggle one technology flag in the named specials attribute ("specials",
/// "specials2", or "infantry_specials"), mirroring the native per-group flags.
pub fn edit_special(
    attributes: &mut BTreeMap<String, String>,
    attribute: &str,
    flag: &str,
    enabled: bool,
) {
    let mut values: Vec<String> = attributes
        .get(attribute)
        .into_iter()
        .flat_map(|value| value.split_ascii_whitespace())
        .filter(|value| *value != "-" && !value.eq_ignore_ascii_case(flag))
        .map(str::to_owned)
        .collect();
    if enabled {
        values.push(flag.to_owned());
    }
    attributes.insert(
        attribute.into(),
        if values.is_empty() {
            "-".into()
        } else {
            values.join(" ")
        },
    );
}

impl BattleTemplate {
    /// The system installed at a section's critical slot, if the slot names one.
    pub fn system_at(&self, section: BattleSection, slot: u8) -> Option<BattleSystem> {
        let critical = self.sections.get(&section)?.criticals.get(&slot)?;
        BattleSystem::named(&critical.equipment)
    }

    /// Whether the installed equipment implies administrative technology `code`, as template
    /// loading does for arm flipping and compact engines.
    pub fn infers_technology(&self, code: i32) -> bool {
        match code {
            // The loader permits arm flipping only when both arms omit their lower
            // and hand actuators from the conventional third and fourth slots.
            6 => {
                !matches!(
                    self.system_at(BattleSection::LeftArm, 2),
                    Some(BattleSystem::LowerActuator)
                ) && !matches!(
                    self.system_at(BattleSection::RightArm, 2),
                    Some(BattleSystem::LowerActuator)
                ) && !matches!(
                    self.system_at(BattleSection::LeftArm, 3),
                    Some(BattleSystem::HandOrFootActuator)
                ) && !matches!(
                    self.system_at(BattleSection::RightArm, 3),
                    Some(BattleSystem::HandOrFootActuator)
                )
            }
            // Fewer than four center-torso engine criticals identify a compact
            // engine, including sparse but valid inspection templates.
            26 => {
                self.sections
                    .get(&BattleSection::CenterTorso)
                    .map_or(0, |section| {
                        section
                            .criticals
                            .values()
                            .filter(|critical| {
                                BattleSystem::named(&critical.equipment)
                                    .is_some_and(|system| system == BattleSystem::Engine)
                            })
                            .count()
                    })
                    < 4
            }
            _ => false,
        }
    }
}
