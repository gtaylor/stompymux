//! Pilot-selected weapon groups shared by Mechs and vehicles; membership uses stable weapon numbers.
use crate::{CommandAction, CommandContext, CommandInput, CommandReport, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Four independent, ordered groups; damage and ammunition depletion do not change membership.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleTics(pub(crate) [BTreeSet<usize>; 4]);

impl BattleTics {
    /// Stored groups may reference only installed weapons within the reference numbering bound.
    pub(super) fn validate(&self, weapons: usize) -> Result<()> {
        ensure!(
            self.0
                .iter()
                .flatten()
                .all(|&index| index < weapons && index < 96),
            "TIC contains an invalid weapon number"
        );
        Ok(())
    }
}

/// Membership changes are idempotent and validated before any unit state changes.
#[derive(Debug, Clone)]
pub enum BattleTicEdit {
    Add(Vec<usize>),
    Remove(Vec<usize>),
    Clear,
}

/// Read a group's ordered weapon numbers under the same cockpit authority as membership edits.
pub fn battle_tic(
    world: &World,
    id: ObjectId,
    pilot: ObjectId,
    group: usize,
) -> Result<Vec<usize>> {
    ensure!(group < 4, "TIC number must be between 0 and 3");
    let (groups, _) = controlled(world, id, pilot)?;
    Ok(groups.0[group].iter().copied().collect())
}

/// Apply a single validated membership edit without depending on chassis-specific firing rules.
pub fn edit_battle_tic(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    group: usize,
    edit: BattleTicEdit,
) -> Result<()> {
    ensure!(group < 4, "TIC number must be between 0 and 3");
    let (groups, count) = controlled(world, id, pilot)?;
    let mut next = groups.clone();
    match edit {
        BattleTicEdit::Add(indices) | BattleTicEdit::Remove(indices)
            if indices.iter().any(|&i| i >= count || i >= 96) =>
        {
            anyhow::bail!("Weapon number is out of bounds");
        }
        BattleTicEdit::Add(indices) => next.0[group].extend(indices),
        BattleTicEdit::Remove(indices) => next.0[group].retain(|i| !indices.contains(i)),
        BattleTicEdit::Clear => next.0[group].clear(),
    }
    crate::btech::with_unit_mut!(world.btech.unit_mut(id).unwrap(), |unit| {
        unit.tics = next;
    });
    Ok(())
}

/// Adapt owned state and cockpit authority; selection and ordering remain chassis-independent.
fn controlled(world: &World, id: ObjectId, pilot: ObjectId) -> Result<(&BattleTics, usize)> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        super::vehicle_power::controlled(world, id, pilot)?;
        return Ok((&unit.tics, unit.loadout()?.weapons.len()));
    }
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    Ok((&unit.tics, unit.loadout()?.weapons.len()))
}

/// Parse bounded comma-separated numbers and inclusive ranges before making any changes.
pub(super) fn selection(text: &str, limit: usize) -> Result<Vec<usize>> {
    ensure!(!text.is_empty(), "Missing selection");
    let mut selected = BTreeSet::new();
    for item in text.split(',') {
        let (first, last) = item.split_once('-').unwrap_or((item, item));
        let first: usize = first.parse().context("Invalid selection")?;
        let last: usize = last.parse().context("Invalid selection")?;
        ensure!(first <= last && last < limit, "Selection is out of bounds");
        selected.extend(first..=last);
    }
    Ok(selected.into_iter().collect())
}

/// Native membership commands reuse the same domain operations exposed to Lua.
pub(crate) fn command(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<CommandAction> {
    let result = (|| -> Result<String> {
        let mut args = input.args.split_whitespace();
        let group_text = args.next().context("Supply a TIC number")?;
        let weapons = args.next();
        ensure!(args.next().is_none(), "Too many arguments");
        let mut world = ctx.scripts.world.borrow_mut();
        let id = world
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        let clearing = input.name == "cleartic" || (input.name == "deltic" && weapons.is_none());
        if clearing {
            ensure!(weapons.is_none(), "Usage: cleartic <TIC selection>");
            let groups = selection(group_text, 4)?;
            controlled(&world, id, ctx.player)?;
            for &group in &groups {
                edit_battle_tic(&mut world, id, ctx.player, group, BattleTicEdit::Clear)?;
            }
            return Ok(groups
                .iter()
                .map(|g| format!("TIC #{g} cleared!"))
                .collect::<Vec<_>>()
                .join("\n"));
        }
        let group: usize = group_text.parse().context("Invalid TIC number")?;
        if input.name == "listtic" {
            ensure!(weapons.is_none(), "Usage: listtic <TIC number>");
            let members = battle_tic(&world, id, ctx.player, group)?;
            return Ok(format!(
                "TIC #{group}: {}",
                if members.is_empty() {
                    "(empty)".into()
                } else {
                    super::firing::group_status(&world, id, &members)?
                }
            ));
        }
        let members = selection(weapons.context("Supply a weapon selection")?, 96)?;
        let edit = if input.name == "addtic" {
            BattleTicEdit::Add(members)
        } else {
            BattleTicEdit::Remove(members)
        };
        edit_battle_tic(&mut world, id, ctx.player, group, edit)?;
        Ok(format!("TIC #{group} updated."))
    })();
    Ok(match result {
        Ok(text) => CommandAction::CommitReply(text),
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}

/// Ordered firing outcome, including rejected attempts that did not consume weapon resources.
#[derive(Debug, Serialize)]
pub struct BattleTicShot {
    pub group: usize,
    pub weapon: usize,
    pub report: Option<super::BattleFireReport>,
    pub rejection: Option<String>,
}

/// Fire selected groups through ordinary weapon actions; fatal host errors roll back the batch.
/// Groups and weapons are ordered and deduplicated, but a weapon in two groups is attempted twice.
pub fn fire_battle_tics(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    groups: Vec<usize>,
    target: impl Into<super::BattleFireTarget>,
) -> Result<Vec<BattleTicShot>> {
    fire_tics(
        scripts,
        config,
        id,
        pilot,
        groups,
        super::fire_target::FireTargetRequest::Target(target.into()),
    )
}

/// Execute native or typed targets per weapon within the same recoverable-shot transaction.
pub(super) fn fire_tics(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    groups: Vec<usize>,
    target: super::fire_target::FireTargetRequest<'_>,
) -> Result<Vec<BattleTicShot>> {
    scripts.atomic(|before| {
        super::weapons_hold::admit(before, id, pilot)?;
        before.validate_action(config)?;
        ensure!(!groups.is_empty(), "Supply a TIC selection");
        let groups: BTreeSet<_> = groups.into_iter().collect();
        let selected = groups
            .into_iter()
            .map(|group| Ok((group, battle_tic(before, id, pilot, group)?)))
            .collect::<Result<Vec<_>>>()?;
        ensure!(running(before, id), "Unit must be started");
        let positioned = before.btech.vehicles().get(&id).map_or_else(
            || before.btech.constructed_units()[&id].position().is_some(),
            |unit| unit.position().is_some(),
        );
        ensure!(positioned, "Unit must be on a map");
        let posture = before.btech.constructed_units().get(&id).map(|u| u.posture);
        let mut shots = Vec::new();
        for (group, members) in selected {
            super::notify_message(
                scripts,
                super::BattleMessageTarget::Player(pilot),
                &format!("Firing weapons in tic #{group}!"),
            )?;
            if members.is_empty() {
                super::notify_message(
                    scripts,
                    super::BattleMessageTarget::Player(pilot),
                    "*Click* (the tic contained no weapons)",
                )?;
            }
            for weapon in members {
                let attempt = super::evacuation::attempt_configured_firing_action(
                    scripts, config, id, pilot, weapon, target,
                )?;
                let (report, rejection) = match attempt {
                    Ok(report) => (Some(report), None),
                    Err(message) => {
                        super::notify_message(
                            scripts,
                            super::BattleMessageTarget::Player(pilot),
                            &message,
                        )?;
                        (None, Some(message))
                    }
                };
                shots.push(BattleTicShot {
                    group,
                    weapon,
                    report,
                    rejection,
                });
                let world = scripts.world.borrow();
                let changed =
                    world.btech.constructed_units().get(&id).map(|u| u.posture) != posture;
                let started = running(&world, id);
                drop(world);
                if changed && started {
                    super::notify_unit_text(
                        scripts,
                        id,
                        "That fall causes you to stop your fire!",
                    )?;
                }
                if changed || !started {
                    return Ok(shots);
                }
            }
        }
        Ok(shots)
    })
}

/// Shared batch stop condition; individual firing admission retains all other unit checks.
fn running(world: &World, id: ObjectId) -> bool {
    world.btech.vehicles().get(&id).map_or_else(
        || {
            world
                .btech
                .constructed_units()
                .get(&id)
                .is_some_and(|u| u.power() == super::BattlePower::Running)
        },
        |u| u.power() == super::BattlePower::Running,
    )
}

/// Native group firing uses the same optional target form as the ordinary fire command.
pub(crate) fn fire_command(
    ctx: &CommandContext<'_>,
    input: &CommandInput,
) -> Result<CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        super::weapons_hold::admit(&ctx.scripts.world.borrow(), id, ctx.player)?;
        let (selection_text, arguments) = input
            .args
            .trim()
            .split_once(char::is_whitespace)
            .unwrap_or((input.args.trim(), ""));
        let groups = selection(selection_text, 4)?;
        fire_tics(
            ctx.scripts,
            ctx.config,
            id,
            ctx.player,
            groups,
            super::fire_target::FireTargetRequest::Arguments(arguments),
        )
    })();
    Ok(match result {
        Ok(_) => CommandAction::Continue,
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}
