//! Native speech formatting and authority, with shared bounded notification routing.
use crate::{
    commands::{Action, CommandContext, CommandInput},
    flags::{self, Flag},
    notification::{self, Policy, Request},
    text::{self, Document},
    world::{Kind, ObjectId},
};
use anyhow::{Result, ensure};

#[derive(Clone, Copy)]
enum Mode {
    Say,
    Pose,
    Emit,
    Pemit,
    Oemit,
    Fsay,
    Fpose,
    Femit,
    Wall,
}
fn switches(input: &CommandInput, allowed: &[&str]) -> Result<Vec<String>> {
    let mut result = Vec::new();
    if let Some(s) = &input.switch {
        for part in s.split('/') {
            ensure!(!part.is_empty(), "Unsupported command switch.");
            let matching: Vec<_> = allowed.iter().filter(|v| v.starts_with(part)).collect();
            ensure!(
                matching.len() == 1,
                "Unsupported or ambiguous command switch."
            );
            let key = matching[0].to_string();
            // C permits only one non-SW_MULTIPLE switch per invocation.
            if [
                "default", "nospace", "object", "silent", "emit", "pose", "admin",
            ]
            .contains(&key.as_str())
            {
                ensure!(
                    !result.iter().any(|v: &String| [
                        "default", "nospace", "object", "silent", "emit", "pose", "admin"
                    ]
                    .contains(&v.as_str())),
                    "Invalid switch combination."
                );
            }
            result.push(key);
        }
    }
    Ok(result)
}
fn send(
    ctx: &CommandContext<'_>,
    target: ObjectId,
    document: Document,
    policy: Policy,
    exclusions: Option<Vec<ObjectId>>,
) -> Result<()> {
    notification::send(
        &ctx.scripts.world.borrow(),
        &ctx.scripts.outbox,
        ctx.config,
        Request {
            target,
            sender: ctx.player,
            document,
            policy,
            exclusions,
        },
    )
}
fn name(ctx: &CommandContext<'_>, id: ObjectId) -> String {
    ctx.scripts.world.borrow().objects[&id].name.clone()
}
fn room(ctx: &CommandContext<'_>, id: ObjectId) -> Option<ObjectId> {
    ctx.scripts
        .world
        .borrow()
        .objects
        .get(&id)
        .and_then(|o| o.location)
}
fn emit(
    ctx: &CommandContext<'_>,
    at: Option<ObjectId>,
    doc: Document,
    here: bool,
    enclosing: bool,
) -> Result<()> {
    let Some(mut location) = at else {
        return Ok(());
    };
    if here {
        send(ctx, location, doc.clone(), Policy::ROOM, None)?;
    }
    if !enclosing {
        return Ok(());
    }
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..=20 {
        if !seen.insert(location) {
            return Ok(());
        }
        let kind = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&location)
            .map(|o| o.kind);
        if kind == Some(Kind::Room) {
            if !(here && Some(location) == at) {
                send(ctx, location, doc, Policy::ROOM, None)?;
            }
            return Ok(());
        }
        let Some(parent) = room(ctx, location) else {
            return Ok(());
        };
        location = parent;
    }
    Ok(())
}
fn target(ctx: &CommandContext<'_>, value: &str, controlled: bool) -> Result<ObjectId> {
    let w = ctx.scripts.world.borrow();
    let id = crate::commands::target::builder_target(&w, ctx.player, value).map_err(|e| {
        let text = e.to_string();
        anyhow::anyhow!(if text.contains("which object") {
            "I don't know who you mean!"
        } else {
            "Emit to whom?"
        })
    })?;
    let control = flags::controls(&w, ctx.player, id);
    let near = id == ctx.player
        || w.objects[&id].location == Some(ctx.player)
        || w.objects[&ctx.player].location == Some(id)
        || (w.objects[&ctx.player].location.is_some()
            && w.objects[&id].location == w.objects[&ctx.player].location);
    ensure!(
        if controlled { control } else { control || near },
        if controlled {
            "Permission denied."
        } else {
            "You are too far away to do that."
        }
    );
    Ok(id)
}
fn speaking(ctx: &CommandContext<'_>) -> Result<bool> {
    let location = ctx.location()?;
    let w = ctx.scripts.world.borrow();
    let gagged =
        w.objects[&ctx.player].flags.contains(Flag::Gagged) && !flags::is_wizard(&w, ctx.player);
    let auditorium = w.objects[&location].flags.contains(Flag::Auditorium);
    drop(w);
    if gagged {
        send(
            ctx,
            ctx.player,
            "Sorry. Gagged players cannot speak.".into(),
            Policy::DIRECT,
            None,
        )?;
        return Ok(false);
    }
    if auditorium {
        let invocation = crate::LockInvocation {
            kind: crate::LockType::Speak,
            object: location,
            enactor: ctx.player,
            cause: ctx.cause,
            subject: ctx.player,
            descriptor: ctx.session,
            silent: false,
        };
        let outcome = ctx.scripts.evaluate_lock(invocation)?;
        if !outcome.passes {
            ctx.scripts.deny_action(
                invocation,
                &outcome,
                "Sorry, you may not speak in this place.",
                None,
            )?;
            return Ok(false);
        }
    }
    Ok(true)
}
fn perform(ctx: &CommandContext<'_>, input: &CommandInput, mode: Mode) -> Result<Action> {
    let allowed: &[&str] = match mode {
        Mode::Pose | Mode::Fpose => &["default", "nospace"],
        Mode::Emit | Mode::Femit => &["here", "room"],
        Mode::Pemit => &["contents", "object", "silent", "list"],
        Mode::Wall => &["emit", "pose", "wizard", "admin", "no_prefix"],
        _ => &[],
    };
    let switches = switches(input, allowed)?;
    let has = |s: &str| switches.iter().any(|v| v == s);
    if matches!(mode, Mode::Say | Mode::Pose | Mode::Emit) && !speaking(ctx)? {
        return Ok(Action::Continue);
    }
    let mut message = input.args.as_str();
    let mut speaker = ctx.player;
    // C ORs SAY_NOSPACE into PEMIT_FPOSE (5), leaving it unchanged.
    let mut nospace = has("nospace") && matches!(mode, Mode::Pose);
    if matches!(mode, Mode::Pose) {
        if input.line.starts_with(';') {
            nospace = true;
        }
        if input.line.starts_with(": ") {
            nospace = true;
            message = message.strip_prefix(' ').unwrap_or(message);
        }
    }
    if matches!(
        mode,
        Mode::Pemit | Mode::Oemit | Mode::Fsay | Mode::Fpose | Mode::Femit
    ) {
        let (recipient, text) = input.args.split_once('=').unwrap_or((&input.args, ""));
        message = text;
        if matches!(mode, Mode::Pemit) && has("list") {
            if message.is_empty() {
                return Ok(Action::Continue);
            }
            for item in recipient.split(' ').filter(|s| !s.is_empty()) {
                match target(ctx, item, false) {
                    Ok(id) => send(ctx, id, message.into(), Policy::DIRECT, None)?,
                    Err(e) => send(
                        ctx,
                        ctx.player,
                        Document::Literal(if e.to_string().contains("far away") {
                            "You cannot do that.".into()
                        } else {
                            e.to_string()
                        }),
                        Policy::DIRECT,
                        None,
                    )?,
                }
            }
            return Ok(Action::Continue);
        }
        speaker = target(
            ctx,
            recipient,
            matches!(mode, Mode::Fsay | Mode::Fpose | Mode::Femit),
        )
        .map_err(|e| {
            if matches!(mode, Mode::Oemit) && e.to_string() == "Emit to whom?" {
                anyhow::anyhow!("Emit except to whom?")
            } else {
                e
            }
        })?;
    }
    let plain;
    if matches!(mode, Mode::Say | Mode::Pose | Mode::Emit | Mode::Wall) {
        plain = text::escape(&text::plain_with(&ctx.scripts.palette, message));
        message = &plain;
    }
    match mode {
        Mode::Say | Mode::Fsay => {
            send(
                ctx,
                speaker,
                format!("You say \"{message}\"").into(),
                Policy::DIRECT,
                None,
            )?;
            if let Some(at) = room(ctx, speaker) {
                send(
                    ctx,
                    at,
                    format!("{} says \"{message}\"", name(ctx, speaker)).into(),
                    Policy::ROOM,
                    Some(vec![speaker]),
                )?;
            }
        }
        Mode::Pose | Mode::Fpose => {
            if let Some(at) = room(ctx, speaker) {
                send(
                    ctx,
                    at,
                    format!(
                        "{}{}{message}",
                        name(ctx, speaker),
                        if nospace { "" } else { " " }
                    )
                    .into(),
                    Policy::ROOM,
                    None,
                )?;
            }
        }
        Mode::Emit | Mode::Femit => emit(
            ctx,
            room(ctx, speaker),
            message.into(),
            has("here") || !has("room"),
            has("room"),
        )?,
        Mode::Pemit => {
            let contents = has("contents");
            if contents {
                ensure!(
                    flags::controls(&ctx.scripts.world.borrow(), ctx.player, speaker),
                    "Permission denied."
                );
            }
            send(
                ctx,
                speaker,
                message.into(),
                if contents {
                    Policy::ROOM
                } else {
                    Policy::DIRECT
                },
                None,
            )?;
        }
        Mode::Oemit => {
            // C uses the raw Location slot here: exit destination or room dropto.
            let at = {
                let world = ctx.scripts.world.borrow();
                let object = &world.objects[&speaker];
                match object.kind {
                    Kind::Exit => object.destination,
                    Kind::Room => object.dropto,
                    _ => object.location,
                }
            };
            if let Some(at) = at {
                send(ctx, at, message.into(), Policy::ROOM, Some(vec![speaker]))?;
            }
        }
        Mode::Wall => {
            let wizard = has("wizard") || has("admin");
            let prefix = if has("no_prefix") {
                ""
            } else if has("admin") {
                "Admin: "
            } else if wizard {
                "Broadcast: "
            } else {
                "Announcement: "
            };
            let body = if has("emit") {
                message.to_string()
            } else if has("pose") {
                format!("{} {message}", name(ctx, speaker))
            } else if let Some(m) = message.strip_prefix(':') {
                format!("{} {m}", name(ctx, speaker))
            } else if let Some(m) = message.strip_prefix(';') {
                format!("{}{m}", name(ctx, speaker))
            } else {
                format!(
                    "{} shouts \"{}\"",
                    name(ctx, speaker),
                    message.strip_prefix('"').unwrap_or(message)
                )
            };
            let recipients: Vec<_> = ctx
                .scripts
                .world
                .borrow()
                .objects
                .values()
                .filter(|o| {
                    o.kind == Kind::Player
                        && o.flags.contains(Flag::Connected)
                        && (!wizard || o.flags.contains(Flag::Wizard))
                })
                .map(|o| o.id)
                .collect();
            for id in recipients {
                // raw_broadcast bypasses object forwarding in C.
                notification::direct(
                    &ctx.scripts.outbox,
                    ctx.config,
                    id,
                    format!("{prefix}{body}").into(),
                )?;
            }
        }
    }
    Ok(Action::Continue)
}
macro_rules! handler {
    ($name:ident,$mode:ident) => {
        pub fn $name(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
            let before = ctx.scripts.world.borrow().clone();
            let pending = ctx.scripts.outbox.borrow().clone();
            let flow_effects = crate::lua::flows::snapshot(&ctx.scripts.lua);
            match perform(ctx, input, Mode::$mode) {
                Ok(action) => Ok(action),
                Err(error) => {
                    *ctx.scripts.world.borrow_mut() = before;
                    *ctx.scripts.outbox.borrow_mut() = pending;
                    crate::lua::flows::restore(&ctx.scripts.lua, flow_effects);
                    Ok(Action::Reply(error.to_string()))
                }
            }
        }
    };
}
handler!(say, Say);
handler!(pose, Pose);
handler!(emit_command, Emit);
handler!(pemit, Pemit);
handler!(oemit, Oemit);
handler!(fsay, Fsay);
handler!(fpose, Fpose);
handler!(femit, Femit);
handler!(wall, Wall);
