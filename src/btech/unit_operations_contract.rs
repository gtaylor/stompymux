//! Trusted unit administration primitives shared by the Lua contract surface.

use super::{
    AmmunitionMode, CriticalDefinition, FireMode, Mech, MechSection, MechTemplate, UnitTemplate,
    Vehicle, VehicleSection, VehicleTemplate,
};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

fn unit(world: &mut World, id: ObjectId) -> Result<&mut Mech> {
    world
        .btech
        .constructed
        .get_mut(&id)
        .context("unit runtime state is unavailable")
}

pub(crate) struct UnitDamageRequest<'a> {
    pub amount: u16,
    pub cluster_size: u16,
    pub direction: u8,
    pub critical: bool,
    pub unit_message: Option<&'a str>,
    pub map_message: Option<&'a str>,
}

/// Apply the C cluster-size and direction-code contract under one rollback checkpoint.
pub(crate) fn apply_unit_damage_action(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    request: UnitDamageRequest<'_>,
) -> Result<()> {
    scripts.atomic(|before| {
        super::scenario_damage::admit(before, ObjectId(1), id, i32::from(request.amount))?;
        if let Some(message) = request.unit_message.filter(|message| !message.is_empty()) {
            notify_unit_operation(scripts, id, message)?;
        }
        if let Some(message) = request.map_message.filter(|message| !message.is_empty()) {
            broadcast_unit_operation(scripts, id, message)?;
        }
        let mut remaining = request.amount;
        let mut notices = Vec::new();
        let mut private = Vec::new();
        let mut impacts = Vec::new();
        while remaining > 0 {
            let amount = remaining.min(request.cluster_size);
            let (random, arc, rear) = if request.direction < 16 {
                (
                    false,
                    super::HitArc::Front,
                    (8..16).contains(&request.direction),
                )
            } else {
                let group = ((request.direction - 1) & 3) + 1;
                let arc = match group {
                    1 => super::HitArc::Left,
                    2 => super::HitArc::Right,
                    3 => super::HitArc::Front,
                    _ => super::HitArc::Rear,
                };
                (true, arc, request.direction > 18)
            };
            let rules = super::FallRules::configured(config);
            let (impact, packet_notices) = if random {
                if before.btech.vehicles().contains_key(&id) {
                    let (impact, notices) = super::blast_damage::resolve_packet(
                        &mut scripts.world_mut(),
                        id,
                        super::blast_damage::MaterialPacket {
                            amount,
                            table: super::HitTable::Weapon,
                            arc,
                            character: before.objects[&id].flags.contains(crate::Flag::InCharacter),
                            attacker: Some(id),
                            class: super::DamageClass::Ordinary,
                        },
                        rear,
                        rules,
                    )?;
                    (Some(impact), notices)
                } else {
                    let (mut hit, dice) = {
                        let world = scripts.world();
                        let state = &world.btech.constructed_units()[&id];
                        let mut dice = state.dice.clone();
                        let roll = dice.generic_roll();
                        let hit = rules.hit.resolve(state, arc, roll, &mut dice)?;
                        (hit, dice)
                    };
                    hit.rear_armor = rear;
                    hit.through_armor_critical = request.critical;
                    scripts
                        .world_mut()
                        .btech
                        .constructed
                        .get_mut(&id)
                        .unwrap()
                        .dice = dice;
                    let report = super::impact::resolve_scenario_impact(
                        &mut scripts.world_mut(),
                        id,
                        hit,
                        amount,
                    )?;
                    let notices = report.notices.clone();
                    (Some(super::BlastImpact::Mech(report)), notices)
                }
            } else {
                if before.btech.vehicles().contains_key(&id) {
                    let sections = [
                        VehicleSection::Left,
                        VehicleSection::Right,
                        VehicleSection::Front,
                        VehicleSection::Rear,
                        VehicleSection::Turret,
                        VehicleSection::Rotor,
                    ];
                    let section = *sections
                        .get(usize::from(request.direction % 8))
                        .context("vehicle damage section is unavailable")?;
                    let report = super::vehicle_armor_damage::resolve_rear_followup_in_candidate(
                        &mut scripts.world_mut(),
                        id,
                        super::VehicleArmorHit {
                            damage_class: super::DamageClass::Ordinary,
                            section,
                            amount: u32::from(amount),
                            through_armor_critical: request.critical,
                            armor_piercing: None,
                        },
                        rear,
                        rules.vehicle_impact.criticals,
                        super::vehicle_internal_damage::DamageContext {
                            attacker: Some(id),
                            ..Default::default()
                        },
                    )?;
                    super::piloting::append_feedback(
                        &mut private,
                        report.pilot_notices.iter().cloned(),
                        notices.len(),
                    );
                    let packet_notices = report
                        .notices
                        .iter()
                        .chain(&report.broadcasts)
                        .cloned()
                        .collect();
                    (None, packet_notices)
                } else {
                    let section = MechSection::ALL[usize::from(request.direction % 8)];
                    let report = super::impact::resolve_scenario_impact(
                        &mut scripts.world_mut(),
                        id,
                        super::Hit {
                            section,
                            rear_armor: rear,
                            through_armor_critical: request.critical,
                            crew_stun: false,
                        },
                        amount,
                    )?;
                    let notices = report.notices.clone();
                    (Some(super::BlastImpact::Mech(report)), notices)
                }
            };
            if let Some(impact) = &impact {
                impact.append_feedback(&mut private, notices.len());
            }
            notices.extend(packet_notices);
            impacts.extend(impact);
            remaining -= amount;
        }
        super::piloting::publish_ordered_notices(scripts, &notices, &private)?;
        super::evacuation::publish_blast_consequences(scripts, config, &impacts, None)?;
        super::evacuation::publish_new_casualties(scripts, config, before)?;
        scripts.world().validate_action(config)
    })
}

pub(crate) fn notify_unit_operation(
    scripts: &crate::Scripts,
    id: ObjectId,
    message: &str,
) -> Result<()> {
    super::notify_unit_text(scripts, id, message)
}

pub(crate) fn broadcast_unit_operation(
    scripts: &crate::Scripts,
    id: ObjectId,
    message: &str,
) -> Result<()> {
    for (recipient, text) in super::observer_messages(&scripts.world(), id, message) {
        super::notify_unit_text(scripts, recipient, &text)?;
    }
    Ok(())
}

fn edit(
    world: &mut World,
    id: ObjectId,
    touched: Vec<super::CriticalLocation>,
    change: impl FnOnce(&mut MechTemplate) -> Result<()>,
) -> Result<()> {
    let mut definition = unit(world, id)?.definition().clone();
    change(&mut definition)?;
    unit(world, id)?.replace_construction_contract(definition, &touched)
}

fn vehicle(world: &mut World, id: ObjectId) -> Result<&mut Vehicle> {
    world
        .btech
        .vehicles
        .get_mut(&id)
        .context("unit runtime state is unavailable")
}
fn edit_vehicle(
    world: &mut World,
    id: ObjectId,
    touched: Vec<super::VehicleCriticalLocation>,
    change: impl FnOnce(&mut VehicleTemplate) -> Result<()>,
) -> Result<()> {
    let mut definition = vehicle(world, id)?.definition().clone();
    change(&mut definition)?;
    vehicle(world, id)?.replace_construction_contract(definition, &touched)
}

/// Snapshot the reference finalize's geometry-derived technology bits into the
/// loaded definition: flippable arms follow the arm actuator layout, and a
/// compact engine is any center-torso installation below four criticals.
fn finalize_load_specials(mech: &mut MechTemplate) {
    use MechSection::{LeftArm, RightArm};
    let actuator = |section: MechSection, slot: u8| {
        mech.sections
            .get(&section)
            .and_then(|layout| layout.criticals.get(&slot))
            .is_some_and(|critical| {
                critical.equipment.eq_ignore_ascii_case("LowerActuator")
                    || critical
                        .equipment
                        .eq_ignore_ascii_case("HandOrFootActuator")
            })
    };
    let flippable = !actuator(LeftArm, 2)
        && !actuator(RightArm, 2)
        && !actuator(LeftArm, 3)
        && !actuator(RightArm, 3);
    let compact = mech
        .sections
        .get(&MechSection::CenterTorso)
        .is_none_or(|layout| {
            layout
                .criticals
                .values()
                .filter(|critical| critical.equipment.eq_ignore_ascii_case("Engine"))
                .count()
                < 4
        });
    super::edit_special(&mut mech.attributes, "specials", "FlipArms", flippable);
    if compact {
        super::edit_special(&mut mech.attributes, "specials", "CompactEngine_Tech", true);
    }
}

pub(crate) fn load_unit_template(
    world: &mut World,
    id: ObjectId,
    requested_reference: &str,
    definition: UnitTemplate,
) -> Result<()> {
    // The reference load finalize derives two technology bits from the loaded
    // geometry (FLIPABLE_ARMS reset by actuator layout, CE_TECH for compact
    // engine installations) and stores them until the next template load.
    let mut definition = definition;
    if let UnitTemplate::Mech(mech) = &mut definition {
        finalize_load_specials(mech);
    }
    let configured = world.btech.constructed_units().contains_key(&id)
        || world.btech.vehicles().contains_key(&id);
    match definition {
        UnitTemplate::Mech(definition) if world.btech.constructed_units().contains_key(&id) => {
            let communications = {
                let state = unit(world, id)?;
                (state.definition().reference == requested_reference)
                    .then(|| (state.radio.clone(), state.tics.clone()))
            };
            let mut replacement = Mech::from_contract_template(definition)?;
            if let Some((radio, tics)) = communications {
                replacement.radio = radio;
                replacement.tics = tics;
            }
            world.btech.units.insert(id, replacement.identity());
            *unit(world, id)? = replacement;
        }
        UnitTemplate::Vehicle(definition) if world.btech.vehicles().contains_key(&id) => {
            let communications = {
                let state = vehicle(world, id)?;
                (state.definition().reference == requested_reference)
                    .then(|| (state.radio.clone(), state.tics.clone()))
            };
            let mut replacement = Vehicle::new_contract(definition)?;
            if let Some((radio, tics)) = communications {
                replacement.radio = radio;
                replacement.tics = tics;
            }
            world.btech.units.insert(id, replacement.identity());
            *vehicle(world, id)? = replacement;
        }
        definition if !configured => {
            ensure!(
                world.btech.registrations().get(&id).map(String::as_str) == Some("MECH"),
                "unit is not registered"
            );
            super::inventory_mass(world, id)?;
            match definition {
                UnitTemplate::Mech(definition) => {
                    let unit = Mech::from_contract_template(definition)?;
                    world.btech.units.insert(id, unit.identity());
                    world.btech.constructed.insert(id, unit);
                }
                UnitTemplate::Vehicle(definition) => {
                    let unit = Vehicle::new_contract(definition)?;
                    world.btech.units.insert(id, unit.identity());
                    world.btech.vehicles.insert(id, unit);
                }
            }
        }
        _ => anyhow::bail!("template class does not match unit"),
    }
    Ok(())
}

pub(crate) fn unit_piloting_check_action(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    roll_modifier: i32,
    damage_modifier: i32,
) -> Result<bool> {
    let check = super::piloting::roll_piloting_i32(
        &mut scripts.world_mut(),
        id,
        roll_modifier,
        config.battletech.extended_piloting != 0,
    )?;
    if check.success {
        return Ok(true);
    }
    let placed = {
        let world = scripts.world();
        world
            .btech
            .constructed_units()
            .get(&id)
            .and_then(Mech::position)
            .or_else(|| world.btech.vehicles().get(&id).and_then(Vehicle::position))
            .is_some()
    };
    if placed {
        broadcast_unit_operation(scripts, id, "falls down!")?;
    }
    let rules = super::FallRules::configured(config);
    if scripts.world().btech.vehicles().contains_key(&id) {
        let report = super::evacuation::vehicle_fall_contract_action(
            scripts,
            config,
            id,
            damage_modifier,
            rules,
        )?;
        drop(report);
    } else {
        let report = super::evacuation::fall_unit_contract_action(
            scripts,
            config,
            id,
            damage_modifier,
            rules,
        )?;
        drop(report);
    }
    Ok(false)
}

pub(crate) fn reset_unit_criticals(world: &mut World, id: ObjectId) -> Result<()> {
    if world.btech.vehicles().contains_key(&id) {
        let touched = [
            VehicleSection::Left,
            VehicleSection::Right,
            VehicleSection::Front,
            VehicleSection::Rear,
            VehicleSection::Turret,
            VehicleSection::Rotor,
        ]
        .into_iter()
        .flat_map(|section| {
            (0..12).map(move |slot| super::VehicleCriticalLocation { section, slot })
        })
        .collect();
        return edit_vehicle(world, id, touched, |definition| {
            for layout in definition.sections.values_mut() {
                layout.criticals.clear();
            }
            Ok(())
        });
    }
    let touched = MechSection::ALL
        .into_iter()
        .flat_map(|section| (0..12).map(move |slot| super::CriticalLocation { section, slot }))
        .collect();
    edit(world, id, touched, |definition| {
        let chassis = definition.chassis()?;
        for section in MechSection::ALL {
            let criticals = &mut definition
                .sections
                .get_mut(&section)
                .context("unit section is unavailable")?
                .criticals;
            criticals.clear();
            let mut put = |slot: u8, equipment: &str| {
                criticals.insert(
                    slot,
                    CriticalDefinition {
                        equipment: equipment.into(),
                        data: "-".into(),
                        modes: Vec::new(),
                    },
                );
            };
            use MechSection::*;
            match section {
                Head => {
                    for (slot, part) in [
                        (0, "LifeSupport"),
                        (1, "Sensors"),
                        (2, "Cockpit"),
                        (4, "Sensors"),
                        (5, "LifeSupport"),
                    ] {
                        put(slot, part);
                    }
                }
                CenterTorso => {
                    for (slot, part) in [
                        (0, "Engine"),
                        (1, "Engine"),
                        (2, "Engine"),
                        (3, "Gyro"),
                        (4, "Gyro"),
                        (5, "Gyro"),
                        (6, "Gyro"),
                        (7, "Engine"),
                        (8, "Engine"),
                        (9, "Engine"),
                    ] {
                        put(slot, part);
                    }
                }
                LeftArm | RightArm | LeftLeg | RightLeg => {
                    for (slot, part) in [
                        (0, "ShoulderOrHip"),
                        (1, "UpperActuator"),
                        (2, "LowerActuator"),
                        (3, "HandOrFootActuator"),
                    ] {
                        put(slot, part);
                    }
                    for slot in chassis.critical_slots(section)..12 {
                        criticals.remove(&slot);
                    }
                }
                LeftTorso | RightTorso => {}
            }
        }
        Ok(())
    })
}

pub(crate) fn install_vehicle_weapon_named(
    world: &mut World,
    id: ObjectId,
    equipment: &str,
    section: VehicleSection,
    slot: u8,
    modes: Vec<String>,
) -> Result<()> {
    edit_vehicle(
        world,
        id,
        vec![super::VehicleCriticalLocation { section, slot }],
        |definition| {
            definition
                .sections
                .get_mut(&section)
                .context("unit section is unavailable")?
                .criticals
                .insert(
                    slot,
                    CriticalDefinition {
                        equipment: equipment.into(),
                        data: "-".into(),
                        modes,
                    },
                );
            Ok(())
        },
    )
}

pub(crate) fn install_unit_weapon_named(
    world: &mut World,
    id: ObjectId,
    equipment: &str,
    section: MechSection,
    slots: &[u8],
    modes: Vec<String>,
) -> Result<()> {
    let touched = slots
        .iter()
        .map(|&slot| super::CriticalLocation { section, slot })
        .collect();
    edit(world, id, touched, |definition| {
        let layout = definition
            .sections
            .get_mut(&section)
            .context("unit section is unavailable")?;
        let critical = CriticalDefinition {
            equipment: equipment.into(),
            data: "-".into(),
            modes,
        };
        for &slot in slots {
            layout.criticals.insert(slot, critical.clone());
        }
        Ok(())
    })
}

pub(crate) fn configure_unit_ammunition(
    world: &mut World,
    id: ObjectId,
    weapon: super::Weapon,
    section: MechSection,
    slot: u8,
    half_ton: bool,
    modes: Vec<String>,
) -> Result<()> {
    edit(
        world,
        id,
        vec![super::CriticalLocation { section, slot }],
        |definition| {
            let capacity =
                u16::from(weapon.profile().ammunition_per_ton) / if half_ton { 2 } else { 1 };
            let mut flags = modes;
            if half_ton {
                flags.push("Halfton".into());
            }
            definition
                .sections
                .get_mut(&section)
                .context("unit section is unavailable")?
                .criticals
                .insert(
                    slot,
                    CriticalDefinition {
                        equipment: format!("Ammo_{}", weapon.name()),
                        data: capacity.to_string(),
                        modes: flags,
                    },
                );
            Ok(())
        },
    )
}

pub(crate) fn configure_vehicle_ammunition(
    world: &mut World,
    id: ObjectId,
    weapon: super::Weapon,
    section: VehicleSection,
    slot: u8,
    half_ton: bool,
    modes: Vec<String>,
) -> Result<()> {
    edit_vehicle(
        world,
        id,
        vec![super::VehicleCriticalLocation { section, slot }],
        |definition| {
            let mut flags = modes;
            if half_ton {
                flags.push("Halfton".into())
            }
            let capacity =
                u16::from(weapon.profile().ammunition_per_ton) / if half_ton { 2 } else { 1 };
            definition
                .sections
                .get_mut(&section)
                .context("unit section is unavailable")?
                .criticals
                .insert(
                    slot,
                    CriticalDefinition {
                        equipment: format!("Ammo_{}", weapon.name()),
                        data: capacity.to_string(),
                        modes: flags,
                    },
                );
            Ok(())
        },
    )
}

pub(crate) fn restock_unit_ammunition(
    world: &mut World,
    id: ObjectId,
    section: MechSection,
    slot: u8,
) -> Result<()> {
    let state = unit(world, id)?;
    let loadout = state.loadout()?;
    let index = loadout
        .ammunition
        .iter()
        .position(|bin| bin.location.section == section && bin.location.slot == slot)
        .context("critical slot is not ammunition")?;
    let location = loadout.ammunition[index].location;
    let authored_destroyed = state.definition().sections[&location.section].criticals
        [&location.slot]
        .modes
        .iter()
        .any(|mode| mode == "Destroyed");
    ensure!(
        !state.critical_destroyed(location) && !authored_destroyed,
        "destroyed ammunition cannot be restocked"
    );
    state.ammunition[index] = loadout.ammunition[index].capacity;
    state.live_mass.invalidate();
    Ok(())
}

pub(crate) fn restock_vehicle_ammunition(
    world: &mut World,
    id: ObjectId,
    section: VehicleSection,
    slot: u8,
) -> Result<()> {
    let state = vehicle(world, id)?;
    let loadout = state.loadout()?;
    let index = loadout
        .ammunition
        .iter()
        .position(|bin| bin.location.section == section && bin.location.slot == slot)
        .context("critical slot is not ammunition")?;
    let location = loadout.ammunition[index].location;
    let authored_destroyed = state.definition().sections[&location.section].criticals
        [&location.slot]
        .modes
        .iter()
        .any(|mode| mode == "Destroyed");
    ensure!(
        !state.critical_destroyed(location) && !authored_destroyed,
        "destroyed ammunition cannot be restocked"
    );
    state.ammunition[index] = loadout.ammunition[index].capacity;
    state.live_mass.invalidate();
    Ok(())
}

pub(crate) fn set_unit_weapon_modes(
    world: &mut World,
    id: ObjectId,
    number: usize,
    fire: FireMode,
    ammunition: AmmunitionMode,
    fire_names: Vec<String>,
    ammunition_names: Vec<String>,
) -> Result<()> {
    let state = world
        .btech
        .unit_mut(id)
        .context("unit runtime state is unavailable")?;
    super::with_unit_mut!(state, |state| {
        ensure!(
            number < state.loadout()?.weapons.len(),
            "weapon number is not mounted"
        );
        state.set_contract_weapon_mode_names(number, fire_names, ammunition_names)?;
        if fire == FireMode::Normal {
            state.fire_modes.remove(&number);
        } else {
            state.fire_modes.insert(number, fire);
        }
        if ammunition == AmmunitionMode::Normal {
            state.ammunition_modes.remove(&number);
        } else {
            state.ammunition_modes.insert(number, ammunition);
        }
        Ok(())
    })
}

pub(crate) fn install_unit_special(
    world: &mut World,
    id: ObjectId,
    equipment: Option<String>,
    section: MechSection,
    slot: u8,
    data: i32,
) -> Result<()> {
    edit(
        world,
        id,
        vec![super::CriticalLocation { section, slot }],
        |definition| {
            let criticals = &mut definition
                .sections
                .get_mut(&section)
                .context("unit section is unavailable")?
                .criticals;
            if let Some(equipment) = equipment {
                criticals.insert(
                    slot,
                    CriticalDefinition {
                        equipment,
                        data: data.to_string(),
                        modes: Vec::new(),
                    },
                );
            } else {
                criticals.remove(&slot);
            }
            Ok(())
        },
    )
}

pub(crate) fn install_vehicle_special(
    world: &mut World,
    id: ObjectId,
    equipment: Option<String>,
    section: VehicleSection,
    slot: u8,
    data: i32,
) -> Result<()> {
    edit_vehicle(
        world,
        id,
        vec![super::VehicleCriticalLocation { section, slot }],
        |definition| {
            let criticals = &mut definition
                .sections
                .get_mut(&section)
                .context("unit section is unavailable")?
                .criticals;
            if let Some(equipment) = equipment {
                criticals.insert(
                    slot,
                    CriticalDefinition {
                        equipment,
                        data: data.to_string(),
                        modes: Vec::new(),
                    },
                );
            } else {
                criticals.remove(&slot);
            }
            Ok(())
        },
    )
}
