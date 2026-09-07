//! Persistent channel aliases, listening preferences and visibility-filtered member queries.
use super::*;

impl Service<'_> {
    /// Create an alias and join atomically under the caller transaction.
    pub fn add(
        &self,
        who: ObjectId,
        channel: &str,
        alias: &str,
        quiet: bool,
        trusted: bool,
    ) -> Result<()> {
        ensure!(!alias.is_empty(), "You need to specify an alias.");
        ensure!(
            alias.len() <= ALIAS_LIMIT && alias.bytes().all(|b| (33..=126).contains(&b)),
            "Channel aliases must be 1-5 printable ASCII characters without spaces."
        );
        ensure!(!channel.is_empty(), "You need to specify a channel.");
        ensure!(channel.len() < 200, "Channel name too long.");
        ensure!(
            !channel.contains(' '),
            "Channel name cannot contain spaces."
        );
        let name = self
            .name(channel)
            .map_err(|_| anyhow::anyhow!("Channel {channel} does not exist yet."))?;
        let identity = self.world.borrow().channels[&name].id;
        ensure!(
            trusted || self.allowed(who, &name, Access::Join)?,
            "Sorry, this channel type does not allow you to join."
        );
        ensure!(
            self.by_id(identity)? == name,
            "channel replaced by callback"
        );
        let existing = self
            .world
            .borrow()
            .channel_aliases
            .get(&who)
            .and_then(|a| a.iter().find(|a| a.alias.eq_ignore_ascii_case(alias)))
            .map(|a| a.channel.clone());
        let listed = !trusted
            && self.world.borrow().channels[&name]
                .users
                .iter()
                .any(|u| u.who == who);
        let warning = "Warning: you are already listed on that channel.";
        if let Some(existing) = existing {
            // Return both lines as the refusal so transactional rollback retains the warning.
            anyhow::bail!(
                "{}That alias is already in use for channel {existing}.",
                if listed {
                    format!("{warning}\n")
                } else {
                    String::new()
                }
            );
        }
        if listed {
            self.notify(who, warning)?;
        }
        {
            let mut w = self.world.borrow_mut();
            let aliases = w.channel_aliases.entry(who).or_default();
            aliases.push(ChannelAlias {
                alias: alias.into(),
                channel: name.clone(),
            });
            aliases.sort_by_key(|a| a.alias.to_ascii_lowercase());
        }
        self.join(who, &name, quiet)?;
        self.notify(who, format!("Channel {name} added with alias {alias}."))
    }

    /// Enable membership, growing capacity in the C ten-entry increments.
    pub fn join(&self, who: ObjectId, channel: &str, quiet: bool) -> Result<()> {
        let name = self.name(channel)?;
        let already = self.world.borrow().channels[&name]
            .users
            .iter()
            .any(|u| u.who == who && u.listening);
        if already {
            return self.notify(who, format!("You are already on channel {name}."));
        }
        self.online_members(&name)?;
        let (player, dark) = {
            let mut w = self.world.borrow_mut();
            let o = w
                .objects
                .get(&who)
                .ok_or_else(|| anyhow::anyhow!("object does not exist"))?;
            let details = (o.name.clone(), o.flags.contains(Flag::Dark));
            let online = o.kind != Kind::Player || o.flags.contains(Flag::Connected);
            let c = w.channels.get_mut(&name).unwrap();
            if let Some(u) = c.users.iter_mut().find(|u| u.who == who) {
                u.listening = true;
            } else {
                c.users.push(Membership {
                    who,
                    listening: true,
                });
                c.users.sort_by_key(|u| u.who);
                if online {
                    c.online.insert(0, who);
                }
                if c.users.len() >= c.max_users {
                    c.max_users += 10;
                }
            }
            details
        };
        self.notify(who, format!("You have joined channel {name}."))?;
        if !quiet && !dark {
            self.emit(&name, &format!("{player} has joined this channel."), false)?;
        }
        Ok(())
    }

    /// Announce departure and run channel-object callbacks before turning off.
    pub fn leave(&self, who: ObjectId, channel: &str) -> Result<()> {
        let name = self.name(channel)?;
        let (player, dark, on) = {
            let w = self.world.borrow();
            let o = &w.objects[&who];
            (
                o.name.clone(),
                o.flags.contains(Flag::Dark),
                w.channels[&name]
                    .users
                    .iter()
                    .any(|u| u.who == who && u.listening),
            )
        };
        let identity = self.world.borrow().channels[&name].id;
        self.leave_callbacks(who, &name)?;
        ensure!(
            self.by_id(identity)? == name,
            "channel replaced by callback"
        );
        self.notify(who, format!("You have left channel {name}."))?;
        if on && !dark {
            self.emit(&name, &format!("{player} has left this channel."), false)?;
        }
        self.world
            .borrow_mut()
            .channels
            .get_mut(&name)
            .ok_or_else(|| anyhow::anyhow!("channel removed by callback"))?
            .users
            .iter_mut()
            .find(|u| u.who == who)
            .ok_or_else(|| anyhow::anyhow!("membership removed by callback"))?
            .listening = false;
        Ok(())
    }

    /// Remove membership independently of aliases, excluding the departing member from the announcement.
    pub fn remove_member(&self, who: ObjectId, channel: &str) -> Result<()> {
        let Ok(name) = self.name(channel) else {
            return self.notify(who, format!("Unknown channel {channel}."));
        };
        let identity = self.world.borrow().channels[&name].id;
        self.leave_callbacks(who, &name)?;
        ensure!(
            self.by_id(identity)? == name,
            "channel replaced by callback"
        );
        let Some(on) = self.world.borrow().channels[&name]
            .users
            .iter()
            .find(|u| u.who == who)
            .map(|u| u.listening)
        else {
            return Ok(());
        };
        let (player, dark) = {
            let w = self.world.borrow();
            let o = w
                .objects
                .get(&who)
                .ok_or_else(|| anyhow::anyhow!("member removed by callback"))?;
            (o.name.clone(), o.flags.contains(Flag::Dark))
        };
        self.online_members(&name)?;
        self.world
            .borrow_mut()
            .channels
            .get_mut(&name)
            .unwrap()
            .online
            .retain(|id| *id != who);
        if on && !dark && !player.is_empty() {
            self.emit_excluding(
                &name,
                &format!("{player} has left this channel."),
                false,
                Some(who),
            )?;
        }
        self.notify(who, format!("You have left channel {channel}."))?;
        let mut w = self.world.borrow_mut();
        let c = w
            .channels
            .get_mut(&name)
            .ok_or_else(|| anyhow::anyhow!("channel removed by callback"))?;
        ensure!(c.id == identity, "channel replaced by callback");
        c.users.retain(|u| u.who != who);
        c.online.retain(|id| *id != who);
        Ok(())
    }

    /// Delete exactly one alias, removing membership even when other aliases remain.
    pub fn remove_alias(&self, who: ObjectId, alias: &str) -> Result<()> {
        let name = self
            .world
            .borrow()
            .channel_aliases
            .get(&who)
            .and_then(|a| a.iter().find(|a| a.alias.eq_ignore_ascii_case(alias)))
            .map(|a| a.channel.clone())
            .ok_or_else(|| anyhow::anyhow!("Unable to find that alias."))?;
        self.remove_member(who, &name)?;
        self.notify(who, format!("Channel {name} deleted."))?;
        self.delete_alias_entry(who, alias);
        Ok(())
    }

    /// Remove only the alias record after the membership operation succeeds.
    pub(super) fn delete_alias_entry(&self, who: ObjectId, alias: &str) {
        if let Some(aliases) = self.world.borrow_mut().channel_aliases.get_mut(&who) {
            aliases.retain(|a| !a.alias.eq_ignore_ascii_case(alias));
        }
    }

    /// Announce an administrative removal, retaining the member's aliases.
    pub fn boot(&self, actor: ObjectId, who: ObjectId, channel: &str) -> Result<()> {
        let name = self.name(channel)?;
        ensure!(
            self.world.borrow().channels[&name]
                .users
                .iter()
                .any(|u| u.who == who),
            "object is not a member of this channel"
        );
        let (a, b) = {
            let w = self.world.borrow();
            (
                format!("{}(#{})", w.objects[&actor].name, actor.0),
                format!("{}(#{})", w.objects[&who].name, who.0),
            )
        };
        self.emit(&name, &format!("{a} boots {b} off the channel."), false)?;
        self.remove_member(who, &name)
    }

    /// Reconcile live eligibility while retaining the C online insertion order.
    /// Missing entries are populated in reverse persisted slot order on first use.
    pub(super) fn online_members(&self, channel: &str) -> Result<Vec<Membership>> {
        let name = self.name(channel)?;
        let mut w = self.world.borrow_mut();
        let eligible = w.channels[&name]
            .users
            .iter()
            .filter(|u| {
                w.objects.get(&u.who).is_some_and(|o| {
                    o.kind != Kind::Garbage
                        && (o.kind != Kind::Player || o.flags.contains(Flag::Connected))
                })
            })
            .map(|u| u.who)
            .collect::<Vec<_>>();
        let c = w.channels.get_mut(&name).unwrap();
        c.online.retain(|id| eligible.contains(id));
        if !c.online_initialized {
            c.online = eligible.into_iter().rev().collect();
            c.online_initialized = true;
        }
        Ok(c.online
            .iter()
            .filter_map(|id| c.users.iter().find(|u| u.who == *id))
            .cloned()
            .collect())
    }

    /// Return valid active or all memberships for native and Lua queries.
    pub fn members(&self, channel: &str, all: bool) -> Result<Vec<Membership>> {
        let name = self.name(channel)?;
        let w = self.world.borrow();
        Ok(w.channels[&name]
            .users
            .iter()
            .filter(|u| {
                w.objects.get(&u.who).is_some_and(|o| {
                    o.kind != Kind::Garbage
                        && (all || o.kind != Kind::Player || o.flags.contains(Flag::Connected))
                })
            })
            .cloned()
            .collect())
    }

    /// Render C player/object groups and Wizard-level identity details.
    pub fn who(&self, viewer: ObjectId, channel: &str, all: bool, admin: bool) -> Result<()> {
        let name = self.name(channel)?;
        let members = if admin {
            self.members(&name, all)?
        } else {
            self.online_members(&name)?
        };
        if admin {
            self.notify(viewer, format!("-- {name} --"))?;
        }
        if admin {
            self.notify(viewer, "Name                          Status Player")?;
        } else {
            self.notify(viewer, "-- Players --")?;
        }
        for group in if admin {
            vec![None]
        } else {
            vec![Some(true), Some(false)]
        } {
            if group == Some(false) {
                self.notify(viewer, "-- Objects --")?;
            }
            for u in &members {
                let output = {
                    let w = self.world.borrow();
                    let o = &w.objects[&u.who];
                    let kind = o.kind == Kind::Player;
                    if group.is_some_and(|group| group != kind) || (!admin && !u.listening) {
                        continue;
                    }
                    if (kind || admin) && o.flags.contains(Flag::Dark) && !wizard(&w, viewer) {
                        continue;
                    }
                    if !admin && kind && in_character(&w, &self.config, u.who) && !wizard(&w, u.who)
                    {
                        continue;
                    }
                    if !admin && !kind && o.flags.contains(Flag::Going) {
                        continue;
                    }
                    let display = if wizard(&w, viewer) {
                        format!("{}{}", o.name, crate::find::suffix(o))
                    } else {
                        o.name.clone()
                    };
                    if admin {
                        format!(
                            "{} {:6} {:6}",
                            column(&crate::text::plain_with(&w.palette, &display), 29),
                            if u.listening { "on" } else { "off" },
                            if kind { "yes" } else { "no" }
                        )
                    } else {
                        let idle = if kind { self.idle(u.who) } else { 0 };
                        if idle > 30 {
                            format!("{display} [[idle {}]", duration(idle))
                        } else {
                            display
                        }
                    }
                };
                self.notify(viewer, output)?;
            }
        }
        self.notify(viewer, format!("-- {name} --"))?;
        Ok(())
    }
}

impl Service<'_> {
    /// Channel things receive C's leave event, with the departing member as cause.
    fn leave_callbacks(&self, who: ObjectId, channel: &str) -> Result<()> {
        let objects = {
            let w = self.world.borrow();
            w.channels[channel]
                .users
                .iter()
                .skip(1)
                .rev()
                .filter_map(|u| w.objects.get(&u.who))
                .filter(|o| o.kind == Kind::Thing && !o.flags.contains(Flag::Halted))
                .map(|o| o.id)
                .collect::<Vec<_>>()
        };
        let scripts =
            crate::lua::Scripts::services(self.lua).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for object in objects {
            crate::lua::transactions::run(self.lua, self.world, self.outbox, || {
                scripts
                    .channel_leave_event(object, who)
                    .map_err(mlua::Error::external)
            })
            .map_err(|e| anyhow::anyhow!("channel {channel} leave callback: {e}"))?;
        }
        Ok(())
    }
}

/// Use live session snapshots; multiple sessions use the shortest idle interval.
impl Service<'_> {
    fn idle(&self, who: ObjectId) -> u64 {
        crate::lua::sessions::players(self.lua)
            .ok()
            .map(|list| {
                list.sequence_values::<mlua::Table>()
                    .filter_map(|row| row.ok())
                    .filter(|row| row.get::<i64>("dbref").ok() == Some(who.0))
                    .filter_map(|row| row.get::<u64>("idle_for").ok())
                    .min()
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }
}

/// Display-column clipping never splits a Unicode grapheme.
pub(super) fn column(s: &str, width: usize) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    let mut result = String::new();
    let mut used = 0;
    for g in s.graphemes(true) {
        let columns = unicode_width::UnicodeWidthStr::width(g);
        if used + columns > width {
            break;
        }
        result.push_str(g);
        used += columns;
    }
    result.push_str(&" ".repeat(width - used));
    result
}

/// C's verbose idle duration uses thirty-day months and twelve-month years.
fn duration(mut seconds: u64) -> String {
    let mut parts = Vec::new();
    for (size, name) in [
        (31_104_000, "year"),
        (2_592_000, "month"),
        (86_400, "day"),
        (3_600, "hour"),
        (60, "minute"),
        (1, "second"),
    ] {
        let count = seconds / size;
        seconds %= size;
        if count > 0 {
            parts.push(format!(
                "{count} {name}{}",
                if count == 1 { "" } else { "s" }
            ));
        }
    }
    if parts.len() > 1 {
        let last = parts.pop().unwrap();
        format!("{} and {last}", parts.join(", "))
    } else {
        parts.join("")
    }
}
