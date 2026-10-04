//! Trusted weapons-hold state and shared cockpit admission before any firing intent.
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Read the operator-imposed firing restriction independently of mechanical readiness.
pub fn battle_weapons_hold(world: &World, id: ObjectId) -> Result<bool> {
    let unit = world.btech.unit(id).context("Unit is not constructed")?;
    Ok(unit.weapons_hold())
}

/// Trusted scenario edit; the caller owns administrative authority and transaction publication.
pub fn set_battle_weapons_hold(world: &mut World, id: ObjectId, enabled: bool) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|o| o.kind == Kind::Thing && !o.flags.contains(Flag::Going)),
        "Unit must be a live thing"
    );
    super::with_unit_mut!(
        world
            .btech
            .unit_mut(id)
            .context("Unit is not constructed")?,
        |unit| {
            unit.weapons_hold = enabled;
            Ok(())
        }
    )
}

/// Running cockpit authority precedes hold; hold precedes argument decoding and cover loss.
pub(super) fn admit(world: &World, id: ObjectId, pilot: ObjectId) -> Result<()> {
    super::power::controlled_running_unit(world, id, pilot)?;
    check(world, id)
}

/// Apply the physical unit's firing hold after the caller has admitted its operator.
pub(super) fn check(world: &World, id: ObjectId) -> Result<()> {
    ensure!(
        !battle_weapons_hold(world, id)?,
        "Currently in weapons hold. Unable to fire weapons."
    );
    Ok(())
}

/// Damage attribution can warn a held attacker without preventing the accepted damage.
/// Self-attributed environmental damage has no distinct attacker audience.
pub(super) fn damage_notice(
    world: &World,
    attacker: Option<ObjectId>,
    target: ObjectId,
) -> Option<super::BattleNotice> {
    let attacker = attacker.filter(|id| *id != target)?;
    battle_weapons_hold(world, attacker)
        .ok()
        .filter(|held| *held)
        .map(|_| super::BattleNotice {
            unit: attacker,
            text: "You are currently in weapons hold!".into(),
        })
}
