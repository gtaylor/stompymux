//! Source-derived FASA construction cost for supported BattleMech templates.
use super::*;
use anyhow::{Result, ensure};
use std::collections::BTreeMap;

/// Market prices of loose parts by part id. Parts without an entry cost nothing.
pub type PartPrices = BTreeMap<i32, u64>;

/// The market price of one catalogued part.
pub fn part_price(prices: &PartPrices, part_id: i32) -> Result<u64> {
    ensure!(Part::from_id(part_id).is_some(), "Unknown inventory part");
    Ok(prices.get(&part_id).copied().unwrap_or(0))
}
const WEAPON_COST: [u64; 196] = [
    1500, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 200000, 80000, 11250, 10000, 300000, 7500, 250000, 100000, 20000, 175000, 60000,
    16000, 12500, 400000, 150000, 30000, 100000, 300000, 150000, 250000, 400000, 600000, 5000,
    5000, 7500, 120000, 200000, 320000, 480000, 450000, 50000, 125000, 225000, 350000, 30000,
    100000, 175000, 250000, 100000, 10000, 60000, 80000, 200000, 100000, 40000, 11250, 7500,
    300000, 200000, 80000, 11250, 175000, 60000, 16000, 275000, 110000, 31000, 300000, 150000,
    250000, 75000, 125000, 200000, 300000, 7500, 5000, 100000, 300000, 275000, 500000, 150000,
    250000, 400000, 600000, 175000, 275000, 120000, 200000, 320000, 480000, 450000, 300000, 187500,
    20000, 100000, 150000, 650000, 475000, 200000, 7500, 260000, 45000, 75000, 105000, 125000,
    30000, 100000, 175000, 250000, 10000, 60000, 80000, 50000, 125000, 225000, 350000, 100000,
    250000, 15000, 90000, 120000, 15000, 30000, 45000, 50000, 175000, 325000, 450000, 0, 0, 0, 0,
    0, 0, 0, 8500, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    // Enhanced LRM-5, -10, -15 and -20.
    60000, 125000, 175000, 250000,
    // Inner Sphere LRT-5 to -20 and SRT-2 to -6, then the Clan launchers.
    30000, 100000, 175000, 250000, 10000, 60000, 80000, 30000, 100000, 175000, 250000, 10000, 60000,
    80000,
];
const AMMO_COST: [u64; 196] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2000, 20000, 2000, 9000, 12000, 20000,
    1000, 500, 1000, 1000, 9000, 12000, 20000, 10000, 75000, 75000, 75000, 75000, 30000, 30000,
    30000, 30000, 6000, 27000, 27000, 27000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1000, 4500, 6000, 10000, 1000, 1000, 2000, 20000, 20000, 20000, 2000, 9000, 12000, 20000, 3000,
    12000, 1000, 9000, 12000, 20000, 1000, 6000, 4500, 2000, 2000, 5000, 20000, 15000, 10000, 1000,
    10000, 27000, 27000, 27000, 27000, 30000, 30000, 30000, 30000, 27000, 27000, 27000, 5000, 5000,
    5000, 5000, 6000, 7500, 54000, 54000, 54000, 0, 0, 0, 50000, 50000, 50000, 50000, 0, 0, 0, 0,
    0, 0, 0, 1000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31000, 31000, 31000,
    31000, 30000, 30000, 30000, 30000, 27000, 27000, 27000, 30000, 30000, 30000, 30000, 27000,
    27000, 27000,
];
fn flag(template: &MechTemplate, name: &str) -> bool {
    ["specials", "specials2"]
        .iter()
        .filter_map(|key| template.attributes.get(*key))
        .any(|v| {
            v.split_ascii_whitespace()
                .any(|v| flag_spells_technology(v, name))
        })
        || (0..=56).any(|code| {
            administrative_technology(code).is_some_and(|(candidate, _)| {
                candidate.eq_ignore_ascii_case(name) && template.infers_technology(code)
            })
        })
}
fn half(value: u32) -> u32 {
    value.div_ceil(512) * 512
}

fn raw_flag(template: &RawTemplate, name: &str) -> bool {
    ["specials", "specials2"]
        .iter()
        .filter_map(|key| template.attributes.get(*key))
        .any(|value| {
            value
                .split_ascii_whitespace()
                .any(|value| flag_spells_technology(value, name))
        })
}

fn raw_weapon_cost(id: i32) -> u64 {
    usize::try_from(id - 1)
        .ok()
        .and_then(|index| WEAPON_COST.get(index))
        .copied()
        .unwrap_or_default()
}

fn raw_weapon_mass(id: i32) -> u32 {
    if let Some(weapon) = Weapon::from_part_id(id) {
        return weapon.mass();
    }
    match id {
        171 => 512,
        172 => 768,
        173 | 174 => 1024,
        175 | 176 => 768,
        177 | 178 => 1024,
        _ => 0,
    }
}

fn raw_weapon_is_energy(id: i32) -> bool {
    Weapon::from_part_id(id).is_some_and(|weapon| weapon.gunnery_skill(true) == "Gunnery-Laser")
        || matches!(id, 175 | 176)
}

fn raw_ammunition_cost(template: &RawTemplate) -> Result<f64> {
    let mut total = 0.0;
    for section in RawSectionCode::for_unit(template.class, template.movement) {
        for row in inspect_raw_template_criticals(template, *section)? {
            let Some(part) = row.part else { continue };
            let Some(weapon_id) = Part::ammunition_weapon_id(part.id) else {
                continue;
            };
            let Some((_, maximum)) = row.ammunition else {
                continue;
            };
            let Some(weapon) = Weapon::from_part_id(weapon_id) else {
                continue;
            };
            let per_ton = u64::from(weapon.profile().ammunition_per_ton);
            if per_ton == 0 || maximum == 0 {
                continue;
            }
            let cost = AMMO_COST[(weapon_id - 1) as usize];
            total += if u64::from(maximum) < per_ton {
                (cost / (per_ton / u64::from(maximum))) as f64
            } else {
                (cost * (u64::from(maximum) / per_ton)) as f64
            };
        }
    }
    Ok(total)
}

fn raw_equipment_cost(prices: &PartPrices, template: &RawTemplate) -> Result<f64> {
    let mut total = 0.0;
    let mut bloodhound = 0_u64;
    for definition in template.sections.values() {
        for critical in definition.criticals.values() {
            let Some(part) = Part::parse(&critical.equipment).ok() else {
                continue;
            };
            if matches!(part.kind, PartKind::Weapon | PartKind::Ammunition) {
                continue;
            }
            total += if let Some(system) = System::named(&critical.equipment) {
                match system {
                    System::Case => 50000.0,
                    System::CaseIi => 175000.0,
                    System::BeagleProbe => 100000.0,
                    System::LightProbe => 50000.0,
                    System::BloodhoundProbe => {
                        bloodhound += 1;
                        0.0
                    }
                    System::ArtemisIv => {
                        if raw_flag(template, "ArtemisV_Tech") {
                            250000.0
                        } else {
                            100000.0
                        }
                    }
                    System::AngelEcm => 375000.0,
                    System::C3Master => 300000.0,
                    System::C3Slave => 250000.0,
                    System::C3i => 375000.0,
                    System::Ecm => {
                        if raw_flag(template, "WatchDog_Tech") {
                            500000.0
                        } else {
                            100000.0
                        }
                    }
                    System::Tag => 50000.0,
                    System::TargetingComputer => 10000.0,
                    System::Axe => 5000.0,
                    System::ShoulderOrHip
                    | System::UpperActuator
                    | System::LowerActuator
                    | System::HandOrFootActuator
                    | System::LifeSupport
                    | System::Sensors
                    | System::Cockpit
                    | System::Engine
                    | System::Gyro
                    | System::HeatSink
                    | System::JumpJet
                    | System::FerroFibrous
                    | System::LightFerroFibrous
                    | System::EndoSteel
                    | System::TripleStrengthMyomer
                    | System::StealthArmor
                    | System::LaserReflective
                    | System::Masc
                    | System::Sword => 0.0,
                    _ => part_price(prices, part.part_id)? as f64,
                }
            } else if part.part_id == 427 {
                175000.0
            } else if (443..=445).contains(&part.part_id) {
                0.0
            } else {
                part_price(prices, part.part_id)? as f64
            };
        }
    }
    Ok(total + (bloodhound / 3 * 500000) as f64)
}

fn raw_mech_template(template: &RawTemplate) -> Result<MechTemplate> {
    anyhow::ensure!(
        template.class == RawUnitClass::Mech,
        "template is not a Mech"
    );
    let movement = template.movement;
    let mut sections = std::collections::BTreeMap::new();
    for (ordinal, section) in MechSection::ALL.into_iter().enumerate() {
        let raw = RawSectionCode::from_ordinal(template.class, movement, ordinal)
            .and_then(|code| template.sections.get(&code))
            .cloned()
            .unwrap_or_default();
        sections.insert(section, raw);
    }
    Ok(MechTemplate {
        name: template.name.clone(),
        reference: template.reference.clone(),
        tons: u16::try_from(template.tons)?,
        max_speed: template.max_speed,
        jump_speed: template.jump_speed,
        heat_sinks: u16::try_from(template.heat_sinks)?,
        sections,
        attributes: template.attributes.clone(),
    })
}

fn raw_vehicle_base_cost(prices: &PartPrices, template: &RawTemplate) -> Result<u64> {
    let tons = u64::try_from(template.tons)?;
    let weapons = inspect_raw_template_weapons(template)?;
    let mut total = (tons * 1500) as f64;
    let ice = raw_flag(template, "ICEEngine_Tech");
    let mut turret_mass = 0_u64;
    let mut amplifier_mass = 0_u64;
    for mount in &weapons {
        let mass = u64::from(raw_weapon_mass(mount.part.id));
        if mount.section == RawSectionCode::Turret as i32 {
            turret_mass += mass;
        }
        if raw_weapon_is_energy(mount.part.id) {
            amplifier_mass += mass;
        }
    }
    if ice {
        total += (20000 * (amplifier_mass / 1024) / 10) as f64;
    }
    total += (5000 * (turret_mass / 10) / 1024) as f64;
    if matches!(
        template.movement,
        RawMovement::Hover | RawMovement::Foil | RawMovement::Submarine
    ) {
        total += (2000 * tons) as f64;
    }
    if template.movement == RawMovement::Vtol {
        total += (4000 * tons) as f64;
    }
    let (rating, suspension) = inspect_raw_template_engine(template);
    let engine_base = if raw_flag(template, "CompactEngine_Tech") {
        10000_u64
    } else if raw_flag(template, "LightEngine_Tech") {
        15000
    } else if raw_flag(template, "XLEngine_Tech") {
        20000
    } else if raw_flag(template, "XXL_Tech") {
        100000
    } else if ice {
        1250
    } else {
        5000
    };
    let engine_size = u64::try_from(i64::from(rating) - i64::from(suspension))?;
    total += (engine_base * engine_size * tons).div_ceil(75) as f64;
    let clan = raw_flag(template, "Clan");
    let double = clan || raw_flag(template, "DoubleHS");
    let sinks = u64::try_from(template.heat_sinks.max(0))? / if double { 2 } else { 1 };
    total += (if double || ice {
        sinks
    } else {
        sinks.saturating_sub(10)
    } * if double { 6000 } else { 2000 }) as f64;
    let armor = inspect_raw_template_armor(template, None)?.armor.1;
    let armor = if raw_flag(template, "FerroFibrous_Tech") {
        armor * 50 / if clan { 60 } else { 56 }
    } else if raw_flag(template, "HvyFerroFibrous_Tech") {
        armor * 50 / 62
    } else if raw_flag(template, "LtFerroFibrous_Tech") {
        armor * 50 / 53
    } else {
        armor
    };
    let armor_cost = if raw_flag(template, "FerroFibrous_Tech") {
        20000
    } else if raw_flag(template, "StealthArmor_Tech") {
        50000
    } else if raw_flag(template, "LaserRefArmor_Tech") {
        30000
    } else if raw_flag(template, "HardenedArmor_Tech") || raw_flag(template, "LtFerroFibrous_Tech")
    {
        15000
    } else if raw_flag(template, "HvyFerroFibrous_Tech") {
        25000
    } else {
        10000
    };
    total += f64::from(half(armor * 1024 / 16)) / 1024.0 * f64::from(armor_cost);
    for mount in &weapons {
        total += raw_weapon_cost(mount.part.id) as f64;
    }
    total += raw_ammunition_cost(template)?;
    total += raw_equipment_cost(prices, template)?;
    let modifier = match template.movement {
        RawMovement::Tracked => 1.0 + tons as f64 / 100.0,
        RawMovement::Wheeled | RawMovement::Hull => 1.0 + tons as f64 / 200.0,
        RawMovement::Hover | RawMovement::Submarine => 1.0 + tons as f64 / 50.0,
        RawMovement::Vtol => 1.0 + tons as f64 / 30.0,
        RawMovement::Foil => 1.0 + tons as f64 / 75.0,
        _ => 1.0,
    };
    Ok((total * modifier) as u64)
}

/// Calculate native construction cost without requiring a combat runtime class.
pub fn raw_template_base_cost(prices: &PartPrices, template: &RawTemplate) -> Result<u64> {
    match template.class {
        RawUnitClass::Mech => template_base_cost(prices, &raw_mech_template(template)?),
        RawUnitClass::Vehicle | RawUnitClass::Vtol | RawUnitClass::Naval => {
            raw_vehicle_base_cost(prices, template)
        }
        RawUnitClass::BattleSuit => {
            let weapons: u64 = inspect_raw_template_weapons(template)?
                .into_iter()
                .map(|weapon| raw_weapon_cost(weapon.part.id))
                .sum();
            let base = if raw_flag(template, "Clan") {
                3_500_000
            } else {
                2_400_000
            };
            let mut total = (base + weapons) as f64
                + raw_ammunition_cost(template)?
                + raw_equipment_cost(prices, template)?;
            if raw_flag(template, "OmniMech_Tech") {
                total *= 1.25;
            }
            Ok((total * 0.75) as u64)
        }
        RawUnitClass::SpheroidDropship
        | RawUnitClass::AeroFighter
        | RawUnitClass::MechWarrior
        | RawUnitClass::AerodyneDropship => Ok(0),
    }
}

/// A catalogue part paired with the classification flags `inspection_template_part` reports.
type InspectionFlags = (InspectionPart, bool, bool);

/// Build the combat-supported portion of a template while retaining every
/// catalogue part that native inspection and economy code can price without a
/// combat implementation. Structural split links remain in the clone because
/// the supported weapon resolver consumes them.
fn inspection_loadout(template: &MechTemplate) -> Result<(MechLoadout, Vec<InspectionFlags>)> {
    let normalized = inspection_compatible_template(template);
    if let Ok(loadout) = MechLoadout::resolve(&normalized) {
        return Ok((loadout, Vec::new()));
    }
    let mut compatible = normalized;
    let mut raw = Vec::new();
    for definition in compatible.sections.values_mut() {
        definition.criticals.retain(|_, critical| {
            if critical.equipment.eq_ignore_ascii_case("SplitCrit_Left")
                || critical.equipment.eq_ignore_ascii_case("SplitCrit_Right")
                || System::named(&critical.equipment).is_some()
                || Weapon::parse(&critical.equipment).is_ok()
                || strip_name_prefix(&critical.equipment, "Ammo_")
                    .is_some_and(|name| Weapon::parse(name).is_ok())
            {
                return true;
            }
            if let Some(part) = inspection_template_part(&critical.equipment) {
                raw.push(part);
            }
            false
        });
    }
    Ok((MechLoadout::resolve(&compatible)?, raw))
}
/// Calculate the legacy MaxTech/FASA construction estimate from pristine material.
pub fn template_base_cost(prices: &PartPrices, template: &MechTemplate) -> Result<u64> {
    let (loadout, raw_parts) = inspection_loadout(template)?;
    let tons = u64::from(administrative_template_tonnage(
        &template.attributes,
        template.tons,
    ));
    let clan = flag(template, "Clan");
    let mut total = 0f64;
    total += (tons
        * if flag(template, "EndoSteel_Tech") || flag(template, "CompositeInternal_Tech") {
            1600
        } else if flag(template, "ReinforcedInternal_Tech") {
            6400
        } else {
            400
        }) as f64;
    total += if flag(template, "SmallCockpit_Tech") {
        175000.0
    } else {
        200000.0
    };
    total += 50000.0
        + (tons * 2000) as f64
        + (tons
            * if flag(template, "TripleMyomerTech") {
                16000
            } else {
                2000
            }) as f64;
    for part in &loadout.systems {
        let arm = matches!(
            part.location.section,
            MechSection::LeftArm | MechSection::RightArm
        );
        let leg = template.chassis()?.is_leg(part.location.section);
        total += match part.system {
            System::UpperActuator if arm => (tons * 100) as f64,
            System::LowerActuator if arm => (tons * 50) as f64,
            System::HandOrFootActuator if arm => (tons * 80) as f64,
            System::UpperActuator if leg => (tons * 150) as f64,
            System::LowerActuator if leg => (tons * 80) as f64,
            System::HandOrFootActuator if leg => (tons * 120) as f64,
            _ => 0.0,
        };
    }
    let rating = inspection_engine_rating(template)? as u64;
    let gyro = rating.div_ceil(100);
    total += (gyro
        * if flag(template, "XLGyro_Tech") {
            375000
        } else if flag(template, "CompactGyro_Tech") {
            600000
        } else if flag(template, "HDGyro_Tech") {
            1000000
        } else {
            300000
        }) as f64;
    let engine_base = if flag(template, "CompactEngine_Tech") {
        10000
    } else if flag(template, "LightEngine_Tech") {
        15000
    } else if flag(template, "XLEngine_Tech") {
        20000
    } else if flag(template, "XXL_Tech") {
        100000
    } else if flag(template, "ICEEngine_Tech") {
        1250
    } else {
        5000
    };
    total += (engine_base * rating * tons).div_ceil(75) as f64;
    // `do_sub_magic` replaces the authored speed with the count admitted by
    // the critical layout before `mech_fasa_cost` observes it. Improved jets
    // consume two criticals per MP and use the running-MP ceiling. Preserve
    // the source's integer `2 / 3` standard-jet multiplier as well.
    let improved_jump = flag(template, "ImprovedJJ_Tech");
    let normalized_jump =
        (inspection_normalized_jump_speed(template) as f32 * 0.093_023_3_f32).trunc() as u64;
    total +=
        (tons * normalized_jump * normalized_jump * if improved_jump { 500 } else { 200 }) as f64;
    let double = clan || flag(template, "DoubleHS");
    let sinks = if double {
        u64::from(template.heat_sinks) / 2
    } else {
        u64::from(template.heat_sinks)
    };
    total += (if double || flag(template, "CompactHS") || flag(template, "ICEEngine_Tech") {
        sinks
    } else {
        sinks.saturating_sub(10)
    } * if double {
        6000
    } else if flag(template, "CompactHS") {
        3000
    } else {
        2000
    }) as f64;
    let mut armor: u32 = template
        .sections
        .values()
        .map(|s| u32::from(s.armor) + u32::from(s.rear))
        .sum();
    if flag(template, "FerroFibrous_Tech") {
        armor = armor * 50 / if clan { 60 } else { 56 }
    } else if flag(template, "HvyFerroFibrous_Tech") {
        armor = armor * 50 / 62
    } else if flag(template, "LtFerroFibrous_Tech") {
        armor = armor * 50 / 53
    }
    let armor_tons = f64::from(half(armor * 1024 / 16)) / 1024.0;
    let armor_cost = if flag(template, "FerroFibrous_Tech") {
        20000
    } else if flag(template, "StealthArmor_Tech") {
        50000
    } else if flag(template, "LaserRefArmor_Tech") {
        30000
    } else if flag(template, "HardenedArmor_Tech") || flag(template, "LtFerroFibrous_Tech") {
        15000
    } else if flag(template, "HvyFerroFibrous_Tech") {
        25000
    } else {
        10000
    };
    total += armor_tons * f64::from(armor_cost);
    for mount in &loadout.weapons {
        total += WEAPON_COST[(mount.weapon.part_id() - 1) as usize] as f64;
    }
    for (part, is_weapon, _) in &raw_parts {
        if *is_weapon && (1..=178).contains(&part.id) {
            total += WEAPON_COST[(part.id - 1) as usize] as f64;
        }
    }
    for bin in &loadout.ammunition {
        let per = u64::from(
            bin.weapon
                .profile_for_ammunition(bin.mode)
                .ammunition_per_ton,
        );
        let cost = AMMO_COST[(bin.weapon.part_id() - 1) as usize];
        total += if u64::from(bin.capacity) < per {
            (cost / (per / u64::from(bin.capacity))) as f64
        } else {
            (cost
                * (u64::from(bin.capacity) / per)
                * if bin.mode.munition() == AmmunitionMode::Artemis {
                    2
                } else {
                    1
                }) as f64
        };
    }
    let mut masc = 0u64;
    let mut bloodhound = 0u64;
    let mut sword = false;
    let mut clan_case = std::collections::BTreeSet::new();
    for part in &loadout.systems {
        match part.system {
            System::Masc => masc += 1,
            System::Sword => sword = true,
            System::Case => total += 50000.0,
            System::CaseIi => total += 175000.0,
            System::BeagleProbe => total += 100000.0,
            System::LightProbe => total += 50000.0,
            System::BloodhoundProbe => bloodhound += 1,
            System::ArtemisIv => {
                total += if flag(template, "ArtemisV_Tech") {
                    250000.0
                } else {
                    100000.0
                }
            }
            System::AngelEcm => total += 375000.0,
            System::C3Master => total += 300000.0,
            System::C3Slave => total += 250000.0,
            System::C3i => total += 375000.0,
            System::Ecm => {
                total += if flag(template, "WatchDog_Tech") {
                    500000.0
                } else {
                    100000.0
                }
            }
            System::Tag => total += 50000.0,
            System::TargetingComputer => total += 10000.0,
            System::Axe => total += 5000.0,
            System::ShoulderOrHip
            | System::UpperActuator
            | System::LowerActuator
            | System::HandOrFootActuator
            | System::LifeSupport
            | System::Sensors
            | System::Cockpit
            | System::Engine
            | System::Gyro
            | System::HeatSink
            | System::JumpJet
            | System::FerroFibrous
            | System::LightFerroFibrous
            | System::EndoSteel
            | System::TripleStrengthMyomer
            | System::StealthArmor
            | System::LaserReflective => {}
            _ => {
                if let Ok(part_id) = Part::parse(
                    &template.sections[&part.location.section].criticals[&part.location.slot]
                        .equipment,
                ) {
                    total += part_price(prices, part_id.part_id)? as f64;
                }
            }
        }
    }
    for (part, is_weapon, is_payload) in &raw_parts {
        if *is_weapon || *is_payload {
            continue;
        }
        total += match part.id {
            427 => 175000.0,
            443..=445 => 0.0,
            id => part_price(prices, id)? as f64,
        };
    }
    if clan {
        for bin in &loadout.ammunition {
            clan_case.insert(bin.location.section);
        }
        total += (clan_case.len() as f64) * 50000.0
    }
    total += (bloodhound / 3 * 500000 + masc * rating * 1000) as f64;
    if sword {
        total += f64::from(half(
            administrative_template_tonnage(&template.attributes, template.tons) * 1024 / 20,
        )) / 1024.0
            * 10000.0
    }
    if flag(template, "OmniMech_Tech") {
        total *= 1.25
    }
    Ok((total * (1.0 + tons as f64 / 100.0)) as u64)
}

/// Calculate the legacy vehicle/VTOL branch of the FASA construction estimate.
pub fn vehicle_template_base_cost(prices: &PartPrices, template: &VehicleTemplate) -> Result<u64> {
    let compatible = inspection_compatible_vehicle_template(template);
    let loadout = VehicleLoadout::resolve(&compatible)?;
    let tons = u64::from(administrative_template_tonnage(
        &template.attributes,
        template.tons,
    ));
    let mut total = (tons * 1000 + tons * 500) as f64;
    let ice = flag_vehicle(template, "ICEEngine_Tech");
    let movement = administrative_template_movement(&template.attributes, template.movement);
    let mut turret_mass = 0u64;
    let mut amplifier_mass = 0u64;
    for mount in &loadout.weapons {
        if mount.criticals[0].section == VehicleSection::Turret {
            turret_mass += u64::from(mount.weapon.mass())
        }
        if mount.weapon.gunnery_skill(true) == "Gunnery-Laser" {
            amplifier_mass += u64::from(mount.weapon.mass())
        }
    }
    if ice {
        total += (20000 * (amplifier_mass / 1024) / 10) as f64
    }
    total += (5000 * (turret_mass / 10) / 1024) as f64;
    if movement == VehicleMovement::Hover {
        total += (2000 * tons) as f64
    }
    if movement == VehicleMovement::Vtol {
        total += (4000 * tons) as f64
    }
    let (rating, suspension) = inspection_vehicle_engine_rating(template)?;
    let base = if flag_vehicle(template, "CompactEngine_Tech") {
        10000
    } else if flag_vehicle(template, "LightEngine_Tech") {
        15000
    } else if flag_vehicle(template, "XLEngine_Tech") {
        20000
    } else if flag_vehicle(template, "XXL_Tech") {
        100000
    } else if ice {
        1250
    } else {
        5000
    };
    let size = u64::try_from((i64::from(rating) - i64::from(suspension)).max(0))?;
    total += (base * size * tons).div_ceil(75) as f64;
    let double = flag_vehicle(template, "DoubleHS") || flag_vehicle(template, "Clan");
    let sinks = u64::from(template.heat_sinks.unwrap_or(0)) / if double { 2 } else { 1 };
    total += (if double || ice {
        sinks
    } else {
        sinks.saturating_sub(10)
    } * if double { 6000 } else { 2000 }) as f64;
    let mut armor: u32 = template
        .sections
        .values()
        .map(|s| u32::from(s.armor) + u32::from(s.rear))
        .sum();
    let clan = flag_vehicle(template, "Clan");
    if flag_vehicle(template, "FerroFibrous_Tech") {
        armor = armor * 50 / if clan { 60 } else { 56 };
    } else if flag_vehicle(template, "HvyFerroFibrous_Tech") {
        armor = armor * 50 / 62;
    } else if flag_vehicle(template, "LtFerroFibrous_Tech") {
        armor = armor * 50 / 53;
    }
    let armor_cost = if flag_vehicle(template, "FerroFibrous_Tech") {
        20000
    } else if flag_vehicle(template, "StealthArmor_Tech") {
        50000
    } else if flag_vehicle(template, "LaserRefArmor_Tech") {
        30000
    } else if flag_vehicle(template, "HardenedArmor_Tech")
        || flag_vehicle(template, "LtFerroFibrous_Tech")
    {
        15000
    } else if flag_vehicle(template, "HvyFerroFibrous_Tech") {
        25000
    } else {
        10000
    };
    total += f64::from(half(armor * 1024 / 16)) / 1024.0 * f64::from(armor_cost);
    for mount in &loadout.weapons {
        total += WEAPON_COST[(mount.weapon.part_id() - 1) as usize] as f64
    }
    for bin in &loadout.ammunition {
        let per = u64::from(
            bin.weapon
                .profile_for_ammunition(bin.mode)
                .ammunition_per_ton,
        );
        let cost = AMMO_COST[(bin.weapon.part_id() - 1) as usize];
        total += if u64::from(bin.capacity) < per {
            (cost / (per / u64::from(bin.capacity))) as f64
        } else {
            (cost
                * (u64::from(bin.capacity) / per)
                * if bin.mode.munition() == AmmunitionMode::Artemis {
                    2
                } else {
                    1
                }) as f64
        }
    }
    let mut bloodhound = 0_u64;
    for part in &loadout.systems {
        total += match part.system {
            System::Case => 50000.0,
            System::CaseIi => 175000.0,
            System::BeagleProbe => 100000.0,
            System::LightProbe => 50000.0,
            System::BloodhoundProbe => {
                bloodhound += 1;
                0.0
            }
            System::ArtemisIv => {
                if flag_vehicle(template, "ArtemisV_Tech") {
                    250000.0
                } else {
                    100000.0
                }
            }
            System::AngelEcm => 375000.0,
            System::C3Master => 300000.0,
            System::C3Slave => 250000.0,
            System::C3i => 375000.0,
            System::Ecm => {
                if flag_vehicle(template, "WatchDog_Tech") {
                    500000.0
                } else {
                    100000.0
                }
            }
            System::Tag => 50000.0,
            System::TargetingComputer => 10000.0,
            System::ShoulderOrHip
            | System::UpperActuator
            | System::LowerActuator
            | System::HandOrFootActuator
            | System::LifeSupport
            | System::Sensors
            | System::Cockpit
            | System::Engine
            | System::Gyro
            | System::HeatSink
            | System::JumpJet
            | System::FerroFibrous
            | System::LightFerroFibrous
            | System::EndoSteel
            | System::TripleStrengthMyomer
            | System::StealthArmor
            | System::LaserReflective => 0.0,
            _ => Part::parse(
                &template.sections[&part.location.section].criticals[&part.location.slot].equipment,
            )
            .ok()
            .and_then(|part| part_price(prices, part.part_id).ok())
            .unwrap_or(0) as f64,
        };
    }
    total += (bloodhound / 3 * 500000) as f64;
    let modifier = match movement {
        VehicleMovement::Tracked => 1.0 + tons as f64 / 100.0,
        VehicleMovement::Wheeled => 1.0 + tons as f64 / 200.0,
        VehicleMovement::Hover => 1.0 + tons as f64 / 50.0,
        VehicleMovement::Vtol => 1.0 + tons as f64 / 30.0,
        VehicleMovement::Stationary => 1.0,
    };
    Ok((total * modifier) as u64)
}
fn flag_vehicle(template: &VehicleTemplate, name: &str) -> bool {
    ["specials", "specials2"]
        .iter()
        .filter_map(|key| template.attributes.get(*key))
        .any(|v| {
            v.split_ascii_whitespace()
                .any(|v| flag_spells_technology(v, name))
        })
}
