//! Located vehicle impacts commit hit-table effects, armor damage and critical cascades together.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::sync::Arc;

/// Vehicle hit-table, critical and environmental fire policy; the caller supplies configuration.
#[derive(Debug, Clone, Copy)]
pub struct BattleVehicleImpactRules {
    /// Inferno ignites section fires instead of rolling for a heat explosion.
    pub advanced_fire: bool,
    pub criticals: BattleVehicleCriticalRules,
    pub fasa: BattleVehicleFasaHitRules,
}

/// One damage group's table selection and all staged effects, before visibility publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish impact effects only with the enclosing attack transaction"]
pub struct BattleVehicleImpact {
    /// Initial routing roll, plus the selected table's own roll for FASA, advanced or critical-proof routing.
    pub rolls: Vec<u8>,
    /// Combat-safe routing has no hit location or motive effects.
    pub hit: Option<BattleVehicleHit>,
    pub damage: Option<BattleVehicleArmorDamage>,
    pub notices: Vec<BattleNotice>,
    /// Pilot-only control feedback indexed into the damage notice stream.
    pub pilot_notices: Vec<BattlePilotNotice>,
    pub broadcasts: Vec<BattleNotice>,
}

/// Resolve one already successful attack group from its incoming arc, preserving all-or-nothing dice and damage.
/// Weapon targeting and publication still belong to the enclosing attack.
pub fn resolve_vehicle_impact(
    world: &mut World,
    id: ObjectId,
    arc: BattleHitArc,
    amount: u32,
    armor_piercing: Option<BattleWeapon>,
    rules: BattleVehicleImpactRules,
) -> Result<BattleVehicleImpact> {
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    ensure!(!vehicle.is_destroyed(), "Vehicle is destroyed");
    let mut candidate = world.clone();
    let result = resolve_followup_in_candidate(
        &mut candidate,
        id,
        arc,
        ImpactRequest {
            amount,
            armor_piercing,
            rear: false,
            attacker: None,
        },
        rules,
    )?;
    *world = candidate;
    Ok(result)
}

/// One admitted vehicle damage entry, independent of the attacker's chassis.
#[derive(Debug, Clone, Copy)]
pub(super) struct ImpactRequest {
    pub amount: u32,
    pub armor_piercing: Option<BattleWeapon>,
    pub rear: bool,
    pub attacker: Option<ObjectId>,
}

/// Continue a previously admitted attack, retaining location and damage rolls after hull loss.
pub(super) fn resolve_followup_in_candidate(
    world: &mut World,
    id: ObjectId,
    arc: BattleHitArc,
    request: ImpactRequest,
    rules: BattleVehicleImpactRules,
) -> Result<BattleVehicleImpact> {
    resolve_directed_followup(world, id, arc, request, rules, None)
}

/// Directed hits bypass random location effects while sharing material damage and critical resolution.
pub(super) fn resolve_directed_followup(
    world: &mut World,
    id: ObjectId,
    arc: BattleHitArc,
    request: ImpactRequest,
    rules: BattleVehicleImpactRules,
    forced: Option<BattleVehicleHit>,
) -> Result<BattleVehicleImpact> {
    let ImpactRequest {
        amount,
        armor_piercing,
        rear,
        attacker,
    } = request;
    ensure!(amount > 0, "Impact damage must be positive");
    if let Some(weapon) = armor_piercing {
        ensure!(
            BattleAmmunitionMode::ArmorPiercing.supports(weapon),
            "Weapon cannot fire armor-piercing ammunition"
        );
    }
    let mut result = if let Some(hit) = forced {
        BattleVehicleImpact {
            rolls: Vec::new(),
            hit: Some(hit),
            damage: None,
            notices: Vec::new(),
            pilot_notices: Vec::new(),
            broadcasts: Vec::new(),
        }
    } else {
        resolve_location_followup(world, id, arc, rules)?
    };
    if let Some(hit) = result.hit {
        let damage = super::vehicle_armor_damage::resolve_rear_followup_in_candidate(
            world,
            id,
            BattleVehicleArmorHit {
                section: hit.section,
                amount,
                through_armor_critical: hit.through_armor_critical,
                armor_piercing,
            },
            rear,
            rules.criticals,
            super::vehicle_internal_damage::DamageContext { attacker, depth: 0 },
        )?;
        super::piloting::append_feedback(
            &mut result.pilot_notices,
            damage.pilot_notices.iter().cloned(),
            result.notices.len(),
        );
        result.notices.extend(damage.notices.clone());
        result.broadcasts.extend(damage.broadcasts.clone());
        result.damage = Some(damage);
    } else {
        result
            .notices
            .extend(super::combat_safe::notice(attacker, id));
        // Safe material impacts still enter the damage stage and consume its diagnostic roll.
        Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .unwrap()
            .dice
            .generic_roll();
    }
    Ok(result)
}

/// Resolve table routing and its direct effects inside an unpublished attack candidate.
/// Pod hits stop here; ordinary impacts continue into armor and internal damage.
pub(super) fn resolve_location(
    world: &mut World,
    id: ObjectId,
    arc: BattleHitArc,
    rules: BattleVehicleImpactRules,
) -> Result<BattleVehicleImpact> {
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    ensure!(!vehicle.is_destroyed(), "Vehicle is destroyed");
    resolve_location_followup(world, id, arc, rules)
}

/// Table routing also applies to later packets against a wreck's remaining sections.
fn resolve_location_followup(
    world: &mut World,
    id: ObjectId,
    arc: BattleHitArc,
    rules: BattleVehicleImpactRules,
) -> Result<BattleVehicleImpact> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Vehicle is unavailable"
    );
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    let table = rules.criticals.table_for(vehicle);
    let proof = vehicle.definition().has_special("CritProof_Tech");
    let mut dice = vehicle.dice.clone();
    let mut rolls = vec![dice.generic_roll()];
    if table != BattleVehicleCriticalTable::Standard || proof {
        rolls.push(dice.generic_roll());
    }
    let mut result = BattleVehicleImpact {
        rolls,
        hit: None,
        damage: None,
        notices: Vec::new(),
        pilot_notices: Vec::new(),
        broadcasts: Vec::new(),
    };
    if rules.criticals.combat_safe || vehicle.combat_safe {
        Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .unwrap()
            .dice = dice;
        return Ok(result);
    }
    let roll = *result.rolls.last().unwrap();
    if vehicle.definition().is_vtol() {
        let selected = if table == BattleVehicleCriticalTable::Advanced {
            vehicle.advanced_vtol_hit(
                arc,
                roll,
                rules.fasa.critical_mode,
                rules.fasa.critical_level,
                &mut dice,
            )?
        } else {
            vehicle
                .definition()
                .vtol_hit(arc, roll, table == BattleVehicleCriticalTable::Fasa)?
        };
        let main_weapon = if selected.destroy_main_weapon {
            vehicle.rank_main_weapon(&mut dice)?
        } else {
            None
        };
        Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .unwrap()
            .dice = dice;
        if let Some(effect) = selected.rotor {
            let rotor = super::rotor_damage::apply_in_world(world, id, effect)?;
            result.notices.push(BattleNotice {
                unit: id,
                text: rotor.message().into(),
            });
        }
        let vehicle = Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .unwrap();
        if let Some((index, weapon)) = main_weapon {
            let mount = vehicle.loadout()?.weapons[index].clone();
            for location in mount.criticals {
                vehicle.destroy_critical(location)?;
            }
            result.notices.push(BattleNotice {
                unit: id,
                text: format!(
                    "[fg=red bold]Your {} is destroyed![reset]",
                    weapon
                        .name()
                        .split_once('.')
                        .map_or(weapon.name(), |(_, name)| name)
                ),
            });
        }
        result.hit = Some(selected.hit);
        return Ok(result);
    }

    let turret = if table == BattleVehicleCriticalTable::Advanced {
        vehicle
            .sections()
            .get(&BattleVehicleSection::Turret)
            .is_some_and(|section| section.internal > 0)
    } else {
        vehicle
            .definition()
            .sections
            .get(&BattleVehicleSection::Turret)
            .is_some_and(|section| section.internal > 0)
    };
    let exposed_turret = vehicle.dig_state().dug_in && turret && dice.die(100)? >= 42;
    let hit = if exposed_turret {
        super::BattleVehicleHit {
            section: BattleVehicleSection::Turret,
            through_armor_critical: false,
            motive: None,
            lock_turret: false,
            motive_roll: None,
            piloting_penalty: 0,
        }
    } else if table == BattleVehicleCriticalTable::Advanced {
        vehicle.advanced_hit(
            arc,
            roll,
            rules.fasa.critical_mode,
            rules.fasa.critical_level,
            &mut dice,
        )?
    } else if proof {
        vehicle.critical_proof_hit(arc, roll)?
    } else if table == BattleVehicleCriticalTable::Fasa {
        vehicle.fasa_hit(arc, roll, rules.fasa, vehicle.hit_condition(), &mut dice)?
    } else {
        vehicle.standard_hit(arc, roll, rules.fasa.critical_mode, &mut dice)?
    };
    let vehicle = Arc::make_mut(&mut world.btech.vehicles)
        .get_mut(&id)
        .unwrap();
    vehicle.dice = dice;
    let (notices, broadcasts) = super::vehicle_motive_effects::apply(
        vehicle,
        id,
        hit.motive,
        hit.piloting_penalty,
        hit.motive_roll,
    );
    result.notices.extend(notices);
    result.broadcasts.extend(broadcasts);
    if hit.lock_turret {
        vehicle.lock_turret()?;
    }
    if hit.lock_turret {
        result.notices.push(BattleNotice {
            unit: id,
            text: "Your turret takes a direct hit and locks up!".into(),
        });
    }
    result.hit = Some(hit);
    Ok(result)
}

impl BattleVehicleImpactRules {
    /// Standard tactical vehicle hit and critical policy.
    pub const STANDARD: Self = Self {
        advanced_fire: false,
        criticals: BattleVehicleCriticalRules {
            rotor_damage_divisor: 0,
            extended_piloting: false,
            vtol_table: None,
            table: BattleVehicleCriticalTable::Standard,
            enabled: true,
            combat_safe: false,
            toughness: false,
        },
        fasa: BattleVehicleFasaHitRules {
            friendly_criticals: false,
            critical_shielding: false,
            critical_mode: 2,
            critical_level: 60,
        },
    };

    /// Apply configured hit-table and critical policy consistently for either shooter class.
    pub(crate) fn configured(config: &crate::config::BattleTechConfig, toughness: bool) -> Self {
        Self {
            advanced_fire: config.fasaadvvhlfire != 0,
            criticals: BattleVehicleCriticalRules {
                rotor_damage_divisor: config.divrotordamage.clamp(0, i64::from(u32::MAX)) as u32,
                extended_piloting: config.extended_piloting != 0,
                vtol_table: Some(BattleVehicleCriticalTable::from_settings(
                    config.fasaadvvtolcrit != 0,
                    config.fasacrit != 0,
                )),
                table: BattleVehicleCriticalTable::from_settings(
                    config.fasaadvvhlcrit != 0,
                    config.fasacrit != 0,
                ),
                enabled: config.vcrit != 0,
                combat_safe: false,
                toughness,
            },
            fasa: BattleVehicleFasaHitRules {
                friendly_criticals: config.tankfriendly != 0,
                critical_shielding: config.tankshield != 0,
                critical_mode: config.vcrit,
                critical_level: config.critlevel,
            },
        }
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    /// Host settings propagate to exterior damage while nonpositive settings disable scaling.
    #[test]
    fn configured_rotor_divisor_is_bounded_and_retained() {
        for (setting, expected) in [(-1, 0), (0, 0), (1, 1), (5, 5), (i64::MAX, u32::MAX)] {
            let config = crate::config::BattleTechConfig {
                divrotordamage: setting,
                ..Default::default()
            };
            let rules = BattleVehicleImpactRules::configured(&config, false);
            assert_eq!(rules.criticals.rotor_damage_divisor, expected);
        }
    }

    #[test]
    fn configured_critical_tables_are_selected_per_victim_class() {
        let ground = BattleVehicle::new(
            BattleVehicleTemplate::parse(include_str!("../../game/mechs/Demolisher")).unwrap(),
        )
        .unwrap();
        let aircraft = BattleVehicle::new(
            BattleVehicleTemplate::parse(include_str!("../../game/mechs/Kestrel")).unwrap(),
        )
        .unwrap();
        let observation = BattleVehicle::new(
            BattleVehicleTemplate::parse(include_str!("../../game/mechs/ObservationVTOL")).unwrap(),
        )
        .unwrap();
        for ground_advanced in [false, true] {
            for aircraft_advanced in [false, true] {
                for fasa in [false, true] {
                    let config = crate::config::BattleTechConfig {
                        fasaadvvhlcrit: i64::from(ground_advanced),
                        fasaadvvtolcrit: i64::from(aircraft_advanced),
                        fasacrit: i64::from(fasa),
                        ..Default::default()
                    };
                    let rules = BattleVehicleImpactRules::configured(&config, true);
                    for (unit, advanced) in [
                        (&ground, ground_advanced),
                        (&aircraft, aircraft_advanced),
                        (&observation, aircraft_advanced),
                    ] {
                        let expected = if advanced {
                            BattleVehicleCriticalTable::Advanced
                        } else if fasa {
                            BattleVehicleCriticalTable::Fasa
                        } else {
                            BattleVehicleCriticalTable::Standard
                        };
                        assert_eq!(rules.criticals.table_for(unit), expected);
                    }
                    assert!(rules.criticals.toughness);
                    // Shared launcher policy updates must preserve both victim choices.
                    let launch = BattleVehicleCriticalRules {
                        toughness: false,
                        ..rules.criticals
                    };
                    assert_eq!(
                        launch.table_for(&aircraft),
                        rules.criticals.table_for(&aircraft)
                    );
                }
            }
        }
    }
}
