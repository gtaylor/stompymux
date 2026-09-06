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
    if matches!(command.as_str(), "@flag" | "@list" | "@examine") {
        let response = flag_command(s, c, player, &command, args).unwrap_or_else(|e| e.to_string());
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
                        let destination = *destination;
                        let valid = s
                            .world
                            .borrow()
                            .objects
                            .get(&destination)
                            .is_some_and(|o| o.kind == Kind::Room);
                        if valid {
                            s.movement_event("on_exit", player, room, session)?;
                            s.world
                                .borrow_mut()
                                .objects
                                .get_mut(&player)
                                .unwrap()
                                .location = Some(destination);
                            s.movement_event("on_enter", player, destination, session)?;
                            s.outbox
                                .borrow_mut()
                                .push((player, s.appearance(player, destination, session)?));
                        } else {
                            s.outbox
                                .borrow_mut()
                                .push((player, "That exit has no usable destination.".into()));
                        }
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
fn flag_target(w: &crate::world::World, player: ObjectId, name: &str) -> Result<ObjectId> {
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
/// Flag-only administration surface; broader builder commands remain deferred.
fn flag_command(
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
        ensure!(
            args.trim().eq_ignore_ascii_case("flags"),
            "Usage: @list flags"
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
        let target = flag_target(&w, player, args)?;
        let o = &w.objects[&target];
        return Ok(format!(
            "{}(#{})\r\nType: {} Flags: {}",
            o.name,
            o.id.0,
            format!("{:?}", o.kind).to_uppercase(),
            o.flags.names().join(" ")
        ));
    }
    let (target, flag) = args
        .split_once('=')
        .context("Usage: @flag <target>=<flag> or !<flag>")?;
    let target = flag_target(&w, player, target)?;
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
