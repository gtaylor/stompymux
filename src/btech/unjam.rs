//! Timed ammunition-feed recovery on committed simulation seconds.
//!
//! Every jammed ballistic weapon is cleared with the rotary autocannon procedure (Total Warfare
//! p. 140, Tactical Operations p. 73): declare the attempt, hold to walking speed with no weapon
//! attacks, then make a Gunnery Skill Roll at +3. A broken feed cannot be cleared in the field.
use super::{Mech, MessageTarget as Recipient, Power};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// One active attempt per unit; expiry resolves against current crew, equipment and supply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unjam {
    pub weapon_index: usize,
    pub remaining: u8,
}

impl super::Vehicle {
    /// The saved active ammunition-feed clearing attempt, including remaining seconds.
    pub fn unjam(&self) -> Option<Unjam> {
        self.unjam
    }
}

impl Mech {
    /// Pending recovery, including its saved countdown.
    pub fn unjam(&self) -> Option<Unjam> {
        self.unjam
    }
}

/// Start shaking a feed loose; the player must be walking or slower with no weapons recycling.
pub fn begin_unjam(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<String> {
    super::power::controlled(world, id, pilot)?;
    let state = super::unjam_unit::state(world, id, index)?;
    ensure!(
        state.power == Power::Running && !state.destroyed,
        "Start the unit first"
    );
    ensure!(state.ready.intact, "That weapon has been destroyed");
    ensure!(
        state.ready.weapon.profile().ammunition_per_ton > 0,
        "Weapon does not use an ammunition feed"
    );
    ensure!(
        state.jammed,
        "The ammo feed mechanism for that weapon is not jammed."
    );
    ensure!(
        !state.broken,
        "The ammo feed on that weapon is broken beyond field repair."
    );
    ensure!(
        !state.jumping,
        "You can't unjam the ammo feed while jumping!"
    );
    ensure!(
        state.desired_speed <= state.cruise_speed + 0.1,
        "You can't unjam the ammo feed while running!"
    );
    ensure!(!state.recycling, "You have weapons recycling.");
    ensure!(
        state.pending.is_none(),
        "You are already unjamming a weapon!"
    );
    *super::unjam_unit::pending(world, id)? = Some(Unjam {
        weapon_index: index,
        remaining: 60,
    });
    Ok(format!(
        "You begin to shake the jammed ammo loose on weapon #{index}"
    ))
}

/// Recovery feedback owned by the enclosing host transaction.
#[derive(Default)]
pub(crate) struct UnjamReport {
    messages: Vec<(Recipient, String)>,
}

/// Advance tactical attempts atomically. Character attempts require the host action.
/// Expiry consumes one Gunnery Skill Roll at +3 and at most one discarded round.
pub fn advance_unjamming(
    world: &mut World,
    extended_gunnery: bool,
) -> Result<Vec<(Recipient, String)>> {
    ensure!(
        !world
            .btech
            .constructed_units()
            .iter()
            .filter_map(|(id, unit)| unit.unjam().map(|_| id))
            .chain(
                world
                    .btech
                    .vehicles()
                    .iter()
                    .filter_map(|(id, unit)| unit.unjam().map(|_| id))
            )
            .any(|id| {
                world.objects.get(id).is_some_and(|object| {
                    object.flags.contains(crate::Flag::InCharacter)
                        && !object.flags.contains(crate::Flag::Going)
                })
            }),
        "Character unjamming requires a host action"
    );
    Ok(advance_unjamming_in_action(world, extended_gunnery)?.messages)
}

/// Resolve character-capable attempts without publishing; the simulation tick owns rollback.
/// Every jammed weapon is cleared with the rotary autocannon procedure: a Gunnery Skill Roll
/// at +3 (Total Warfare p. 140, Tactical Operations p. 73).
pub(crate) fn advance_unjamming_in_action(
    world: &mut World,
    extended_gunnery: bool,
) -> Result<UnjamReport> {
    world.attempt(|world| {
        let ids: Vec<_> = world
            .btech
            .constructed_units()
            .iter()
            .filter_map(|(id, unit)| unit.unjam().map(|_| *id))
            .chain(
                world
                    .btech
                    .vehicles()
                    .iter()
                    .filter_map(|(id, unit)| unit.unjam().map(|_| *id)),
            )
            .filter(|id| {
                world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(crate::Flag::Going))
            })
            .collect();
        let mut messages = Vec::new();
        for id in ids {
            let pending = super::unjam_unit::pending(world, id)?;
            let attempt = pending.as_mut().unwrap();
            attempt.remaining -= 1;
            if attempt.remaining > 0 {
                continue;
            }
            let index = attempt.weapon_index;
            *pending = None;
            let state = super::unjam_unit::state(world, id, index)?;
            if state.power != Power::Running
                || !state.ready.intact
                || !state.jammed
                || state.broken
            {
                continue;
            }
            if state
                .pilot
                .is_some_and(|pilot| world.btech.unconscious(pilot))
            {
                continue;
            }
            let weapon = state.ready.weapon;
            let name = weapon.name();
            let Some(bin) = state.bin else {
                super::unjam_unit::clear(world, id, index, None)?;
                messages.push((Recipient::Unit(id), format!("You finish bouncing around and realize you no longer have ammo for your {name}!")));
                continue;
            };
            let recipient = state
                .pilot
                .map(Recipient::Player)
                .unwrap_or(Recipient::Unit(id));
            let target = super::unit_gunnery_target(world, id, index, extended_gunnery)? + 3;
            let roll = super::dice::unit_dice_mut(world, id)?.generic_roll();
            messages.push((recipient, "You make a roll to unjam the weapon!".into()));
            messages.push((
                recipient,
                format!("Modified Gunnery Skill: BTH {target}\tRoll: {roll}"),
            ));
            if i16::from(roll) < target {
                messages.push((Recipient::Unit(id), "Your attempt to remove the jammed slug fails. You'll need to try again to clear it.".into()));
                continue;
            }
            super::unjam_unit::clear(world, id, index, Some(bin))?;
            messages.push((
                Recipient::Unit(id),
                format!("You manage to clear the jam on your {name}!"),
            ));
            messages.extend(
                super::observer_messages(world, id, "ejects a mangled shell!")
                    .into_iter()
                    .map(|(id, text)| (Recipient::Unit(id), text)),
            );
        }
        Ok(UnjamReport { messages })
    })
}

/// Publish feedback under the caller's checkpoint.
pub(crate) fn publish_unjamming(scripts: &crate::Scripts, report: &UnjamReport) -> Result<()> {
    for (recipient, text) in &report.messages {
        super::notify_message(scripts, *recipient, text)?;
    }
    Ok(())
}

/// Advance and publish recovery, restoring countdowns, ammunition and output on failure.
pub fn advance_unjamming_action(
    scripts: &crate::Scripts,
    config: &crate::Config,
    extended_gunnery: bool,
) -> Result<()> {
    scripts.atomic(|_| {
        let report =
            advance_unjamming_in_action(&mut scripts.world.borrow_mut(), extended_gunnery)?;
        publish_unjamming(scripts, &report)?;
        scripts.world.borrow().validate_action(config)
    })
}

/// Shared bounded cockpit selection parser.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, begin_unjam)
}
