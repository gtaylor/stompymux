//! Player controls for gatling firing of machine guns.
use super::{FireMode, FireModeFeedback, Weapon};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// Toggle an intact, recycled machine gun between normal and gatling firing.
pub fn toggle_gatling(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<FireMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        ready.weapon.supports_gatling(),
        "That weapon cannot be set to do gattling fire!"
    );
    Ok(super::weapon_controls::toggle_fire_mode(
        world,
        id,
        index,
        FireMode::Gatling,
    ))
}

/// Native Gatling controls share bounded weapon selections and transaction handling.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_gatling(world, id, pilot, index).map(|mode| mode.gatling_message(index))
    })
}

/// A preparation roll owned by one unpublished firing attempt; consumption must not reroll it.
#[derive(Debug)]
pub(super) struct GatlingPreparation {
    damage: Option<u8>,
}

impl GatlingPreparation {
    /// Transfer the supply-limited result to the chassis expenditure adapter.
    pub fn damage(self) -> Option<u8> {
        self.damage
    }
}

/// Draw gatling preparation before sensor and attack dice, using the shared ammunition feed.
pub(super) fn prepare(
    world: &World,
    shooter: ObjectId,
    index: usize,
    dice: &mut super::Dice,
) -> Result<GatlingPreparation> {
    let unit = world
        .btech
        .unit(shooter)
        .ok_or_else(|| anyhow::anyhow!("Shooter is unavailable"))?;
    let (mode, weapon, rounds) = super::with_unit!(unit, |unit| {
        let mode = unit.fire_mode(index)?;
        let weapon = unit.weapon_readiness(index)?.weapon;
        let rounds = if mode == FireMode::Gatling {
            unit.ammunition_feed(index, MAXIMUM_ROUNDS)?
                .iter()
                .map(|draw| draw.rounds)
                .sum()
        } else {
            0
        };
        (mode, weapon, rounds)
    });
    let damage = if mode == FireMode::Gatling {
        Some(roll_damage(weapon, rounds, dice)?)
    } else {
        None
    };
    Ok(GatlingPreparation { damage })
}

/// The most rounds a rapid-fire burst can use: three for each point of a heavy machine gun's
/// best roll of 7.
const MAXIMUM_ROUNDS: u16 = 21;

/// Roll supply-limited rapid-fire machine gun damage before the attack roll for either unit
/// class (Tactical Operations p. 100): 1D6, 1D6-1 (at least 1) for a light machine gun and
/// 1D6+1 for a heavy one. The burst uses three rounds per point, so a short supply caps it.
pub(super) fn roll_damage(weapon: Weapon, rounds: u16, dice: &mut super::Dice) -> Result<u8> {
    ensure!(rounds > 0, "No usable ammunition");
    let roll = dice.d6();
    let damage = match weapon {
        Weapon::LightMachineGun | Weapon::ClanLightMachineGun => roll.saturating_sub(1).max(1),
        Weapon::HeavyMachineGun | Weapon::ClanHeavyMachineGun => roll + 1,
        _ => roll,
    };
    Ok(damage.min((rounds.min(MAXIMUM_ROUNDS) / 3).max(1) as u8))
}
