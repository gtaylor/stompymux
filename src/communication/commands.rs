//! Registered communication handlers and player-local alias interception.
use super::*;
use crate::commands::{Action, CommandContext, CommandInput, Report};

/// C comlist uses at least 79 columns and caps layout at its LBUF size.
const COMLIST_MIN_WIDTH: usize = 79;
const COMLIST_MAX_WIDTH: usize = 8192;

/// Native adapters select an operation at registration time, independently of aliases.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Add,
    Delete,
    Clear,
    List,
    All,
    Page,
    Admin,
}

/// Execute within the existing world transaction; policy errors discard provisional output.
fn run(ctx: &CommandContext<'_>, input: &CommandInput, operation: Operation) -> Result<Action> {
    ctx.scripts.sync_parents()?;
    let service = ctx.scripts.communication(ctx.config);
    transaction(&service, || {
        service.command(ctx.player, input, operation, ctx.session)
    })
}

/// Registered handler for `addcom`.
pub fn addcom(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    run(ctx, input, Operation::Add)
}

/// Registered handler for `delcom`.
pub fn delcom(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    run(ctx, input, Operation::Delete)
}

/// Registered handler for `clearcom`.
pub fn clearcom(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    run(ctx, input, Operation::Clear)
}

/// Registered handler for `comlist`.
pub fn comlist(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    run(ctx, input, Operation::List)
}

/// Registered handler for `allcom`.
pub fn allcom(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    run(ctx, input, Operation::All)
}

/// Registered handler for `page`.
pub fn page(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    run(ctx, input, Operation::Page)
}

/// Registered handler for `@chan`.
pub fn admin(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    run(ctx, input, Operation::Admin)
}

/// Player aliases precede native commands, as in the C comsys dispatcher.
pub fn alias(
    s: &crate::lua::Scripts,
    c: &Config,
    player: ObjectId,
    line: &str,
) -> Result<Option<Action>> {
    let (token, tail) = line
        .trim()
        .split_once(char::is_whitespace)
        .unwrap_or((line.trim(), ""));
    let channel = s
        .world
        .borrow()
        .channel_aliases
        .get(&player)
        .and_then(|aliases| aliases.iter().find(|a| a.alias.eq_ignore_ascii_case(token)))
        .map(|a| a.channel.clone());
    match channel {
        Some(channel) => {
            s.sync_parents()?;
            let service = s.communication(c);
            transaction(&service, || {
                service.say(player, &channel, tail.trim_start())
            })
            .map(Some)
        }
        None => Ok(None),
    }
}

/// Errors are session-private and cannot leave lock-side effects behind.
fn transaction(service: &Service<'_>, work: impl FnOnce() -> Result<()>) -> Result<Action> {
    let before = service.world.borrow().clone();
    let checkpoint = service.effects.checkpoint();
    match work() {
        Ok(()) => Ok(Action::Continue),
        Err(error) => {
            *service.world.borrow_mut() = before;
            service.effects.restore(checkpoint);
            Ok(Action::Report(Report::Reply(error.to_string())))
        }
    }
}

impl Service<'_> {
    /// Command syntax adapters share domain operations with Lua and presence handling.
    fn command(
        &self,
        who: ObjectId,
        input: &CommandInput,
        operation: Operation,
        session: Option<u64>,
    ) -> Result<()> {
        let args = input.args.trim();
        if !matches!(operation, Operation::Page | Operation::Admin) {
            ensure!(
                wizard(&self.world.borrow(), who)
                    || !in_character(&self.world.borrow(), &self.config, who),
                "Permission denied."
            );
        }
        match operation {
            Operation::Page => self.page(who, args),
            Operation::Add => {
                let (alias, channel) = args.split_once('=').unwrap_or((args, ""));
                self.add(who, channel.trim(), alias.trim(), false, false)
            }
            Operation::Delete => self.remove_alias(who, args),
            Operation::Clear => {
                let aliases = self
                    .world
                    .borrow()
                    .channel_aliases
                    .get(&who)
                    .cloned()
                    .unwrap_or_default();
                for a in aliases.into_iter().rev() {
                    self.remove_member(who, &a.channel)?;
                    self.delete_alias_entry(who, &a.alias);
                }
                Ok(())
            }
            Operation::List => {
                self.notify(who, "Alias     Channel             Status Description")?;
                let width = self
                    .host
                    .terminal_width(session)
                    .unwrap_or(COMLIST_MIN_WIDTH)
                    .clamp(COMLIST_MIN_WIDTH, COMLIST_MAX_WIDTH)
                    - 37;
                let aliases = self
                    .world
                    .borrow()
                    .channel_aliases
                    .get(&who)
                    .cloned()
                    .unwrap_or_default();
                for a in aliases {
                    let entry = self.name(&a.channel).ok().and_then(|name| {
                        let w = self.world.borrow();
                        let c = &w.channels[&name];
                        c.users.iter().find(|u| u.who == who).map(|u| {
                            let description = c
                                .object
                                .and_then(|id| w.objects.get(&id))
                                .and_then(|o| o.description.as_deref())
                                .filter(|s| !s.is_empty())
                                .unwrap_or("No description.");
                            (
                                u.listening,
                                crate::text::truncate_with(
                                    &w.palette,
                                    &description.replace(['\r', '\n'], " "),
                                    width,
                                ),
                            )
                        })
                    });
                    let output = if let Some((on, description)) = entry {
                        format!(
                            "{} {} {:6} {}",
                            super::membership::column(&a.alias, 9),
                            super::membership::column(&a.channel, 19),
                            if on { "on" } else { "off" },
                            description
                        )
                    } else {
                        format!("Bad Comsys Alias: {} for Channel: {}", a.alias, a.channel)
                    };
                    self.notify(who, output)?;
                }
                self.notify(who, "-- End of comlist --")
            }
            Operation::All => {
                ensure!(
                    ["on", "off", "who"]
                        .iter()
                        .any(|s| args.eq_ignore_ascii_case(s)),
                    "Only options available are: on, off and who."
                );
                let channels = self
                    .world
                    .borrow()
                    .channel_aliases
                    .get(&who)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|a| a.channel)
                    .collect::<Vec<_>>();
                for c in channels {
                    let before = self.world.borrow().clone();
                    let checkpoint = self.effects.checkpoint();
                    if let Err(error) = self.say(who, &c, args) {
                        *self.world.borrow_mut() = before;
                        self.effects.restore(checkpoint);
                        self.notify(who, error.to_string())?;
                    }
                    if args.eq_ignore_ascii_case("who") {
                        self.notify(who, "")?;
                    }
                }
                Ok(())
            }
            Operation::Admin => self.admin(who, args, input.switch.as_deref()),
        }
    }

    /// Validate switch combinations before any channel mutation.
    fn admin(&self, who: ObjectId, args: &str, switch: Option<&str>) -> Result<()> {
        let Some(switch) = switch else {
            return self.notify(who, "@chan command switches:\n  /boot     Remove a member from a channel.\n  /create   Create a channel.\n  /destroy  Destroy a channel.\n  /emit     Send an administrative message.\n  /list     List channels.\n  /object   Attach a channel object.\n  /oflags   Set channel permissions for objects.\n  /pflags   Set channel permissions for players.\n  /flags    Set or clear channel flags.\n  /status   Show a channel's status.\n  /who      List a channel's members.");
        };
        let parts = switch.split('/').collect::<Vec<_>>();
        let operation = parts[0];
        let modifier = parts.get(1).copied();
        ensure!(
            parts.len() <= 2
                && (modifier.is_none()
                    || matches!(
                        (operation, modifier),
                        ("emit", Some("noheader")) | ("list" | "status", Some("full"))
                    )),
            "Illegal combination of @chan switches."
        );
        let (first, second) = args.split_once('=').unwrap_or((args, ""));
        let first = first.trim();
        let second = second.trim();
        match operation {
            "create" => {
                ensure!(self.name(first).is_err(), "Channel {first} already exists.");
                self.create(first)?;
                self.notify(who, format!("Channel {first} created."))
            }
            "destroy" => {
                let name = self.admin_name(first, operation)?;
                self.destroy(&name)?;
                self.notify(who, format!("Channel {first} destroyed."))
            }
            "list" | "status" => {
                let names = if operation == "status" {
                    match self.name(first) {
                        Ok(name) => vec![name],
                        Err(_) => {
                            self.list_header(who, modifier == Some("full"))?;
                            return self.notify(who, "@chan/status: Unknown channel.");
                        }
                    }
                } else {
                    let mut names = self
                        .world
                        .borrow()
                        .channels
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>();
                    names.sort_by_key(|n| n.to_ascii_lowercase());
                    names
                };
                self.list(who, &names, modifier == Some("full"))
            }
            "emit" => {
                self.admin_name(first, operation)?;
                self.emit(first, second, modifier == Some("noheader"))
            }
            "who" => {
                let (channel, all) = first
                    .split_once('/')
                    .map_or((first, false), |(c, s)| (c, s.starts_with('a')));
                let channel = self.admin_name(channel, operation)?;
                self.who(who, &channel, all, true)
            }
            "object" => {
                let name = self.admin_name(first, operation)?;
                let object = if second.is_empty() || second == "#-1" {
                    None
                } else {
                    match crate::commands::target::builder_target(&self.world.borrow(), who, second)
                    {
                        Ok(id) => Some(id),
                        Err(e)
                            if matches!(
                                e.to_string().as_str(),
                                "No such object." | "No such player."
                            ) =>
                        {
                            None
                        }
                        Err(e) => return Err(e),
                    }
                };
                self.world
                    .borrow_mut()
                    .channels
                    .get_mut(&name)
                    .unwrap()
                    .object = object;
                if let Some(object) = object {
                    let display = {
                        let w = self.world.borrow();
                        let object = &w.objects[&object];
                        format!("{}{}", object.name, crate::find::suffix(object))
                    };
                    self.notify(
                        who,
                        format!("Channel {name} is now using {display} as channel object."),
                    )
                } else {
                    self.notify(who, "@chan/object: Set.")
                }
            }
            "boot" => {
                let name = self.admin_name(first, operation)?;
                ensure!(
                    self.world.borrow().channels[&name]
                        .users
                        .iter()
                        .any(|u| u.who == who),
                    "@chan/boot: You are not on that channel."
                );
                let target =
                    crate::commands::target::builder_target(&self.world.borrow(), who, second)
                        .map_err(|e| {
                            if matches!(
                                e.to_string().as_str(),
                                "No such object." | "No such player."
                            ) {
                                anyhow::anyhow!("I don't see that here.")
                            } else {
                                e
                            }
                        })?;
                ensure!(
                    self.world.borrow().channels[&name]
                        .users
                        .iter()
                        .any(|u| u.who == target),
                    "@chan/boot: {} is not on the channel.",
                    self.world.borrow().objects[&target].name
                );
                self.boot(who, target, &name)
            }
            "flags" | "pflags" | "oflags" => {
                let name = self.admin_name(first, operation)?;
                let (enabled, flag) = second
                    .strip_prefix('!')
                    .map_or((true, second), |s| (false, s));
                let bit = if operation == "flags" {
                    ChannelFlag::parse(flag)
                        .map_err(|_| anyhow::anyhow!("@chan/{operation}: Unknown flag."))?
                        .bit()
                } else {
                    let access = match flag.to_ascii_lowercase().as_str() {
                        "join" => Access::Join,
                        "transmit" => Access::Transmit,
                        "receive" => Access::Receive,
                        _ => anyhow::bail!("@chan/{operation}: Unknown flag."),
                    };
                    access.bit() * if operation == "oflags" { 16 } else { 1 }
                };
                {
                    let mut w = self.world.borrow_mut();
                    let flags = &mut w.channels.get_mut(&name).unwrap().flags.0;
                    if enabled {
                        *flags |= bit;
                    } else {
                        *flags &= !bit;
                    }
                }
                self.notify(
                    who,
                    format!(
                        "@chan/{operation}: {}.",
                        if enabled { "Set" } else { "Cleared" }
                    ),
                )
            }
            _ => anyhow::bail!("Illegal combination of @chan switches."),
        }
    }

    /// Native operations supply their own channel lookup diagnostics.
    fn admin_name(&self, name: &str, operation: &str) -> Result<String> {
        self.name(name)
            .map_err(|_| anyhow::anyhow!("@chan/{operation}: Unknown channel."))
    }

    /// Header is emitted even when a status lookup cannot find its channel.
    fn list_header(&self, who: ObjectId, full: bool) -> Result<()> {
        self.notify(
            who,
            if full {
                "** Channel             --Flags--  Obj  Users   Messages"
            } else {
                "** Channel       Description"
            },
        )
    }

    /// Stable channel metadata listing; source styles render through the shared pipeline.
    fn list(&self, who: ObjectId, names: &[String], full: bool) -> Result<()> {
        self.list_header(who, full)?;
        for name in names {
            let output = {
                let w = self.world.borrow();
                let c = &w.channels[name];
                let public = if c.flags.has(ChannelFlag::Public) {
                    'P'
                } else {
                    '-'
                };
                let loud = if c.flags.has(ChannelFlag::Loud) {
                    'L'
                } else {
                    '-'
                };
                if full {
                    let bits = [
                        (1, 'J'),
                        (2, 'X'),
                        (4, 'R'),
                        (16, 'j'),
                        (32, 'x'),
                        (64, 'r'),
                    ]
                    .map(|(b, c1)| if c.flags.0 & b != 0 { c1 } else { '-' });
                    format!(
                        "{public}{loud} {:20} {}{}{}/{}{}{} {:5} {:6} {:10}",
                        super::membership::column(name, 20),
                        bits[0],
                        bits[1],
                        bits[2],
                        bits[3],
                        bits[4],
                        bits[5],
                        c.object.map_or(-1, |id| id.0),
                        c.users.len(),
                        c.messages
                    )
                } else {
                    let desc = c
                        .object
                        .and_then(|id| w.objects.get(&id))
                        .and_then(|o| o.description.as_deref())
                        .filter(|s| !s.is_empty())
                        .unwrap_or("No description.");
                    format!(
                        "{public}{loud} {} {}",
                        super::membership::column(name, 13),
                        super::membership::column(
                            super::membership::column(desc, 54).trim_end(),
                            60
                        )
                    )
                }
            };
            self.notify(who, output)?;
        }
        self.notify(who, "-- End of list of Channels --")
    }
}
