//! Recipient policy, bounded message history and connection announcements.
use super::*;
use crate::text;

impl Service<'_> {
    /// Deliver channel traffic directly to players and through object AUDIBLE exits.
    pub fn deliver(&self, target: ObjectId, message: &str) -> Result<()> {
        let player = self
            .world
            .borrow()
            .objects
            .get(&target)
            .is_some_and(|o| o.kind == Kind::Player);
        if player {
            return self.notify(target, message.to_string());
        }
        crate::notification::send(
            &self.world.borrow(),
            self.outbox,
            &self.config,
            crate::notification::Request {
                target,
                sender: target,
                document: message.into(),
                policy: crate::notification::Policy::DIRECT,
                exclusions: None,
            },
        )
    }

    /// Stage one history entry irrespective of the number of sessions receiving it.
    pub fn emit(&self, channel: &str, message: &str, no_header: bool) -> Result<()> {
        self.emit_excluding(channel, message, no_header, None)
    }

    /// Removal announcements retain membership for callbacks but omit its online recipient.
    pub(super) fn emit_excluding(
        &self,
        channel: &str,
        message: &str,
        no_header: bool,
        excluded: Option<ObjectId>,
    ) -> Result<()> {
        ensure!(
            !message.contains('\0'),
            "message contains an embedded NUL byte"
        );
        let name = self.name(channel)?;
        let message = if no_header {
            message.to_string()
        } else {
            format!("{} {message}", text::escape(&format!("[{channel}]")))
        };
        let identity = self.world.borrow().channels[&name].id;
        {
            let mut w = self.world.borrow_mut();
            let c = w.channels.get_mut(&name).unwrap();
            c.messages = c
                .messages
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("channel message counter overflow"))?;
        }
        let users = self.online_members(&name)?;
        for user in users {
            if Some(user.who) == excluded {
                continue;
            }
            let eligible = {
                let w = self.world.borrow();
                w.objects.get(&user.who).is_some_and(|o| {
                    o.kind != Kind::Garbage
                        && (o.kind != Kind::Player || o.flags.contains(Flag::Connected))
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
            if user.listening
                && eligible
                && self.allowed(user.who, &name, Access::Receive)?
                && (wizard(&self.world.borrow(), user.who)
                    || !in_character(&self.world.borrow(), &self.config, user.who))
            {
                self.deliver(user.who, &message)?;
            }
        }
        let mut w = self.world.borrow_mut();
        let ch = w
            .channels
            .get_mut(&name)
            .ok_or_else(|| anyhow::anyhow!("channel removed by callback"))?;
        ensure!(ch.id == identity, "channel replaced by callback");
        ch.history.push(ChannelMessage {
            at: crate::clock::wall_time(),
            message,
        });
        let expired = ch.history.len().saturating_sub(HISTORY_LIMIT);
        ch.history.drain(..expired);
        Ok(())
    }

    /// Source text in ordinary speech is plain; names retain their configured styles.
    pub fn say(&self, who: ObjectId, channel: &str, message: &str) -> Result<()> {
        ensure!(!message.is_empty(), "No message.");
        ensure!(
            wizard(&self.world.borrow(), who)
                || !in_character(&self.world.borrow(), &self.config, who),
            "Permission denied."
        );
        let name = self.name(channel)?;
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
                crate::notification::send(
                    &self.world.borrow(),
                    self.outbox,
                    &self.config,
                    crate::notification::Request {
                        target: who,
                        sender: who,
                        document: format!("{} {}", timestamp(m.at), m.message).into(),
                        policy: crate::notification::Policy::DIRECT,
                        exclusions: None,
                    },
                )?;
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
        let (name, dark, aliases) = {
            let w = self.world.borrow();
            let o = &w.objects[&who];
            (
                o.name.clone(),
                o.flags.contains(Flag::Dark),
                w.channel_aliases.get(&who).cloned().unwrap_or_default(),
            )
        };
        // Presence follows alias slots, including repeated aliases and stale targets.
        for alias in aliases {
            let member = self.name(&alias.channel).ok().and_then(|name| {
                let w = self.world.borrow();
                let c = &w.channels[&name];
                c.users
                    .iter()
                    .find(|u| u.who == who)
                    .map(|u| (name.clone(), u.listening && c.flags.has(ChannelFlag::Loud)))
            });
            let Some((channel, loud)) = member else {
                if connected {
                    self.notify(
                        who,
                        format!(
                            "Bad Comsys Alias: {} for Channel: {}",
                            alias.alias, alias.channel
                        ),
                    )?;
                }
                continue;
            };
            self.online_members(&channel)?;
            {
                let mut w = self.world.borrow_mut();
                let c = w.channels.get_mut(&channel).unwrap();
                c.online.retain(|id| *id != who);
                if connected {
                    c.online.insert(0, who);
                }
            }
            if loud && !dark {
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
