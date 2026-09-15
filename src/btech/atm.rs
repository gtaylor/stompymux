//! ATM ammunition controls reuse indirect-launcher eligibility, feed selection and transaction ordering.
use super::BattleAmmunitionMode;
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

impl BattleAmmunitionMode {
    /// Cockpit wording for the selected ATM ammunition marker; normal profiles remain unchanged.
    pub(crate) fn atm_message(self, index: usize) -> String {
        let label = match self {
            Self::ExtendedRange => "Extended Range",
            Self::HighExplosive => "High Explosive",
            _ => "normal",
        };
        format!("Weapon {index} has been set to fire {label} missiles.")
    }
}

/// Toggle either reference ATM ammunition marker using the shared supported indirect-launcher gate.
pub fn toggle_atm_ammunition(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
    mode: BattleAmmunitionMode,
) -> Result<BattleAmmunitionMode> {
    ensure!(
        matches!(
            mode,
            BattleAmmunitionMode::ExtendedRange | BattleAmmunitionMode::HighExplosive
        ),
        "Select Extended Range or High Explosive ammunition"
    );
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        mode.supports(ready.weapon),
        "That weapon cannot fire this ammunition!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world, id, index, mode,
    ))
}

/// Both native mode names share weapon selection and publication rollback.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let mode = if input.name == "atmrange" {
        BattleAmmunitionMode::ExtendedRange
    } else {
        BattleAmmunitionMode::HighExplosive
    };
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_atm_ammunition(world, id, pilot, index, mode).map(|mode| mode.atm_message(index))
    })
}
