//! Construction cost in C-bills for BattleMech, vehicle and raw templates, following the
//! TechManual cost rules (pp. 275-285) and equipment tables (pp. 290-294), with Tactical
//! Operations: Advanced Units & Equipment prices for advanced equipment.
//!
//! Component costs keep their fractions; only the final cost is rounded, up to the next whole
//! C-bill (TechManual p. 275 rounds up to the nearest 0.01 C-bill, which a whole-C-bill result
//! cannot represent). Ammunition is not part of a unit's cost (TechManual p. 283).
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

/// Omni technology multiplies a unit's cost before the final multiplier (TechManual p. 285).
const OMNI_MULTIPLIER: f64 = 1.25;

/// A one-shot launcher costs half its launcher (TechManual p. 291). Rocket launchers are built
/// one-shot and their listed price already reflects it.
fn weapon_mount_price<L>(mount: &WeaponMount<L>) -> f64 {
    let price = mount.weapon.cost() as f64;
    if mount.one_shot && !mount.weapon.is_rocket() {
        price * 0.5
    } else {
        price
    }
}

/// The final rounding: up to the next whole C-bill.
fn final_cost(total: f64) -> u64 {
    total.max(0.0).ceil() as u64
}

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

fn raw_weapon_cost(id: i32) -> f64 {
    Weapon::from_part_id(id).map_or(0.0, |weapon| weapon.cost() as f64)
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

/// Power amplifiers weigh a tenth of the energy weapons they feed and turrets a tenth of the
/// items they carry, each rounded up to the half ton (TechManual pp. 102-104); returns tons.
fn tenth_rounded_to_half_ton(mass: u64) -> f64 {
    let tenth = u32::try_from(mass.div_ceil(10)).unwrap_or(u32::MAX);
    f64::from(half(tenth)) / 1024.0
}

/// Price vehicle and raw-template equipment one entry per installation.
fn vehicle_system_price(
    prices: &PartPrices,
    system: System,
    context: SystemContext,
    equipment: &str,
) -> Result<f64> {
    if let Some(price) = system.installation_price(context) {
        return Ok(price);
    }
    Ok(Part::parse(equipment)
        .ok()
        .map(|part| part_price(prices, part.part_id))
        .transpose()?
        .unwrap_or(0) as f64)
}

fn raw_equipment_cost(
    prices: &PartPrices,
    template: &RawTemplate,
    context: SystemContext,
) -> Result<f64> {
    let mut total = 0.0;
    for definition in template.sections.values() {
        for critical in definition.criticals.values() {
            let Some(part) = Part::parse(&critical.equipment).ok() else {
                continue;
            };
            if matches!(part.kind, PartKind::Weapon | PartKind::Ammunition) {
                continue;
            }
            total += if let Some(system) = System::named(&critical.equipment) {
                vehicle_system_price(prices, system, context, &critical.equipment)?
            } else if (443..=445).contains(&part.part_id) {
                0.0
            } else {
                part_price(prices, part.part_id)? as f64
            };
        }
    }
    Ok(total)
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
        walk_mp: super::inspection_raw_walking_mp(template),
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
        total += 20000.0 * tenth_rounded_to_half_ton(amplifier_mass);
    }
    total += 5000.0 * tenth_rounded_to_half_ton(turret_mass);
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
    total += (engine_base * engine_size * tons) as f64 / 75.0;
    let clan = raw_flag(template, "Clan");
    // Vehicles mount single heat sinks whatever their technology base (TechManual p. 104).
    let double = raw_flag(template, "DoubleHS");
    let sinks = u64::try_from(template.heat_sinks.max(0))?;
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
        total += raw_weapon_cost(mount.part.id);
    }
    let context = SystemContext {
        clan,
        watchdog: raw_flag(template, "WatchDog_Tech"),
        tons: u16::try_from(tons).unwrap_or(u16::MAX),
        engine_rating: u16::try_from(rating.max(0)).unwrap_or(u16::MAX),
    };
    total += raw_equipment_cost(prices, template, context)?;
    if raw_flag(template, "OmniMech_Tech") {
        total *= OMNI_MULTIPLIER;
    }
    let modifier = match template.movement {
        RawMovement::Tracked => 1.0 + tons as f64 / 100.0,
        RawMovement::Wheeled | RawMovement::Hull => 1.0 + tons as f64 / 200.0,
        RawMovement::Hover | RawMovement::Submarine => 1.0 + tons as f64 / 50.0,
        RawMovement::Vtol => 1.0 + tons as f64 / 30.0,
        RawMovement::Foil => 1.0 + tons as f64 / 75.0,
        _ => 1.0,
    };
    Ok(final_cost(total * modifier))
}

/// Calculate native construction cost without requiring a combat runtime class.
pub fn raw_template_base_cost(prices: &PartPrices, template: &RawTemplate) -> Result<u64> {
    match template.class {
        RawUnitClass::Mech => template_base_cost(prices, &raw_mech_template(template)?),
        RawUnitClass::Vehicle | RawUnitClass::Vtol | RawUnitClass::Naval => {
            raw_vehicle_base_cost(prices, template)
        }
        RawUnitClass::BattleSuit => {
            let weapons: f64 = inspect_raw_template_weapons(template)?
                .into_iter()
                .map(|weapon| raw_weapon_cost(weapon.part.id))
                .sum();
            let clan = raw_flag(template, "Clan");
            let base = if clan { 3_500_000.0 } else { 2_400_000.0 };
            let context = SystemContext {
                clan,
                watchdog: raw_flag(template, "WatchDog_Tech"),
                tons: u16::try_from(template.tons.max(0)).unwrap_or(u16::MAX),
                engine_rating: 0,
            };
            let mut total = base + weapons + raw_equipment_cost(prices, template, context)?;
            if raw_flag(template, "OmniMech_Tech") {
                total *= OMNI_MULTIPLIER;
            }
            Ok(final_cost(total * 0.75))
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

/// Calculate a BattleMech's construction cost from pristine material (TechManual pp. 275-285).
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
    total += (engine_base * rating * tons) as f64 / 75.0;
    // Jump jets cost 200 (improved 500) x jump MP squared x tons (TechManual p. 278); each
    // installed jet gives one MP, and improved jets fill two slots per MP.
    let improved_jump = flag(template, "ImprovedJJ_Tech");
    let jets = loadout
        .systems
        .iter()
        .filter(|part| part.system == System::JumpJet)
        .count() as u64;
    let jump = if improved_jump { jets / 2 } else { jets };
    total += (tons * jump * jump * if improved_jump { 500 } else { 200 }) as f64;
    // A Clan Mech's sinks are double unless it declares single sinks (TechManual p. 52).
    let double = template.has_double_heat_sinks();
    let sinks = u64::from(template.heat_sinks);
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
        total += weapon_mount_price(mount);
    }
    for (part, is_weapon, _) in &raw_parts {
        if *is_weapon {
            total += raw_weapon_cost(part.id);
        }
    }
    let context = SystemContext {
        clan,
        watchdog: flag(template, "WatchDog_Tech"),
        tons: u16::try_from(tons).unwrap_or(u16::MAX),
        engine_rating: u16::try_from(rating).unwrap_or(u16::MAX),
    };
    for part in &loadout.systems {
        if let Some(price) = part
            .system
            .slot_price(context.for_installation(part.tech_base))
        {
            total += price;
            continue;
        }
        if let Ok(stock) = Part::parse(
            &template.sections[&part.location.section].criticals[&part.location.slot].equipment,
        ) {
            total += part_price(prices, stock.part_id)? as f64;
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
    // Clan CASE is built into every location that holds ammunition.
    if clan {
        let sections: std::collections::BTreeSet<_> = loadout
            .ammunition
            .iter()
            .map(|bin| bin.location.section)
            .collect();
        total += (sections.len() as f64) * 50000.0;
    }
    if flag(template, "OmniMech_Tech") {
        total *= OMNI_MULTIPLIER;
    }
    Ok(final_cost(total * (1.0 + tons as f64 / 100.0)))
}

/// Calculate a combat vehicle's construction cost (TechManual pp. 279 and 285).
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
        total += 20000.0 * tenth_rounded_to_half_ton(amplifier_mass);
    }
    total += 5000.0 * tenth_rounded_to_half_ton(turret_mass);
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
    total += (base * size * tons) as f64 / 75.0;
    // Combat vehicles mount single heat sinks only, whatever their technology base
    // (TechManual p. 104).
    let double = flag_vehicle(template, "DoubleHS");
    let sinks = u64::from(template.heat_sinks.unwrap_or(0));
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
        total += weapon_mount_price(mount);
    }
    let context = SystemContext {
        clan,
        watchdog: flag_vehicle(template, "WatchDog_Tech"),
        tons: u16::try_from(tons).unwrap_or(u16::MAX),
        engine_rating: u16::try_from(rating).unwrap_or(u16::MAX),
    };
    // Vehicle templates list each system once, whatever its BattleMech slot count.
    for part in &loadout.systems {
        let equipment =
            &template.sections[&part.location.section].criticals[&part.location.slot].equipment;
        total += vehicle_system_price(prices, part.system, context, equipment)?;
    }
    if flag_vehicle(template, "OmniMech_Tech") {
        total *= OMNI_MULTIPLIER;
    }
    let modifier = match movement {
        VehicleMovement::Tracked => 1.0 + tons as f64 / 100.0,
        VehicleMovement::Wheeled => 1.0 + tons as f64 / 200.0,
        VehicleMovement::Hover => 1.0 + tons as f64 / 50.0,
        VehicleMovement::Vtol => 1.0 + tons as f64 / 30.0,
        VehicleMovement::Stationary => 1.0,
    };
    Ok(final_cost(total * modifier))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn jenner() -> MechTemplate {
        MechTemplate::parse("JR7-D", include_str!("../tests/fixtures/JR7-D.toml")).unwrap()
    }

    /// The JR7-D priced by hand from TechManual pp. 277-285: structure 14,000, cockpit 200,000,
    /// life support 50,000, sensors and musculature 70,000 each, two upper arm actuators
    /// 7,000, two legs 24,500, a 3-ton gyro 900,000, the 245 engine 571,666.67, five jump jets
    /// 175,000, 4 tons of armor 40,000, four medium lasers 160,000 and an SRM-4 60,000. The
    /// ammunition is free (p. 283), and the 2,342,166.67 total times 1.35 rounds up to
    /// 3,161,925.
    #[test]
    fn jenner_costs_its_book_price() {
        assert_eq!(
            template_base_cost(&PartPrices::new(), &jenner()).unwrap(),
            3_161_925
        );
    }

    /// A Clan Mech that declares single heat sinks (TechManual p. 52) prices them as singles.
    #[test]
    fn clan_single_heat_sinks_cost_as_singles() {
        let clan = |sinks: &str| {
            let mut template = jenner();
            let specials = template.attributes.entry("specials".into()).or_default();
            specials.push_str(sinks);
            template.heat_sinks = 12;
            for section in template.sections.values_mut() {
                section
                    .criticals
                    .retain(|_, part| part.equipment != "HeatSink");
            }
            template_base_cost(&PartPrices::new(), &template).unwrap()
        };
        assert!(clan(" Clan SingleHS") < clan(" Clan"));
    }

    /// One-shot launchers cost half their launcher (TechManual p. 291), and equipment prices
    /// follow the book whatever the slot layout.
    #[test]
    fn one_shot_launchers_and_equipment_follow_the_price_table() {
        let base = template_base_cost(&PartPrices::new(), &jenner()).unwrap() as f64;
        let mut one_shot = jenner();
        one_shot
            .sections
            .get_mut(&MechSection::CenterTorso)
            .unwrap()
            .criticals
            .get_mut(&10)
            .unwrap()
            .modes
            .push("OneShot".into());
        let cost = template_base_cost(&PartPrices::new(), &one_shot).unwrap() as f64;
        assert!(
            (base - cost - 30_000.0 * 1.35).abs() <= 1.0,
            "{base} {cost}"
        );
        let mut beagle = jenner();
        let torso = beagle.sections.get_mut(&MechSection::LeftTorso).unwrap();
        for slot in [4, 5] {
            torso.criticals.insert(
                slot,
                CriticalDefinition {
                    equipment: "BeagleProbe".into(),
                    data: "-".into(),
                    modes: Vec::new(),
                },
            );
        }
        let cost = template_base_cost(&PartPrices::new(), &beagle).unwrap() as f64;
        assert!(
            (cost - base - 200_000.0 * 1.35).abs() <= 1.0,
            "{base} {cost}"
        );
    }
}
