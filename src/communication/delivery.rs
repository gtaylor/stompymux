//! Recipient policy, bounded message history and connection announcements.
use super::*;
use crate::text;
use std::collections::BTreeSet;

impl Service<'_> {
    /// Deliver directly to players or down through a non-player recipient's contents.
    pub fn deliver(&self, target: ObjectId, message: &str, down: bool) -> Result<()> {
        let mut pending = vec![target];
        let mut seen = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !seen.insert(id) {
                continue;
            }
            let (player, connected, children) = {
                let w = self.world.borrow();
                let Some(o) = w.objects.get(&id) else {
                    continue;
                };
                (
                    o.kind == Kind::Player,
                    o.flags.contains(Flag::Connected),
                    if down || o.kind != Kind::Player {
                        w.objects
                            .values()
                            .filter(|o| o.location == Some(id))
                            .map(|o| o.id)
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    },
                )
            };
            if player && connected {
                self.notify(id, message.to_string())?;
            }
            pending.extend(children);
        }
        Ok(())
    }

    /// Stage one history entry irrespective of the number of sessions receiving it.
    pub fn emit(&self, channel: &str, message: &str, no_header: bool) -> Result<()> {
        ensure!(
            !message.contains('\0'),
            "message contains an embedded NUL byte"
        );
        let name = self.name(channel)?;
        let message = if no_header {
            message.to_string()
        } else {
            format!("{} {message}", text::escape(&format!("[{name}]")))
        };
        let identity = self.world.borrow().channels[&name].id;
        let users = self.world.borrow().channels[&name].users.clone();
        for user in users {
            let eligible = {
                let w = self.world.borrow();
                w.objects.get(&user.who).is_some_and(|o| {
                    o.kind != Kind::Garbage
                        && (o.kind != Kind::Player || o.flags.contains(Flag::Connected))
                        && (wizard(&w, user.who) || !in_character(&w, self.config, user.who))
                })
            };
            ensure!(
                self.world
                    .borrow()
                    .channels
                    .get(&name)
                    .is_some_and(|c| c.id == identity),
                "channel replaced by callback"
            );
            if user.listening && eligible && self.allowed(user.who, &name, Access::Receive)? {
                self.deliver(user.who, &message, false)?;
            }
        }
        let mut w = self.world.borrow_mut();
        let ch = w
            .channels
            .get_mut(&name)
            .ok_or_else(|| anyhow::anyhow!("channel removed by callback"))?;
        ensure!(ch.id == identity, "channel replaced by callback");
        ch.messages = ch
            .messages
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("channel message counter overflow"))?;
        ch.history.push(ChannelMessage {
            at: crate::accounts::now(),
            message,
        });
        let expired = ch.history.len().saturating_sub(HISTORY_LIMIT);
        ch.history.drain(..expired);
        Ok(())
    }

    /// Source text in ordinary speech is plain; names retain their configured styles.
    pub fn say(&self, who: ObjectId, channel: &str, message: &str) -> Result<()> {
        let name = self.name(channel)?;
        ensure!(!message.is_empty(), "No message.");
        ensure!(
            wizard(&self.world.borrow(), who)
                || !in_character(&self.world.borrow(), self.config, who),
            "Permission denied."
        );
        let on = self.world.borrow().channels[&name]
            .users
            .iter()
            .find(|u| u.who == who)
            .map(|u| u.listening)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "You are not listed as on that channel.  Delete this alias and re-add."
                )
            })?;
        match message.to_ascii_lowercase().as_str() {
            "on" => return self.join(who, &name, false),
            "off" => return self.leave(who, &name),
            _ => {}
        }
        let lurk = wizard(&self.world.borrow(), who) || self.config.mux.allow_chanlurking != 0;
        ensure!(on || lurk, "You must be on {name} to do that.");
        if message.eq_ignore_ascii_case("who") {
            return self.who(who, &name, false, false);
        }
        if message.eq_ignore_ascii_case("last") {
            let history = self.world.borrow().channels[&name].history.clone();
            if history.is_empty() {
                return self.notify(who, format!("There haven't been any messages on {name}."));
            }
            for m in history.iter().rev() {
                self.notify(who, format!("{} {}", timestamp(m.at), m.message))?;
            }
            return Ok(());
        }
        ensure!(on, "You must be on {name} to do that.");
        let identity = self.world.borrow().channels[&name].id;
        ensure!(
            self.allowed(who, &name, Access::Transmit)?,
            "That channel type cannot be transmitted on."
        );
        ensure!(
            self.by_id(identity)? == name,
            "channel replaced by callback"
        );
        let player = self.world.borrow().objects[&who].name.clone();
        let (separator, body) = if let Some(s) = message.strip_prefix(':') {
            (" ", s)
        } else if let Some(s) = message.strip_prefix(';') {
            ("", s)
        } else {
            (": ", message)
        };
        let plain = text::escape(&text::plain_with(&self.world.borrow().palette, body));
        self.emit(&name, &format!("{player}{separator}{plain}"), false)
    }

    /// Presence is driven by first/final authenticated session transitions, not persisted flags.
    pub fn presence(&self, who: ObjectId, connected: bool, host: Option<&str>) -> Result<()> {
        let (name, dark, channels) = {
            let w = self.world.borrow();
            let o = &w.objects[&who];
            (
                o.name.clone(),
                o.flags.contains(Flag::Dark),
                w.channels
                    .values()
                    .filter(|c| {
                        c.flags.has(ChannelFlag::Loud)
                            && c.users.iter().any(|u| u.who == who && u.listening)
                    })
                    .map(|c| c.name.clone())
                    .collect::<Vec<_>>(),
            )
        };
        if !dark {
            for channel in channels {
                self.emit(
                    &channel,
                    &format!(
                        "{name} has {}.",
                        if connected {
                            "connected"
                        } else {
                            "disconnected"
                        }
                    ),
                    false,
                )?;
            }
        }
        if self.name("MUXConnections").is_ok() {
            let message = if connected {
                format!(
                    "* {name} has connected from {} *",
                    host.unwrap_or("somewhere")
                )
            } else {
                format!("* {name} has disconnected *")
            };
            self.emit("MUXConnections", &message, false)?;
        }
        Ok(())
    }
}

/// C history uses local dates, with shorter timestamps for today's messages.
fn timestamp(at: i64) -> String {
    // Lua's sandbox intentionally removes os.date; formatting is performed by the host.
    use chrono::{Datelike, Local, TimeZone};
    let Some(time) = Local.timestamp_opt(at, 0).single() else {
        return "[??.?? / ??:??]".into();
    };
    if time.day() == Local::now().day() {
        time.format("[%H:%M]").to_string()
    } else {
        time.format("[%m.%d / %H:%M]").to_string()
    }
}
