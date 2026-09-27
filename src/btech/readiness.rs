//! Ammunition expenditure and committed-second recycle timers for conventional weapons.
use super::{BattleNotice, BattlePower, BattleUnit, BattleWeapon};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::BTreeMap;

/// Mechanical readiness; authorization and target-dependent firing checks remain separate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleWeaponReadiness {
    pub weapon: BattleWeapon,
    pub intact: bool,
    pub ammunition: u32,
    pub recycle_remaining: u16,
    /// Whether chassis-specific prone support permits this mount.
    pub posture_ready: bool,
    /// True after a self-contained salvo has launched, including misses.
    pub spent: bool,
    /// Ammunition-feed failure; independent of destruction, supply and recycling.
    pub jammed: bool,
    pub ready: bool,
}

/// Expenditure that must accompany the attack, including misses; heat has already been added to the unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleWeaponUse {
    pub weapon: BattleWeapon,
    /// Actual draws from live bins; empty for energy, self-contained or unlaunched shots.
    pub ammunition: Vec<super::BattleAmmunitionDraw>,
    /// Effective mode after supply-dependent fallback.
    pub fire_mode: super::BattleFireMode,
    pub heat: u8,
    /// Energy damage lost to the firing mount’s damaged focusing components.
    pub damage_penalty: u8,
    /// Enhanced critical component responsible for a terminal launch failure.
    pub critical_failure: Option<super::BattleWeaponDamageKind>,
    /// Supply-limited gatling roll, used for both heat and pre-glancing damage.
    pub gatling_damage: Option<u8>,
    pub ammunition_mode: super::BattleAmmunitionMode,
}

impl BattleUnit {
    /// Active recycle countdowns keyed by zero-based resolved weapon index.
    pub fn weapon_recycle(&self) -> &BTreeMap<usize, u16> {
        &self.weapon_recycle
    }

    /// Inspect functioning equipment, remaining matching salvos and recycle time.
    pub fn weapon_readiness(&self, index: usize) -> Result<BattleWeaponReadiness> {
        let _measurement = crate::btech::autopilot::diagnostics::measure(
            crate::btech::autopilot::diagnostics::Category::Readiness,
        );
        let loadout = self.loadout()?;
        self.weapon_readiness_with_loadout(&loadout, index)
    }

    /// Inspect every resolved mount while sharing one immutable equipment projection.
    pub(crate) fn weapon_readiness_batch(&self) -> Result<Vec<BattleWeaponReadiness>> {
        let _measurement = crate::btech::autopilot::diagnostics::measure(
            crate::btech::autopilot::diagnostics::Category::Readiness,
        );
        let loadout = self.loadout()?;
        loadout
            .weapons
            .iter()
            .enumerate()
            .map(|(index, _)| self.weapon_readiness_with_loadout(&loadout, index))
            .collect()
    }

    /// Inspect live state against an equipment projection from the same immutable unit.
    pub(crate) fn weapon_readiness_with_loadout(
        &self,
        loadout: &super::BattleLoadout,
        index: usize,
    ) -> Result<BattleWeaponReadiness> {
        let weapon = loadout
            .weapons
            .get(index)
            .context("Weapon index out of bounds")?
            .weapon;
        let mechanics = self.weapon_mechanics_with_loadout(loadout, index)?;
        let intact = mechanics.intact;
        let mode = self
            .ammunition_modes
            .get(&index)
            .copied()
            .unwrap_or_default();
        let spent = self.spent_launchers.contains(&index);
        let one_shot = loadout.weapons[index].one_shot;
        let ammunition = if one_shot {
            u32::from(!spent)
        } else {
            loadout
                .ammunition
                .iter()
                .enumerate()
                .filter(|(_, bin)| {
                    bin.weapon == weapon
                        && bin.mode == mode
                        && !self.critical_unavailable(bin.location)
                })
                .map(|(i, _)| u32::from(self.ammunition[i]))
                .sum()
        };
        let recycle_remaining = self.weapon_recycle.get(&index).copied().unwrap_or(0);
        let posture_ready = mechanics.posture_failure.is_none();
        Ok(BattleWeaponReadiness {
            weapon,
            intact,
            ammunition,
            recycle_remaining,
            posture_ready,
            spent,
            jammed: self.jammed_weapons.contains(&index)
                || self.weapon_damage_jams.contains(&index)
                || self.weapon_failures.contains_key(&index),
            ready: self.power() == BattlePower::Running
                && !self.is_destroyed()
                && mechanics.check().is_ok()
                && !spent
                && !self.jammed_weapons.contains(&index)
                && self.unjam.is_none()
                && (weapon.profile().ammunition_per_ton == 0 || ammunition > 0),
        })
    }

    /// Resolve anatomy into the shared pre-target firing conditions.
    pub(super) fn weapon_mechanics(
        &self,
        index: usize,
    ) -> Result<super::weapon_admission::WeaponMechanics> {
        let loadout = self.loadout()?;
        self.weapon_mechanics_with_loadout(&loadout, index)
    }

    fn weapon_mechanics_with_loadout(
        &self,
        loadout: &super::BattleLoadout,
        index: usize,
    ) -> Result<super::weapon_admission::WeaponMechanics> {
        let mount = loadout
            .weapons
            .get(index)
            .context("Weapon index out of bounds")?;
        let section = mount.criticals[0].section;
        Ok(super::weapon_admission::WeaponMechanics {
            weapon: mount.weapon,
            intact: mount
                .criticals
                .iter()
                .all(|location| !self.critical_unavailable(*location))
                && !self.powered_down_weapons.contains(&index),
            stunned: self.stun_remaining() > 0,
            temporary_failure: self.weapon_damage_jams.contains(&index)
                || self.weapon_failures.contains_key(&index),
            recycle_remaining: self.weapon_recycle.get(&index).copied().unwrap_or(0),
            section_recycle: self
                .limb_recycle
                .contains_key(&section)
                .then_some(self.chassis().section_name(section)),
            carried_club: self
                .carried_club()
                .is_some_and(|arm| arm.section() == section),
            posture_failure: self.prone_support_failure(loadout, section),
            covered: false,
        })
    }

    /// A prone arm weapon needs the opposite arm; other non-leg mounts can use either arm.
    fn prone_support_failure(
        &self,
        loadout: &super::BattleLoadout,
        section: super::BattleSection,
    ) -> Option<&'static str> {
        use super::BattleSection::*;
        if self.posture() != super::BattlePosture::Prone {
            return None;
        }
        if self.chassis() == super::BattleMechChassis::Quad {
            match self.unavailable_legs() {
                0 => return None,
                3.. => return Some("Quads need at least 3 legs to fire while prone."),
                _ => {}
            }
        }
        let available = |arm| {
            self.sections()[&arm].internal > 0
                && !self.limb_recycle.contains_key(&arm)
                && !loadout.weapons.iter().enumerate().any(|(index, mount)| {
                    mount.criticals[0].section == arm
                        && self
                            .weapon_recycle
                            .get(&index)
                            .is_some_and(|remaining| *remaining > 0)
                })
        };
        match section {
            LeftLeg | RightLeg => Some("You cannot fire leg mounted weapons when prone."),
            LeftArm if !available(RightArm) => {
                Some("Your currently can't use your Right Arm to prop yourself up.")
            }
            RightArm if !available(LeftArm) => {
                Some("You currently can't use your Left Arm to prop yourself up.")
            }
            LeftArm | RightArm => None,
            _ if !available(LeftArm) && !available(RightArm) => {
                Some("You currently don't have any arms to spare to prop yourself up.")
            }
            _ => None,
        }
    }
}

/// Spend a firing cycle in the selected mode and start its recycle timer inside the caller's attack transaction.
/// The caller must apply hit/miss effects before committing this world. Returned heat is informational.
pub fn spend_weapon(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleWeaponUse> {
    world.attempt(|world| {
        super::combat_operator::controlled_mech(world, id, pilot)?;
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        ensure!(unit.weapon_readiness(index)?.ready, "Weapon is not ready");
        let mut dice = unit.dice.clone();
        let gatling_damage = super::gatling::prepare(world, id, index, &mut dice)?.damage();
        world.btech.constructed.get_mut(&id).unwrap().dice = dice;
        let result = use_weapon(world, id, pilot, index, true, gatling_damage)?;
        Ok(result)
    })
}

/// Start a firing cycle; a failed Streak lock recycles without expending ammunition or heat.
pub(super) fn use_weapon(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
    launched: bool,
    gatling_damage: Option<u8>,
) -> Result<BattleWeaponUse> {
    super::combat_operator::controlled_mech(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(unit.weapon_readiness(index)?.ready, "Weapon is not ready");
    let loadout = unit.loadout()?;
    let mount = &loadout.weapons[index];
    let weapon = mount.weapon;
    unit.check_spotter_fire(id, index)?;
    ensure!(!weapon.is_ams(), "That weapon is defensive only!");
    let mode = unit.ammunition_mode(index)?;
    ensure!(
        launched || weapon.is_streak(),
        "Only Streak launchers can fail to launch"
    );
    let fire_mode = unit.effective_fire_mode(index)?;
    ensure!(
        gatling_damage.is_some() == (fire_mode == super::BattleFireMode::Gatling),
        "Invalid gatling firing cycle"
    );
    let rounds = if let Some(damage) = gatling_damage {
        ensure!((1..=6).contains(&damage), "Invalid gatling damage");
        u16::from(damage) * 3
    } else {
        fire_mode.rounds_per_cycle()
    };
    let damage_penalty = unit.weapon_damage_effects(index)?.damage;
    let heat = fire_mode.launch_heat(weapon, gatling_damage, launched)
        + if launched {
            unit.weapon_damage_effects(index)?.heat
        } else {
            0
        };
    let ammunition = if !launched || mount.one_shot || weapon.profile().ammunition_per_ton == 0 {
        Vec::new()
    } else {
        let draws = unit.ammunition_feed(index, rounds)?;
        ensure!(
            !draws.is_empty()
                && (gatling_damage.is_some()
                    || draws.iter().map(|draw| draw.rounds).sum::<u16>() == rounds),
            "No usable ammunition"
        );
        draws
    };
    let recycle = world.btech.weapon_settings.recycle_seconds(weapon);
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    if fire_mode == super::BattleFireMode::Normal {
        unit.fire_modes.remove(&index);
    }
    for draw in &ammunition {
        unit.ammunition[draw.bin_index] -= draw.rounds;
        unit.live_mass.invalidate();
    }
    if launched && mount.one_shot {
        unit.spent_launchers.insert(index);
    }
    unit.fired_recently |= launched;
    unit.heat.stored += f64::from(heat);
    unit.weapon_recycle.insert(index, u16::from(recycle));
    Ok(BattleWeaponUse {
        weapon,
        ammunition,
        fire_mode,
        heat,
        damage_penalty,
        critical_failure: None,
        gatling_damage,
        ammunition_mode: mode,
    })
}

/// Advance active, powered weapon timers by one committed second; shutdown pauses their countdown.
pub fn advance_recycle(world: &mut World) -> Vec<BattleNotice> {
    let ids: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .filter(|(id, unit)| {
            unit.power() == BattlePower::Running
                && (!unit.weapon_recycle.is_empty() || !unit.limb_recycle.is_empty())
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .map(|(&id, _)| id)
        .collect();
    let mut notices = super::vehicle_readiness::advance(world);
    for id in ids {
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        let loadout = unit.loadout().expect("validated weapon loadout");
        for index in unit.weapon_recycle.keys().copied().collect::<Vec<_>>() {
            if !unit.weapon_intact(index).expect("validated weapon index") {
                unit.weapon_recycle.remove(&index);
                unit.weapon_failures.remove(&index);
                continue;
            }
            let remaining = super::weapon_failure::remaining(
                unit.weapon_failures.get(&index).copied(),
                unit.weapon_recycle[&index],
            );
            if remaining > 0 {
                unit.weapon_recycle.insert(index, remaining);
                continue;
            }
            unit.weapon_recycle.remove(&index);
            let text = if unit.weapon_failures.remove(&index).is_some() {
                super::weapon_failure::recovery_notice(loadout.weapons[index].weapon)
            } else {
                loadout.weapons[index].weapon.recycle_notice().to_owned()
            };
            notices.push(BattleNotice { unit: id, text });
        }
        for section in unit.limb_recycle.keys().copied().collect::<Vec<_>>() {
            if unit.sections()[&section].internal == 0 {
                unit.limb_recycle.remove(&section);
                continue;
            }
            let remaining = unit.limb_recycle[&section] - 1;
            if remaining > 0 {
                unit.limb_recycle.insert(section, remaining);
                continue;
            }
            unit.limb_recycle.remove(&section);
            notices.push(BattleNotice {
                unit: id,
                text: format!(
                    "[fg=green]Your {} has finished its previous action.[reset]",
                    section.name().replace('_', " ")
                ),
            });
        }
    }
    notices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BattleUnitTemplate, Config, Kind, ObjectId, World};

    #[test]
    fn batch_readiness_matches_each_mount_after_live_changes() {
        let config = Config::load("tests/fixtures/game").unwrap();
        let mut world = World::default();
        let id = world.create(&config, "Readiness batch test".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        BattleUnitTemplate::parse(include_str!("../../tests/fixtures/btech/mechs/JR7-D"))
            .unwrap()
            .create(&mut world, id)
            .unwrap();

        let (ammo_index, ammo_bin, damaged_location, weapon_count) = {
            let unit = world.btech.constructed_units().get(&id).unwrap();
            let loadout = unit.loadout().unwrap();
            let ammo_index = loadout
                .weapons
                .iter()
                .position(|mount| mount.weapon.profile().ammunition_per_ton > 0)
                .unwrap();
            let weapon = loadout.weapons[ammo_index].weapon;
            let ammo_bin = loadout
                .ammunition
                .iter()
                .position(|bin| bin.weapon == weapon)
                .unwrap();
            (
                ammo_index,
                ammo_bin,
                loadout.weapons[ammo_index].criticals[0],
                loadout.weapons.len(),
            )
        };
        world.btech.constructed.get_mut(&id).unwrap().power = BattlePower::Running;
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        unit.ammunition[ammo_bin] = 0;
        unit.weapon_recycle.insert(ammo_index, 3);
        unit.lost_criticals.insert(damaged_location);

        let unit = world.btech.constructed_units().get(&id).unwrap();
        let individual: Vec<_> = (0..weapon_count)
            .map(|index| unit.weapon_readiness(index).unwrap())
            .collect();
        assert_eq!(unit.weapon_readiness_batch().unwrap(), individual);
    }
}
