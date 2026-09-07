//! Wizard account requests, player lookup and read-only login history.
use crate::{
    accounts,
    commands::{Action, CommandContext, CommandInput},
    flags::Flag,
    world::{ObjectId, World},
};
use anyhow::{Context, Result, ensure};
use zeroize::Zeroizing;

/// Secrets travel only to bounded hashing workers, never to diagnostic metadata.
pub enum Request {
    Create {
        name: String,
        password: Zeroizing<String>,
    },
    Reset {
        target: ObjectId,
        password: Zeroizing<String>,
    },
    Boot {
        target: BootTarget,
        quiet: bool,
    },
}
/// A player selects all sessions; a port denotes one stable Rust session ID.
pub enum BootTarget {
    Player(ObjectId),
    Session(u64),
}

/// Account operations exclude garbage and objects awaiting destruction.
pub fn player(world: &World, caller: ObjectId, name: &str) -> Result<ObjectId> {
    let name = name.trim();
    let id = if name.eq_ignore_ascii_case("me") {
        Some(caller)
    } else {
        world.find_player(name.strip_prefix('*').unwrap_or(name))
    };
    id.filter(|id| {
        world
            .objects
            .get(id)
            .is_some_and(|o| !o.flags.contains(Flag::Going))
    })
    .context("No such player.")
}

/// Registered native handler; asynchronous work is delegated to the world owner.
pub fn command(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    let result = (|| -> Result<Action> {
        if input.name == "@boot" {
            let mut port = false;
            let mut quiet = false;
            if let Some(s) = &input.switch {
                for part in s.split('/') {
                    ensure!(!part.is_empty(), "Unsupported command switch.");
                    if "port".starts_with(part) {
                        port = true;
                    } else if "quiet".starts_with(part) {
                        quiet = true;
                    } else {
                        anyhow::bail!("Unsupported command switch.");
                    }
                }
            }
            ensure!(
                !input.args.trim().is_empty(),
                "Boot which player or session?"
            );
            let target = if port {
                BootTarget::Session(input.args.trim().parse().context("That's not a number!")?)
            } else {
                BootTarget::Player(player(
                    &ctx.scripts.world.borrow(),
                    ctx.player,
                    &input.args,
                )?)
            };
            return Ok(Action::AccountAdmin(Request::Boot { target, quiet }));
        }
        ensure!(input.switch.is_none(), "Unsupported command switch.");
        if input.name == "@last" {
            let world = ctx.scripts.world.borrow();
            let target = if input.args.trim().is_empty() {
                ctx.player
            } else {
                player(&world, ctx.player, &input.args)?
            };
            let account = world
                .accounts
                .get(&target)
                .context("I couldn't find that player.")?;
            let mut lines = Vec::new();
            for (success, count) in [(true, account.successes), (false, account.failures)] {
                lines.push(format!(
                    "Total {} connects: {count}",
                    if success { "successful" } else { "failed" }
                ));
                let mut records: Vec<_> = account
                    .history
                    .iter()
                    .filter(|r| r.success == success)
                    .collect();
                records.sort_by_key(|r| std::cmp::Reverse(r.at));
                for record in records {
                    if !record.host.is_empty()
                        && let Some(at) = chrono::DateTime::from_timestamp(record.at, 0)
                    {
                        lines.push(format!(
                            "     From: {}   On: {}",
                            record.host,
                            at.format("%Y-%m-%dT%H:%M:%SZ")
                        ));
                    }
                }
            }
            return Ok(Action::Report(lines.join("\n")));
        }
        let (name, password) = input
            .args
            .split_once('=')
            .context("Usage: <player>=<password>")?;
        accounts::validate_password(password, ctx.config)?;
        let password = Zeroizing::new(password.to_owned());
        if input.name == "@pcreate" {
            let name = name.trim();
            accounts::validate_name(name, ctx.config)?;
            ensure!(
                ctx.scripts.world.borrow().find_player(name).is_none(),
                "That name is not available."
            );
            Ok(Action::AccountAdmin(Request::Create {
                name: name.into(),
                password,
            }))
        } else {
            let target = player(&ctx.scripts.world.borrow(), ctx.player, name)?;
            ensure!(
                target != ObjectId(1),
                "You cannot change that player's password."
            );
            Ok(Action::AccountAdmin(Request::Reset { target, password }))
        }
    })();
    Ok(result.unwrap_or_else(|e| Action::Reply(e.to_string())))
}
