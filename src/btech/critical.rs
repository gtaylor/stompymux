//! Critical-slot outcomes and derived equipment availability for supported Mech chassis.
use super::{BattlePower, BattleSection, BattleSystem, BattleUnit, CriticalLocation};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::BTreeSet;

/// The equipment affected by a critical hit; combat resolution owns its secondary effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BattleCriticalLoss {
    WeaponDamage {
        index: usize,
        damage: super::BattleWeaponDamageKind,
    },
    Weapon {
        index: usize,
        /// Enclosing combat resolution applies this after disabling the entire explosive mount.
        explosion_damage: u8,
    },
    Ammunition {
        index: usize,
        rounds: u16,
        explosion_damage: u32,
    },
    System {
        system: BattleSystem,
    },
}

/// An available bin's full internal explosion potential, measured in damage points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleAmmunitionHazard {
    pub index: usize,
    pub location: CriticalLocation,
    pub damage: u32,
}

impl BattleUnit {
    /// A shared targeting computer needs installed slots with no critical, section or flood losses.
    pub fn targeting_computer_operational(&self) -> Result<bool> {
        let loadout = self.loadout()?;
        let mut installed = false;
        for part in loadout
            .systems
            .iter()
            .filter(|part| part.system == BattleSystem::TargetingComputer)
        {
            installed = true;
            if self.critical_unavailable(part.location) {
                return Ok(false);
            }
        }
        Ok(installed)
    }

    /// Installed CASE, CASE II or Clan containment stops internal explosions from transferring,
    /// including after section loss.
    pub fn has_case(&self, section: BattleSection) -> bool {
        if self.definition().has_special("Clan") || self.has_case_ii(section) {
            return true;
        }
        self.definition()
            .sections
            .get(&section)
            .is_some_and(|layout| {
                layout.criticals.values().any(|part| {
                    BattleSystem::parse(&part.equipment).ok() == Some(BattleSystem::Case)
                })
            })
    }

    /// Installed CASE II vents a local explosion through armor after one internal point.
    pub fn has_case_ii(&self, section: BattleSection) -> bool {
        self.definition()
            .sections
            .get(&section)
            .is_some_and(|layout| {
                layout.criticals.values().any(|part| {
                    BattleSystem::parse(&part.equipment).ok() == Some(BattleSystem::CaseIi)
                })
            })
    }

    /// Select the most destructive remaining bin; equal hazards retain the first section/slot.
    /// No dice are consumed, and unavailable or empty bins cannot become heat hazards.
    pub fn ammunition_hazard_maximum(&self) -> Result<Option<BattleAmmunitionHazard>> {
        self.maximum_ammunition_hazard(false)
    }

    /// Inferno penalties select the largest available inferno bin before other ammunition.
    pub(super) fn inferno_ammunition_hazard(&self) -> Result<Option<BattleAmmunitionHazard>> {
        self.maximum_ammunition_hazard(true)
    }

    /// Shared stable hazard ordering, optionally restricted to inferno ammunition.
    fn maximum_ammunition_hazard(
        &self,
        inferno_only: bool,
    ) -> Result<Option<BattleAmmunitionHazard>> {
        let loadout = self.loadout()?;
        let mut maximum: Option<BattleAmmunitionHazard> = None;
        for index in 0..loadout.ammunition.len() {
            if inferno_only
                && loadout.ammunition[index].mode != super::BattleAmmunitionMode::Inferno
            {
                continue;
            }
            let hazard = self.ammunition_hazard(index)?;
            if hazard.damage == 0 || self.critical_unavailable(hazard.location) {
                continue;
            }
            if maximum.is_none_or(|previous| hazard.damage > previous.damage) {
                maximum = Some(hazard);
            }
        }
        Ok(maximum)
    }

    /// Calculate one bin's potential from its live salvo count and conventional weapon profile.
    pub(super) fn ammunition_hazard(&self, index: usize) -> Result<BattleAmmunitionHazard> {
        let loadout = self.loadout()?;
        let bin = loadout
            .ammunition
            .get(index)
            .context("Ammunition index out of bounds")?;
        Ok(BattleAmmunitionHazard {
            index,
            location: bin.location,
            damage: bin.weapon.ammunition_explosion_damage_for_mode(
                *self
                    .ammunition
                    .get(index)
                    .context("Ammunition state is incomplete")?,
                bin.mode,
            ),
        })
    }

    /// Explicit slot losses, excluding implicit unavailability from destroyed sections.
    pub fn lost_criticals(&self) -> &BTreeSet<CriticalLocation> {
        &self.lost_criticals
    }

    /// Whether a known installed slot has been destroyed or its section is gone.
    pub fn critical_destroyed(&self, location: CriticalLocation) -> bool {
        self.lost_criticals.contains(&location)
            || self
                .sections
                .get(&location.section)
                .is_none_or(|section| section.internal == 0)
    }

    /// Installed slots still eligible for conventional random critical selection.
    /// Surviving slots of a broken multi-slot weapon remain eligible for subsequent hits.
    pub fn critical_candidates(&self, section: BattleSection) -> Vec<CriticalLocation> {
        self.definition().sections[&section]
            .criticals
            .iter()
            .filter(|(_, part)| {
                !BattleSystem::parse(&part.equipment).is_ok_and(BattleSystem::is_noncritical)
            })
            .map(|(&slot, _)| CriticalLocation { section, slot })
            .filter(|location| {
                !self.critical_destroyed(*location)
                    && !self
                        .weapon_damage
                        .iter()
                        .any(|damage| damage.location == *location)
            })
            .collect()
    }

    /// Choose uniformly from installed slots; immune units and empty sections consume no dice.
    pub fn choose_critical(&mut self, section: BattleSection) -> Option<CriticalLocation> {
        if self.combat_safe || self.definition().has_special("CritProof_Tech") {
            return None;
        }
        let candidates = self.critical_candidates(section);
        if candidates.is_empty() {
            return None;
        }
        let index = self
            .dice
            .die(candidates.len() as u16)
            .expect("bounded occupied slots")
            - 1;
        Some(candidates[usize::from(index)])
    }

    /// A weapon requires every mounting slot to remain operational; ammo and recycle are separate.
    pub fn weapon_intact(&self, index: usize) -> Result<bool> {
        let loadout = self.loadout()?;
        let weapon = loadout
            .weapons
            .get(index)
            .context("Weapon index out of bounds")?;
        Ok(weapon
            .criticals
            .iter()
            .all(|location| !self.critical_unavailable(*location)))
    }

    /// Count effective losses: sink cooling capacity and jump jets, rather than their grouped slots.
    /// Flooded and vacuum-exposed engines, heat sinks and jump jets also count.
    pub fn system_hits(&self, system: BattleSystem) -> u8 {
        // Undamaged units are the common case in scanner and movement queries.
        // No installed system can be lost without one of these live conditions.
        if self.lost_criticals.is_empty()
            && self.flooded_sections.is_empty()
            && self.breached_sections.is_empty()
            && self.definition().sections.keys().all(|section| {
                self.sections
                    .get(section)
                    .is_some_and(|state| state.internal > 0)
            })
        {
            return 0;
        }
        if system == BattleSystem::JumpJet && self.definition().has_special("ImprovedJJ_Tech") {
            return self
                .loadout()
                .expect("validated loadout")
                .jump_jet_groups(true)
                .expect("validated jet groups")
                .iter()
                .filter(|group| {
                    group
                        .iter()
                        .any(|location| self.critical_unavailable(*location))
                })
                .count() as u8;
        }
        if system == BattleSystem::HeatSink && self.definition().has_double_heat_sinks() {
            return (self
                .loadout()
                .expect("validated loadout")
                .heat_sink_groups(self.definition().heat_sink_slots())
                .expect("validated sink groups")
                .iter()
                .filter(|group| {
                    group
                        .iter()
                        .any(|location| self.critical_unavailable(*location))
                })
                .count()
                * 2) as u8;
        }
        self.definition()
            .sections
            .iter()
            .flat_map(|(&section, layout)| {
                layout
                    .criticals
                    .iter()
                    .map(move |(&slot, part)| (CriticalLocation { section, slot }, part))
            })
            .filter(|(location, part)| {
                // Destruction checks run for every sensor snapshot. Most slots are
                // intact: inspect their live condition before parsing equipment names.
                (self.critical_destroyed(*location)
                    || (matches!(
                        system,
                        BattleSystem::Engine | BattleSystem::HeatSink | BattleSystem::JumpJet
                    ) && self.section_disabled(location.section)))
                    && BattleSystem::parse(&part.equipment).ok() == Some(system)
            })
            .count() as u8
    }

    /// Inspect a pending critical loss so combat can choose degradation before material destruction.
    pub(super) fn critical_loss(
        &self,
        location: CriticalLocation,
    ) -> Result<Option<BattleCriticalLoss>> {
        let loadout = self.loadout()?;
        let loss = if let Some(index) = loadout
            .weapons
            .iter()
            .position(|mount| mount.criticals.contains(&location))
        {
            BattleCriticalLoss::Weapon {
                index,
                explosion_damage: if self.weapon_intact(index)?
                    && !super::weapon_failure::explosion_disabled(
                        index,
                        &self.powered_down_weapons,
                        &self.weapon_failures,
                    ) {
                    let weapon = loadout.weapons[index].weapon;
                    let hotload_supply = weapon.hotload_supply_mode(self.ammunition_mode(index)?);
                    if self.fire_mode(index)? == super::BattleFireMode::Hotload
                        && loadout.ammunition.iter().enumerate().any(|(i, bin)| {
                            bin.weapon == weapon
                                && bin.mode == hotload_supply
                                && self.ammunition[i] > 0
                                && !self.critical_unavailable(bin.location)
                        })
                    {
                        weapon.profile_for_ammunition(hotload_supply).damage
                            * weapon.profile().missiles.max(1)
                    } else if self.ammunition_mode(index)?
                        == super::BattleAmmunitionMode::Incendiary
                        && self
                            .weapon_recycle
                            .get(&index)
                            .is_some_and(|remaining| *remaining > 0)
                        && loadout.ammunition.iter().enumerate().any(|(i, bin)| {
                            bin.weapon == weapon
                                && bin.mode == super::BattleAmmunitionMode::Incendiary
                                && self.ammunition[i] > 0
                                && !self.critical_unavailable(bin.location)
                        })
                    {
                        weapon.profile().damage
                    } else if weapon.jammed_explosion_damage() > 0 && self.weapon_jammed(index)? {
                        weapon.jammed_explosion_damage()
                    } else {
                        weapon.weapon_explosion_damage()
                    }
                } else {
                    0
                },
            }
        } else if let Some(index) = loadout
            .ammunition
            .iter()
            .position(|bin| bin.location == location)
        {
            let rounds = self.ammunition[index];
            BattleCriticalLoss::Ammunition {
                index,
                rounds,
                explosion_damage: loadout.ammunition[index]
                    .weapon
                    .ammunition_explosion_damage_for_mode(rounds, loadout.ammunition[index].mode),
            }
        } else {
            let part = loadout
                .systems
                .iter()
                .find(|part| part.location == location)
                .context("Critical slot is not installed")?;
            BattleCriticalLoss::System {
                system: part.system,
            }
        };
        if self.critical_destroyed(location) {
            return Ok(None);
        }
        Ok(Some(loss))
    }

    /// Mark one occupied slot lost and return any effect still needed by the enclosing action.
    /// Invalid/repeated requests change nothing. Ammunition contents are captured before clearing.
    pub fn destroy_critical(
        &mut self,
        location: CriticalLocation,
    ) -> Result<Option<BattleCriticalLoss>> {
        let Some(loss) = self.critical_loss(location)? else {
            return Ok(None);
        };
        let loadout = self.loadout()?;
        let recalculate_propulsion = self.chassis().is_leg(location.section)
            && matches!(
                loss,
                BattleCriticalLoss::System {
                    system: BattleSystem::ShoulderOrHip
                }
            )
            || self.chassis().is_leg(location.section)
                && !self.critical_destroyed(CriticalLocation {
                    section: location.section,
                    slot: 0,
                })
                && matches!(
                    loss,
                    BattleCriticalLoss::System {
                        system: BattleSystem::UpperActuator
                            | BattleSystem::LowerActuator
                            | BattleSystem::HandOrFootActuator
                    }
                );
        let gyro_before = if matches!(
            loss,
            BattleCriticalLoss::System {
                system: BattleSystem::Gyro
            }
        ) {
            Some((self.gyro_damage(), self.gyro_piloting_modifier()))
        } else {
            None
        };
        self.lost_criticals.insert(location);
        if let Some((damage, modifier)) = gyro_before {
            self.record_gyro_critical(damage, modifier);
        }
        if recalculate_propulsion {
            self.recalculate_actuators();
        }
        if let BattleCriticalLoss::Weapon { index, .. } = loss {
            self.weapon_damage_jams.remove(&index);
            self.weapon_failures.remove(&index);
        }
        if self.carried_club.map(super::BattleArm::section) == Some(location.section)
            && matches!(
                loss,
                BattleCriticalLoss::System {
                    system: BattleSystem::HandOrFootActuator
                }
            )
        {
            self.carried_club = None;
        }
        if matches!(
            loss,
            BattleCriticalLoss::System {
                system: BattleSystem::ArtemisIv
            }
        ) {
            // The reference critical handler uses the raw value as a same-section zero-based slot.
            let data =
                &self.definition().sections[&location.section].criticals[&location.slot].data;
            let slot = if data == "-" { 0 } else { data.parse::<u8>()? };
            for (index, mount) in loadout.weapons.iter().enumerate() {
                if mount.criticals[0]
                    == (CriticalLocation {
                        section: location.section,
                        slot,
                    })
                    && let Some(&mode) = self.ammunition_modes.get(&index)
                    && mode.munition() == super::BattleAmmunitionMode::Artemis
                {
                    // Losing the controller keeps an MML's long-range family selected.
                    let mode = mode.with_munition(super::BattleAmmunitionMode::Normal);
                    if mode == super::BattleAmmunitionMode::Normal {
                        self.ammunition_modes.remove(&index);
                    } else {
                        self.ammunition_modes.insert(index, mode);
                    }
                }
            }
        }
        if let BattleCriticalLoss::Weapon { index, .. } = loss
            && loadout.weapons[index].weapon.is_ams()
        {
            self.ams_enabled = false;
        }
        if let BattleCriticalLoss::Weapon {
            index,
            explosion_damage,
        } = loss
            && explosion_damage > 0
        {
            self.lost_criticals
                .extend(loadout.weapons[index].criticals.iter().copied());
        }
        if matches!(
            loss,
            BattleCriticalLoss::System {
                system: BattleSystem::HeatSink
            }
        ) && self.definition().has_double_heat_sinks()
        {
            let groups = loadout.heat_sink_groups(self.definition().heat_sink_slots())?;
            let group = groups
                .iter()
                .find(|group| group.contains(&location))
                .expect("installed sink group");
            self.lost_criticals.extend(group.iter().copied());
        }
        if matches!(
            loss,
            BattleCriticalLoss::System {
                system: BattleSystem::JumpJet
            }
        ) && self.definition().has_special("ImprovedJJ_Tech")
        {
            let groups = loadout.jump_jet_groups(true)?;
            let group = groups
                .iter()
                .find(|group| group.contains(&location))
                .expect("installed jet group");
            self.lost_criticals.extend(group.iter().copied());
        }
        if let BattleCriticalLoss::Ammunition { index, .. } = loss {
            self.ammunition[index] = 0;
            self.live_mass.invalidate();
        }
        if matches!(
            loss,
            BattleCriticalLoss::System {
                system: BattleSystem::LightProbe
            }
        ) {
            self.critical_conditions.lose_light_probe();
        }
        self.reconcile_damage();
        Ok(Some(loss))
    }

    /// Damage clamps controls; destruction cancels power and releases the cockpit claim.
    pub(super) fn reconcile_damage(&mut self) {
        // Physically destroyed slots no longer carry component degradation or its firing penalties.
        self.weapon_damage.retain(|damage| {
            !self.lost_criticals.contains(&damage.location)
                && self
                    .sections
                    .get(&damage.location.section)
                    .is_some_and(|section| section.internal > 0)
        });
        self.weapon_damage_jams = self
            .weapon_damage_jams
            .iter()
            .copied()
            .filter(|index| self.weapon_intact(*index).unwrap_or(false))
            .collect();
        self.weapon_failures = self
            .weapon_failures
            .iter()
            .filter(|(index, _)| self.weapon_intact(**index).unwrap_or(false))
            .map(|(index, failure)| (*index, *failure))
            .collect();
        self.reconcile_electronics();
        if !self.c3_operational().unwrap_or(false) {
            self.c3_network = None;
        }
        if !self
            .c3_hardware()
            .is_ok_and(|hardware| hardware.c3i_operational)
        {
            self.c3i_network = None;
        }
        if self
            .carried_club
            .is_some_and(|arm| self.sections()[&arm.section()].internal == 0)
        {
            self.carried_club = None;
        }
        if self.is_destroyed() {
            super::spotter_events::clear(&mut self.spotter_events);
            self.self_destruct = None;
            self.crew_recovery.clear();
            self.stagger = Default::default();
            self.overheat_clock = Default::default();
            self.power = BattlePower::Off;
            self.hide_elapsed = None;
            self.masc.shutdown();
            self.supercharger.shutdown();
            self.reconcile_electronics();
            self.charge.target = None;
            self.carried_club = None;
            self.flight = None;
            self.jump_stabilization = 0;
            self.pilot = None;
            self.stand_timer = None;
            self.hull_down.cancel();
            self.target_lock = None;
            self.weapon_recycle.clear();
            self.stun_remaining = 0;
        }
        let maximum = self.movement_maximum_speed();
        if let Some(motion) = &mut self.motion {
            motion.speed = motion.speed.clamp(-maximum * 2.0 / 3.0, maximum);
            motion.desired_speed = motion.desired_speed.clamp(-maximum * 2.0 / 3.0, maximum);
            if maximum == 0.0 {
                motion.desired_heading = motion.heading;
            }
        }
    }
}

/// Destroy a selected slot within the caller's world transaction, returning secondary effects.
pub fn destroy_unit_critical(
    world: &mut crate::World,
    id: crate::ObjectId,
    location: CriticalLocation,
) -> Result<Option<BattleCriticalLoss>> {
    anyhow::ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Unit is unavailable"
    );
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?;
    unit.validate()?;
    world
        .btech
        .constructed
        .get_mut(&id)
        .unwrap()
        .destroy_critical(location)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test Atlas with its AC/20 swapped for `item` and a bin of its ammunition.
    fn atlas(item: &str, slots: &str, rounds: u16) -> BattleUnit {
        let source = include_str!("../../tests/fixtures/btech/mechs/AS7-D.toml")
            .replace(
                "{ at = \"1-10\", item = \"IS.AC/20\" }",
                &format!("{{ at = \"{slots}\", item = \"{item}\" }}"),
            )
            .replace(
                "{ at = \"11-12\", item = \"Ammo_IS.AC/20\", rounds = 5 }",
                &format!("{{ at = 11, item = \"Ammo_{item}\", rounds = {rounds} }}"),
            );
        BattleUnit::from_template(super::super::BattleTemplate::parse("AS7-D", &source).unwrap())
            .unwrap()
    }

    /// The explosion a critical hit on the right torso's first slot would release.
    fn explosion(unit: &BattleUnit) -> u8 {
        let location = CriticalLocation {
            section: BattleSection::RightTorso,
            slot: 0,
        };
        match unit.critical_loss(location).unwrap() {
            Some(BattleCriticalLoss::Weapon {
                explosion_damage, ..
            }) => explosion_damage,
            other => panic!("expected a weapon loss, got {other:?}"),
        }
    }

    #[test]
    fn rotary_autocannons_explode_only_while_jammed() {
        let mut unit = atlas("IS.RotaryAC/5", "1-6", 20);
        let index = unit
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == super::super::BattleWeapon::RotaryAc5)
            .unwrap();
        assert_eq!(explosion(&unit), 0);
        assert!(unit.jam_weapon(index).unwrap());
        assert_eq!(explosion(&unit), 5);

        let mut gauss = atlas("IS.GaussRifle", "1-7", 8);
        assert_eq!(explosion(&gauss), 20);
        let index = gauss
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == super::super::BattleWeapon::GaussRifle)
            .unwrap();
        gauss.jam_weapon(index).unwrap();
        assert_eq!(explosion(&gauss), 20);
    }
}
