//! Timed ammunition-feed recovery on committed simulation seconds.
use super::{BattleMessageTarget as Recipient, BattlePower, BattleUnit};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// One active attempt per unit; expiry resolves against current crew, equipment and supply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleUnjam {
    pub weapon_index: usize,
    pub remaining: u8,
}

impl super::BattleVehicle {
    /// The saved active ammunition-feed clearing attempt, including remaining seconds.
    pub fn unjam(&self) -> Option<BattleUnjam> {
        self.unjam
    }
}

impl BattleUnit {
    /// Pending recovery, including its saved countdown.
    pub fn unjam(&self) -> Option<BattleUnjam> {
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
    if world.btech.vehicles().contains_key(&id) {
        super::vehicle_power::controlled(world, id, pilot)?;
    } else {
        super::power::controlled_unit(world, id, pilot)?;
    }
    let state = super::unjam_unit::state(world, id, index)?;
    ensure!(
        state.power == BattlePower::Running && !state.destroyed,
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
    *super::unjam_unit::pending(world, id)? = Some(BattleUnjam {
        weapon_index: index,
        remaining: 60,
    });
    Ok(format!(
        "You begin to shake the jammed ammo loose on weapon #{index}"
    ))
}

/// Recovery feedback and accepted XP diagnostics owned by the enclosing host transaction.
#[derive(Default)]
pub(crate) struct UnjamReport {
    messages: Vec<(Recipient, String)>,
    /// Diagnostic records positioned before their corresponding cockpit message.
    diagnostics: Vec<(usize, super::BattleChannelMessage)>,
    experience_messages: Vec<super::BattleChannelMessage>,
}

/// Advance tactical attempts atomically. Character attempts require the host action.
/// Expiry consumes one applicable skill roll and at most one discarded round.
pub fn advance_unjamming(
    world: &mut World,
    extended_piloting: bool,
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
    Ok(advance_unjamming_inner(world, extended_piloting, extended_gunnery, false)?.messages)
}

/// Resolve character-capable attempts without publishing; the simulation tick owns rollback.
pub(crate) fn advance_unjamming_in_action(
    world: &mut World,
    extended_piloting: bool,
    extended_gunnery: bool,
) -> Result<UnjamReport> {
    advance_unjamming_inner(world, extended_piloting, extended_gunnery, true)
}

/// Shared countdown and recovery resolution for tactical and host-capable callers.
fn advance_unjamming_inner(
    world: &mut World,
    extended_piloting: bool,
    extended_gunnery: bool,
    character: bool,
) -> Result<UnjamReport> {
    let mut candidate = world.clone();
    let ids: Vec<_> = candidate
        .btech
        .constructed_units()
        .iter()
        .filter_map(|(id, unit)| unit.unjam().map(|_| *id))
        .chain(
            candidate
                .btech
                .vehicles()
                .iter()
                .filter_map(|(id, unit)| unit.unjam().map(|_| *id)),
        )
        .filter(|id| {
            candidate
                .objects
                .get(id)
                .is_some_and(|object| !object.flags.contains(crate::Flag::Going))
        })
        .collect();
    let mut messages = Vec::new();
    let mut diagnostics = Vec::new();
    let mut experience_messages = Vec::new();
    for id in ids {
        let pending = super::unjam_unit::pending(&mut candidate, id)?;
        let attempt = pending.as_mut().unwrap();
        attempt.remaining -= 1;
        if attempt.remaining > 0 {
            continue;
        }
        let index = attempt.weapon_index;
        *pending = None;
        let state = super::unjam_unit::state(&candidate, id, index)?;
        if state.power != BattlePower::Running || !state.ready.intact || !state.jammed {
            continue;
        }
        if state
            .pilot
            .is_some_and(|pilot| candidate.btech.unconscious(pilot))
        {
            continue;
        }
        let weapon = state.ready.weapon;
        let name = weapon.name();
        let Some(bin) = state.bin else {
            super::unjam_unit::clear(&mut candidate, id, index, None)?;
            messages.push((Recipient::Unit(id), format!("You finish bouncing around and realize you no longer have ammo for your {name}!")));
            continue;
        };
        let recipient = state
            .pilot
            .map(Recipient::Player)
            .unwrap_or(Recipient::Unit(id));
        let success = if weapon.is_rotary() {
            let target = super::unit_gunnery_target(&candidate, id, index, extended_gunnery)? + 3;
            let roll = super::dice::unit_dice_mut(&mut candidate, id)?.generic_roll();
            messages.push((recipient, "You make a roll to unjam the weapon!".into()));
            messages.push((
                recipient,
                format!("Modified Gunnery Skill: BTH {target}\tRoll: {roll}"),
            ));
            i16::from(roll) >= target
        } else {
            let mut check = super::roll_piloting(&mut candidate, id, 0, extended_piloting)?;
            if let Some(diagnostic) = check.diagnostic(true) {
                diagnostics.push((messages.len(), diagnostic));
            }
            if character {
                experience_messages.extend(super::piloting::award_control_check(
                    &mut candidate,
                    id,
                    &mut check,
                    extended_piloting,
                )?);
            }
            if let Some(feedback) = check.messages() {
                messages.extend(feedback.into_iter().map(|text| (recipient, text)));
            }
            check.success
        };
        if !success {
            messages.push((Recipient::Unit(id), "Your attempt to remove the jammed slug fails. You'll need to try again to clear it.".into()));
            continue;
        }
        super::unjam_unit::clear(&mut candidate, id, index, Some(bin))?;
        messages.push((
            Recipient::Unit(id),
            format!("You manage to clear the jam on your {name}!"),
        ));
        messages.extend(
            super::observer_messages(&candidate, id, "ejects a mangled shell!")
                .into_iter()
                .map(|(id, text)| (Recipient::Unit(id), text)),
        );
    }
    *world = candidate;
    Ok(UnjamReport {
        messages,
        diagnostics,
        experience_messages,
    })
}

/// Publish feedback and diagnostics under the caller's checkpoint.
pub(crate) fn publish_unjamming(
    scripts: &crate::Scripts,
    config: &crate::Config,
    report: &UnjamReport,
) -> Result<()> {
    let mut diagnostics = report.diagnostics.iter().peekable();
    for index in 0..=report.messages.len() {
        while diagnostics
            .peek()
            .is_some_and(|(before, _)| *before == index)
        {
            let (_, diagnostic) = diagnostics.next().unwrap();
            super::channels::publish(scripts, config, std::slice::from_ref(diagnostic))?;
        }
        if let Some((recipient, text)) = report.messages.get(index) {
            super::notify_message(scripts, *recipient, text)?;
        }
    }
    super::channels::publish(scripts, config, &report.experience_messages)
}

/// Advance and publish recovery, restoring countdowns, ammunition, XP and output on failure.
pub fn advance_unjamming_action(
    scripts: &crate::Scripts,
    config: &crate::Config,
    extended_piloting: bool,
    extended_gunnery: bool,
) -> Result<()> {
    let before = scripts.world.borrow().clone();
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        let report = advance_unjamming_in_action(
            &mut scripts.world.borrow_mut(),
            extended_piloting,
            extended_gunnery,
        )?;
        publish_unjamming(scripts, config, &report)?;
        scripts.world.borrow().validate(config)
    })();
    if result.is_err() {
        *scripts.world.borrow_mut() = before;
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Shared bounded cockpit selection parser.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, begin_unjam)
}
