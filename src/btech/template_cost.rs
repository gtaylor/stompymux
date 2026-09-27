//! Source-derived FASA construction cost for supported BattleMech templates.
use super::*;
use crate::World;
use anyhow::Result;
const WEAPON_COST: [u64; 178] = [
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
];
const AMMO_COST: [u64; 178] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2000, 20000, 2000, 9000, 12000, 20000,
    1000, 500, 1000, 1000, 9000, 12000, 20000, 10000, 75000, 75000, 75000, 75000, 30000, 30000,
    30000, 30000, 6000, 27000, 27000, 27000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1000, 4500, 6000, 10000, 1000, 1000, 2000, 20000, 20000, 20000, 2000, 9000, 12000, 20000, 3000,
    12000, 1000, 9000, 12000, 20000, 1000, 6000, 4500, 2000, 2000, 5000, 20000, 15000, 10000, 1000,
    10000, 27000, 27000, 27000, 27000, 30000, 30000, 30000, 30000, 27000, 27000, 27000, 5000, 5000,
    5000, 5000, 6000, 7500, 54000, 54000, 54000, 0, 0, 0, 50000, 50000, 50000, 50000, 0, 0, 0, 0,
    0, 0, 0, 1000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
fn flag(template: &BattleTemplate, name: &str) -> bool {
    ["specials", "specials2"]
        .iter()
        .filter_map(|key| template.attributes.get(*key))
        .any(|v| {
            v.split_ascii_whitespace()
                .any(|v| v.eq_ignore_ascii_case(name))
        })
        || (0..=56).any(|code| {
            admin_contract::administrative_technology(code).is_some_and(|(candidate, _)| {
                candidate.eq_ignore_ascii_case(name)
                    && inspection::inspection_template_inferred_technology(template, code)
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
                .any(|value| value.eq_ignore_ascii_case(name))
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
    if let Some(weapon) = BattleWeapon::from_part_id(id) {
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
    BattleWeapon::from_part_id(id)
        .is_some_and(|weapon| weapon.gunnery_skill(true) == "Gunnery-Laser")
        || matches!(id, 175 | 176)
}

fn raw_ammunition_cost(template: &RawTemplate) -> Result<f64> {
    let mut total = 0.0;
    for section in RawSectionCode::for_unit(template.class, template.movement) {
        for row in inspection::inspect_raw_template_criticals(template, *section)? {
            let Some(part) = row.part else { continue };
            if !(193..=370).contains(&part.id) {
                continue;
            }
            let weapon_id = part.id - 192;
            let Some((_, maximum)) = row.ammunition else {
                continue;
            };
            let Some(weapon) = BattleWeapon::from_part_id(weapon_id) else {
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

fn raw_equipment_cost(world: &World, template: &RawTemplate) -> Result<f64> {
    let mut total = 0.0;
    let mut bloodhound = 0_u64;
    for definition in template.sections.values() {
        for critical in definition.criticals.values() {
            let Some(part) = BattlePart::parse(&critical.equipment).ok() else {
                continue;
            };
            if matches!(
                part.kind,
                BattlePartKind::Weapon | BattlePartKind::Ammunition
            ) {
                continue;
            }
            total += if let Ok(system) = BattleSystem::parse(&critical.equipment) {
                match system {
                    BattleSystem::Case => 50000.0,
                    BattleSystem::BeagleProbe => 100000.0,
                    BattleSystem::LightProbe => 50000.0,
                    BattleSystem::BloodhoundProbe => {
                        bloodhound += 1;
                        0.0
                    }
                    BattleSystem::ArtemisIv => 100000.0,
                    BattleSystem::AngelEcm => 375000.0,
                    BattleSystem::C3Master => 300000.0,
                    BattleSystem::C3Slave => 250000.0,
                    BattleSystem::C3i => 375000.0,
                    BattleSystem::Ecm => 100000.0,
                    BattleSystem::Tag => 50000.0,
                    BattleSystem::TargetingComputer => 10000.0,
                    BattleSystem::Axe => 5000.0,
                    BattleSystem::ShoulderOrHip
                    | BattleSystem::UpperActuator
                    | BattleSystem::LowerActuator
                    | BattleSystem::HandOrFootActuator
                    | BattleSystem::LifeSupport
                    | BattleSystem::Sensors
                    | BattleSystem::Cockpit
                    | BattleSystem::Engine
                    | BattleSystem::Gyro
                    | BattleSystem::HeatSink
                    | BattleSystem::JumpJet
                    | BattleSystem::FerroFibrous
                    | BattleSystem::LightFerroFibrous
                    | BattleSystem::EndoSteel
                    | BattleSystem::TripleStrengthMyomer
                    | BattleSystem::StealthArmor
                    | BattleSystem::Masc
                    | BattleSystem::Sword => 0.0,
                    _ => part_cost(world, part.part_id)? as f64,
                }
            } else if part.part_id == 427 {
                175000.0
            } else if (443..=445).contains(&part.part_id) {
                0.0
            } else {
                part_cost(world, part.part_id)? as f64
            };
        }
    }
    Ok(total + (bloodhound / 3 * 500000) as f64)
}

fn raw_mech_template(template: &RawTemplate) -> Result<BattleTemplate> {
    anyhow::ensure!(
        template.class == RawUnitClass::Mech,
        "template is not a Mech"
    );
    let movement = template.movement;
    let mut sections = std::collections::BTreeMap::new();
    for (ordinal, section) in BattleSection::ALL.into_iter().enumerate() {
        let raw = RawSectionCode::from_ordinal(template.class, movement, ordinal)
            .and_then(|code| template.sections.get(&code))
            .cloned()
            .unwrap_or_default();
        sections.insert(section, raw);
    }
    Ok(BattleTemplate {
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

fn raw_vehicle_base_cost(world: &World, template: &RawTemplate) -> Result<u64> {
    let tons = u64::try_from(template.tons)?;
    let weapons = inspection::inspect_raw_template_weapons(template)?;
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
    let (rating, suspension) = inspection::inspect_raw_template_engine(template);
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
    let armor = inspection::inspect_raw_template_armor(template, None)?
        .armor
        .1;
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
    total += raw_equipment_cost(world, template)?;
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
pub fn raw_template_base_cost(world: &World, template: &RawTemplate) -> Result<u64> {
    match template.class {
        RawUnitClass::Mech => template_base_cost(world, &raw_mech_template(template)?),
        RawUnitClass::Vehicle | RawUnitClass::Vtol | RawUnitClass::Naval => {
            raw_vehicle_base_cost(world, template)
        }
        RawUnitClass::BattleSuit => {
            let weapons: u64 = inspection::inspect_raw_template_weapons(template)?
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
                + raw_equipment_cost(world, template)?;
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
type InspectionFlags = (inspection::InspectionPart, bool, bool);

/// Build the combat-supported portion of a template while retaining every
/// catalogue part that native inspection and economy code can price without a
/// combat implementation. Structural split links remain in the clone because
/// the supported weapon resolver consumes them.
fn inspection_loadout(template: &BattleTemplate) -> Result<(BattleLoadout, Vec<InspectionFlags>)> {
    let normalized = inspection::inspection_compatible_template(template);
    if let Ok(loadout) = BattleLoadout::resolve(&normalized) {
        return Ok((loadout, Vec::new()));
    }
    let mut compatible = normalized;
    let mut raw = Vec::new();
    for definition in compatible.sections.values_mut() {
        definition.criticals.retain(|_, critical| {
            if critical.equipment.eq_ignore_ascii_case("SplitCrit_Left")
                || critical.equipment.eq_ignore_ascii_case("SplitCrit_Right")
                || BattleSystem::parse(&critical.equipment).is_ok()
                || BattleWeapon::parse(&critical.equipment).is_ok()
                || super::equipment::strip_name_prefix(&critical.equipment, "Ammo_")
                    .is_some_and(|name| BattleWeapon::parse(name).is_ok())
            {
                return true;
            }
            if let Some(part) =
                inspection::inspection_template_part(&critical.equipment, critical.brand)
            {
                raw.push(part);
            }
            false
        });
    }
    Ok((BattleLoadout::resolve(&compatible)?, raw))
}
/// Calculate the legacy MaxTech/FASA construction estimate from pristine material.
pub fn template_base_cost(world: &World, template: &BattleTemplate) -> Result<u64> {
    let (loadout, raw_parts) = inspection_loadout(template)?;
    let tons = u64::from(super::administrative_template_tonnage(
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
            BattleSection::LeftArm | BattleSection::RightArm
        );
        let leg = template.chassis()?.is_leg(part.location.section);
        total += match part.system {
            BattleSystem::UpperActuator if arm => (tons * 100) as f64,
            BattleSystem::LowerActuator if arm => (tons * 50) as f64,
            BattleSystem::HandOrFootActuator if arm => (tons * 80) as f64,
            BattleSystem::UpperActuator if leg => (tons * 150) as f64,
            BattleSystem::LowerActuator if leg => (tons * 80) as f64,
            BattleSystem::HandOrFootActuator if leg => (tons * 120) as f64,
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
    let normalized_jump = (inspection::inspection_normalized_jump_speed(template) as f32
        * 0.093_023_3_f32)
        .trunc() as u64;
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
                * if bin.mode == BattleAmmunitionMode::Artemis {
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
            BattleSystem::Masc => masc += 1,
            BattleSystem::Sword => sword = true,
            BattleSystem::Case => total += 50000.0,
            BattleSystem::BeagleProbe => total += 100000.0,
            BattleSystem::LightProbe => total += 50000.0,
            BattleSystem::BloodhoundProbe => bloodhound += 1,
            BattleSystem::ArtemisIv => total += 100000.0,
            BattleSystem::AngelEcm => total += 375000.0,
            BattleSystem::C3Master => total += 300000.0,
            BattleSystem::C3Slave => total += 250000.0,
            BattleSystem::C3i => total += 375000.0,
            BattleSystem::Ecm => total += 100000.0,
            BattleSystem::Tag => total += 50000.0,
            BattleSystem::TargetingComputer => total += 10000.0,
            BattleSystem::Axe => total += 5000.0,
            BattleSystem::ShoulderOrHip
            | BattleSystem::UpperActuator
            | BattleSystem::LowerActuator
            | BattleSystem::HandOrFootActuator
            | BattleSystem::LifeSupport
            | BattleSystem::Sensors
            | BattleSystem::Cockpit
            | BattleSystem::Engine
            | BattleSystem::Gyro
            | BattleSystem::HeatSink
            | BattleSystem::JumpJet
            | BattleSystem::FerroFibrous
            | BattleSystem::LightFerroFibrous
            | BattleSystem::EndoSteel
            | BattleSystem::TripleStrengthMyomer
            | BattleSystem::StealthArmor => {}
            _ => {
                if let Ok(part_id) = BattlePart::parse(
                    &template.sections[&part.location.section].criticals[&part.location.slot]
                        .equipment,
                ) {
                    total += super::part_cost(world, part_id.part_id)? as f64;
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
            id => super::part_cost(world, id)? as f64,
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
            super::administrative_template_tonnage(&template.attributes, template.tons) * 1024 / 20,
        )) / 1024.0
            * 10000.0
    }
    if flag(template, "OmniMech_Tech") {
        total *= 1.25
    }
    Ok((total * (1.0 + tons as f64 / 100.0)) as u64)
}

/// Calculate the legacy vehicle/VTOL branch of the FASA construction estimate.
pub fn vehicle_template_base_cost(world: &World, template: &BattleVehicleTemplate) -> Result<u64> {
    let compatible = inspection::inspection_compatible_vehicle_template(template);
    let loadout = BattleVehicleLoadout::resolve(&compatible)?;
    let tons = u64::from(super::administrative_template_tonnage(
        &template.attributes,
        template.tons,
    ));
    let mut total = (tons * 1000 + tons * 500) as f64;
    let ice = flag_vehicle(template, "ICEEngine_Tech");
    let movement = super::administrative_template_movement(&template.attributes, template.movement);
    let mut turret_mass = 0u64;
    let mut amplifier_mass = 0u64;
    for mount in &loadout.weapons {
        if mount.criticals[0].section == BattleVehicleSection::Turret {
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
    if movement == BattleVehicleMovement::Hover {
        total += (2000 * tons) as f64
    }
    if movement == BattleVehicleMovement::Vtol {
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
                * if bin.mode == BattleAmmunitionMode::Artemis {
                    2
                } else {
                    1
                }) as f64
        }
    }
    let mut bloodhound = 0_u64;
    for part in &loadout.systems {
        total += match part.system {
            BattleSystem::Case => 50000.0,
            BattleSystem::BeagleProbe => 100000.0,
            BattleSystem::LightProbe => 50000.0,
            BattleSystem::BloodhoundProbe => {
                bloodhound += 1;
                0.0
            }
            BattleSystem::ArtemisIv => 100000.0,
            BattleSystem::AngelEcm => 375000.0,
            BattleSystem::C3Master => 300000.0,
            BattleSystem::C3Slave => 250000.0,
            BattleSystem::C3i => 375000.0,
            BattleSystem::Ecm => 100000.0,
            BattleSystem::Tag => 50000.0,
            BattleSystem::TargetingComputer => 10000.0,
            BattleSystem::ShoulderOrHip
            | BattleSystem::UpperActuator
            | BattleSystem::LowerActuator
            | BattleSystem::HandOrFootActuator
            | BattleSystem::LifeSupport
            | BattleSystem::Sensors
            | BattleSystem::Cockpit
            | BattleSystem::Engine
            | BattleSystem::Gyro
            | BattleSystem::HeatSink
            | BattleSystem::JumpJet
            | BattleSystem::FerroFibrous
            | BattleSystem::LightFerroFibrous
            | BattleSystem::EndoSteel
            | BattleSystem::TripleStrengthMyomer
            | BattleSystem::StealthArmor => 0.0,
            _ => BattlePart::parse(
                &template.sections[&part.location.section].criticals[&part.location.slot].equipment,
            )
            .ok()
            .and_then(|part| super::part_cost(world, part.part_id).ok())
            .unwrap_or(0) as f64,
        };
    }
    total += (bloodhound / 3 * 500000) as f64;
    let modifier = match movement {
        BattleVehicleMovement::Tracked => 1.0 + tons as f64 / 100.0,
        BattleVehicleMovement::Wheeled => 1.0 + tons as f64 / 200.0,
        BattleVehicleMovement::Hover => 1.0 + tons as f64 / 50.0,
        BattleVehicleMovement::Vtol => 1.0 + tons as f64 / 30.0,
        BattleVehicleMovement::Stationary => 1.0,
    };
    Ok((total * modifier) as u64)
}
fn flag_vehicle(template: &BattleVehicleTemplate, name: &str) -> bool {
    ["specials", "specials2"]
        .iter()
        .filter_map(|key| template.attributes.get(*key))
        .any(|v| {
            v.split_ascii_whitespace()
                .any(|v| v.eq_ignore_ascii_case(name))
        })
}
