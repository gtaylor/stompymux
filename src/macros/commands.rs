//! Registered dot commands; mutations are staged for the invoking session's transaction.
use super::*;
use crate::{
    commands::{Action, CommandContext, CommandDefinition, CommandInput, CommandPermissions},
    world::Kind,
};

/// Register direct-input commands in the same enumerable catalog as other native commands.
pub fn definitions() -> Vec<CommandDefinition> {
    use CommandPermissions as P;
    [
        (".add", add as crate::commands::NativeHandler),
        (".clear", clear),
        (".chmod", chmod),
        (".chown", chown),
        (".create", create),
        (".def", define),
        (".del", detach),
        (".name", name),
        (".chslot", select),
        (".ex", examine),
        (".gex", global_examine),
        (".glist", global_list),
        (".list", list),
        (".undef", undefine),
    ]
    .into_iter()
    .map(|(name, handler)| {
        let mut d = CommandDefinition::native(
            name,
            if name == ".chown" {
                P::WIZARD
            } else {
                P::EVERYONE
            },
            handler,
        );
        d.direct_input_only = true;
        d.private_errors = true;
        d
    })
    .collect()
}

/// Isolate failed edits even for direct library callers; server commit owns durable success.
fn execute(
    ctx: &CommandContext<'_>,
    operation: impl FnOnce(&mut World) -> Result<String>,
    mutation: bool,
) -> Result<Action> {
    let mut w = ctx.scripts.world.borrow_mut();
    if !w
        .objects
        .get(&ctx.player)
        .is_some_and(|o| o.kind == Kind::Player)
    {
        return Ok(Action::Report(crate::commands::Report::Reply(
            "MACRO: Only players may use macro sets.".into(),
        )));
    }
    let before = mutation.then(|| w.macros.clone());
    match operation(&mut w) {
        Ok(message) if mutation => Ok(Action::CommitReply(message)),
        Ok(message) => Ok(Action::Report(crate::commands::Report::Reply(message))),
        Err(error) => {
            if let Some(before) = before {
                w.macros = before;
            }
            Ok(Action::Report(crate::commands::Report::Reply(format!(
                "MACRO: {error}"
            ))))
        }
    }
}

fn current(w: &World, who: ObjectId) -> Result<usize> {
    let slots = w
        .macros
        .players
        .get(&who)
        .context("You have no current slot set.")?;
    slots
        .current
        .and_then(|i| slots.slots.get(i).copied().flatten())
        .context("You have no current slot set.")
}

fn slot(args: &str) -> Result<usize> {
    let slot: usize = args
        .trim()
        .parse()
        .context("Specify a macro slot from 0 to 4.")?;
    ensure!(slot < SLOT_COUNT, "That is not a legal macro slot.");
    Ok(slot)
}

fn set_number(w: &World, args: &str) -> Result<usize> {
    let index = args
        .trim()
        .parse::<usize>()
        .context("Specify a macro set number.")?;
    ensure!(
        index < w.macros.sets.len(),
        "That macro set does not exist."
    );
    Ok(index)
}

fn empty_slot(w: &World, who: ObjectId) -> Result<usize> {
    w.macros
        .players
        .get(&who)
        .cloned()
        .unwrap_or_default()
        .slots
        .iter()
        .position(Option::is_none)
        .context("Sorry, you already have 5 sets defined on you.")
}

fn no_args(input: &CommandInput) -> Result<()> {
    ensure!(input.args.is_empty(), "This command takes no arguments.");
    Ok(())
}

fn create(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    execute(
        ctx,
        |w| {
            text(&input.args)?;
            let slot = empty_slot(w, ctx.player)?;
            let index = w.macros.sets.len();
            w.macros.sets.push(MacroSet {
                origin: Default::default(),
                owner: ctx.player,
                modes: Default::default(),
                description: input.args.clone(),
                entries: vec![],
            });
            let attached = w.macros.players.entry(ctx.player).or_default();
            attached.slots[slot] = Some(index);
            attached.current = Some(slot);
            Ok(format!("MACRO: Macro set {index} created in slot {slot}."))
        },
        true,
    )
}

fn add(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    execute(
        ctx,
        |w| {
            let index = set_number(w, &input.args)?;
            ensure!(
                w.macros.sets[index]
                    .readable(ctx.player, crate::authority::is_wizard(w, ctx.player)),
                "Permission denied."
            );
            let slot = empty_slot(w, ctx.player)?;
            w.macros.players.entry(ctx.player).or_default().slots[slot] = Some(index);
            Ok(format!(
                "MACRO: Macro set {index} added in the {slot} slot."
            ))
        },
        true,
    )
}

fn detach(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    execute(
        ctx,
        |w| {
            let slot = slot(&input.args)?;
            let slots = w.macros.players.entry(ctx.player).or_default();
            slots.slots[slot] = None;
            let selected = slots.current == Some(slot);
            if selected {
                slots.current = None;
            }
            Ok(format!(
                "MACRO: Macro slot {slot} cleared.{}",
                if selected {
                    "\nMACRO: Deleted current slot, resetting to none."
                } else {
                    ""
                }
            ))
        },
        true,
    )
}

fn select(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    execute(
        ctx,
        |w| {
            let slot = slot(&input.args)?;
            let slots = w
                .macros
                .players
                .get_mut(&ctx.player)
                .context("That is not a legal macro slot.")?;
            ensure!(
                slots.slots[slot].is_some(),
                "That is not a legal macro slot."
            );
            slots.current = Some(slot);
            Ok(format!("MACRO: Current slot set to {slot}."))
        },
        true,
    )
}

fn name(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    execute(
        ctx,
        |w| {
            let index = current(w, ctx.player)?;
            text(&input.args)?;
            let set = &mut w.macros.sets[index];
            ensure!(set.writable(ctx.player), "Permission denied.");
            set.description = input.args.clone();
            Ok(format!(
                "MACRO: Current slot description to {}.",
                safe(&input.args)
            ))
        },
        true,
    )
}

fn chmod(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    execute(
        ctx,
        |w| {
            let index = current(w, ctx.player)?;
            let wizard = crate::authority::is_wizard(w, ctx.player);
            let set = &mut w.macros.sets[index];
            ensure!(wizard || set.owner == ctx.player, "Permission denied.");
            let (enabled, value) = input
                .args
                .strip_prefix('!')
                .map_or((true, input.args.as_str()), |v| (false, v));
            let bit = match value.to_ascii_uppercase().as_str() {
                "L" => MacroModes::LOCKED,
                "R" => MacroModes::READ,
                "W" => MacroModes::WRITE,
                _ => anyhow::bail!("Unknown mode. Legal modes are: L R W (prefix ! to clear)."),
            };
            if enabled {
                set.modes.0 |= bit;
            } else {
                set.modes.0 &= !bit;
            }
            Ok(format!(
                "MACRO: Current set modes: {}.",
                set.modes.letters()
            ))
        },
        true,
    )
}

fn chown(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    execute(
        ctx,
        |w| {
            ensure!(
                crate::authority::is_wizard(w, ctx.player),
                "Permission denied."
            );
            let index = current(w, ctx.player)?;
            let owner = crate::commands::target::admin_target(w, ctx.player, &input.args)?;
            w.macros.sets[index].owner = owner;
            Ok(format!("MACRO: Macro set {index} chowned to #{}.", owner.0))
        },
        true,
    )
}

fn clear(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    execute(
        ctx,
        |w| {
            no_args(input)?;
            let index = current(w, ctx.player)?;
            let set = &w.macros.sets[index];
            if set.owner == ctx.player {
                ensure!(
                    !set.modes.has(MacroModes::LOCKED),
                    "Sorry, that macro set is locked."
                );
            } else {
                ensure!(
                    crate::authority::is_wizard(w, ctx.player),
                    "You may only CLEAR your own macro sets."
                );
            }
            let message = format!(
                "MACRO: Clearing macro set {index}: {}.",
                safe(&set.description)
            );
            w.macros.remove(index);
            Ok(message)
        },
        true,
    )
}

fn define(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    execute(
        ctx,
        |w| {
            let index = current(w, ctx.player)?;
            let set = &mut w.macros.sets[index];
            ensure!(set.writable(ctx.player), "Permission denied.");
            let (name, expansion) = input
                .args
                .split_once('=')
                .context("You must specify an = in your macro definition.")?;
            let name = name.trim();
            let expansion = expansion.trim_start();
            alias(name)?;
            ensure!(
                !name.contains(char::is_whitespace),
                "Alias cannot contain whitespace."
            );
            text(expansion)?;
            ensure!(
                !expansion.is_empty(),
                "You must specify a string to substitute for."
            );
            ensure!(
                !set.entries
                    .iter()
                    .any(|e| e.alias.eq_ignore_ascii_case(name)),
                "That alias is already defined in this set."
            );
            set.entries.push(MacroEntry {
                origin: Default::default(),
                alias: name.into(),
                expansion: expansion.into(),
            });
            set.entries.sort_by_key(|e| e.alias.to_ascii_lowercase());
            Ok(format!(
                "MACRO: Macro {}:{} defined.",
                safe(name),
                safe(expansion)
            ))
        },
        true,
    )
}

fn undefine(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    execute(
        ctx,
        |w| {
            let index = current(w, ctx.player)?;
            let set = &mut w.macros.sets[index];
            ensure!(set.writable(ctx.player), "Permission denied.");
            let position = set
                .entries
                .iter()
                .position(|e| e.alias.eq_ignore_ascii_case(&input.args))
                .context("That macro is not in this set.")?;
            set.entries.remove(position);
            Ok("MACRO: Macro deleted from set.".into())
        },
        true,
    )
}

/// Diagnostics render client text literally, never as terminal control bytes.
fn safe(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_control() {
                c.escape_default().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// Bound intermediate inspection allocations and clearly mark omitted entries.
fn lines(rows: impl IntoIterator<Item = String>, limit: usize) -> String {
    let mut result = String::new();
    let budget = limit.saturating_sub(32);
    for row in rows {
        if result.len() + row.len() + 2 > budget {
            let mut remaining = budget.saturating_sub(result.len());
            while !row.is_char_boundary(remaining.min(row.len())) {
                remaining -= 1;
            }
            result.push_str(&row[..remaining.min(row.len())]);
            result.push_str("\r\n[truncated]");
            break;
        }
        result.push_str(&row);
        result.push_str("\r\n");
    }
    result
}

fn listing(w: &World, index: usize) -> String {
    let set = &w.macros.sets[index];
    let owner = w
        .objects
        .get(&set.owner)
        .map(|o| safe(&o.name))
        .unwrap_or_else(|| format!("#{}", set.owner.0));
    format!(
        "{index:<4} {:35} {owner:24} {}",
        safe(&set.description),
        set.modes.letters()
    )
}

fn global_list(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    execute(
        ctx,
        |w| {
            no_args(input)?;
            let wizard = crate::authority::is_wizard(w, ctx.player);
            Ok(lines(
                std::iter::once(
                    "Num  Description                         Owner                    LRW".into(),
                )
                .chain(
                    w.macros
                        .sets
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| s.readable(ctx.player, wizard))
                        .map(|(i, _)| listing(w, i)),
                ),
                ctx.config.runtime.output_message_limit,
            ))
        },
        false,
    )
}

fn list(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    execute(
        ctx,
        |w| {
            no_args(input)?;
            let slots = w
                .macros
                .players
                .get(&ctx.player)
                .cloned()
                .unwrap_or_default();
            Ok(lines(
                slots
                    .slots
                    .iter()
                    .enumerate()
                    .map(|(slot, index)| match index {
                        Some(index) => format!("Slot {slot}: {}", listing(w, *index)),
                        None => format!("Slot {slot}: Empty"),
                    })
                    .chain(std::iter::once(format!(
                        "Current slot: {}",
                        slots.current.map_or("none".into(), |i| i.to_string())
                    ))),
                ctx.config.runtime.output_message_limit,
            ))
        },
        false,
    )
}

fn inspect(w: &World, index: usize, limit: usize) -> String {
    lines(
        std::iter::once(listing(w, index)).chain(
            w.macros.sets[index]
                .entries
                .iter()
                .map(|e| format!("{:4}: {}", safe(&e.alias), safe(&e.expansion))),
        ),
        limit,
    )
}

fn examine(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    execute(
        ctx,
        |w| {
            let index = if input.args.is_empty() {
                current(w, ctx.player)?
            } else {
                let slot = slot(&input.args)?;
                w.macros
                    .players
                    .get(&ctx.player)
                    .and_then(|p| p.slots[slot])
                    .context("Illegal macro set to examine.")?
            };
            Ok(inspect(w, index, ctx.config.runtime.output_message_limit))
        },
        false,
    )
}

fn global_examine(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    execute(
        ctx,
        |w| {
            let index = set_number(w, &input.args)?;
            ensure!(
                w.macros.sets[index]
                    .readable(ctx.player, crate::authority::is_wizard(w, ctx.player)),
                "Permission denied."
            );
            Ok(inspect(w, index, ctx.config.runtime.output_message_limit))
        },
        false,
    )
}
