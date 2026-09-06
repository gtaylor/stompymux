//! Player commands and transactional flag administration.
use crate::{
    config::Config,
    scripting::Scripts,
    world::{Kind, ObjectId},
};
use anyhow::{Context, Result};
pub enum Action {
    Continue,
    Quit,
}
pub fn run(s: &Scripts, c: &Config, player: ObjectId, session: u64, input: &str) -> Result<Action> {
    let line = input.trim();
    let (verb, args) = line.split_once(' ').unwrap_or((line, ""));
    let mut command = verb.to_ascii_lowercase();
    if let Some(alias) = c.aliases.commands.get(&command) {
        command = alias.to_ascii_lowercase();
    }
    let (base, switch) = command
        .split_once('/')
        .map_or((command.as_str(), None), |(base, switch)| {
            (base, Some(switch))
        });
    let base = c
        .aliases
        .commands
        .get(base)
        .map_or(base, String::as_str)
        .to_ascii_lowercase();
    if matches!(base.as_str(), "home" | "@teleport") {
        let result = movement_command(s, player, session, &base, args, switch);
        if let Err(error) = result {
            s.outbox.borrow_mut().push((player, error.to_string()));
        }
        return Ok(Action::Continue);
    }
    if matches!(command.as_str(), "@flag" | "@power" | "@list" | "@examine") {
        let response =
            admin_command(s, c, player, &command, args).unwrap_or_else(|e| e.to_string());
        s.outbox.borrow_mut().push((player, response));
        return Ok(Action::Continue);
    }
    let room = s
        .world
        .borrow()
        .objects
        .get(&player)
        .context("player missing")?
        .location
        .context("player has no location")?;
    match command.as_str() {
        "quit" => return Ok(Action::Quit),
        "look" | "l" => {
            let result = s.appearance(player, room, session)?;
            s.outbox.borrow_mut().push((player, result));
        }
        "say" => say(s, player, room, args),
        _ if line.starts_with('"') => say(s, player, room, &line[1..]),
        _ => {
            if s.dispatch(player, session, line)? {
                return Ok(Action::Continue);
            }
            let exits: Vec<_> = s
                .world
                .borrow()
                .objects
                .values()
                .filter(|o| {
                    o.kind == Kind::Exit
                        && o.location == Some(room)
                        && o.name.split(';').any(|n| n.eq_ignore_ascii_case(line))
                })
                .map(|o| (o.id, o.destination))
                .collect();
            match exits.as_slice() {
                [(exit, Some(destination))] => {
                    if !s.lock(player, *exit).unwrap_or(false) {
                        s.outbox
                            .borrow_mut()
                            .push((player, "You cannot go that way.".into()));
                    } else {
                        crate::movement::perform(
                            s,
                            player,
                            player,
                            *destination,
                            Some(session),
                            crate::movement::Route::Exit,
                        )?;
                    }
                }
                [] => s.outbox.borrow_mut().push((
                    player,
                    "Huh? (Type look, say <message>, WHO, an exit name, or quit.)".into(),
                )),
                _ => s
                    .outbox
                    .borrow_mut()
                    .push((player, "I don't know which exit you mean.".into())),
            }
        }
    }
    Ok(Action::Continue)
}
fn say(s: &Scripts, player: ObjectId, room: ObjectId, message: &str) {
    let w = s.world.borrow();
    let p = &w.objects[&player];
    if p.flags.contains(crate::flags::Flag::Gagged) {
        s.outbox
            .borrow_mut()
            .push((player, "You cannot speak.".into()));
        return;
    }
    for o in w
        .objects
        .values()
        .filter(|o| o.kind == Kind::Player && o.location == Some(room))
    {
        s.outbox.borrow_mut().push((
            o.id,
            if o.id == player {
                format!("You say, \"{message}\"")
            } else {
                format!("{} says, \"{message}\"", p.name)
            },
        ));
    }
}
/// Resolve explicit identities and exact visible local names without guessing.
fn admin_target(w: &crate::world::World, player: ObjectId, name: &str) -> Result<ObjectId> {
    use anyhow::{bail, ensure};
    let name = name.trim();
    let room = w.objects.get(&player).and_then(|o| o.location);
    let explicit = if name.eq_ignore_ascii_case("me") {
        Some(player)
    } else if name.eq_ignore_ascii_case("here") {
        room
    } else if let Some(n) = name.strip_prefix('#') {
        Some(ObjectId(n.parse().context("Invalid dbref.")?))
    } else {
        None
    };
    if let Some(id) = explicit {
        ensure!(
            w.objects
                .get(&id)
                .is_some_and(|o| o.kind != Kind::Garbage && w.visible(o, player)),
            "No such object."
        );
        return Ok(id);
    }
    let matches: Vec<_> = w
        .objects
        .values()
        .filter(|o| {
            o.kind != Kind::Garbage
                && w.visible(o, player)
                && (o.id == player
                    || Some(o.id) == room
                    || o.location == room
                    || o.location == Some(player))
                && if o.kind == Kind::Exit {
                    o.name.split(';').any(|n| n.eq_ignore_ascii_case(name))
                } else {
                    o.name.eq_ignore_ascii_case(name)
                }
        })
        .map(|o| o.id)
        .collect();
    match matches.as_slice() {
        [id] => Ok(*id),
        [] => bail!("No such object."),
        _ => bail!("I don't know which object you mean."),
    }
}
/// Flag and power administration; broader builder commands remain deferred.
fn admin_command(
    s: &Scripts,
    c: &Config,
    player: ObjectId,
    command: &str,
    args: &str,
) -> Result<String> {
    use crate::flags::{self, Flag};
    use anyhow::ensure;
    let mut w = s.world.borrow_mut();
    ensure!(
        player == ObjectId(1)
            || w.objects
                .get(&player)
                .is_some_and(|o| o.flags.contains(Flag::Wizard)),
        "Permission denied."
    );
    if command == "@list" {
        if args.trim().eq_ignore_ascii_case("powers") {
            return Ok(format!(
                "Powers: {}",
                crate::powers::ALL
                    .iter()
                    .map(|p| p.display_name())
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        ensure!(
            args.trim().eq_ignore_ascii_case("flags"),
            "Usage: @list flags or @list powers"
        );
        return Ok(format!(
            "Flags: {}",
            flags::ALL
                .iter()
                .map(|f| format!("{}({})", f.world_name(), f.letter()))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    if command == "@examine" {
        let target = admin_target(&w, player, args)?;
        let o = &w.objects[&target];
        return Ok(format!(
            "{}(#{})\r\nType: {} Flags: {}\r\nPowers: {}",
            o.name,
            o.id.0,
            format!("{:?}", o.kind).to_uppercase(),
            o.flags.names().join(" "),
            o.powers.description()
        ));
    }
    if command == "@power" {
        let (target, power) = args
            .split_once('=')
            .context("Usage: @power <target>=<power> or !<power>")?;
        let target = admin_target(&w, player, target)?;
        ensure!(flags::controls(&w, player, target), "Permission denied.");
        let power = power.trim();
        let (value, name) = power
            .strip_prefix('!')
            .map_or((true, power), |name| (false, name.trim()));
        ensure!(
            !name.is_empty(),
            "You must specify a power to {}.",
            if value { "set" } else { "clear" }
        );
        let power = crate::powers::Power::parse(name)
            .map_err(|_| anyhow::anyhow!("I don't understand that power."))?;
        crate::powers::change(&mut w, player, target, power, value)?;
        return Ok(format!(
            "{} - {} {}.",
            w.objects[&target].name,
            power.display_name(),
            if value { "granted" } else { "removed" }
        ));
    }
    let (target, flag) = args
        .split_once('=')
        .context("Usage: @flag <target>=<flag> or !<flag>")?;
    let target = admin_target(&w, player, target)?;
    ensure!(flags::controls(&w, player, target), "Permission denied.");
    let flag = flag.trim();
    let (value, name) = flag
        .strip_prefix('!')
        .map_or((true, flag), |name| (false, name.trim()));
    ensure!(
        !name.is_empty(),
        "You must specify a flag to {}.",
        if value { "set" } else { "clear" }
    );
    let flag = Flag::resolve(name, &c.aliases.flags)
        .map_err(|_| anyhow::anyhow!("I don't understand that flag."))?;
    flags::change(&mut w, player, target, flag, value)?;
    Ok(format!(
        "{} - {} {}.",
        w.objects[&target].name,
        flag.world_name(),
        if value { "set" } else { "cleared" }
    ))
}

/// Resolve wizard movement syntax before applying the shared transaction.
fn movement_command(
    s: &Scripts,
    player: ObjectId,
    session: u64,
    command: &str,
    args: &str,
    switch: Option<&str>,
) -> Result<()> {
    use crate::{
        flags::Flag,
        movement::{self, Route},
    };
    use anyhow::ensure;
    ensure!(
        player == ObjectId(1)
            || s.world.borrow().objects[&player]
                .flags
                .contains(Flag::Wizard),
        "Permission denied."
    );
    ensure!(
        switch.is_none(),
        "Movement command switches are not supported."
    );
    let (object, destination, route) = {
        let w = s.world.borrow();
        if command == "home" {
            ensure!(args.trim().is_empty(), "Usage: home");
            (
                player,
                w.objects[&player].home.context("Your home is not set.")?,
                Route::Home,
            )
        } else {
            ensure!(
                !args.trim().is_empty(),
                "Usage: @teleport <destination> or <object>=<destination>"
            );
            let (object, destination) = match args.split_once('=') {
                Some((object, destination)) => {
                    ensure!(
                        !object.trim().is_empty() && !destination.trim().is_empty(),
                        "Both target and destination are required."
                    );
                    (
                        admin_target(&w, player, object)?,
                        admin_target(&w, player, destination)?,
                    )
                }
                None => (player, admin_target(&w, player, args)?),
            };
            (object, destination, Route::Teleport)
        }
    };
    movement::perform(
        s,
        player,
        object,
        destination,
        (object == player).then_some(session),
        route,
    )
}
