//! iNarc ammunition selection shared by native and Lua weapon controls.
use super::{BattleAmmunitionMode as Mode, BattleWeapon};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// Interpret the cockpit selector; unknown or omitted selectors choose homing ammunition.
pub(crate) fn selector(value: Option<&str>) -> Mode {
    match value
        .and_then(|value| value.chars().next())
        .map(|c| c.to_ascii_uppercase())
    {
        Some('X') => Mode::INarcExplosive,
        Some('Y') => Mode::INarcHaywire,
        Some('E') => Mode::INarcEcm,
        Some('Z') => Mode::INarcNemesis,
        _ => Mode::Normal,
    }
}

/// Select a pod type without toggling an already selected mode off.
pub fn set_inarc_ammunition(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
    mode: Mode,
) -> Result<()> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        ready.weapon == BattleWeapon::INarcBeacon,
        "That weapon is not an INARC launcher!"
    );
    ensure!(
        matches!(
            mode,
            Mode::Normal
                | Mode::INarcExplosive
                | Mode::INarcHaywire
                | Mode::INarcEcm
                | Mode::INarcNemesis
        ),
        "Unsupported iNarc pod selection"
    );
    super::weapon_controls::set_ammunition_mode(world, id, index, mode);
    Ok(())
}

/// Preserve explicit selection feedback when the same pod type is requested again.
pub(crate) fn select(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
    mode: Mode,
) -> Result<String> {
    let previous = world
        .btech
        .vehicles()
        .get(&id)
        .map(|unit| unit.ammunition_mode(index))
        .or_else(|| {
            world
                .btech
                .constructed_units()
                .get(&id)
                .map(|unit| unit.ammunition_mode(index))
        })
        .transpose()?;
    set_inarc_ammunition(world, id, pilot, index, mode)?;
    let name = match mode {
        Mode::INarcExplosive => "Explosive",
        Mode::INarcHaywire => "Haywire",
        Mode::INarcEcm => "ECM",
        Mode::INarcNemesis => "Nemesis",
        _ => "Homing",
    };
    let verb = if previous == Some(mode) {
        "is already set"
    } else {
        "has been set"
    };
    Ok(format!("Weapon {index} {verb} to fire INARC {name} pods"))
}

/// Bounded weapon selection with an optional pod-type argument.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let mode = selector(input.args.split_ascii_whitespace().nth(1));
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        select(world, id, pilot, index, mode)
    })
}
