//! Player controls for gatling firing of machine guns.
use super::{FireMode, FireModeFeedback};
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
    let (mode, rounds) = super::with_unit!(unit, |unit| {
        let mode = unit.fire_mode(index)?;
        let rounds = if mode == FireMode::Gatling {
            unit.ammunition_feed(index, 18)?
                .iter()
                .map(|draw| draw.rounds)
                .sum()
        } else {
            0
        };
        (mode, rounds)
    });
    let damage = if mode == FireMode::Gatling {
        Some(roll_damage(rounds, dice)?)
    } else {
        None
    };
    Ok(GatlingPreparation { damage })
}

/// Roll supply-limited gatling damage before the attack roll for either unit class.
pub(super) fn roll_damage(rounds: u16, dice: &mut super::Dice) -> Result<u8> {
    ensure!(rounds > 0, "No usable ammunition");
    Ok(dice.d6().min((rounds.min(18) / 3).max(1) as u8))
}
