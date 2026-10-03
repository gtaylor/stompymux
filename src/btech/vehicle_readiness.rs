//! Vehicle weapon reservations publish ammunition and recycle state for an enclosing attack transaction.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Reserved firing cycle; attack effects still belong to the enclosing transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Apply attack effects before publishing the enclosing firing transaction"]
pub struct BattleVehicleWeaponUse {
    pub weapon: BattleWeapon,
    pub ammunition: Vec<BattleAmmunitionDraw>,
    pub ammunition_mode: BattleAmmunitionMode,
    /// Effective behavior after supply fallback, required when resolving this attack.
    pub fire_mode: BattleFireMode,
    /// Supply-limited gatling damage rolled before the enclosing attack's hit dice.
    pub gatling_damage: Option<u8>,
    /// False for a failed Streak lock or a loader failure; the launch result describes consequences.
    pub launched: bool,
    /// Already applied weapon heat for this launch.
    pub heat: u8,
}

impl From<BattleVehicleWeaponUse> for BattleWeaponUse {
    /// Common expenditure facts are independent of the shooter's heat storage or anatomy.
    fn from(value: BattleVehicleWeaponUse) -> Self {
        Self {
            weapon: value.weapon,
            ammunition: value.ammunition,
            fire_mode: value.fire_mode,
            heat: value.heat,
            damage_penalty: 0,
            critical_failure: None,
            gatling_damage: value.gatling_damage,
            ammunition_mode: value.ammunition_mode,
        }
    }
}

impl BattleVehicle {
    /// Resolve burst fallback with the common firing rule and this vehicle's ammunition feed.
    pub(super) fn effective_fire_mode(&self, index: usize) -> Result<BattleFireMode> {
        let loadout = self.loadout()?;
        self.effective_fire_mode_with_loadout(&loadout, index)
    }

    /// Preserve live supply fallback while sharing this immutable equipment projection.
    pub(crate) fn effective_fire_mode_with_loadout(
        &self,
        loadout: &super::BattleVehicleLoadout,
        index: usize,
    ) -> Result<BattleFireMode> {
        ensure!(index < loadout.weapons.len(), "Weapon index out of bounds");
        super::fire_mode::effective_mode(
            self.fire_modes.get(&index).copied().unwrap_or_default(),
            |rounds| self.ammunition_feed_with_loadout(loadout, index, rounds),
        )
    }

    /// Non-normal firing selections keyed by zero-based weapon index.
    pub fn fire_modes(&self) -> &BTreeMap<usize, BattleFireMode> {
        &self.fire_modes
    }

    /// Current firing behavior, initialized from the template only at construction.
    pub fn fire_mode(&self, index: usize) -> Result<BattleFireMode> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        Ok(self.fire_modes.get(&index).copied().unwrap_or_default())
    }

    /// Non-normal selections, keyed by zero-based weapon index.
    pub fn ammunition_modes(&self) -> &BTreeMap<usize, BattleAmmunitionMode> {
        &self.ammunition_modes
    }

    /// Current ammunition selection, independent of matching supply.
    pub fn ammunition_mode(&self, index: usize) -> Result<BattleAmmunitionMode> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        Ok(self
            .ammunition_modes
            .get(&index)
            .copied()
            .unwrap_or_default())
    }

    /// Active weapon countdowns, paused while power is off.
    pub fn weapon_recycle(&self) -> &BTreeMap<usize, u16> {
        &self.weapon_recycle
    }

    /// One-shot launchers that cannot draw another salvo.
    pub fn spent_launchers(&self) -> &BTreeSet<usize> {
        &self.spent_launchers
    }

    /// Mechanical readiness; full firing authority and targeting remain separate.
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
        loadout: &BattleVehicleLoadout,
        index: usize,
    ) -> Result<BattleWeaponReadiness> {
        let mount = loadout
            .weapons
            .get(index)
            .context("Weapon index out of bounds")?;
        let mechanics = self.weapon_mechanics_with_loadout(loadout, index)?;
        let intact = mechanics.intact;
        let spent = self.spent_launchers.contains(&index);
        let ammunition = if mount.one_shot {
            u32::from(!spent)
        } else {
            loadout
                .ammunition
                .iter()
                .enumerate()
                .filter(|(_, bin)| {
                    bin.weapon == mount.weapon
                        && bin.mode
                            == self
                                .ammunition_modes
                                .get(&index)
                                .copied()
                                .unwrap_or_default()
                        && !self.critical_unavailable(bin.location)
                })
                .map(|(index, _)| u32::from(self.ammunition()[index]))
                .sum()
        };
        let recycle_remaining = self.weapon_recycle.get(&index).copied().unwrap_or(0);
        let posture_ready = !mechanics.covered;
        Ok(BattleWeaponReadiness {
            weapon: mount.weapon,
            intact,
            ammunition,
            recycle_remaining,
            posture_ready,
            spent,
            jammed: self.weapon_failures.contains_key(&index)
                || self.jammed_weapons.contains(&index),
            ready: mechanics.admits()
                && self.power() == BattlePower::Running
                && !self.is_destroyed()
                && self.turret_repairs().is_empty()
                && self.pod_removal().is_none()
                && !self.jammed_weapons.contains(&index)
                && !spent
                && (mount.weapon.profile().ammunition_per_ton == 0 || ammunition > 0),
        })
    }

    /// Vehicle anatomy supplies the same mechanical admission inputs as Mechs.
    pub(super) fn weapon_mechanics(
        &self,
        index: usize,
    ) -> Result<super::weapon_admission::WeaponMechanics> {
        let loadout = self.loadout()?;
        self.weapon_mechanics_with_loadout(&loadout, index)
    }

    fn weapon_mechanics_with_loadout(
        &self,
        loadout: &BattleVehicleLoadout,
        index: usize,
    ) -> Result<super::weapon_admission::WeaponMechanics> {
        let mount = loadout
            .weapons
            .get(index)
            .context("Weapon index out of bounds")?;
        Ok(super::weapon_admission::WeaponMechanics {
            weapon: mount.weapon,
            intact: !self.critical_unavailable(mount.criticals[0])
                && !self.powered_down_weapons.contains(&index),
            stunned: self.crew_stunned(),
            temporary_failure: self.weapon_failures.contains_key(&index),
            recycle_remaining: self.weapon_recycle.get(&index).copied().unwrap_or(0),
            section_recycle: None,
            carried_club: false,
            posture_failure: None,
            covered: self.dig.dug_in && mount.criticals[0].section != BattleVehicleSection::Turret,
        })
    }
}

/// Reserve a firing cycle atomically, including supply fallback and a gatling damage roll.
/// Call before rolling attack dice so gatling preparation preserves random-stream ordering.
/// Pass `launched = false` only for a failed Streak lock. It still requires a ready,
/// loaded weapon and starts its normal recycle timer without spending ammunition.
/// The caller resolves targeting and attack effects; ground vehicles retain passive weapon heat.
pub fn reserve_vehicle_weapon(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
    launched: bool,
) -> Result<BattleVehicleWeaponUse> {
    reserve_prepared_weapon(world, id, pilot, index, launched, None)
}

/// Spend a firing cycle using an optional pre-aim roll, without drawing gatling dice twice.
pub(super) fn reserve_prepared_weapon(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
    launched: bool,
    prepared: Option<super::gatling::GatlingPreparation>,
) -> Result<BattleVehicleWeaponUse> {
    super::combat_operator::controlled(world, id, pilot)?;
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    ensure!(
        !vehicle.crew_stunned(),
        "You are too stunned to fire a weapon!"
    );
    ensure!(
        vehicle.turret_repairs().is_empty(),
        "You are too busy unjamming your turret!"
    );
    ensure!(
        vehicle.weapon_failures().get(&index) != Some(&BattleEquipmentFailure::Disabled),
        "The weapons system chirps: 'That weapon has been destroyed!'"
    );
    ensure!(
        vehicle.weapon_readiness(index)?.ready,
        "Weapon is not ready"
    );
    let loadout = vehicle.loadout()?;
    let mount = &loadout.weapons[index];
    let ammunition_mode = vehicle.ammunition_mode(index)?;
    ensure!(!mount.weapon.is_ams(), "That weapon is defensive only!");
    ensure!(
        launched || mount.weapon.is_streak(),
        "Only Streak weapons can fail to lock"
    );
    let timer = u16::from(world.btech.weapon_settings.recycle_seconds(mount.weapon));
    ensure!(timer > 0, "Weapon has no firing cycle");
    let fire_mode = vehicle.effective_fire_mode(index)?;
    let mut dice = vehicle.dice.clone();
    let gatling_damage = match prepared {
        Some(prepared) => prepared.damage(),
        None => super::gatling::prepare(world, id, index, &mut dice)?.damage(),
    };
    let mut rounds = fire_mode.rounds_per_cycle();
    if let Some(damage) = gatling_damage {
        rounds = u16::from(damage) * 3;
    }
    let ammunition =
        if !launched || mount.one_shot || mount.weapon.profile().ammunition_per_ton == 0 {
            Vec::new()
        } else {
            let draws = vehicle.ammunition_feed(index, rounds)?;
            ensure!(
                !draws.is_empty()
                    && (gatling_damage.is_some()
                        || draws.iter().map(|draw| draw.rounds).sum::<u16>() == rounds),
                "No usable ammunition"
            );
            draws
        };
    let heat = fire_mode.launch_heat(mount.weapon, gatling_damage, launched);
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    vehicle.weapon_heat += f64::from(heat);
    vehicle.fired_recently |= launched;
    for draw in &ammunition {
        vehicle
            .expend_ammunition(draw.bin_index, draw.rounds)
            .expect("validated ammunition draw");
    }
    vehicle.dice = dice;
    if fire_mode == BattleFireMode::Normal {
        vehicle.fire_modes.remove(&index);
    }
    if launched && mount.one_shot {
        vehicle.spent_launchers.insert(index);
    }
    vehicle.weapon_recycle.insert(index, timer);
    Ok(BattleVehicleWeaponUse {
        weapon: mount.weapon,
        ammunition,
        fire_mode,
        gatling_damage,
        ammunition_mode,
        launched,
        heat,
    })
}

/// Advance powered vehicle weapon timers inside the ordinary server tick candidate.
pub(super) fn advance(world: &mut World) -> Vec<BattleNotice> {
    let ids: Vec<_> = world
        .btech
        .vehicles()
        .iter()
        .filter(|(id, vehicle)| {
            vehicle.power() == BattlePower::Running
                && !vehicle.weapon_recycle.is_empty()
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .map(|(&id, _)| id)
        .collect();
    let mut notices = Vec::new();
    for id in ids {
        let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
        let loadout = vehicle.loadout().expect("validated vehicle loadout");
        vehicle.weapon_recycle.retain(|index, remaining| {
            *remaining = super::weapon_failure::remaining(
                vehicle.weapon_failures.get(index).copied(),
                *remaining,
            );
            if *remaining > 0 {
                return true;
            }
            notices.push(BattleNotice {
                unit: id,
                text: if vehicle.weapon_failures.remove(index).is_some() {
                    super::weapon_failure::recovery_notice(loadout.weapons[*index].weapon)
                } else {
                    loadout.weapons[*index].weapon.recycle_notice().into()
                },
            });
            false
        });
    }
    notices
}
