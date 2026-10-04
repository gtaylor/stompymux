//! Automatic anti-missile defenses, with expenditure owned by the enclosing shot transaction.
use super::{BattleNotice, BattlePower, BattleUnit, BattleWeapon};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};
use serde::Serialize;

/// One defensive activation against an admitted missile hit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleAmsReport {
    pub weapon_index: usize,
    /// Bin that fed the defense; laser AMS has none.
    pub ammunition_bin: Option<usize>,
    pub roll: u8,
    pub ammunition_spent: u16,
    pub shot_down: u8,
}

impl BattleUnit {
    /// Pilot-selected automatic defense state, initially disabled.
    pub fn ams_enabled(&self) -> bool {
        self.ams_enabled
    }
}

impl super::BattleVehicle {
    /// Pilot-selected automatic defense state, initially disabled and saved with the vehicle.
    pub fn ams_enabled(&self) -> bool {
        self.ams_enabled
    }
}

/// Set the whole unit's defense state after ordinary cockpit and power checks.
pub fn set_ams(world: &mut World, id: ObjectId, pilot: ObjectId, enabled: bool) -> Result<()> {
    super::targeting::controlled(world, id, pilot)?;
    ensure!(
        super::ams_unit::available(world, id)?,
        "This mech is not equipped with AMS"
    );
    super::ams_unit::set_enabled(world, id, enabled)
}

/// Select the first ready mount, then its first usable normal-ammunition bin; one activation per attack.
/// The attacker owns interception dice. A shortage caps expenditure, not interception capacity.
pub(super) fn intercept(
    world: &mut World,
    attacker: ObjectId,
    target: ObjectId,
    incoming: u8,
) -> Result<Option<BattleAmsReport>> {
    if incoming == 0
        || !super::ams_unit::enabled(world, target)?
        || super::scanner::scanner_unit(world, target)
            .is_none_or(|unit| unit.power != BattlePower::Running)
        || !super::ams_unit::available(world, target)?
    {
        return Ok(None);
    }
    let Some(super::ams_unit::Defense { index, weapon, bin }) =
        super::ams_unit::select(world, target)?
    else {
        return Ok(None);
    };
    let clan = matches!(
        weapon,
        BattleWeapon::ClanAntiMissileSystem | BattleWeapon::ClanLaserAms
    );
    let dice = super::dice::unit_dice_mut(world, attacker)?;
    let roll = if clan { dice.generic_roll() } else { dice.d6() };
    let count = roll.min(incoming);
    let spent = super::ams_unit::expend(world, target, index, weapon, bin, u16::from(count))?;
    Ok(Some(BattleAmsReport {
        weapon_index: index,
        ammunition_bin: bin,
        roll,
        ammunition_spent: spent,
        shot_down: count,
    }))
}

impl BattleAmsReport {
    /// Defense feedback after cluster resolution determines whether every potential hit was intercepted.
    pub(super) fn notices(
        &self,
        attacker: ObjectId,
        target: ObjectId,
        hits: Option<u8>,
    ) -> Vec<BattleNotice> {
        let Some(hits) = hits else {
            return vec![BattleNotice {
                unit: target,
                text: "Your Anti-Missile System activates and shoots at the incoming missiles!"
                    .into(),
            }];
        };
        let all = self.shot_down >= hits;
        vec![
            BattleNotice {
                unit: attacker,
                text: if all {
                    "All of your missiles are shot down by the target!".into()
                } else {
                    format!(
                        "The target shoots down {} of your missiles!",
                        self.shot_down
                    )
                },
            },
            BattleNotice {
                unit: target,
                text: if all {
                    "Your Anti-Missile System activates and shoots all the incoming missiles!"
                        .into()
                } else {
                    format!(
                        "Your Anti-Missile System activates and shoots down {} incoming missiles!",
                        self.shot_down
                    )
                },
            },
        ]
    }
}

/// Shared native/Lua switch publication, including rollback when cockpit delivery fails.
pub(crate) fn configure(
    scripts: &crate::Scripts,
    id: ObjectId,
    pilot: ObjectId,
    enabled: Option<bool>,
) -> Result<bool> {
    scripts.atomic(|before| {
        super::targeting::controlled(before, id, pilot)?;
        let enabled = enabled.unwrap_or(!super::ams_unit::enabled(before, id)?);
        set_ams(&mut scripts.world.borrow_mut(), id, pilot, enabled)?;
        super::notify_unit_text(
            scripts,
            id,
            if enabled {
                "Anti-Missile System turned ON"
            } else {
                "Anti-Missile System turned OFF"
            },
        )?;
        Ok(enabled)
    })
}

/// The cockpit command toggles the whole defense system; the optional weapon argument is ignored.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    _: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let id = ctx
        .scripts
        .world
        .borrow()
        .objects
        .get(&ctx.player)
        .and_then(|p| p.location);
    let result = id
        .ok_or_else(|| anyhow::anyhow!("Enter a unit first"))
        .and_then(|id| configure(ctx.scripts, id, ctx.player, None));
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
