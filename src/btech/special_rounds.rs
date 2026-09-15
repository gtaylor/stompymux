//! Smoke and mine missile controls share inventory, authority and feedback across unit classes.
use super::BattleAmmunitionMode;
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// Select the reference cockpit mode: missile launchers accept these controls, artillery does not.
/// Artillery Smoke/Mine supplies remain valid authored payloads with delayed environmental effects.
pub fn toggle_missile_rounds(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
    mode: BattleAmmunitionMode,
) -> Result<BattleAmmunitionMode> {
    ensure!(
        matches!(
            mode,
            BattleAmmunitionMode::Smoke | BattleAmmunitionMode::Mine
        ),
        "Invalid missile round mode"
    );
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        !ready.weapon.is_rocket(),
        "Rocket launchers' mode cannot be altered!"
    );
    ensure!(
        ready.weapon.profile().missiles > 0 && !ready.weapon.is_artillery(),
        "Invalid weapon type!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world, id, index, mode,
    ))
}

impl BattleAmmunitionMode {
    /// Both host interfaces use the same cockpit message after a successful selection.
    pub(crate) fn special_round_message(self, index: usize) -> String {
        let round = match self {
            Self::Smoke => "smoke",
            Self::Mine => "mine",
            _ => {
                return format!("Weapon {index} has been set to fire normal rounds");
            }
        };
        format!("Weapon {index} has been set to fire {round} rounds.")
    }
}

/// Reuse bounded multi-weapon parsing, cockpit authority and world/effect publication.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let mode = if input.name.eq_ignore_ascii_case("firesmoke") {
        BattleAmmunitionMode::Smoke
    } else {
        BattleAmmunitionMode::Mine
    };
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_missile_rounds(world, id, pilot, index, mode)
            .map(|mode| mode.special_round_message(index))
    })
}
