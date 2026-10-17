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

/// Price of the armor the flags `has` names: its tonnage, converted from `points` without
/// truncating and rounded up to the half ton exactly as its mass is (TechManual pp. 274-275,
/// 278), times its price per ton. Points per ton follow the armor type (TechManual p. 56),
/// with eight for hardened armor (Tactical Operations: Advanced Units & Equipment p. 92), and
/// `clan_armor` picks Clan ferro-fibrous. Hardened and light ferro-fibrous armor cost 15,000
/// a ton (ibid. p. 217; TechManual p. 279).
fn armor_price(points: u32, has: impl Fn(&str) -> bool, clan_armor: bool) -> f64 {
    let denominator = if has("FerroFibrous_Tech") {
        if clan_armor { 60 } else { 56 }
    } else if has("HvyFerroFibrous_Tech") {
        62
    } else if has("LtFerroFibrous_Tech") {
        53
    } else if has("HardenedArmor_Tech") {
        25
    } else {
        50
    };
    let per_ton = if has("FerroFibrous_Tech") {
        20000
    } else if has("StealthArmor_Tech") {
        50000
    } else if has("LaserRefArmor_Tech") {
        30000
    } else if has("HardenedArmor_Tech") || has("LtFerroFibrous_Tech") {
        15000
    } else if has("HvyFerroFibrous_Tech") {
        25000
    } else {
        10000
    };
    f64::from(armor_mass(points, denominator)) / 1024.0 * f64::from(per_ton)
}

/// The engine's price per point of `rating` per ton of unit, before the division by 75: ICE
/// 1,250, standard fusion 5,000, compact 10,000, light 15,000, XL 20,000 and XXL 100,000
/// (TechManual p. 277; Tactical Operations: Advanced Units & Equipment p. 219), doubled for a
/// large engine rated above 400 (ibid. p. 219). An internal combustion engine is never a
/// fusion family.
fn engine_price_factor(has: impl Fn(&str) -> bool, ice: bool, rating: u64) -> u64 {
    let base = if ice {
        1250
    } else if has("CompactEngine_Tech") {
        10000
    } else if has("LightEngine_Tech") {
        15000
    } else if has("XLEngine_Tech") {
        20000
    } else if has("XXL_Tech") {
        100000
    } else {
        5000
    };
    if rating > 400 { base * 2 } else { base }
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
    let engine_size = u64::try_from(i64::from(rating) - i64::from(suspension))?;
    let engine_base = engine_price_factor(|name| raw_flag(template, name), ice, engine_size);
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
    total += armor_price(armor, |name| raw_flag(template, name), clan);
    for mount in &weapons {
        total += raw_weapon_cost(mount.part.id);
    }
    let context = SystemContext {
        clan,
        watchdog: raw_flag(template, "WatchDog_Tech"),
        tons: u16::try_from(tons).unwrap_or(u16::MAX),
        engine_rating: u16::try_from(rating.max(0)).unwrap_or(u16::MAX),
        legs: 0,
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
                legs: 0,
            };
            let mut total = base + weapons + raw_equipment_cost(prices, template, context)?;
            if raw_flag(template, "OmniMech_Tech") {
                total *= OMNI_MULTIPLIER;
            }
            Ok(final_cost(total * 0.75))
        }
        RawUnitClass::MechWarrior => Ok(0),
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
    // A torso-mounted cockpit costs 750,000 C-bills (Tactical Operations: Advanced Units &
    // Equipment p. 218).
    total += if flag(template, "SmallCockpit_Tech") {
        175000.0
    } else if flag(template, "TorsoCockpit_Tech") {
        750000.0
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
    // A declared internal combustion engine takes precedence over the fusion family the slot
    // inventory suggests: a center torso with fewer than four engine slots infers a compact
    // fusion engine, but an ICE is never one (TechManual p. 277; `Construction::from_flags`
    // resolves the same way).
    let engine_base = engine_price_factor(
        |name| flag(template, name),
        flag(template, "ICEEngine_Tech"),
        rating,
    );
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
    let armor: u32 = template
        .sections
        .values()
        .map(|s| u32::from(s.armor) + u32::from(s.rear))
        .sum();
    total += armor_price(armor, |name| flag(template, name), template.clan_armor());
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
        legs: if template.chassis()? == super::MechChassis::Quad {
            4
        } else {
            2
        },
    };
    for part in &loadout.systems {
        if let Some(price) = part.system.slot_price(context.for_part(part)) {
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
    // The turret and power amplifiers are priced on the tonnage construction gives them: the
    // turret a tenth of every item in it, the amplifiers a tenth of the energy weapons an ICE
    // vehicle feeds, each rounded up to the half ton (TechManual pp. 106, 279).
    let mass = compatible.mass()?;
    total += 20000.0 * f64::from(mass.power_amplifiers) / 1024.0;
    total += 5000.0 * f64::from(mass.turret) / 1024.0;
    if movement == VehicleMovement::Hover {
        total += (2000 * tons) as f64
    }
    if movement == VehicleMovement::Vtol {
        total += (4000 * tons) as f64
    }
    let (rating, suspension) = inspection_vehicle_engine_rating(template)?;
    let size = u64::try_from((i64::from(rating) - i64::from(suspension)).max(0))?;
    let base = engine_price_factor(|name| flag_vehicle(template, name), ice, size);
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
    let armor: u32 = template
        .sections
        .values()
        .filter(|section| section.internal > 0)
        .map(|s| u32::from(s.armor) + u32::from(s.rear))
        .sum();
    let clan = flag_vehicle(template, "Clan");
    total += armor_price(
        armor,
        |name| flag_vehicle(template, name),
        template.clan_armor(),
    );
    for mount in &loadout.weapons {
        total += weapon_mount_price(mount);
    }
    let context = SystemContext {
        clan,
        watchdog: flag_vehicle(template, "WatchDog_Tech"),
        tons: u16::try_from(tons).unwrap_or(u16::MAX),
        engine_rating: u16::try_from(rating).unwrap_or(u16::MAX),
        legs: 0,
    };
    // Vehicle templates list each system once, whatever its BattleMech slot count. A targeting
    // computer is priced per ton it weighs (TechManual pp. 238, 293).
    for part in &loadout.systems {
        let equipment =
            &template.sections[&part.location.section].criticals[&part.location.slot].equipment;
        let installations = if part.system == System::TargetingComputer {
            f64::from(compatible.system_mass(&loadout, part) / 1024)
        } else {
            1.0
        };
        total += installations * vehicle_system_price(prices, part.system, context, equipment)?;
    }
    // Transport bays beyond the free standard cargo bay (TechManual p. 293).
    total += template.transport_bays()?.cost();
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

    /// The server's PARITY template fixture (`tests/fixtures/btech/units/PARITY.toml`).
    const PARITY: &str = r#"name = "Parity Probe"
class = "mech"
movement = "biped"
tons = 75
walk_mp = 4
heat_sinks = 10

[construction]
tech_base = "inner_sphere"
engine = "ice"
engine_rating = 300

[sections.left_arm]
armor = 10
internals = 5
explicit = true
slots = [
    { at = "1-3", item = "IS.PPC" },
]

[sections.right_arm]
armor = 11
internals = 6
explicit = true

[sections.left_torso]
armor = 12
internals = 7
rear = 3

[sections.right_torso]
armor = 13
internals = 8
rear = 4

[sections.center_torso]
armor = 20
internals = 12
rear = 5
explicit = true

[sections.left_leg]
armor = 14
internals = 9
explicit = true

[sections.right_leg]
armor = 15
internals = 10
explicit = true

[sections.head]
armor = 9
explicit = true
"#;

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

    /// The PARITY fixture: an ICE Mech whose center torso lists no engine slots, which would
    /// otherwise infer a compact fusion engine, prices its declared ICE. By hand from TechManual
    /// pp. 277-285: structure 400 x 75 = 30,000, cockpit 200,000, life support 50,000, sensors
    /// and musculature 2,000 x 75 = 150,000 each, a 3-ton gyro 900,000, the 300 ICE
    /// 1,250 x 300 x 75 / 75 = 375,000, ten sinks with a non-fusion engine 20,000, 116 armor
    /// points (7.5 tons) 75,000 and a PPC 200,000. It lists no actuators. The 2,150,000 total
    /// times 1.75 is 3,762,500.
    #[test]
    fn declared_ice_engine_outranks_an_inferred_compact_engine() {
        let template = MechTemplate::parse("PARITY", PARITY).unwrap();
        assert!(flag(&template, "CompactEngine_Tech"));
        assert_eq!(
            template_base_cost(&PartPrices::new(), &template).unwrap(),
            3_762_500
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

    /// Armor is priced on the tonnage it weighs: the points convert to tons without
    /// truncating before the half-ton round-up (TechManual pp. 274-275), and hardened armor
    /// carries eight points a ton at 15,000 C-bills a ton (Tactical Operations: Advanced Units &
    /// Equipment pp. 92, 217).
    #[test]
    fn armor_is_priced_on_its_mounted_tonnage() {
        let ferro = |name: &str| name == "FerroFibrous_Tech";
        // 144 Inner Sphere ferro-fibrous points weigh 8.04 tons, built as 8.5.
        assert_eq!(armor_price(144, ferro, false), 8.5 * 20_000.0);
        // 173 points weigh 9.65 tons, 9.5 had the points been truncated first.
        assert_eq!(armor_price(173, ferro, false), 10.0 * 20_000.0);
        // 77 Clan points weigh 4.01 tons.
        assert_eq!(armor_price(77, ferro, true), 4.5 * 20_000.0);
        let hardened = |name: &str| name == "HardenedArmor_Tech";
        assert_eq!(armor_price(80, hardened, false), 10.0 * 15_000.0);
        assert_eq!(armor_price(64, |_| false, false), 4.0 * 10_000.0);
    }

    /// A large engine, rated above 400, costs twice its family's price (Tactical Operations:
    /// Advanced Units & Equipment p. 219).
    #[test]
    fn large_engines_cost_double() {
        for (flag, ice, normal) in [
            ("", true, 1250),
            ("", false, 5000),
            ("LightEngine_Tech", false, 15000),
            ("XLEngine_Tech", false, 20000),
            ("XXL_Tech", false, 100000),
        ] {
            let has = |name: &str| name == flag;
            assert_eq!(engine_price_factor(has, ice, 400), normal, "{flag}");
            assert_eq!(engine_price_factor(has, ice, 405), normal * 2, "{flag}");
        }
        let mut large = jenner();
        large.tons = 85;
        large.walk_mp = 5;
        let mut standard = large.clone();
        standard.walk_mp = 4;
        let difference = template_base_cost(&PartPrices::new(), &large).unwrap() as f64
            - template_base_cost(&PartPrices::new(), &standard).unwrap() as f64;
        // 85 x 5 = 425 at 10,000 against 85 x 4 = 340 at 5,000, and a five-ton gyro against a
        // four-ton one, before the 1.85 multiplier.
        let engines = (10_000.0 * 425.0 * 85.0 - 5_000.0 * 340.0 * 85.0) / 75.0;
        let gyro = 300_000.0;
        assert!(
            (difference - (engines + gyro) * 1.85).abs() <= 1.0,
            "{difference}"
        );
    }

    /// A 75-ton ICE tank with two turret AC/10s, after the TechManual's Zhukov (pp. 96-107).
    const TANK: &str = r#"
name = "Tank"
class = "vehicle"
movement = "track"
tons = 75
walk_mp = 3

[construction]
engine = "ice"

[sections.left_side]
armor = 32

[sections.right_side]
armor = 32

[sections.front_side]
armor = 44

[sections.aft_side]
armor = 24

[sections.turret]
armor = 44
slots = [
    { at = "1-2", item = "IS.AC/10" },
]
"#;

    /// The turret is priced on its whole tonnage, a tenth of every item in it, and an ICE
    /// vehicle's power amplifiers on theirs, a tenth of every weapon they feed, plasma rifles
    /// included (TechManual pp. 106, 279).
    #[test]
    fn vehicle_turrets_and_amplifiers_are_priced_on_their_tonnage() {
        let cost = |source: &str| {
            vehicle_template_base_cost(
                &PartPrices::new(),
                &VehicleTemplate::parse("Tank", source).unwrap(),
            )
            .unwrap() as f64
        };
        let with = |section: &str, item: &str| {
            TANK.replace(
                &format!("[sections.{section}]\narmor = 44\n"),
                &format!(
                    "[sections.{section}]\narmor = 44\nslots = [{{ at = 5, item = \"{item}\" }}]\n"
                ),
            )
        };
        // A Beagle probe beside the two turret AC/10s raises the turret's items from 24 to 25.5
        // tons, and its mechanism from 2.5 to 3 tons: 2,500 more, times the tracked
        // multiplier of 1.75.
        let turret = TANK.replace(
            "{ at = \"1-2\", item = \"IS.AC/10\" },",
            "{ at = \"1-2\", item = \"IS.AC/10\" },\n    { at = 3, item = \"BeagleProbe\" },",
        );
        let front = with("front_side", "BeagleProbe");
        assert!((cost(&turret) - cost(&front) - 2_500.0 * 1.75).abs() <= 1.0);
        // A plasma rifle's 6 tons need a ton of amplifiers at 20,000 C-bills.
        let plasma = with("front_side", "IS.PlasmaRifle");
        let rifle = Weapon::PlasmaRifle.cost() as f64;
        assert!((cost(&plasma) - cost(TANK) - (rifle + 20_000.0) * 1.75).abs() <= 1.0);
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
