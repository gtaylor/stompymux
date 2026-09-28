//! Trusted scalar construction edits used by administrative adapters.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdministrativeRepairKind {
    Reattach,
    Part,
    Armor,
    RearArmor,
    Internal,
}

/// C mech_re_attach hull classification (mech_classification_state.c:12-18 and
/// mech_maintenance.c:501-514): dropship hulls never read as destroyed,
/// aerospace hulls need only armor loss, and ground hulls need both exhausted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReattachHull {
    Ground,
    Aerospace,
    Dropship,
}

impl ReattachHull {
    /// Whether the section reads as destroyed under this hull classification.
    pub(super) fn destroyed(self, armor: u16, internal: u16) -> bool {
        match self {
            Self::Ground => armor == 0 && internal == 0,
            Self::Aerospace => armor == 0,
            Self::Dropship => false,
        }
    }
}

pub fn apply_administrative_repair(
    world: &mut World,
    id: ObjectId,
    section_code: i32,
    kind: AdministrativeRepairKind,
    value: u16,
) -> Result<()> {
    let class = administrative_unit_class(world, id)
        .and_then(|value| RawUnitClass::parse(&value).ok())
        .context("Unit class is unavailable")?;
    let movement = administrative_unit_movement(world, id)
        .and_then(|value| RawMovement::parse(&value).ok())
        .context("Unit movement is unavailable")?;
    let hull = match class {
        RawUnitClass::AeroFighter => ReattachHull::Aerospace,
        RawUnitClass::AerodyneDropship | RawUnitClass::SpheroidDropship => ReattachHull::Dropship,
        _ => ReattachHull::Ground,
    };
    let ordinal = RawSectionCode::for_unit(class, movement)
        .iter()
        .position(|section| *section as i32 == section_code)
        .context("Section is not valid for this unit")?;
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        let section = administrative_mech_section(unit, section_code)?;
        unit.apply_immediate_repair(section, kind, value, hull)?;
    } else {
        let vehicles = &mut world.btech.vehicles;
        let unit = vehicles
            .get_mut(&id)
            .context("Unit runtime state is unavailable")?;
        const PHYSICAL: [BattleVehicleSection; 6] = [
            BattleVehicleSection::Left,
            BattleVehicleSection::Right,
            BattleVehicleSection::Front,
            BattleVehicleSection::Rear,
            BattleVehicleSection::Turret,
            BattleVehicleSection::Rotor,
        ];
        if let Some(section) = PHYSICAL
            .get(ordinal)
            .copied()
            .filter(|section| unit.definition().sections.contains_key(section))
        {
            unit.apply_immediate_repair(section, kind, value, hull)?;
        } else {
            let section = unit
                .administrative_raw_mut(class, movement)
                .extra_sections
                .entry(ordinal)
                .or_default();
            apply_raw_section_repair(section, kind, value, hull)?;
        }
    }
    Ok(())
}

fn apply_raw_section_repair(
    section: &mut AdministrativeRawSection,
    kind: AdministrativeRepairKind,
    value: u16,
    hull: ReattachHull,
) -> Result<()> {
    match kind {
        AdministrativeRepairKind::Armor => section.current.armor = value,
        AdministrativeRepairKind::Internal => section.current.internal = value,
        AdministrativeRepairKind::RearArmor => section.current.rear = value,
        AdministrativeRepairKind::Reattach => {
            if hull.destroyed(section.current.armor, section.current.internal) {
                section.current.internal = if hull == ReattachHull::Aerospace {
                    1
                } else {
                    section.definition.internal
                };
            }
        }
        AdministrativeRepairKind::Part => {
            // C mech_repair_part succeeds silently on empty slots.
            if let Some(critical) = section.definition.criticals.get_mut(&(value as u8)) {
                critical.modes.retain(|mode| {
                    !matches!(
                        mode.as_str(),
                        "Destroyed"
                            | "Disabled"
                            | "Broken"
                            | "Damaged"
                            | "OneShot_Used"
                            | "Jettisoned"
                            | "RocketFired"
                    )
                });
                critical.brand = critical.brand.map(|brand| brand % 16);
                let equipment = critical.equipment.as_str();
                if equipment.starts_with("Ammo_")
                    || equipment.starts_with("IS.")
                    || equipment.starts_with("CL.")
                {
                    critical.data = "0".into();
                }
            }
        }
    }
    Ok(())
}

pub fn set_administrative_heat_sinks(world: &mut World, id: ObjectId, count: u16) -> Result<()> {
    super::ensure_registered_unit_runtime(world, id)?;
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        unit.set_administrative_heat_sinks(count);
    } else {
        world
            .btech
            .vehicles
            .get_mut(&id)
            .context("Unit runtime state is unavailable")?
            .set_administrative_heat_sinks(count);
    }
    Ok(())
}

pub fn set_administrative_armor(
    world: &mut World,
    id: ObjectId,
    section_code: i32,
    armor: Option<u16>,
    internal: Option<u16>,
    rear: Option<u16>,
) -> Result<()> {
    super::ensure_registered_unit_runtime(world, id)?;
    let class = administrative_unit_class(world, id)
        .and_then(|value| RawUnitClass::parse(&value).ok())
        .context("Unit class is unavailable")?;
    let movement = administrative_unit_movement(world, id)
        .and_then(|value| RawMovement::parse(&value).ok())
        .context("Unit movement is unavailable")?;
    let ordinal = RawSectionCode::for_unit(class, movement)
        .iter()
        .position(|section| *section as i32 == section_code)
        .context("Section is not valid for this unit")?;
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        let section = administrative_mech_section(unit, section_code)?;
        unit.set_administrative_armor(section, armor, internal, rear);
    } else {
        let vehicles = &mut world.btech.vehicles;
        let unit = vehicles
            .get_mut(&id)
            .context("Unit runtime state is unavailable")?;
        const PHYSICAL: [BattleVehicleSection; 6] = [
            BattleVehicleSection::Left,
            BattleVehicleSection::Right,
            BattleVehicleSection::Front,
            BattleVehicleSection::Rear,
            BattleVehicleSection::Turret,
            BattleVehicleSection::Rotor,
        ];
        if let Some(section) = PHYSICAL
            .get(ordinal)
            .copied()
            .filter(|section| unit.definition().sections.contains_key(section))
        {
            unit.set_administrative_armor(section, armor, internal, rear);
        } else {
            let raw = unit.administrative_raw_mut(class, movement);
            let section = raw.extra_sections.entry(ordinal).or_default();
            if let Some(value) = armor {
                section.definition.armor = value;
                section.current.armor = value;
            }
            if let Some(value) = internal {
                section.definition.internal = value;
                section.current.internal = value;
            }
            if let Some(value) = rear {
                section.definition.rear = value;
                section.current.rear = value;
            }
        }
    }
    Ok(())
}

fn administrative_mech_section(unit: &BattleUnit, code: i32) -> Result<BattleSection> {
    let class = unit
        .administrative_attribute("administrative_unit_type")
        .unwrap_or("Mech");
    if class == "Mech"
        && unit
            .administrative_attribute("administrative_movement_type")
            .is_none()
    {
        return inspection_section(unit.definition(), code);
    }
    let codes: &[i32] = match class {
        "Mech" if unit.administrative_attribute("administrative_movement_type") == Some("Quad") => {
            &[0, 1, 2, 3, 4, 5, 6, 7]
        }
        "Mech" => &[8, 9, 2, 3, 4, 10, 11, 7],
        "Mechwarrior" => &[8, 9, 2, 3, 4, 10, 11, 7],
        "Battlesuit" => &[12, 13, 14, 15, 16, 17, 18, 19],
        "Vehicle" | "Naval" => &[20, 21, 22, 23, 24],
        "VTOL" => &[20, 21, 22, 23, 24, 25],
        "AeroFighter" => &[26, 27, 28, 23],
        "Aerodyne_DropShip" => &[28, 27, 29, 30, 31, 26],
        "Spheroid_DropShip" => &[32, 33, 34, 35, 31, 26],
        _ => &[],
    };
    let index = codes
        .iter()
        .position(|candidate| *candidate == code)
        .context("Section is not valid for this unit")?;
    const RAW: [BattleSection; 8] = [
        BattleSection::LeftArm,
        BattleSection::RightArm,
        BattleSection::LeftTorso,
        BattleSection::RightTorso,
        BattleSection::CenterTorso,
        BattleSection::LeftLeg,
        BattleSection::RightLeg,
        BattleSection::Head,
    ];
    RAW.get(index).copied().context("Section is unavailable")
}

fn storage(world: &mut World, id: ObjectId) -> Result<AdministrativeStorage<'_>> {
    if world.btech.constructed.contains_key(&id) {
        return Ok(AdministrativeStorage::Mech(
            world.btech.constructed.get_mut(&id).expect("checked unit"),
        ));
    }
    Ok(AdministrativeStorage::Vehicle(
        world
            .btech
            .vehicles
            .get_mut(&id)
            .context("Unit runtime state is unavailable")?,
    ))
}

enum AdministrativeStorage<'a> {
    Mech(&'a mut BattleUnit),
    Vehicle(&'a mut BattleVehicle),
}

impl AdministrativeStorage<'_> {
    fn attribute(&mut self, name: &str, value: impl ToString) {
        match self {
            Self::Mech(unit) => unit.set_administrative_attribute(name, value),
            Self::Vehicle(unit) => unit.set_administrative_attribute(name, value),
        }
    }

    fn raw_identity(&mut self, class: RawUnitClass, movement: RawMovement) {
        let count = RawSectionCode::for_unit(class, movement).len();
        match self {
            Self::Mech(unit) => {
                let raw = unit.administrative_raw_mut(class, movement);
                raw.class = class;
                raw.movement = movement;
                raw.extra_sections.retain(|index, _| *index >= 8);
                for index in 8..count {
                    raw.extra_sections.entry(index).or_default();
                }
            }
            Self::Vehicle(unit) => {
                const PHYSICAL: [BattleVehicleSection; 6] = [
                    BattleVehicleSection::Left,
                    BattleVehicleSection::Right,
                    BattleVehicleSection::Front,
                    BattleVehicleSection::Rear,
                    BattleVehicleSection::Turret,
                    BattleVehicleSection::Rotor,
                ];
                let unrepresented: Vec<usize> = (0..8)
                    .filter(|index| {
                        *index >= PHYSICAL.len()
                            || !unit.definition().sections.contains_key(&PHYSICAL[*index])
                    })
                    .collect();
                let missing: Vec<usize> = unrepresented
                    .iter()
                    .copied()
                    .filter(|index| *index < count)
                    .collect();
                let raw = unit.administrative_raw_mut(class, movement);
                raw.class = class;
                raw.movement = movement;
                // Native class changes only reinterpret the eight raw section
                // ordinals; data hidden by the new class remains in storage.
                raw.extra_sections
                    .retain(|index, _| unrepresented.contains(index));
                for index in missing {
                    raw.extra_sections.entry(index).or_default();
                }
            }
        }
    }
}

pub fn set_administrative_assigned_pilot(
    world: &mut World,
    id: ObjectId,
    pilot: Option<ObjectId>,
) -> Result<()> {
    super::set_unit_configuration(world, id, |configuration| {
        configuration.assigned_pilot = pilot;
    });
    Ok(())
}

pub fn administrative_assigned_pilot(world: &World, id: ObjectId) -> Option<ObjectId> {
    super::unit_configuration(world, id).assigned_pilot
}

pub fn administrative_template_tonnage(
    attributes: &std::collections::BTreeMap<String, String>,
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
    attributes: &std::collections::BTreeMap<String, String>,
    fallback: BattleVehicleMovement,
) -> BattleVehicleMovement {
    attributes
        .get("administrative_movement_type")
        .and_then(|value| BattleVehicleMovement::parse(value).ok())
        .unwrap_or(fallback)
}

pub fn administrative_unit_tonnage(world: &World, id: ObjectId) -> Option<u32> {
    world
        .btech
        .constructed_units()
        .get(&id)
        .map(|unit| {
            administrative_template_tonnage(&unit.definition().attributes, unit.definition().tons)
        })
        .or_else(|| {
            world.btech.vehicles().get(&id).map(|unit| {
                administrative_template_tonnage(
                    &unit.definition().attributes,
                    unit.definition().tons,
                )
            })
        })
        .or_else(|| {
            world
                .btech
                .units()
                .get(&id)
                .and_then(|unit| u32::try_from(unit.tons).ok())
        })
}

pub fn administrative_unit_class(world: &World, id: ObjectId) -> Option<String> {
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        return Some(
            unit.administrative_attribute("administrative_unit_type")
                .unwrap_or("Mech")
                .to_owned(),
        );
    }
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Some(
            unit.administrative_attribute("administrative_unit_type")
                .unwrap_or(if unit.definition().is_vtol() {
                    "VTOL"
                } else {
                    "Vehicle"
                })
                .to_owned(),
        );
    }
    (world.btech.registrations().get(&id).map(String::as_str) == Some("MECH"))
        .then(|| "Mech".to_owned())
}

pub fn administrative_unit_movement(world: &World, id: ObjectId) -> Option<String> {
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        return Some(
            unit.administrative_attribute("administrative_movement_type")
                .unwrap_or_else(|| match unit.chassis() {
                    BattleMechChassis::Biped => "Biped",
                    BattleMechChassis::Quad => "Quad",
                })
                .to_owned(),
        );
    }
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Some(
            unit.administrative_attribute("administrative_movement_type")
                .unwrap_or(if unit.definition().is_vtol() {
                    "VTOL"
                } else {
                    "Track"
                })
                .to_owned(),
        );
    }
    (world.btech.registrations().get(&id).map(String::as_str) == Some("MECH"))
        .then(|| "Biped".to_owned())
}

pub fn administrative_section_valid(world: &World, id: ObjectId, code: i32) -> bool {
    let Some(class) = administrative_unit_class(world, id) else {
        return false;
    };
    match class.as_str() {
        "Mech" => match administrative_unit_movement(world, id).as_deref() {
            Some("Quad") => matches!(code, 0..=7),
            _ => matches!(code, 2 | 3 | 4 | 7 | 8 | 9 | 10 | 11),
        },
        "Vehicle" | "Naval" => matches!(code, 20..=24),
        "VTOL" => matches!(code, 20..=25),
        "Battlesuit" => matches!(code, 12..=19),
        "Mechwarrior" => matches!(code, 2 | 3 | 4 | 7 | 8 | 9 | 10 | 11),
        "AeroFighter" => matches!(code, 23 | 26 | 27 | 28),
        "Aerodyne_DropShip" => matches!(code, 26..=31),
        "Spheroid_DropShip" => matches!(code, 26 | 31..=35),
        _ => false,
    }
}

pub fn administrative_section_info(
    world: &World,
    id: ObjectId,
    code: i32,
) -> Option<(bool, usize)> {
    let class = administrative_unit_class(world, id)?;
    administrative_section_valid(world, id, code).then(|| {
        let rear = class.as_str() == "Mech" && matches!(code, 2..=4);
        // C crits_in_loc (mech_identity.c:135-152): mech heads and legs carry six
        // rows, quad mech arms six, mechwarrior sections two, others twelve.
        let slots = match class.as_str() {
            "Mech" => {
                let quad = administrative_unit_movement(world, id).as_deref() == Some("Quad");
                match code {
                    5..=7 => 6,
                    0 | 1 if quad => 6,
                    _ => 12,
                }
            }
            "Mechwarrior" => 2,
            _ => 12,
        };
        (rear, slots)
    })
}

/// Native repair fixability projected through the currently configured raw unit class.
pub fn administrative_is_fixable(world: &World, id: ObjectId) -> Option<bool> {
    let class = administrative_unit_class(world, id)?;
    // C mech_section_is_destroyed (mech_equipment_state.c:282-291) needs zero
    // armor and, for ground classes, zero internal; aerospace hulls count armor
    // loss alone and dropship hulls never read as destroyed.
    let destroyed = |armor: u16, internal: u16| match class.as_str() {
        "AeroFighter" => armor == 0,
        "Aerodyne_DropShip" | "Spheroid_DropShip" => false,
        _ => armor == 0 && internal == 0,
    };
    let forbidden = match class.as_str() {
        "Mech" => &[4_usize][..],
        "Vehicle" => &[0, 1, 2, 3, 5, 6, 7][..],
        "VTOL" => &[0, 1, 2, 3, 4, 6, 7][..],
        // Native fixability has no destroyed-section restriction for these classes.
        _ => return Some(true),
    };
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        const PHYSICAL: [BattleSection; 8] = [
            BattleSection::LeftArm,
            BattleSection::RightArm,
            BattleSection::LeftTorso,
            BattleSection::RightTorso,
            BattleSection::CenterTorso,
            BattleSection::LeftLeg,
            BattleSection::RightLeg,
            BattleSection::Head,
        ];
        return Some(!forbidden.iter().copied().any(|index| {
            let Some(section) = PHYSICAL.get(index).copied() else {
                return false;
            };
            unit.definition()
                .sections
                .get(&section)
                .is_some_and(|original| original.internal > 0)
                && unit
                    .sections()
                    .get(&section)
                    .is_some_and(|current| destroyed(current.armor, current.internal))
        }));
    }
    if let Some(unit) = world.btech.vehicles().get(&id) {
        const PHYSICAL: [BattleVehicleSection; 6] = [
            BattleVehicleSection::Left,
            BattleVehicleSection::Right,
            BattleVehicleSection::Front,
            BattleVehicleSection::Rear,
            BattleVehicleSection::Turret,
            BattleVehicleSection::Rotor,
        ];
        return Some(!forbidden.iter().copied().any(|index| {
            if let Some(section) = PHYSICAL.get(index).copied() {
                return unit
                    .definition()
                    .sections
                    .get(&section)
                    .is_some_and(|original| original.internal > 0)
                    && unit
                        .sections()
                        .get(&section)
                        .is_some_and(|current| destroyed(current.armor, current.internal));
            }
            unit.administrative_raw()
                .and_then(|raw| raw.extra_sections.get(&index))
                .is_some_and(|section| {
                    section.definition.internal > 0
                        && destroyed(section.current.armor, section.current.internal)
                })
        }));
    }
    // A native registration owns a zero-initialized default Mech. With no positive
    // original internals, no destroyed section makes it unfixable.
    (world.btech.registrations().get(&id).map(String::as_str) == Some("MECH")).then_some(true)
}

/// Apply raw C administration fields without actor notifications or construction admission.
pub fn set_administrative_scalar(
    world: &mut World,
    id: ObjectId,
    field: &str,
    value: f64,
) -> Result<()> {
    super::ensure_registered_unit_runtime(world, id)?;
    let mut storage = storage(world, id)?;
    match (&mut storage, field) {
        (AdministrativeStorage::Mech(unit), "maxspeed") => {
            unit.propulsion.set(f64::from(value as f32 * 10.75_f32))
        }
        (AdministrativeStorage::Vehicle(unit), "maxspeed") => {
            unit.propulsion.set(f64::from(value as f32 * 10.75_f32))
        }
        (AdministrativeStorage::Mech(unit), "maxjumpspeed") => unit
            .propulsion
            .set_jump_raw(f64::from(value as f32 * 10.75_f32)),
        (AdministrativeStorage::Vehicle(unit), "maxjumpspeed") => {
            unit.propulsion
                .set_jump_raw(f64::from(value as f32 * 10.75_f32));
        }
        (_, "tons") => storage.attribute("administrative_tonnage", value as u32),
        (_, "lrsrange") => storage.attribute("lrs_range", value as u8),
        (_, "tacrange") => storage.attribute("tac_range", value as u8),
        (_, "scanrange") => storage.attribute("scan_range", value as u8),
        (AdministrativeStorage::Mech(unit), "radiorange") => {
            unit.hardware.radio_range = Some(value as u16)
        }
        (AdministrativeStorage::Vehicle(unit), "radiorange") => {
            unit.hardware.radio_range = Some(value as u16)
        }
        _ => anyhow::bail!("Unknown administrative scalar"),
    }
    Ok(())
}

pub fn set_administrative_radio_quality(
    world: &mut World,
    id: ObjectId,
    quality: u8,
) -> Result<()> {
    super::ensure_registered_unit_runtime(world, id)?;
    let configuration = match quality {
        1 => 2,
        2 => 4,
        3 => 5,
        4 => 24,
        5 => 27,
        _ => unreachable!(),
    };
    let range = match quality {
        1 => 64,
        2 => 80,
        3 => 100,
        4 => 120,
        5 => 140,
        _ => unreachable!(),
    };
    let mut storage = storage(world, id)?;
    match &mut storage {
        AdministrativeStorage::Mech(unit) => {
            unit.set_administrative_attribute("radio", quality);
            unit.hardware.radio_configuration = Some(configuration);
            unit.hardware.radio_range = Some(range);
        }
        AdministrativeStorage::Vehicle(unit) => {
            unit.set_administrative_attribute("radio", quality);
            unit.hardware.radio_configuration = Some(configuration);
            unit.hardware.radio_range = Some(range);
        }
    }
    Ok(())
}

pub fn set_administrative_cargo(
    world: &mut World,
    id: ObjectId,
    space: u32,
    maximum_tons: u8,
) -> Result<()> {
    super::ensure_registered_unit_runtime(world, id)?;
    let mut storage = storage(world, id)?;
    storage.attribute("cargo_space", space.saturating_mul(50));
    storage.attribute("carrier_maximum_tonnage", maximum_tons);
    Ok(())
}

pub fn set_administrative_unit_type(world: &mut World, id: ObjectId, code: i32) -> Result<()> {
    super::ensure_registered_unit_runtime(world, id)?;
    const TYPES: [&str; 9] = [
        "Mech",
        "Vehicle",
        "VTOL",
        "Naval",
        "Spheroid_DropShip",
        "AeroFighter",
        "Mechwarrior",
        "Aerodyne_DropShip",
        "Battlesuit",
    ];
    let class = RawUnitClass::parse(TYPES[code as usize])?;
    let current_movement = administrative_unit_movement(world, id)
        .and_then(|value| RawMovement::parse(&value).ok())
        .unwrap_or(RawMovement::Biped);
    let mut storage = storage(world, id)?;
    storage.attribute("administrative_unit_type", class.name());
    let movement = match code {
        0 | 8 => Some("Biped"),
        2 => Some("VTOL"),
        4 | 5 | 7 => Some("Fly"),
        _ => None,
    };
    let movement = movement
        .and_then(|value| RawMovement::parse(value).ok())
        .unwrap_or(current_movement);
    storage.attribute("administrative_movement_type", movement.name());
    storage.raw_identity(class, movement);
    Ok(())
}

pub fn set_administrative_movement_type(world: &mut World, id: ObjectId, code: i32) -> Result<()> {
    super::ensure_registered_unit_runtime(world, id)?;
    const MOVEMENT: [&str; 11] = [
        "Biped", "Track", "Wheel", "Hover", "VTOL", "Hull", "Foil", "Fly", "Quad", "Sub", "None",
    ];
    let class = administrative_unit_class(world, id)
        .and_then(|value| RawUnitClass::parse(&value).ok())
        .unwrap_or(RawUnitClass::Mech);
    let movement = RawMovement::parse(MOVEMENT[code as usize])?;
    let mut storage = storage(world, id)?;
    storage.attribute("administrative_movement_type", movement.name());
    storage.raw_identity(class, movement);
    Ok(())
}

pub(crate) fn administrative_technology(code: i32) -> Option<(&'static str, &'static str)> {
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

/// Attribute holding each technology group's flags, matching the native
/// primary/secondary/infantry flag words.
fn special_attribute(group: &str) -> &'static str {
    match group {
        "primary" => "specials",
        "secondary" => "specials2",
        _ => "infantry_specials",
    }
}

pub fn set_administrative_technology(
    world: &mut World,
    id: ObjectId,
    code: i32,
    enabled: bool,
) -> Result<()> {
    super::ensure_registered_unit_runtime(world, id)?;
    let (flag, group) = administrative_technology(code)
        .context("Technology gameplay is unavailable for this code")?;
    let attribute = special_attribute(group);
    let was_enabled = world
        .btech
        .constructed_units()
        .get(&id)
        .is_some_and(|unit| unit.definition().has_special(flag))
        || world.btech.vehicles().get(&id).is_some_and(|unit| {
            unit.definition()
                .attributes
                .get(attribute)
                .is_some_and(|value| {
                    value
                        .split_ascii_whitespace()
                        .any(|value| value.eq_ignore_ascii_case(flag))
                })
        });
    if !enabled && was_enabled && matches!(code, 0 | 4) {
        remove_administrative_systems(
            world,
            id,
            &[if code == 0 {
                BattleSystem::TripleStrengthMyomer
            } else {
                BattleSystem::Masc
            }],
            false,
        )?;
    }
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        unit.set_administrative_special(attribute, flag, enabled)
    } else {
        world
            .btech
            .vehicles
            .get_mut(&id)
            .context("Unit runtime state is unavailable")?
            .set_administrative_special(attribute, flag, enabled)
    }
    Ok(())
}

pub fn clear_administrative_technologies(
    world: &mut World,
    id: ObjectId,
    group: i32,
) -> Result<()> {
    super::ensure_registered_unit_runtime(world, id)?;
    let range = match group {
        0 => 0..=56,
        1 => 57..=66,
        _ => 0..=66,
    };
    if group != 1 {
        remove_administrative_systems(
            world,
            id,
            &[
                BattleSystem::TripleStrengthMyomer,
                BattleSystem::Masc,
                BattleSystem::Case,
            ],
            true,
        )?;
    }
    for code in range {
        if administrative_technology(code).is_some() {
            set_administrative_technology(world, id, code, false)?;
        }
    }
    Ok(())
}

fn remove_administrative_systems(
    world: &mut World,
    id: ObjectId,
    systems: &[BattleSystem],
    clear_case: bool,
) -> Result<()> {
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        let mut definition = unit.definition().clone();
        let mut touched = Vec::new();
        for (&section, layout) in &mut definition.sections {
            if clear_case
                && layout
                    .configuration
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case("Case"))
            {
                layout.configuration = None;
            }
            layout.criticals.retain(|&slot, critical| {
                let remove = BattleSystem::parse(&critical.equipment)
                    .is_ok_and(|system| systems.contains(&system));
                if remove {
                    touched.push(CriticalLocation { section, slot });
                }
                !remove
            });
        }
        if !touched.is_empty() || clear_case {
            // Administrative clears follow the C administrator's leniency:
            // units admitted through the contract loader keep that loader.
            unit.replace_construction_contract(definition, &touched)?;
        }
        return Ok(());
    }
    let unit = world
        .btech
        .vehicles
        .get_mut(&id)
        .context("Unit runtime state is unavailable")?;
    let mut definition = unit.definition().clone();
    let mut touched = Vec::new();
    for (&section, layout) in &mut definition.sections {
        if clear_case
            && layout
                .configuration
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case("Case"))
        {
            layout.configuration = None;
        }
        layout.criticals.retain(|&slot, critical| {
            let remove = BattleSystem::parse(&critical.equipment)
                .is_ok_and(|system| systems.contains(&system));
            if remove {
                touched.push(VehicleCriticalLocation { section, slot });
            }
            !remove
        });
    }
    if !touched.is_empty() || clear_case {
        unit.replace_construction_contract(definition, &touched)?;
    }
    Ok(())
}
