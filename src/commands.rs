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
    if let Some(alias) = c
        .get("aliases.commands")
        .and_then(|v| v.get(&command))
        .and_then(|v| v.as_str())
    {
        command = alias.to_ascii_lowercase();
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
    if p.flags.contains("GAGGED") {
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
