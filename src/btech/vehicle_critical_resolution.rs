//! Vehicle critical selection and the Total Warfare critical hit effects (pp. 194–195 and 197),
//! committed together or leaving no changes.
use super::vehicle_internal_damage::DamageContext;
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

/// Applied critical with occupant feedback staged for the enclosing attack transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish critical notices with the enclosing attack transaction"]
pub struct VehicleCriticalResolution {
    pub selection: VehicleCriticalReport,
    /// Engine-loss landing check when terrain permits an attempt, retained for host diagnostics.
    pub emergency_landing: Option<PilotingCheck>,
    /// The immediate Driving Skill Roll a VTOL Pilot Hit forces.
    pub control_check: Option<PilotingCheck>,
    pub notices: Vec<Notice>,
    /// Pilot-only control feedback indexed into the damage notice stream.
    pub pilot_notices: Vec<PilotNotice>,
    pub pilot_injury: Option<TacticalPilotInjury>,
    /// Direct character injury; nested explosions retain their own ordered reports.
    pub character_injury: Option<CharacterPilotInjury>,
    pub internal_damage: Vec<VehicleInternalDamage>,
    pub ammunition_cascade: Option<VehicleAmmunitionCascade>,
    pub explosion: Option<VehicleExplosion>,
    /// Visibility-filtered broadcasts owed by the enclosing attack, not global messages.
    pub broadcasts: Vec<Notice>,
    /// Whether the vehicle was a wreck once the critical and its cascades had resolved.
    pub unit_destroyed: bool,
}

/// Resolve selection and its consequences atomically. Unsupported character casualties
/// fail without consuming dice; callers must not treat errors as a harmless hit.
pub fn resolve_vehicle_critical(
    world: &mut World,
    id: ObjectId,
    section: VehicleSection,
    rules: VehicleCriticalRules,
) -> Result<VehicleCriticalResolution> {
    world.attempt(|world| {
        let report = resolve_in_candidate(world, id, section, rules, DamageContext::default())?;
        Ok(report)
    })
}

/// Resolve a nested critical in an already isolated attack candidate.
pub(super) fn resolve_in_candidate(
    candidate: &mut World,
    id: ObjectId,
    section: VehicleSection,
    rules: VehicleCriticalRules,
    context: DamageContext,
) -> Result<VehicleCriticalResolution> {
    resolve_modified_in_candidate(candidate, id, section, rules, context, 0)
}

/// Resolve a critical whose table roll carries an armor-piercing, inferno or hardened-armor modifier.
pub(super) fn resolve_modified_in_candidate(
    candidate: &mut World,
    id: ObjectId,
    section: VehicleSection,
    rules: VehicleCriticalRules,
    context: DamageContext,
    modifier: i8,
) -> Result<VehicleCriticalResolution> {
    ensure!(
        context.depth < 64,
        "Vehicle critical cascade limit exceeded"
    );
    let selection = roll_vehicle_critical_modified(candidate, id, section, rules, modifier)?;
    let mut resolution = VehicleCriticalResolution {
        selection: selection.clone(),
        emergency_landing: None,
        control_check: None,
        notices: Vec::new(),
        pilot_notices: Vec::new(),
        pilot_injury: None,
        character_injury: None,
        internal_damage: Vec::new(),
        ammunition_cascade: None,
        explosion: None,
        broadcasts: Vec::new(),
        unit_destroyed: candidate.btech.vehicles()[&id].is_destroyed(),
    };
    let Some(effect) = selection.effect else {
        return Ok(resolution);
    };
    resolution.notices.push(Notice {
        unit: id,
        text: "[fg=yellow bold]CRITICAL HIT![reset]".into(),
    });
    use VehicleCriticalEffect as E;
    match effect {
        E::CrewKilled => kill_crew(candidate, id, context, &mut resolution)?,
        E::FuelTank => explode_fuel_tank(candidate, id, context, &mut resolution)?,
        E::Ammunition => {
            explode_ammunition(candidate, id, section, rules, context, &mut resolution)?
        }
        E::WeaponDestroyed => {
            let loss = destroy_weapon(candidate, id, section, rules, context)?;
            loss.append(&mut resolution);
        }
        E::TurretBlownOff => blow_off_turret(candidate, id, context, &mut resolution)?,
        E::Engine => engine_hit(candidate, id, rules, &mut resolution)?,
        E::Rotor(rotor) => {
            let report =
                super::rotor_damage::apply_in_world(candidate, id, rotor, rules.extended_piloting)?;
            resolution.notices.push(Notice {
                unit: id,
                text: report.message().into(),
            });
            resolution.emergency_landing = report.landing;
            super::vehicle_impact::append_landing(
                candidate,
                id,
                report,
                &mut resolution.notices,
                &mut resolution.pilot_notices,
            );
            if report.lost_rotor && candidate.btech.vehicles()[&id].is_destroyed() {
                resolution.broadcasts.push(Notice {
                    unit: id,
                    text: "'s rotor is blown apart!".into(),
                });
            }
        }
        E::VtolPilot => pilot_hit(candidate, id, rules, context, &mut resolution)?,
        E::Driver | E::VtolCopilot | E::Commander | E::CrewStunned | E::Sensors => {
            crew_hit(candidate, id, effect, context, &mut resolution)?
        }
        E::Stabilizer | E::WeaponJam | E::TurretJam | E::TurretLock => {
            apply(candidate, id, &selection, effect, &mut resolution.notices)?
        }
        E::Cargo => bail!(
            "Cargo/Infantry Hit selected without a cargo bay; the attack must remain uncommitted"
        ),
    }
    resolution.unit_destroyed = candidate.btech.vehicles()[&id].is_destroyed();
    Ok(resolution)
}

/// Crew Killed: the vehicle remains intact but is destroyed for every game purpose.
fn kill_crew(
    candidate: &mut World,
    id: ObjectId,
    context: DamageContext,
    resolution: &mut VehicleCriticalResolution,
) -> Result<()> {
    let vehicle = candidate.btech.vehicles.get_mut(&id).unwrap();
    let pilot = vehicle.pilot();
    let was_destroyed = vehicle.is_destroyed();
    vehicle.kill_crew();
    let destroyed = vehicle.is_destroyed();
    super::kill_counters::transition(candidate, id, context.attacker, was_destroyed, destroyed)?;
    if let Some(pilot) = pilot
        && let Some(recovery) = candidate.btech.recoveries.get_mut(&pilot)
    {
        recovery.remaining = 0;
    }
    resolution.notices.push(Notice {
        unit: id,
        text: "[fg=red bold]The shot ricochets around the crew compartment, instantly killing everyone![reset]".into(),
    });
    Ok(())
}

/// Fuel Tank: an internal-combustion vehicle explodes; a VTOL settles to the surface first.
fn explode_fuel_tank(
    candidate: &mut World,
    id: ObjectId,
    context: DamageContext,
    resolution: &mut VehicleCriticalResolution,
) -> Result<()> {
    resolution.notices.push(Notice {
        unit: id,
        text: "Your fuel tank explodes in a ball of fire!".into(),
    });
    resolution.broadcasts.push(Notice {
        unit: id,
        text: "'s fuel tank explodes in a ball of fire!".into(),
    });
    // Aircraft catastrophes settle at the local surface before shared destruction.
    if candidate.btech.vehicles()[&id].definition().is_vtol()
        && let Some(position) = candidate.btech.vehicles()[&id].position()
    {
        let height = candidate
            .btech
            .maps()
            .get(&position.map)
            .context("Aircraft explosion map is unavailable")?
            .base_hex(i64::from(position.x), i64::from(position.y))?
            .surface_height();
        let vehicle = candidate.btech.vehicles.get_mut(&id).unwrap();
        let flight = vehicle.vtol_flight.as_mut().unwrap();
        flight.phase = VtolFlightPhase::Landed;
        flight.fall = None;
        flight.altitude = f64::from(height);
        flight.vertical_speed = 0.0;
        vehicle.halt();
    }
    let was_destroyed = candidate.btech.vehicles()[&id].is_destroyed();
    let result = super::vehicle_explosion::explode_in_candidate(candidate, id, false)?;
    let destroyed = candidate.btech.vehicles()[&id].is_destroyed();
    super::kill_counters::transition(candidate, id, context.attacker, was_destroyed, destroyed)?;
    resolution.notices.extend(result.notices.clone());
    resolution.broadcasts.extend(result.broadcasts.clone());
    resolution.explosion = Some(result);
    Ok(())
}

/// Ammunition: every bin explodes into the struck location's structure, or with CASE into the
/// rear armor with the excess ignored and the crew stunned (TW p. 194).
fn explode_ammunition(
    candidate: &mut World,
    id: ObjectId,
    section: VehicleSection,
    rules: VehicleCriticalRules,
    context: DamageContext,
    resolution: &mut VehicleCriticalResolution,
) -> Result<()> {
    let cascade = discharge_vehicle_ammunition_cascade(candidate, id, section)?;
    ensure!(
        cascade.damage > 0,
        "Ammunition critical selected without live ammunition"
    );
    resolution.ammunition_cascade = Some(cascade.clone());
    resolution.notices.push(Notice {
        unit: id,
        text: "[fg=red bold]One of your ammo bins is struck causing a cascading explosion![reset]"
            .into(),
    });
    resolution.broadcasts.push(Notice {
        unit: id,
        text: "has an internal ammo explosion!".into(),
    });
    if !candidate.btech.vehicles()[&id].has_powerplant_containment() {
        let damage = super::vehicle_internal_damage::resolve_in_candidate(
            candidate,
            id,
            section,
            cascade.damage,
            rules,
            context.nested(),
        )?;
        super::piloting::append_feedback(
            &mut resolution.pilot_notices,
            damage.pilot_notices.iter().cloned(),
            resolution.notices.len(),
        );
        resolution.notices.extend(damage.notices.clone());
        resolution.broadcasts.extend(damage.broadcasts.clone());
        resolution.internal_damage.push(damage);
        return Ok(());
    }
    let vehicle = candidate.btech.vehicles.get_mut(&id).unwrap();
    let absorbed = vehicle
        .damage_phase(
            VehicleSection::Rear,
            cascade.damage.min(u32::from(u16::MAX)) as u16,
            DamagePhase::Armor { rear: false },
        )?
        .absorbed;
    resolution.notices.push(Notice {
        unit: id,
        text: format!(
            "[fg=red bold]CASE vents the blast out the rear, blowing {absorbed} points of armor off your rear![reset]"
        ),
    });
    stun_crew(candidate, id, context, resolution)
}

/// Crew Stunned, including the escalation to Crew Killed after Driver and Commander hits.
fn stun_crew(
    candidate: &mut World,
    id: ObjectId,
    context: DamageContext,
    resolution: &mut VehicleCriticalResolution,
) -> Result<()> {
    let was_destroyed = candidate.btech.vehicles()[&id].is_destroyed();
    let outcome = damage_vehicle_controls(candidate, id, VehicleControlHit::CrewStun)?;
    let destroyed = candidate.btech.vehicles()[&id].is_destroyed();
    super::kill_counters::transition(candidate, id, context.attacker, was_destroyed, destroyed)?;
    resolution.notices.push(Notice {
        unit: id,
        text: match outcome {
            VehicleControlOutcome::CrewKilled => {
                "[fg=red bold]The blast finishes off your already wounded crew![reset]"
            }
            _ => "[fg=red bold]The shot resonates throughout the crew compartment, temporarily stunning you![reset]",
        }
        .into(),
    });
    Ok(())
}

/// Driver, Commander, Co-Pilot, Crew Stunned and Sensors hits, with their repeat escalations.
fn crew_hit(
    candidate: &mut World,
    id: ObjectId,
    effect: VehicleCriticalEffect,
    context: DamageContext,
    resolution: &mut VehicleCriticalResolution,
) -> Result<()> {
    use VehicleControlHit as H;
    use VehicleCriticalEffect as E;
    let hit = match effect {
        E::Driver => H::Driver,
        E::Commander => H::Commander,
        E::VtolCopilot => H::Copilot,
        E::Sensors => H::Sensors,
        E::CrewStunned => return stun_crew(candidate, id, context, resolution),
        _ => bail!("{effect:?} is not a crew control hit"),
    };
    let was_destroyed = candidate.btech.vehicles()[&id].is_destroyed();
    let outcome = damage_vehicle_controls(candidate, id, hit)?;
    let destroyed = candidate.btech.vehicles()[&id].is_destroyed();
    super::kill_counters::transition(candidate, id, context.attacker, was_destroyed, destroyed)?;
    let text = match (effect, outcome) {
        (_, VehicleControlOutcome::CrewKilled) => {
            "[fg=red bold]The shot tears through the already wounded crew, killing everyone![reset]"
        }
        (_, VehicleControlOutcome::Stunned) => {
            "[fg=red bold]The shot resonates throughout the crew compartment, temporarily stunning you![reset]"
        }
        (E::Driver, _) => {
            "[fg=red bold]Your vehicle's driver takes a piece of shrapnel, making it harder to control the vehicle![reset]"
        }
        (E::Commander, _) => {
            "[fg=red bold]Your vehicle's commander takes a piece of shrapnel, stunning the crew and making it harder to drive and aim![reset]"
        }
        (E::VtolCopilot, _) => {
            "[fg=red bold]Your VTOL's co-pilot takes a piece of shrapnel, making it harder to aim your weapons![reset]"
        }
        (E::Sensors, _) if candidate.btech.vehicles()[&id].sensors_blind() => {
            "[fg=red bold]Your sensor suite is wrecked! You can no longer fire your weapons![reset]"
        }
        _ => "[fg=red bold]Your sensor suite takes a hit![reset]",
    };
    resolution.notices.push(Notice {
        unit: id,
        text: text.into(),
    });
    Ok(())
}

/// VTOL Pilot Hit: +2 driving, an immediate Driving Skill Roll or a one-level drop that may
/// crash, and Crew Killed on a repeat (TW p. 197).
fn pilot_hit(
    candidate: &mut World,
    id: ObjectId,
    rules: VehicleCriticalRules,
    context: DamageContext,
    resolution: &mut VehicleCriticalResolution,
) -> Result<()> {
    let was_destroyed = candidate.btech.vehicles()[&id].is_destroyed();
    let outcome = damage_vehicle_controls(candidate, id, VehicleControlHit::Pilot)?;
    let destroyed = candidate.btech.vehicles()[&id].is_destroyed();
    super::kill_counters::transition(candidate, id, context.attacker, was_destroyed, destroyed)?;
    if outcome == VehicleControlOutcome::CrewKilled {
        resolution.notices.push(Notice {
            unit: id,
            text: "[fg=red bold]The shot tears through the already wounded crew, killing everyone![reset]".into(),
        });
        return Ok(());
    }
    resolution.notices.push(Notice {
        unit: id,
        text: "[fg=red bold]Your VTOL's pilot takes a piece of shrapnel, making it harder to control the VTOL![reset]".into(),
    });
    let unit = &candidate.btech.vehicles()[&id];
    if unit
        .vtol_flight()
        .is_none_or(|flight| flight.phase != VtolFlightPhase::Airborne)
    {
        return Ok(());
    }
    let position = unit.position().context("Aircraft is not placed")?;
    let surface = candidate
        .btech
        .maps()
        .get(&position.map)
        .context("Aircraft map is unavailable")?
        .hex(i64::from(position.x), i64::from(position.y))?
        .surface_height();
    let check = super::vehicle_piloting::roll(candidate, id, 0, rules.extended_piloting)?;
    super::piloting::capture_feedback(
        id,
        candidate.btech.vehicles()[&id].pilot(),
        &check,
        &mut resolution.notices,
        &mut resolution.pilot_notices,
    );
    resolution.control_check = Some(check);
    if check.success {
        resolution.notices.push(Notice {
            unit: id,
            text: "You wrestle the VTOL back under control.".into(),
        });
        return Ok(());
    }
    let unit = candidate.btech.vehicles.get_mut(&id).unwrap();
    let flight = unit.vtol_flight.as_mut().unwrap();
    if flight.altitude - 1.0 <= f64::from(surface) {
        unit.lose_vtol_lift();
        resolution.notices.push(Notice {
            unit: id,
            text: "[fg=red bold]The VTOL lurches downward and you cannot pull up in time![reset]"
                .into(),
        });
        return Ok(());
    }
    flight.altitude -= 1.0;
    resolution.notices.push(Notice {
        unit: id,
        text: "The VTOL lurches downward before you regain control.".into(),
    });
    Ok(())
}

/// Turret Blown Off destroys the vehicle (TW p. 195).
fn blow_off_turret(
    candidate: &mut World,
    id: ObjectId,
    context: DamageContext,
    resolution: &mut VehicleCriticalResolution,
) -> Result<()> {
    let amount = candidate.btech.vehicles()[&id].sections()[&VehicleSection::Turret].internal;
    let was_destroyed = candidate.btech.vehicles()[&id].is_destroyed();
    damage_vehicle_phase(
        candidate,
        id,
        VehicleSection::Turret,
        amount,
        DamagePhase::Internal,
    )?;
    let destroyed = candidate.btech.vehicles()[&id].is_destroyed();
    super::kill_counters::transition(candidate, id, context.attacker, was_destroyed, destroyed)?;
    resolution.notices.push(Notice {
        unit: id,
        text: "[fg=red bold]The shot pops your turret clear off its housing, wrecking the vehicle![reset]".into(),
    });
    resolution.broadcasts.push(Notice {
        unit: id,
        text: "'s turret flies off!".into(),
    });
    Ok(())
}

/// Engine Hit: a flying VTOL attempts an emergency landing; everything else goes immobile with
/// its energy weapons dead and its turret locked (TW pp. 195 and 197).
fn engine_hit(
    candidate: &mut World,
    id: ObjectId,
    rules: VehicleCriticalRules,
    resolution: &mut VehicleCriticalResolution,
) -> Result<()> {
    resolution.notices.push(Notice {
        unit: id,
        text: "Your engine takes a direct hit!".into(),
    });
    let airborne = candidate.btech.vehicles()[&id]
        .vtol_flight()
        .is_some_and(|flight| flight.phase == VtolFlightPhase::Airborne);
    if !airborne {
        let vehicle = candidate.btech.vehicles.get_mut(&id).unwrap();
        let turret = vehicle.turret_heading().is_some();
        vehicle.disable_engine();
        resolution.notices.push(Notice {
            unit: id,
            text: if turret {
                "Your vehicle grinds to a halt; its energy weapons go dark and the turret drive seizes."
            } else {
                "Your vehicle grinds to a halt and its energy weapons go dark."
            }
            .into(),
        });
        return Ok(());
    }
    resolution.emergency_landing =
        super::vtol_emergency::engine_landing(candidate, id, rules.extended_piloting, false)?;
    if let Some(check) = &resolution.emergency_landing {
        super::piloting::capture_feedback(
            id,
            candidate.btech.vehicles()[&id].pilot(),
            check,
            &mut resolution.notices,
            &mut resolution.pilot_notices,
        );
    }
    resolution.notices.push(Notice {
        unit: id,
        text: if resolution
            .emergency_landing
            .as_ref()
            .is_some_and(|check| check.success)
        {
            "You land safely!"
        } else {
            "You lose lift and start to fall!"
        }
        .into(),
    });
    Ok(())
}

/// Apply one equipment consequence inside the private candidate, never the published world.
fn apply(
    world: &mut World,
    id: ObjectId,
    report: &VehicleCriticalReport,
    effect: VehicleCriticalEffect,
    notices: &mut Vec<Notice>,
) -> Result<()> {
    use VehicleCriticalEffect as E;
    let text = match effect {
        E::Stabilizer => {
            damage_vehicle_controls(
                world,
                id,
                VehicleControlHit::Stabilizers {
                    section: report.section,
                },
            )?;
            format!(
                "The weapon stabilizers in your {} have been destroyed!",
                report.section.name().replace('_', " ")
            )
        }
        E::WeaponJam => {
            let jam = jam_vehicle_weapon(world, id, report.section)?
                .context("Weapon Malfunction selected without a working weapon")?;
            jam.notice(id).text
        }
        E::TurretJam => jam_vehicle_turret(world, id)?.text,
        E::TurretLock => {
            lock_vehicle_turret(world, id)?;
            "[fg=red bold]The shot destroys your turret rotation mechanism![reset]".into()
        }
        _ => bail!("{effect:?} is not an equipment critical"),
    };
    notices.push(Notice { unit: id, text });
    Ok(())
}

/// Effects of one weapon loss, collected before publication by the parent critical.
#[derive(Default)]
struct WeaponDestruction {
    notices: Vec<Notice>,
    pilot_notices: Vec<PilotNotice>,
    broadcasts: Vec<Notice>,
    damage: Vec<VehicleInternalDamage>,
}

impl WeaponDestruction {
    /// Fold the loss into the enclosing critical's report.
    fn append(self, resolution: &mut VehicleCriticalResolution) {
        super::piloting::append_feedback(
            &mut resolution.pilot_notices,
            self.pilot_notices,
            resolution.notices.len(),
        );
        resolution.notices.extend(self.notices);
        resolution.broadcasts.extend(self.broadcasts);
        resolution.internal_damage.extend(self.damage);
    }
}

/// Disable the selected mount before resolving its explosion, preventing it from exploding twice.
/// Realtime stand-in for the chooser roll: the weapon is picked at random.
fn destroy_weapon(
    world: &mut World,
    id: ObjectId,
    section: VehicleSection,
    rules: VehicleCriticalRules,
    context: DamageContext,
) -> Result<WeaponDestruction> {
    let mut effects = WeaponDestruction::default();
    let index = select_vehicle_weapon_critical(world, id, section)?
        .context("Weapon Destroyed selected without a surviving weapon")?;
    let vehicle = &world.btech.vehicles()[&id];
    let loadout = vehicle.loadout()?;
    let mount = loadout.weapons[index].clone();
    let supplied = |mode| {
        loadout
            .ammunition
            .iter()
            .enumerate()
            .any(|(bin_index, bin)| {
                bin.weapon == mount.weapon
                    && bin.mode == mode
                    && vehicle.ammunition()[bin_index] > 0
                    && !vehicle.critical_unavailable(bin.location)
            })
    };
    let disabled = super::weapon_failure::explosion_disabled(
        index,
        &vehicle.powered_down_weapons,
        vehicle.weapon_failures(),
    );
    let gauss = mount.weapon.weapon_explosion_damage() > 0;
    let jammed = mount.weapon.jammed_explosion_damage() > 0 && vehicle.weapon_jammed(index)?;
    let hotload = vehicle.fire_mode(index)? == FireMode::Hotload;
    let hotload_supply = mount
        .weapon
        .hotload_supply_mode(vehicle.ammunition_mode(index)?);
    let explosion = if disabled {
        0
    } else if gauss {
        u32::from(mount.weapon.weapon_explosion_damage())
    } else if jammed {
        u32::from(mount.weapon.jammed_explosion_damage())
    } else if hotload && supplied(hotload_supply) {
        u32::from(mount.weapon.profile_for_ammunition(hotload_supply).damage)
            * u32::from(mount.weapon.profile().missiles.max(1))
    } else if !hotload
        && vehicle.ammunition_mode(index)? == AmmunitionMode::Incendiary
        && vehicle
            .weapon_recycle()
            .get(&index)
            .is_some_and(|remaining| *remaining > 0)
        && supplied(AmmunitionMode::Incendiary)
    {
        u32::from(mount.weapon.profile().damage)
    } else {
        0
    };
    destroy_vehicle_critical(world, id, mount.criticals[0])?;
    let name = mount.weapon.name().split_once('.').unwrap().1;
    if explosion == 0 {
        effects.notices.push(Notice {
            unit: id,
            text: format!("[fg=red bold]Your {name} is destroyed![reset]"),
        });
        return Ok(effects);
    }
    effects.notices.push(Notice {
        unit: id,
        text: format!("Your {name} has been destroyed!"),
    });
    effects.notices.push(Notice {
        unit: id,
        text: if gauss {
            format!("It explodes for {explosion} points damage.")
        } else if jammed {
            format!("[fg=red bold]Your jammed autocannon explodes for {explosion} points of damage![reset]")
        } else if hotload {
            format!("[fg=red bold]Your hotloaded launcher explodes for {explosion} points of damage![reset]")
        } else {
            format!("[fg=red bold]The incendiary ammunition in your launcher ignites for {explosion} points of damage![reset]")
        },
    });
    effects.broadcasts.push(Notice {
        unit: id,
        text: if gauss {
            format!(
                "'s {} is covered in a large electrical discharge!",
                section.name().replace('_', " ")
            )
        } else if jammed {
            " loses an autocannon in a brilliant explosion!".into()
        } else if hotload {
            " loses a launcher in a brilliant explosion!".into()
        } else {
            format!(
                "'s {} is engulfed in a brilliant blue flame!",
                section.name().replace('_', " ")
            )
        },
    });
    // An exploding weapon is an ammunition explosion in its location (TW p. 195); vehicle crews
    // take no injury from internal explosions.
    let report = super::vehicle_internal_damage::resolve_in_candidate(
        world,
        id,
        section,
        explosion,
        rules,
        context.nested(),
    )?;
    super::piloting::append_feedback(
        &mut effects.pilot_notices,
        report.pilot_notices.iter().cloned(),
        effects.notices.len(),
    );
    effects.notices.extend(report.notices.clone());
    effects.broadcasts.extend(report.broadcasts.clone());
    effects.damage.push(report);
    Ok(effects)
}
