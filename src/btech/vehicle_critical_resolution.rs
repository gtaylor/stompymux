//! Vehicle critical selection and implemented consequences commit together or leave no changes.
use super::vehicle_internal_damage::DamageContext;
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

/// Applied critical with occupant feedback staged for the enclosing attack transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish critical notices with the enclosing attack transaction"]
pub struct BattleVehicleCriticalResolution {
    pub selection: BattleVehicleCriticalReport,
    /// Engine-loss landing check when terrain permits an attempt, retained for host diagnostics.
    pub emergency_landing: Option<BattlePilotingCheck>,
    pub notices: Vec<BattleNotice>,
    /// Pilot-only control feedback indexed into the damage notice stream.
    pub pilot_notices: Vec<BattlePilotNotice>,
    pub pilot_injury: Option<BattlePilotInjury>,
    /// Direct character injury; nested explosions retain their own ordered reports.
    pub character_injury: Option<BattleCharacterPilotInjury>,
    pub internal_damage: Vec<BattleVehicleInternalDamage>,
    pub ammunition_cascade: Option<BattleVehicleAmmunitionCascade>,
    pub explosion: Option<BattleVehicleExplosion>,
    /// Visibility-filtered broadcasts owed by the enclosing attack, not global messages.
    pub broadcasts: Vec<BattleNotice>,
}

/// Resolve selection and its consequences atomically. Unsupported character casualties
/// fail without consuming dice; callers must not treat errors as a harmless hit.
pub fn resolve_vehicle_critical(
    world: &mut World,
    id: ObjectId,
    section: BattleVehicleSection,
    rules: BattleVehicleCriticalRules,
) -> Result<BattleVehicleCriticalResolution> {
    world.attempt(|world| {
        let report = resolve_in_candidate(world, id, section, rules, DamageContext::default())?;
        Ok(report)
    })
}

/// Resolve a nested critical in an already isolated attack candidate.
pub(super) fn resolve_in_candidate(
    candidate: &mut World,
    id: ObjectId,
    section: BattleVehicleSection,
    rules: BattleVehicleCriticalRules,
    context: DamageContext,
) -> Result<BattleVehicleCriticalResolution> {
    ensure!(
        context.depth < 64,
        "Vehicle critical cascade limit exceeded"
    );
    let selection = roll_vehicle_critical(candidate, id, section, rules)?;
    let mut emergency_landing = None;
    let mut notices = Vec::new();
    let mut pilot_notices = Vec::new();
    let mut pilot_injury = None;
    let mut character_injury = None;
    let mut internal_damage = Vec::new();
    let mut ammunition_cascade = None;
    let mut explosion = None;
    let mut broadcasts = Vec::new();
    if let Some(effect) = selection.effect {
        notices.push(BattleNotice {
            unit: id,
            text: "[fg=yellow bold]CRITICAL HIT![reset]".into(),
        });
        if matches!(
            effect,
            BattleVehicleCriticalEffect::CrewHit | BattleVehicleCriticalEffect::CrewKilled
        ) {
            let killed = effect == BattleVehicleCriticalEffect::CrewKilled;
            if killed {
                let vehicle = candidate.btech.vehicles.get_mut(&id).unwrap();
                let pilot = vehicle.pilot();
                let was_destroyed = vehicle.is_destroyed();
                vehicle.kill_crew();
                let destroyed = vehicle.is_destroyed();
                super::kill_counters::transition(
                    candidate,
                    id,
                    context.attacker,
                    was_destroyed,
                    destroyed,
                )?;
                if let Some(pilot) = pilot
                    && let Some(recovery) = candidate.btech.recoveries.get_mut(&pilot)
                {
                    recovery.remaining = 0;
                }
                let advanced = selection.table == BattleVehicleCriticalTable::Advanced;
                if advanced {
                    candidate
                        .btech
                        .vehicles
                        .get_mut(&id)
                        .unwrap()
                        .apply_motive_hit(BattleVehicleMotiveHit::Immobilize);
                }
                notices.push(BattleNotice { unit: id, text: if candidate.btech.vehicles()[&id].definition().is_vtol() && !advanced { "Your cockpit is destroyed!" } else if advanced { "[fg=red bold]The shot ricochets around the crew compartment, instantly killing everyone![reset]" } else { "Your armor is pierced and you are killed instantly!" }.into() });
            }
            if !killed {
                let was_destroyed = candidate.btech.vehicles()[&id].is_destroyed();
                let injury =
                    super::pilot_injury::injure_in_candidate(candidate, id, 1, rules.toughness)?;
                let destroyed = candidate.btech.vehicles()[&id].is_destroyed();
                super::kill_counters::transition(
                    candidate,
                    id,
                    context.attacker,
                    was_destroyed,
                    destroyed,
                )?;
                match injury {
                    super::pilot_injury::PilotInjury::Tactical(injury) => {
                        if let Some(notice) = injury.notice(id) {
                            notices.push(notice);
                        }
                        pilot_injury = Some(injury);
                    }
                    super::pilot_injury::PilotInjury::Character(injury) => {
                        character_injury = Some(injury)
                    }
                }
            }
        } else if effect == BattleVehicleCriticalEffect::PowerPlant
            || (effect == BattleVehicleCriticalEffect::FuelTank
                && (selection.table != BattleVehicleCriticalTable::Advanced
                    || candidate.btech.vehicles()[&id]
                        .definition()
                        .has_special("ICEEngine_Tech")))
        {
            let powerplant = effect == BattleVehicleCriticalEffect::PowerPlant;
            let contained = powerplant
                && candidate.btech.vehicles()[&id].definition().is_vtol()
                && candidate.btech.vehicles()[&id].has_powerplant_containment();
            notices.push(BattleNotice {
                unit: id,
                text: if powerplant {
                    "Your power plant explodes!"
                } else {
                    "Your fuel tank explodes in a ball of fire!"
                }
                .into(),
            });
            broadcasts.push(BattleNotice {
                unit: id,
                text: if powerplant {
                    "'s power plant suddenly explodes!"
                } else {
                    "'s fuel tank explodes in a ball of fire!"
                }
                .into(),
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
                flight.phase = BattleVtolFlightPhase::Landed;
                flight.fall = None;
                flight.altitude = f64::from(height);
                flight.vertical_speed = 0.0;
                vehicle.halt();
            }
            let was_destroyed = candidate.btech.vehicles()[&id].is_destroyed();
            let result = super::vehicle_explosion::explode_in_candidate(candidate, id, contained)?;
            let destroyed = candidate.btech.vehicles()[&id].is_destroyed();
            super::kill_counters::transition(
                candidate,
                id,
                context.attacker,
                was_destroyed,
                destroyed,
            )?;
            notices.extend(result.notices.clone());
            broadcasts.extend(result.broadcasts.clone());
            explosion = Some(result);
        } else if effect == BattleVehicleCriticalEffect::Ammunition {
            let cascade = discharge_vehicle_ammunition_cascade(candidate, id, section)?;
            ammunition_cascade = Some(cascade.clone());
            if cascade.damage == 0 {
                let loss = destroy_weapon(candidate, id, section, rules, context)?;
                super::piloting::append_feedback(
                    &mut pilot_notices,
                    loss.pilot_notices.iter().cloned(),
                    notices.len(),
                );
                notices.extend(loss.notices);
                broadcasts.extend(loss.broadcasts);
                internal_damage.extend(loss.damage);
                pilot_injury = loss.injury;
                character_injury = loss.character_injury;
            } else {
                notices.push(BattleNotice { unit: id, text: "[fg=red bold]One of your ammo bins is struck causing a cascading explosion![reset]".into() });
                broadcasts.push(BattleNotice {
                    unit: id,
                    text: "has an internal ammo explosion!".into(),
                });
                let damage = super::vehicle_internal_damage::resolve_in_candidate(
                    candidate,
                    id,
                    section,
                    cascade.damage,
                    rules,
                    context.nested(),
                )?;
                super::piloting::append_feedback(
                    &mut pilot_notices,
                    damage.pilot_notices.iter().cloned(),
                    notices.len(),
                );
                notices.extend(damage.notices.clone());
                broadcasts.extend(damage.broadcasts.clone());
                internal_damage.push(damage);
            }
        } else if effect == BattleVehicleCriticalEffect::WeaponDestroyed {
            let loss = destroy_weapon(candidate, id, section, rules, context)?;
            super::piloting::append_feedback(
                &mut pilot_notices,
                loss.pilot_notices.iter().cloned(),
                notices.len(),
            );
            notices.extend(loss.notices);
            broadcasts.extend(loss.broadcasts);
            internal_damage.extend(loss.damage);
            pilot_injury = loss.injury;
            character_injury = loss.character_injury;
        } else if effect == BattleVehicleCriticalEffect::TurretBlownOff {
            let amount =
                candidate.btech.vehicles()[&id].sections()[&BattleVehicleSection::Turret].internal;
            damage_vehicle_phase(
                candidate,
                id,
                BattleVehicleSection::Turret,
                amount,
                BattleDamagePhase::Internal,
            )?;
            notices.push(BattleNotice {
                unit: id,
                text: "[fg=red bold]The shot pops your turret clear off its housing![reset]".into(),
            });
            broadcasts.push(BattleNotice {
                unit: id,
                text: "'s turret flies off!".into(),
            });
        } else if candidate.btech.vehicles()[&id]
            .vtol_flight()
            .is_some_and(|flight| flight.phase == BattleVtolFlightPhase::Airborne)
            && (effect == BattleVehicleCriticalEffect::Engine
                || (effect == BattleVehicleCriticalEffect::FuelTank
                    && selection.table == BattleVehicleCriticalTable::Advanced))
        {
            emergency_landing = super::vtol_emergency::engine_landing(
                candidate,
                id,
                rules.extended_piloting,
                selection.table == BattleVehicleCriticalTable::Advanced,
            )?;
            notices.push(BattleNotice {
                unit: id,
                text: "Your engine takes a direct hit!".into(),
            });
            if let Some(check) = &emergency_landing {
                super::piloting::capture_feedback(
                    id,
                    candidate.btech.vehicles()[&id].pilot(),
                    check,
                    &mut notices,
                    &mut pilot_notices,
                );
            }
            notices.push(BattleNotice {
                unit: id,
                text: if emergency_landing
                    .as_ref()
                    .is_some_and(|check| check.success)
                {
                    "You land safely!"
                } else {
                    "You lose lift and start to fall!"
                }
                .into(),
            });
        } else {
            apply(candidate, id, &selection, effect, &mut notices)?;
        }
    }
    Ok(BattleVehicleCriticalResolution {
        selection,
        emergency_landing,
        notices,
        pilot_notices,
        pilot_injury,
        character_injury,
        internal_damage,
        ammunition_cascade,
        explosion,
        broadcasts,
    })
}

/// Apply one selected consequence inside the private candidate, never the published world.
fn apply(
    world: &mut World,
    id: ObjectId,
    report: &BattleVehicleCriticalReport,
    effect: BattleVehicleCriticalEffect,
    notices: &mut Vec<BattleNotice>,
) -> Result<()> {
    use BattleVehicleControlHit as H;
    use BattleVehicleCriticalEffect as E;
    let advanced = report.table == BattleVehicleCriticalTable::Advanced;
    let no_effect = "The shot pierces your armor yet fails to hit a critical system!";
    let mut control = None;
    let text = match effect {
        E::Rotor(effect) => super::rotor_damage::apply_in_world(world, id, effect)?.message(),
        E::VtolPilot => {
            control = Some(H::Driver);
            "[fg=red bold]Your VTOL's copilot takes a piece of shrapnel, making it harder to control the VTOL![reset]"
        }
        E::VtolCopilot => {
            control = Some(H::Sensors);
            "[fg=red bold]Your VTOL's pilot takes a piece of shrapnel, making it harder to aim your weapons![reset]"
        }
        E::Driver => {
            control = Some(H::Driver);
            "[fg=red bold]Your vehicle's driver takes a piece of shrapnel, making it harder to control the vehicle![reset]"
        }
        E::Sensors => {
            control = Some(H::Sensors);
            "[fg=red bold]Your sensor suite takes a hit![reset]"
        }
        E::Commander => {
            control = Some(H::Commander);
            "[fg=red bold]Your vehicle's commander takes a piece of shrapnel![reset]"
        }
        E::CrewStunned => {
            control = Some(H::CrewStun);
            ""
        }
        E::Cargo => no_effect,
        E::Stabilizer => {
            let repeated = world.btech.vehicles()[&id]
                .lost_stabilizers()
                .contains(&report.section);
            damage_vehicle_controls(
                world,
                id,
                H::Stabilizers {
                    section: report.section,
                },
            )?;
            let section = report.section.name().replace('_', " ");
            notices.push(BattleNotice {
                unit: id,
                text: if repeated {
                    format!("The destroyed weapon stabilizers in your {section} take another hit!")
                } else {
                    format!("The weapon stabilizers in your {section} have been destroyed!")
                },
            });
            return Ok(());
        }
        E::MainWeaponJam => {
            if let Some(jam) = jam_vehicle_main_weapon(world, id)? {
                notices.push(jam.notice(id));
            }
            return Ok(());
        }
        E::WeaponJam => {
            notices.push(match jam_vehicle_weapon(world, id, report.section)? {
                Some(jam) => jam.notice(id),
                None => BattleNotice {
                    unit: id,
                    text: no_effect.into(),
                },
            });
            return Ok(());
        }
        E::TurretJam => {
            notices.push(jam_vehicle_turret(world, id)?);
            return Ok(());
        }
        E::TurretLock => {
            if world.btech.vehicles()[&id].turret_locked() {
                no_effect
            } else {
                lock_vehicle_turret(world, id)?;
                if advanced {
                    "[fg=red bold]The shot destroys your turret rotation mechanism![reset]"
                } else {
                    "Your turret takes a direct hit and locks up!"
                }
            }
        }
        E::MotiveSpeedLoss | E::Immobilize => {
            let movement = world.btech.vehicles()[&id].definition().movement;
            let immobilize = effect == E::Immobilize;
            damage_vehicle_motive(
                world,
                id,
                if immobilize {
                    BattleVehicleMotiveHit::Immobilize
                } else {
                    BattleVehicleMotiveHit::SpeedLoss { movement_points: 1 }
                },
            )?;
            match (movement, immobilize) {
                (BattleVehicleMovement::Tracked, false) => "One of your tracks is damaged!",
                (BattleVehicleMovement::Wheeled, false) => "One of your wheels is damaged!",
                (BattleVehicleMovement::Hover, false) => "Your air skirt is damaged!",
                (BattleVehicleMovement::Tracked, true) => {
                    "One of your tracks is destroyed, immobilizing your vehicle!"
                }
                (BattleVehicleMovement::Wheeled, true) => {
                    "One of your wheels is destroyed, immobilizing your vehicle!"
                }
                (BattleVehicleMovement::Hover, true) => {
                    "Your lift fan is destroyed, immobilizing your vehicle!"
                }
                (BattleVehicleMovement::Stationary, _) => no_effect,
                (BattleVehicleMovement::Vtol, _) => {
                    anyhow::bail!("VTOL motive hits require rotor consequences")
                }
            }
        }
        E::Engine | E::FuelTank
            if effect == E::Engine
                || (advanced
                    && !world.btech.vehicles()[&id]
                        .definition()
                        .has_special("ICEEngine_Tech")) =>
        {
            let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
            ensure!(
                vehicle.maximum_speed() == 0.0
                    || vehicle.vtol_flight().is_none_or(|flight| matches!(
                        flight.phase,
                        BattleVtolFlightPhase::Landed | BattleVtolFlightPhase::Launching { .. }
                    )),
                "Airborne VTOL engine loss requires emergency landing and crash resolution"
            );
            if advanced && vehicle.immobilized() {
                "Your destroyed engine takes another direct hit!"
            } else {
                vehicle.disable_engine(advanced);
                if advanced {
                    "[fg=red bold]Your engine takes a direct hit![reset]"
                } else {
                    "Your engine takes a direct hit!  You can't move anymore."
                }
            }
        }
        _ => bail!(
            "Vehicle critical consequence {effect:?} is not implemented; the attack must remain uncommitted"
        ),
    };
    if let Some(hit) = control {
        damage_vehicle_controls(world, id, hit)?;
    }
    if !text.is_empty() {
        notices.push(BattleNotice {
            unit: id,
            text: text.into(),
        });
    }
    if matches!(effect, E::Commander | E::CrewStunned) {
        notices.push(BattleNotice { unit: id, text: "[fg=red bold]The shot resonates throughout the crew compartment, temporarily stunning you![reset]".into() });
    }
    Ok(())
}

/// Effects of one weapon loss, collected before publication by the parent critical.
#[derive(Default)]
struct WeaponDestruction {
    notices: Vec<BattleNotice>,
    pilot_notices: Vec<BattlePilotNotice>,
    broadcasts: Vec<BattleNotice>,
    damage: Vec<BattleVehicleInternalDamage>,
    injury: Option<BattlePilotInjury>,
    character_injury: Option<BattleCharacterPilotInjury>,
}

/// Disable the selected mount before resolving its explosion, preventing it from exploding twice.
fn destroy_weapon(
    world: &mut World,
    id: ObjectId,
    section: BattleVehicleSection,
    rules: BattleVehicleCriticalRules,
    context: DamageContext,
) -> Result<WeaponDestruction> {
    let mut effects = WeaponDestruction::default();
    let Some(index) = select_vehicle_weapon_critical(world, id, section)? else {
        effects.notices.push(BattleNotice {
            unit: id,
            text: "The shot pierces your armor yet fails to hit a critical system!".into(),
        });
        return Ok(effects);
    };
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
    let hotload = vehicle.fire_mode(index)? == BattleFireMode::Hotload;
    let hotload_supply = mount
        .weapon
        .hotload_supply_mode(vehicle.ammunition_mode(index)?);
    let explosion = if disabled {
        0
    } else if gauss {
        u32::from(mount.weapon.weapon_explosion_damage())
    } else if hotload && supplied(hotload_supply) {
        u32::from(mount.weapon.profile_for_ammunition(hotload_supply).damage)
            * u32::from(mount.weapon.profile().missiles.max(1))
    } else if !hotload
        && vehicle.ammunition_mode(index)? == BattleAmmunitionMode::Incendiary
        && vehicle
            .weapon_recycle()
            .get(&index)
            .is_some_and(|remaining| *remaining > 0)
        && supplied(BattleAmmunitionMode::Incendiary)
    {
        u32::from(mount.weapon.profile().damage)
    } else {
        0
    };
    destroy_vehicle_critical(world, id, mount.criticals[0])?;
    let name = mount.weapon.name().split_once('.').unwrap().1;
    if explosion == 0 {
        effects.notices.push(BattleNotice {
            unit: id,
            text: format!("[fg=red bold]Your {name} is destroyed![reset]"),
        });
        return Ok(effects);
    }
    effects.notices.push(BattleNotice {
        unit: id,
        text: format!("Your {name} has been destroyed!"),
    });
    effects.notices.push(BattleNotice {
        unit: id,
        text: if gauss {
            format!("It explodes for {explosion} points damage.")
        } else if hotload {
            format!("[fg=red bold]Your hotloaded launcher explodes for {explosion} points of damage![reset]")
        } else {
            format!("[fg=red bold]The incendiary ammunition in your launcher ignites for {explosion} points of damage![reset]")
        },
    });
    effects.broadcasts.push(BattleNotice {
        unit: id,
        text: if gauss {
            format!(
                "'s {} is covered in a large electrical discharge!",
                section.name().replace('_', " ")
            )
        } else if hotload {
            " loses a launcher in a brilliant explosion!".into()
        } else {
            format!(
                "'s {} is engulfed in a brilliant blue flame!",
                section.name().replace('_', " ")
            )
        },
    });
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
    if gauss && !world.btech.vehicles()[&id].is_destroyed() {
        let pilot = world.btech.vehicles()[&id].pilot();
        let pain = pilot
            .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Pain_Resistance"));
        let report = super::pilot_injury::injure_in_candidate(
            world,
            id,
            if pain { 1 } else { 2 },
            rules.toughness,
        )?;
        match report {
            super::pilot_injury::PilotInjury::Tactical(report) => {
                if let Some(notice) = report.notice(id) {
                    effects.notices.push(notice);
                }
                effects.injury = Some(report);
            }
            super::pilot_injury::PilotInjury::Character(report) => {
                effects.character_injury = Some(report)
            }
        }
    }
    Ok(effects)
}
